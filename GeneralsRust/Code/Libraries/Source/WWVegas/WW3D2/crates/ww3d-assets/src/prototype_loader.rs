// Copyright 2025 - Rust port of C&C Generals Zero Hour W3D Prototype Loader System
//
// This module implements the Prototype Loader system from proto.h/proto.cpp
// Original C++ files:
// - proto.h (lines 128-180): PrototypeLoaderClass interface and default loaders
// - proto.cpp (lines 108-179): MeshLoader and HModelLoader implementations
//
// The prototype loader system is responsible for recognizing W3D chunk types
// and creating appropriate prototypes from them.

use glam::{Vec2, Vec4};
use std::{collections::HashMap, f32::consts::TAU};
use ww3d_core::{
    w3d_format::{
        W3dMaterialInfoStruct, W3dMeshHeader3Struct, W3dRGBAStruct, W3dShaderStruct,
        W3dTexCoordStruct, W3dTextureInfoStruct, W3dTextureStruct, W3dTriangleStruct,
        W3dVectorStruct, W3dVertexMaterialNameStruct, W3dVertexMaterialStruct,
    },
    w3d_string_from_bytes, W3DChunkType, W3DError, W3D_CHUNK_ANIMATION,
    W3D_CHUNK_COMPRESSED_ANIMATION, W3D_CHUNK_HIERARCHY, W3D_CHUNK_HMODEL, W3D_CHUNK_MESH,
};

use crate::assets::Prototype;
use crate::chunk_reader::ChunkReader;
use crate::loaders::mesh_loader as detailed_loader;
use crate::prototypes::{
    AnimationPrototype, HModelNodeLink, HModelPrototype, HierarchyPrototype, MapperDefinition,
    MaterialPassInfo, MeshPrototype, VertexMapperConfig,
};

/// Core PrototypeLoader trait - mirrors C++ PrototypeLoaderClass (proto.h:134-151)
///
/// This trait defines the interface for objects that can recognize certain W3D
/// chunk types and convert them into prototypes. Loaders are registered with
/// the asset manager and queried during W3D file loading.
///
/// # C++ Reference
/// From proto.h lines 128-133:
/// ```cpp
/// /*
/// ** PrototypeLoaderClass
/// ** This is the interface for an object which recognizes a certain
/// ** chunk type in a W3D file and can load it and create a PrototypeClass
/// ** for it.
/// */
/// ```
///
/// # Loading Sequence (from proto.h:53-71)
/// 1. At init time, mesh/hmodel loaders are installed automatically
/// 2. User asks asset manager to load "mesh.w3d"
/// 3. Asset manager encounters a W3D_CHUNK_MESH
/// 4. Asset manager searches loaders for one that handles this chunk
/// 5. MeshLoader is found and its Load method is called
/// 6. MeshLoader creates a mesh prototype, asset manager adds to list
/// 7. User asks for render object named "Mesh"
/// 8. Asset manager finds prototype named "Mesh"
/// 9. Asset manager calls prototype's Create method
/// 10. Mesh prototype creates a mesh (clones template) and returns to user
pub trait PrototypeLoader: Send + Sync {
    /// Get the name of this loader for debugging/logging
    fn get_name(&self) -> &str;

    /// Check if this loader can handle the given chunk type
    /// C++ equivalent: `Chunk_Type()` (proto.h:142)
    ///
    /// Returns true if this loader recognizes and can process the chunk type
    fn can_load(&self, chunk_type: u32) -> bool;

    /// Load a W3D chunk and create a prototype from it
    /// C++ equivalent: `Load_W3D(ChunkLoadClass & cload)` (proto.h:143)
    ///
    /// # Arguments
    /// * `data` - The chunk data as a byte slice
    /// * `chunk_type` - The type of chunk being loaded
    /// * `asset_name` - The name of the asset being loaded (for error reporting)
    ///
    /// # Returns
    /// A prototype if loading succeeded, or an error if loading failed
    ///
    /// # Note on Trait Object Compatibility
    /// This method uses `&[u8]` instead of generic `R: Read + Seek` to maintain
    /// dyn-compatibility. Loaders can internally create a Cursor or other reader
    /// from the byte slice as needed.
    fn load_w3d(
        &self,
        data: &[u8],
        chunk_type: u32,
        asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError>;
}

/// MeshLoader - loads W3D_CHUNK_MESH into mesh prototypes
/// C++ implementation: proto.h:157-163, proto.cpp:108-143
///
/// # C++ Reference
/// From proto.cpp:108-119:
/// ```cpp
/// /***********************************************************************************************
///  * MeshLoaderClass::Load -- reads in a mesh and creates a prototype for it                     *
///  *                                                                                             *
///  * INPUT:                                                                                      *
///  *                                                                                             *
///  * OUTPUT:                                                                                     *
///  *                                                                                             *
///  * WARNINGS:                                                                                   *
///  *                                                                                             *
///  * HISTORY:                                                                                    *
///  *   7/28/98    GTH : Created.                                                                 *
///  *=============================================================================================*/
/// ```
#[derive(Debug)]
pub struct MeshLoader;

impl MeshLoader {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MeshLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl PrototypeLoader for MeshLoader {
    fn get_name(&self) -> &str {
        "MeshLoader"
    }

    /// C++: `virtual int Chunk_Type(void) { return W3D_CHUNK_MESH; }` (proto.h:161)
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_MESH
    }

    /// C++: `PrototypeClass * MeshLoaderClass::Load_W3D(ChunkLoadClass & cload)` (proto.cpp:120-143)
    ///
    /// # C++ Implementation (proto.cpp:120-143)
    /// ```cpp
    /// PrototypeClass * MeshLoaderClass::Load_W3D(ChunkLoadClass & cload)
    /// {
    ///     MeshClass * mesh = NEW_REF( MeshClass, () );
    ///     if (mesh == NULL) {
    ///         return NULL;
    ///     }
    ///     if (mesh->Load_W3D(cload) != WW3D_ERROR_OK) {
    ///         // if the load failed, delete the mesh
    ///         assert(mesh->Num_Refs() == 1);
    ///         mesh->Release_Ref();
    ///         return NULL;
    ///     } else {
    ///         // create the prototype and add it to the lists
    ///         PrimitivePrototypeClass * newproto = W3DNEW PrimitivePrototypeClass(mesh);
    ///         mesh->Release_Ref();
    ///         return newproto;
    ///     }
    /// }
    /// ```
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        let mut detailed_reader = ChunkReader::from_slice(data);
        let detail_mesh = detailed_loader::MeshLoader::load_mesh(&mut detailed_reader).ok();

        // Create a new mesh prototype and load its data from the chunk.
        // Prefer the richer core parser when it succeeds, but don't let it veto
        // valid container meshes that the detailed loader can still decode.
        let mut mesh = match Self::load_mesh_from_core(data, asset_name) {
            Ok(mesh) => mesh,
            Err(core_err) => {
                if let Some(detail_mesh) = detail_mesh.as_ref() {
                    Self::load_mesh_from_detailed(detail_mesh, asset_name)
                } else {
                    Self::load_mesh_minimal(data, asset_name).map_err(|_| core_err)?
                }
            }
        };

        if let Some(detail_mesh) = detail_mesh.as_ref() {
            if Self::should_replace_texture_table(&mesh, detail_mesh) {
                mesh.textures = detail_mesh
                    .textures
                    .iter()
                    .map(Self::convert_detailed_texture)
                    .collect();
            }
            if mesh.vertex_materials.is_empty() && !detail_mesh.vertex_materials.is_empty() {
                mesh.vertex_materials = detail_mesh
                    .vertex_materials
                    .iter()
                    .map(|_| W3dVertexMaterialStruct {
                        attributes: 0,
                        ambient: W3dRGBAStruct {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
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
                            a: 0,
                        },
                        emissive: W3dRGBAStruct {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 0,
                        },
                        shininess: 1.0,
                        opacity: 1.0,
                        translucency: 0.0,
                    })
                    .collect();
            }
            if mesh.vertex_material_names.is_empty() && !detail_mesh.vertex_materials.is_empty() {
                mesh.vertex_material_names = detail_mesh
                    .vertex_materials
                    .iter()
                    .map(|_| W3dVertexMaterialNameStruct {
                        material_name: [0; 256],
                    })
                    .collect();
            }
            if mesh.material_info.is_none() {
                mesh.material_info = Some(W3dMaterialInfoStruct {
                    pass_count: detail_mesh.material_passes.len() as u32,
                    vert_matl_count: detail_mesh.vertex_materials.len() as u32,
                    shader_count: detail_mesh.shaders.len() as u32,
                    texture_count: detail_mesh.textures.len() as u32,
                });
            }
            if mesh.shaders.is_empty() && !detail_mesh.shaders.is_empty() {
                mesh.shaders = detail_mesh
                    .shaders
                    .iter()
                    .map(|_| W3dShaderStruct {
                        depth_compare: 0,
                        depth_mask: 0,
                        color_mask: 0,
                        dest_blend: 0,
                        fog_func: 0,
                        pri_gradient: 0,
                        sec_gradient: 0,
                        src_blend: 0,
                        texturing: 0,
                        detail_color_func: 0,
                        detail_alpha_func: 0,
                        shader_preset: 0,
                        alpha_test: 0,
                        post_detail_color_func: 0,
                        post_detail_alpha_func: 0,
                    })
                    .collect();
            }
            Self::populate_material_pass_data(&mut mesh, detail_mesh);
        } else if mesh.per_face_texcoord_ids.is_empty() && !mesh.stage_texcoords.is_empty() {
            mesh.per_face_texcoord_ids = vec![Vec::new(); mesh.stage_texcoords.len()];
        }

        Ok(Box::new(mesh))
    }
}

impl MeshLoader {
    fn should_replace_texture_table(
        mesh: &MeshPrototype,
        detail_mesh: &detailed_loader::W3DMesh,
    ) -> bool {
        if detail_mesh.textures.is_empty() {
            return false;
        }
        if mesh.textures.is_empty() {
            return true;
        }

        let detail_named = detail_mesh
            .textures
            .iter()
            .filter(|tex| !tex.name.trim().is_empty())
            .count();
        let mesh_named = mesh
            .textures
            .iter()
            .filter(|tex| !w3d_string_from_bytes(&tex.name).trim().is_empty())
            .count();

        mesh_named == 0
            || detail_named > mesh_named
            || mesh.textures.len() < detail_mesh.textures.len()
    }

    fn chunk_error<E: std::fmt::Display>(err: E) -> W3DError {
        W3DError::IoError(err.to_string())
    }

    fn load_mesh_from_core(data: &[u8], asset_name: &str) -> Result<MeshPrototype, W3DError> {
        use std::io::Cursor;
        use ww3d_core::w3d_io::W3DReader;

        let mut cursor = Cursor::new(data);
        let mut reader = W3DReader::new(&mut cursor);
        let w3d_mesh = reader.read_mesh(data.len() as u32)?;

        let mesh_name = Self::resolve_mesh_name_from_header(
            &w3d_mesh.header.mesh_name,
            &w3d_mesh.header.container_name,
            asset_name,
        );

        let mut mesh = MeshPrototype::new(mesh_name);
        mesh.header = Some(w3d_mesh.header);
        mesh.vertices = w3d_mesh.vertices;
        mesh.normals = w3d_mesh.normals;
        mesh.triangles = w3d_mesh.triangles;
        mesh.material_info = w3d_mesh.material_info;
        mesh.shaders = w3d_mesh.shaders;
        mesh.vertex_materials = w3d_mesh.materials;
        if !w3d_mesh.texture_coords.is_empty() {
            mesh.stage_texcoords.push(w3d_mesh.texture_coords);
        }
        Ok(mesh)
    }

    fn load_mesh_from_detailed(
        detail_mesh: &detailed_loader::W3DMesh,
        asset_name: &str,
    ) -> MeshPrototype {
        let mesh_name = Self::resolve_mesh_name(
            &detail_mesh.header.mesh_name,
            &detail_mesh.header.container_name,
            asset_name,
        );
        let mut mesh = MeshPrototype::new(mesh_name);
        mesh.header = Some(Self::convert_detailed_header(&detail_mesh.header));
        mesh.vertices = detail_mesh
            .vertices
            .iter()
            .map(|v| W3dVectorStruct {
                x: v.x,
                y: v.y,
                z: v.z,
            })
            .collect();
        mesh.normals = detail_mesh
            .normals
            .iter()
            .map(|v| W3dVectorStruct {
                x: v.x,
                y: v.y,
                z: v.z,
            })
            .collect();
        mesh.triangles = detail_mesh
            .triangles
            .iter()
            .enumerate()
            .map(|(index, tri)| W3dTriangleStruct {
                vindex: *tri,
                attributes: detail_mesh
                    .triangle_attributes
                    .get(index)
                    .copied()
                    .unwrap_or(0),
                normal: W3dVectorStruct {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                distance: 0.0,
            })
            .collect();
        mesh.material_info = Some(W3dMaterialInfoStruct {
            pass_count: detail_mesh.material_passes.len() as u32,
            vert_matl_count: detail_mesh.vertex_materials.len() as u32,
            shader_count: detail_mesh.shaders.len() as u32,
            texture_count: detail_mesh.textures.len() as u32,
        });
        mesh.shaders = detail_mesh
            .shaders
            .iter()
            .map(|_| W3dShaderStruct {
                depth_compare: 0,
                depth_mask: 0,
                color_mask: 0,
                dest_blend: 0,
                fog_func: 0,
                pri_gradient: 0,
                sec_gradient: 0,
                src_blend: 0,
                texturing: 0,
                detail_color_func: 0,
                detail_alpha_func: 0,
                shader_preset: 0,
                alpha_test: 0,
                post_detail_color_func: 0,
                post_detail_alpha_func: 0,
            })
            .collect();
        mesh.textures = detail_mesh
            .textures
            .iter()
            .map(Self::convert_detailed_texture)
            .collect();
        if !detail_mesh.tex_coords.is_empty() {
            mesh.stage_texcoords
                .push(convert_stage_coords(&detail_mesh.tex_coords));
        }
        mesh
    }

    fn load_mesh_minimal(data: &[u8], asset_name: &str) -> Result<MeshPrototype, W3DError> {
        let mut reader = ChunkReader::from_slice(data);
        if !reader.open_chunk().map_err(Self::chunk_error)? {
            return Err(W3DError::IoError("missing mesh header chunk".to_string()));
        }
        if reader.current_chunk_id().map_err(Self::chunk_error)?
            != W3DChunkType::MeshHeader3.as_u32()
        {
            return Err(W3DError::IoError("invalid mesh header chunk".to_string()));
        }

        let header = Self::read_minimal_mesh_header(&mut reader)?;
        reader.close_chunk().map_err(Self::chunk_error)?;

        let mesh_name =
            Self::resolve_mesh_name(&header.mesh_name, &header.container_name, asset_name);
        let mut mesh = MeshPrototype::new(mesh_name);
        mesh.header = Some(Self::convert_detailed_header(&header));

        while reader.open_chunk().map_err(Self::chunk_error)? {
            let chunk_id = reader.current_chunk_id().map_err(Self::chunk_error)?;
            match W3DChunkType::from_u32(chunk_id) {
                Some(W3DChunkType::Vertices) => {
                    let count =
                        reader.current_chunk_length().map_err(Self::chunk_error)? as usize / 12;
                    for _ in 0..count {
                        let v = reader.read_vec3().map_err(Self::chunk_error)?;
                        mesh.vertices.push(W3dVectorStruct {
                            x: v.x,
                            y: v.y,
                            z: v.z,
                        });
                    }
                }
                Some(W3DChunkType::VertexNormals) => {
                    let count =
                        reader.current_chunk_length().map_err(Self::chunk_error)? as usize / 12;
                    for _ in 0..count {
                        let v = reader.read_vec3().map_err(Self::chunk_error)?;
                        mesh.normals.push(W3dVectorStruct {
                            x: v.x,
                            y: v.y,
                            z: v.z,
                        });
                    }
                }
                Some(W3DChunkType::Triangles) => {
                    let count =
                        reader.current_chunk_length().map_err(Self::chunk_error)? as usize / 32;
                    for _ in 0..count {
                        let a = reader.read_u32().map_err(Self::chunk_error)?;
                        let b = reader.read_u32().map_err(Self::chunk_error)?;
                        let c = reader.read_u32().map_err(Self::chunk_error)?;
                        let attr = reader.read_u32().map_err(Self::chunk_error)?;
                        let nx = reader.read_f32().map_err(Self::chunk_error)?;
                        let ny = reader.read_f32().map_err(Self::chunk_error)?;
                        let nz = reader.read_f32().map_err(Self::chunk_error)?;
                        let distance = reader.read_f32().map_err(Self::chunk_error)?;
                        mesh.triangles.push(W3dTriangleStruct {
                            vindex: [a, b, c],
                            attributes: attr,
                            normal: W3dVectorStruct {
                                x: nx,
                                y: ny,
                                z: nz,
                            },
                            distance,
                        });
                    }
                }
                Some(W3DChunkType::MaterialInfo) => {
                    mesh.material_info = Some(W3dMaterialInfoStruct {
                        pass_count: reader.read_u32().map_err(Self::chunk_error)?,
                        vert_matl_count: reader.read_u32().map_err(Self::chunk_error)?,
                        shader_count: reader.read_u32().map_err(Self::chunk_error)?,
                        texture_count: reader.read_u32().map_err(Self::chunk_error)?,
                    });
                }
                Some(W3DChunkType::Textures) => {
                    mesh.textures = Self::read_minimal_textures(&mut reader)?;
                }
                _ => {}
            }

            reader.close_chunk().map_err(Self::chunk_error)?;
        }

        Ok(mesh)
    }

    fn read_minimal_mesh_header<R: std::io::Read + std::io::Seek>(
        reader: &mut ChunkReader<R>,
    ) -> Result<detailed_loader::W3DMeshHeader, W3DError> {
        let version = reader.read_u32().map_err(Self::chunk_error)?;
        let attributes = reader.read_u32().map_err(Self::chunk_error)?;
        let mesh_name = reader.read_fixed_string(16).map_err(Self::chunk_error)?;
        let container_name = reader.read_fixed_string(16).map_err(Self::chunk_error)?;
        let num_tris = reader.read_u32().map_err(Self::chunk_error)?;
        let num_vertices = reader.read_u32().map_err(Self::chunk_error)?;
        let num_materials = reader.read_u32().map_err(Self::chunk_error)?;
        let num_damage_stages = reader.read_u32().map_err(Self::chunk_error)?;
        let sort_level = reader.read_i32().map_err(Self::chunk_error)?;
        let prelit_version = reader.read_u32().map_err(Self::chunk_error)?;
        let future_count = reader.read_u32().map_err(Self::chunk_error)?;
        let vertex_channels = reader.read_u32().map_err(Self::chunk_error)?;
        let face_channels = reader.read_u32().map_err(Self::chunk_error)?;
        let min = reader.read_vec3().map_err(Self::chunk_error)?;
        let max = reader.read_vec3().map_err(Self::chunk_error)?;
        let sph_center = reader.read_vec3().map_err(Self::chunk_error)?;
        let sph_radius = reader.read_f32().map_err(Self::chunk_error)?;

        Ok(detailed_loader::W3DMeshHeader {
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

    fn read_minimal_textures<R: std::io::Read + std::io::Seek>(
        reader: &mut ChunkReader<R>,
    ) -> Result<Vec<W3dTextureStruct>, W3DError> {
        let mut textures = Vec::new();
        while reader.open_chunk().map_err(Self::chunk_error)? {
            if reader.current_chunk_id().map_err(Self::chunk_error)?
                == W3DChunkType::Texture.as_u32()
            {
                let mut texture = W3dTextureStruct {
                    name: [0u8; 256],
                    texture_info: W3dTextureInfoStruct {
                        attributes: 0,
                        animation_type: 0,
                        frame_count: 0,
                        frame_rate: 0.0,
                    },
                };
                while reader.open_chunk().map_err(Self::chunk_error)? {
                    match W3DChunkType::from_u32(
                        reader.current_chunk_id().map_err(Self::chunk_error)?,
                    ) {
                        Some(W3DChunkType::TextureName) => {
                            let name = reader.read_fixed_string(16).map_err(Self::chunk_error)?;
                            copy_fixed_name(&name, &mut texture.name);
                        }
                        Some(W3DChunkType::TextureInfo) => {
                            texture.texture_info.attributes =
                                reader.read_u16().map_err(Self::chunk_error)?;
                            texture.texture_info.animation_type =
                                reader.read_u16().map_err(Self::chunk_error)?;
                            texture.texture_info.frame_count =
                                reader.read_u32().map_err(Self::chunk_error)?;
                            texture.texture_info.frame_rate =
                                reader.read_f32().map_err(Self::chunk_error)?;
                        }
                        _ => {}
                    }
                    reader.close_chunk().map_err(Self::chunk_error)?;
                }
                textures.push(texture);
            }
            reader.close_chunk().map_err(Self::chunk_error)?;
        }
        Ok(textures)
    }

    fn resolve_mesh_name_from_header(
        mesh_name: &[u8; 16],
        container_name: &[u8; 16],
        asset_name: &str,
    ) -> String {
        let mesh = String::from_utf8_lossy(mesh_name)
            .trim_end_matches('\0')
            .to_string();
        let container = String::from_utf8_lossy(container_name)
            .trim_end_matches('\0')
            .to_string();
        Self::resolve_mesh_name(&mesh, &container, asset_name)
    }

    fn resolve_mesh_name(mesh_name: &str, container_name: &str, asset_name: &str) -> String {
        let mesh_name = mesh_name.trim();
        let container_name = container_name.trim();
        if !container_name.is_empty() && !mesh_name.is_empty() {
            format!("{container_name}.{mesh_name}")
        } else if !mesh_name.is_empty() {
            mesh_name.to_string()
        } else {
            asset_name.to_string()
        }
    }

    fn convert_detailed_header(header: &detailed_loader::W3DMeshHeader) -> W3dMeshHeader3Struct {
        let mut mesh_name = [0u8; 16];
        let mut container_name = [0u8; 16];
        copy_fixed_name(&header.mesh_name, &mut mesh_name);
        copy_fixed_name(&header.container_name, &mut container_name);
        W3dMeshHeader3Struct {
            version: header.version,
            attrs: header.attributes,
            mesh_name,
            container_name,
            num_tris: header.num_tris,
            num_verts: header.num_vertices,
            num_materials: header.num_materials,
            num_damage_stages: header.num_damage_stages,
            sort_level: header.sort_level,
            prelit_version: header.prelit_version,
            future_counts: [header.future_count],
            vertex_channels: header.vertex_channels,
            face_channels: header.face_channels,
            bbox_min: W3dVectorStruct {
                x: header.min.x,
                y: header.min.y,
                z: header.min.z,
            },
            bbox_max: W3dVectorStruct {
                x: header.max.x,
                y: header.max.y,
                z: header.max.z,
            },
            sph_center: W3dVectorStruct {
                x: header.sph_center.x,
                y: header.sph_center.y,
                z: header.sph_center.z,
            },
            sph_radius: header.sph_radius,
        }
    }

    fn convert_detailed_texture(texture: &detailed_loader::W3DTexture) -> W3dTextureStruct {
        let mut name = [0u8; 256];
        copy_fixed_name(&texture.name, &mut name);
        W3dTextureStruct {
            name,
            texture_info: W3dTextureInfoStruct {
                attributes: texture.texture_info as u16,
                animation_type: 0,
                frame_count: 0,
                frame_rate: 0.0,
            },
        }
    }

    fn populate_material_pass_data(
        prototype: &mut MeshPrototype,
        detailed: &detailed_loader::W3DMesh,
    ) {
        prototype.vertex_mapper_configs = Self::build_vertex_mapper_configs(
            &prototype.vertex_materials,
            &detailed.vertex_materials,
        );

        if detailed.material_passes.is_empty() {
            if prototype.per_face_texcoord_ids.is_empty() && !prototype.stage_texcoords.is_empty() {
                prototype.per_face_texcoord_ids = vec![Vec::new(); prototype.stage_texcoords.len()];
            }
            return;
        }

        let mut stage_layers: Vec<Vec<W3dTexCoordStruct>> = Vec::new();
        let mut per_face_layers: Vec<Vec<[u32; 3]>> = Vec::new();
        let mut per_pass_stage_texture_ids: Vec<Vec<Vec<u32>>> = Vec::new();
        let mut per_pass_vertex_material_ids: Vec<Vec<u32>> = Vec::new();
        let mut per_pass_shader_ids: Vec<Vec<u32>> = Vec::new();
        let mut per_pass_dcg_colors: Vec<Vec<W3dRGBAStruct>> = Vec::new();
        let mut per_pass_dig_colors: Vec<Vec<W3dRGBAStruct>> = Vec::new();
        let mut pass_infos: Vec<MaterialPassInfo> = Vec::new();

        for pass in &detailed.material_passes {
            pass_infos.push(MaterialPassInfo {
                vm_id: pass.vertex_material_ids.first().copied().unwrap_or(0),
                shader_id: pass.shader_ids.first().copied().unwrap_or(0),
                texture_count: pass.texture_stages.len() as u32,
            });

            per_pass_vertex_material_ids.push(pass.vertex_material_ids.clone());
            per_pass_shader_ids.push(pass.shader_ids.clone());
            per_pass_dcg_colors.push(
                pass.dcg
                    .iter()
                    .map(convert_vec4_to_rgba)
                    .collect::<Vec<_>>(),
            );
            per_pass_dig_colors.push(
                pass.dig
                    .iter()
                    .map(convert_vec4_to_rgba)
                    .collect::<Vec<_>>(),
            );

            let mut stage_texture_ids = Vec::with_capacity(pass.texture_stages.len());
            for stage in &pass.texture_stages {
                stage_texture_ids.push(stage.texture_ids.clone());
                stage_layers.push(convert_stage_coords(&stage.tex_coords));
                per_face_layers.push(stage.per_face_texcoord_ids.clone());
            }
            per_pass_stage_texture_ids.push(stage_texture_ids);
        }

        if !stage_layers.is_empty() {
            prototype.stage_texcoords = stage_layers;
            prototype.per_face_texcoord_ids = per_face_layers;
        } else if prototype.per_face_texcoord_ids.is_empty()
            && !prototype.stage_texcoords.is_empty()
        {
            prototype.per_face_texcoord_ids = vec![Vec::new(); prototype.stage_texcoords.len()];
        }

        prototype.passes = pass_infos;
        prototype.per_pass_stage_texture_ids = per_pass_stage_texture_ids;
        prototype.per_pass_vertex_material_ids = per_pass_vertex_material_ids;
        prototype.per_pass_shader_ids = per_pass_shader_ids;
        prototype.per_pass_dcg_colors = per_pass_dcg_colors;
        prototype.per_pass_dig_colors = per_pass_dig_colors;
    }
}

fn copy_fixed_name(src: &str, dst: &mut [u8]) {
    let bytes = src.as_bytes();
    let len = bytes.len().min(dst.len().saturating_sub(1));
    dst[..len].copy_from_slice(&bytes[..len]);
    if len < dst.len() {
        dst[len] = 0;
    }
}

fn convert_vec4_to_rgba(color: &Vec4) -> W3dRGBAStruct {
    fn clamp_to_byte(value: f32) -> u8 {
        let scaled = (value.clamp(0.0, 1.0) * 255.0).round() as i32;
        scaled.clamp(0, 255) as u8
    }

    W3dRGBAStruct {
        r: clamp_to_byte(color.x),
        g: clamp_to_byte(color.y),
        b: clamp_to_byte(color.z),
        a: clamp_to_byte(color.w),
    }
}

fn convert_stage_coords(coords: &[Vec2]) -> Vec<W3dTexCoordStruct> {
    coords
        .iter()
        .map(|tc| W3dTexCoordStruct { u: tc.x, v: tc.y })
        .collect()
}

const STAGE0_MAPPING_MASK: u32 = 0x00FF_0000;
const STAGE0_MAPPING_SHIFT: u32 = 16;
const STAGE1_MAPPING_MASK: u32 = 0x0000_FF00;
const STAGE1_MAPPING_SHIFT: u32 = 8;

fn stage_mapping_to_mapper_type(value: u32) -> Option<u32> {
    match value {
        0x0000 => Some(0),  // UV
        0x0001 => Some(1),  // Environment
        0x0002 => Some(2),  // CheapEnvironment
        0x0003 => Some(3),  // Screen
        0x0004 => Some(4),  // LinearOffset
        0x0005 => Some(5),  // Silhouette
        0x0006 => Some(6),  // Scale
        0x0007 => Some(7),  // Grid
        0x0008 => Some(8),  // Rotate
        0x0009 => Some(9),  // SineLinearOffset
        0x000A => Some(10), // StepLinearOffset
        0x000B => Some(11), // ZigZagLinearOffset
        0x000C => Some(12), // WSClassicEnvironment
        0x000D => Some(13), // WSEnvironment
        0x000E => Some(14), // GridClassicEnvironment
        0x000F => Some(15), // GridEnvironment
        0x0010 => Some(16), // Random
        0x0011 => Some(17), // Edge
        0x0012 => Some(18), // BumpEnvironment
        _ => None,
    }
}

impl MeshLoader {
    fn build_vertex_mapper_configs(
        vertex_materials: &[ww3d_core::w3d_format::W3dVertexMaterialStruct],
        mapper_args: &[detailed_loader::VertexMapperArgs],
    ) -> Vec<VertexMapperConfig> {
        vertex_materials
            .iter()
            .enumerate()
            .map(|(index, vmat)| {
                let stage0_type =
                    (vmat.attributes & STAGE0_MAPPING_MASK) >> STAGE0_MAPPING_SHIFT;
                let stage1_type =
                    (vmat.attributes & STAGE1_MAPPING_MASK) >> STAGE1_MAPPING_SHIFT;

                let args_entry = mapper_args.get(index);
                let stage0 = stage_mapping_to_mapper_type(stage0_type).and_then(|mapper_type| {
                    parse_mapper_definition(
                        mapper_type,
                        args_entry.and_then(|entry| entry.stage0.as_deref()),
                    )
                });
                let stage1 = stage_mapping_to_mapper_type(stage1_type).and_then(|mapper_type| {
                    parse_mapper_definition(
                        mapper_type,
                        args_entry.and_then(|entry| entry.stage1.as_deref()),
                    )
                });

                VertexMapperConfig { stage0, stage1 }
            })
            .collect()
    }
}

fn parse_mapper_definition(mapper_type: u32, raw_args: Option<&str>) -> Option<MapperDefinition> {
    let raw = raw_args?;
    let args_map = parse_args(raw);

    match mapper_type {
        4 => {
            let u = parse_float(&args_map, "upersec", 0.0);
            let v = parse_float(&args_map, "vpersec", 0.0);
            Some(MapperDefinition {
                mapper_type,
                args: [scale_to_i32(u, 1000.0), scale_to_i32(v, 1000.0), 0, 0],
                float_args: [0.0; 4],
            })
        }
        7 => {
            let log2 = parse_float(&args_map, "log2width", 1.0).max(0.0) as u32;
            let tiles = 1u32.checked_shl(log2.min(30)).unwrap_or(u32::MAX).max(1);
            let frame = parse_float(&args_map, "offset", 0.0).max(0.0) as u32;
            let columns = tiles.max(1);
            let rows = tiles.max(1);
            let frame_index = frame % (columns * rows).max(1);
            let column = frame_index % columns;
            let row = frame_index / columns;
            let u_offset = column as f32 / columns as f32;
            let v_offset = row as f32 / rows as f32;

            Some(MapperDefinition {
                mapper_type,
                args: [
                    columns as i32,
                    rows as i32,
                    scale_to_i32(u_offset, 1000.0),
                    scale_to_i32(v_offset, 1000.0),
                ],
                float_args: [0.0; 4],
            })
        }
        8 => {
            let rotations_per_sec = parse_float(&args_map, "speed", 0.0);
            let rotation_deg = rotations_per_sec * 360.0;
            let u_center = parse_float(&args_map, "ucenter", 0.0);
            let v_center = parse_float(&args_map, "vcenter", 0.0);

            Some(MapperDefinition {
                mapper_type,
                args: [
                    scale_to_i32(rotation_deg, 100.0),
                    scale_to_i32(u_center, 1000.0),
                    scale_to_i32(v_center, 1000.0),
                    0,
                ],
                float_args: [0.0; 4],
            })
        }
        9 => {
            let u_amp = parse_float(&args_map, "uamp", 0.0);
            let v_amp = parse_float(&args_map, "vamp", 0.0);
            let freq = parse_float(&args_map, "ufreq", 0.0);
            let phase = parse_float(&args_map, "uphase", 0.0);

            Some(MapperDefinition {
                mapper_type,
                args: [
                    scale_to_i32(u_amp, 1000.0),
                    scale_to_i32(v_amp, 1000.0),
                    scale_to_i32(freq, 100.0),
                    scale_to_i32(phase, 100.0),
                ],
                float_args: [0.0; 4],
            })
        }
        10 => {
            let u_step = parse_float(&args_map, "ustep", 0.0);
            let v_step = parse_float(&args_map, "vstep", 0.0);
            let steps_per_sec = parse_float(&args_map, "sps", 0.0);
            Some(MapperDefinition {
                mapper_type,
                args: [
                    scale_to_i32(u_step, 1000.0),
                    scale_to_i32(v_step, 1000.0),
                    scale_to_i32(steps_per_sec, 1000.0),
                    0,
                ],
                float_args: [0.0; 4],
            })
        }
        11 => {
            let u_speed = parse_float(&args_map, "upersec", 0.0);
            let v_speed = parse_float(&args_map, "vpersec", 0.0);
            let period = parse_float(&args_map, "period", 0.0).abs();
            Some(MapperDefinition {
                mapper_type,
                args: [
                    scale_to_i32(u_speed, 1000.0),
                    scale_to_i32(v_speed, 1000.0),
                    scale_to_i32(period, 1000.0),
                    0,
                ],
                float_args: [0.0; 4],
            })
        }
        18 => {
            let u_speed = parse_float(&args_map, "upersec", 0.0);
            let v_speed = parse_float(&args_map, "vpersec", 0.0);
            let bump_rotation = parse_float(&args_map, "bumprotation", 0.0);
            let bump_scale = parse_float(&args_map, "bumpscale", 1.0);
            Some(MapperDefinition {
                mapper_type,
                args: [
                    scale_to_i32(u_speed, 1000.0),
                    scale_to_i32(v_speed, 1000.0),
                    0,
                    0,
                ],
                float_args: [bump_scale, bump_rotation * TAU, 0.0, 0.0],
            })
        }
        _ => None,
    }
}

fn parse_args(raw: &str) -> HashMap<String, String> {
    raw.split(['\n', ';'])
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let mut parts = trimmed.splitn(2, '=');
            let key = parts.next()?.trim().to_ascii_lowercase();
            let value = parts
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(';')
                .trim();
            Some((key, value.to_string()))
        })
        .collect()
}

fn parse_float(args: &HashMap<String, String>, key: &str, default: f32) -> f32 {
    args.get(key)
        .and_then(|value| {
            let sanitized = value
                .trim()
                .trim_end_matches(['f', 'F'])
                .trim();
            sanitized.parse::<f32>().ok()
        })
        .unwrap_or(default)
}

fn scale_to_i32(value: f32, scale: f32) -> i32 {
    (value * scale).round() as i32
}

/// HModelLoader - loads W3D_CHUNK_HMODEL into hierarchical model prototypes
/// C++ implementation: proto.h:165-171, proto.cpp:145-179
///
/// # C++ Reference
/// From proto.cpp:146-157:
/// ```cpp
/// /***********************************************************************************************
///  * HModelLoaderClass::Load -- reads in an hmodel and creates a prototype for it                *
///  *                                                                                             *
///  * INPUT:                                                                                      *
///  *                                                                                             *
///  * OUTPUT:                                                                                     *
///  *                                                                                             *
///  * WARNINGS:                                                                                   *
///  *                                                                                             *
///  * HISTORY:                                                                                    *
///  *   7/28/98    GTH : Created.                                                                 *
///  *=============================================================================================*/
/// ```
#[derive(Debug)]
pub struct HModelLoader;

impl HModelLoader {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HModelLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl PrototypeLoader for HModelLoader {
    fn get_name(&self) -> &str {
        "HModelLoader"
    }

    /// C++: `virtual int Chunk_Type(void) { return W3D_CHUNK_HMODEL; }` (proto.h:169)
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_HMODEL
    }

    /// C++: `PrototypeClass * HModelLoaderClass::Load_W3D(ChunkLoadClass & cload)` (proto.cpp:158-179)
    ///
    /// # C++ Implementation (proto.cpp:158-179)
    /// ```cpp
    /// PrototypeClass * HModelLoaderClass::Load_W3D(ChunkLoadClass & cload)
    /// {
    ///     HModelDefClass * hdef = W3DNEW HModelDefClass;
    ///     if (hdef == NULL) {
    ///         return NULL;
    ///     }
    ///     if (hdef->Load_W3D(cload) != HModelDefClass::OK) {
    ///         // load failed, delete the model and return an error
    ///         delete hdef;
    ///         return NULL;
    ///     } else {
    ///         // ok, accept this model!
    ///         HModelPrototypeClass * hproto = W3DNEW HModelPrototypeClass(hdef);
    ///         return hproto;
    ///     }
    /// }
    /// ```
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        // Create HModelDef structure and load from chunk
        // This mirrors the C++ HModelDefClass::Load_W3D() call

        use std::io::Cursor;
        use ww3d_core::w3d_io::W3DReader;

        // Create a reader from the chunk data
        let mut cursor = Cursor::new(data);
        let mut reader = W3DReader::new(&mut cursor);

        // Read the HModel chunk - returns a vec of sub-chunks
        // C++ HModelDefClass::Load_W3D reads:
        // - W3D_CHUNK_HMODEL_HEADER: Model header with hierarchy name and node count
        // - W3D_CHUNK_NODE: Per-node render objects (meshes)
        // - W3D_CHUNK_COLLISION_NODE: Collision geometry nodes
        // - W3D_CHUNK_SKIN_NODE: Skin nodes referencing meshes
        let hmodel_chunks = reader.read_hmodel(data.len() as u32)?;

        // Extract hierarchy name from header chunk and preserve sub-object nodes.
        let mut model_name = asset_name.to_string();
        let mut hierarchy_name = asset_name.to_string();
        let mut version = 0u32;
        let mut nodes = Vec::new();

        for chunk in &hmodel_chunks {
            match chunk {
                ww3d_core::w3d_io::W3DChunk::HModelHeader(ref header) => {
                    version = header.version;
                    let model_name_bytes: Vec<u8> = header
                        .name
                        .iter()
                        .take_while(|&&b| b != 0)
                        .copied()
                        .collect();
                    if let Ok(name) = String::from_utf8(model_name_bytes) {
                        if !name.is_empty() {
                            model_name = name;
                        }
                    }
                    // Extract hierarchy name from header (null-terminated string)
                    let name_bytes: Vec<u8> = header
                        .hierarchy_name
                        .iter()
                        .take_while(|&&b| b != 0)
                        .copied()
                        .collect();
                    if let Ok(name) = String::from_utf8(name_bytes) {
                        if !name.is_empty() {
                            hierarchy_name = name;
                        }
                    }
                }
                ww3d_core::w3d_io::W3DChunk::HModelNode(ref node) => {
                    let render_obj_name = node.render_obj_name_str();
                    if render_obj_name.is_empty() {
                        continue;
                    }

                    let pivot_idx = if version < 0x00030000 {
                        if node.pivot_idx == 65535 {
                            0
                        } else {
                            node.pivot_idx.saturating_add(1)
                        }
                    } else {
                        node.pivot_idx
                    };

                    // C++ hmdldef.cpp stores HMODEL connections as "<ModelName>.<SubObject>".
                    nodes.push(HModelNodeLink {
                        render_obj_name: format!("{model_name}.{render_obj_name}"),
                        pivot_idx,
                    });
                }
                _ => {}
            }
        }

        let mut hmodel = HModelPrototype::new(model_name, hierarchy_name);
        hmodel.nodes = nodes;

        Ok(Box::new(hmodel))
    }
}

/// HTreeLoader - loads W3D_CHUNK_HIERARCHY into hierarchy tree prototypes
/// Hierarchies define skeleton structures for animated models
#[derive(Debug)]
pub struct HTreeLoader;

impl HTreeLoader {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HTreeLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl PrototypeLoader for HTreeLoader {
    fn get_name(&self) -> &str {
        "HTreeLoader"
    }

    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_HIERARCHY
    }

    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        // Load hierarchy pivot structure
        // This mirrors C++ HTreeClass::Load_W3D() (htree.cpp)

        use std::io::Cursor;
        use ww3d_core::w3d_io::W3DReader;

        // Create a reader from the chunk data
        let mut cursor = Cursor::new(data);
        let mut reader = W3DReader::new(&mut cursor);

        // Read the hierarchy chunk - W3DReader::read_hierarchy handles sub-chunks:
        // - W3D_CHUNK_HIERARCHY_HEADER: Hierarchy header with name, num_pivots, and center_pos
        // - W3D_CHUNK_PIVOTS: Array of pivot (bone) structures
        // - W3D_CHUNK_PIVOT_FIXUPS: Optional fixup matrices for pre-3.0 compatibility
        let w3d_hierarchy = reader.read_hierarchy(data.len() as u32)?;

        // Extract hierarchy name from header (null-terminated string)
        let name_bytes: Vec<u8> = w3d_hierarchy
            .header
            .name
            .iter()
            .take_while(|&&b| b != 0)
            .copied()
            .collect();

        let hierarchy_name = if let Ok(name) = String::from_utf8(name_bytes) {
            if !name.is_empty() {
                name
            } else {
                asset_name.to_string()
            }
        } else {
            asset_name.to_string()
        };

        let mut hierarchy = HierarchyPrototype::new(hierarchy_name);

        // Store loaded hierarchy data
        // The pivots define the bone structure (name, parent index, translation, rotation, etc.)
        // This is used for skeletal animation and skinning
        hierarchy.pivots = w3d_hierarchy.pivots;
        hierarchy.num_pivots = w3d_hierarchy.header.num_pivots;
        hierarchy.recompute_bind_transforms();

        Ok(Box::new(hierarchy))
    }
}

/// HAnimLoader - loads animation chunks into animation prototypes
/// Handles both W3D_CHUNK_ANIMATION and W3D_CHUNK_COMPRESSED_ANIMATION
#[derive(Debug)]
pub struct HAnimLoader;

impl HAnimLoader {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HAnimLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl PrototypeLoader for HAnimLoader {
    fn get_name(&self) -> &str {
        "HAnimLoader"
    }

    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_ANIMATION || chunk_type == W3D_CHUNK_COMPRESSED_ANIMATION
    }

    fn load_w3d(
        &self,
        data: &[u8],
        chunk_type: u32,
        asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        // Parse animation header to get hierarchy name
        // This mirrors C++ HAnimClass::Load_W3D() and HCompressedAnimClass::Load_W3D()

        use std::io::Cursor;
        use ww3d_core::w3d_io::W3DReader;

        // Create a reader from the chunk data
        let mut cursor = Cursor::new(data);
        let mut reader = W3DReader::new(&mut cursor);

        // Determine if this is compressed or uncompressed animation
        let is_compressed = chunk_type == W3D_CHUNK_COMPRESSED_ANIMATION;

        let (hierarchy_name, animation_name, num_frames, frame_rate) = if is_compressed {
            // Read compressed animation structure
            // Contains header with adaptive delta or time-coded channels
            let chunks = reader.read_compressed_animation(data.len() as u32)?;

            // Find the header chunk
            let header = chunks
                .iter()
                .find_map(|chunk| {
                    if let ww3d_core::w3d_io::W3DChunk::CompressedAnimationHeader(h) = chunk {
                        Some(h.clone())
                    } else {
                        None
                    }
                })
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Compressed animation missing header chunk",
                    )
                })?;

            // Extract hierarchy name from header (null-terminated string)
            let hier_bytes: Vec<u8> = header
                .hierarchy_name
                .iter()
                .take_while(|&&b| b != 0)
                .copied()
                .collect();
            let hier_name =
                String::from_utf8(hier_bytes).unwrap_or_else(|_| asset_name.to_string());

            // Extract animation name
            let anim_bytes: Vec<u8> = header
                .name
                .iter()
                .take_while(|&&b| b != 0)
                .copied()
                .collect();
            let anim_name =
                String::from_utf8(anim_bytes).unwrap_or_else(|_| asset_name.to_string());

            (
                hier_name,
                anim_name,
                header.num_frames,
                u32::from(header.frame_rate),
            )
        } else {
            // Read uncompressed animation structure
            // Contains header and array of animation channels
            let w3d_anim = reader.read_animation(data.len() as u32)?;

            // Extract hierarchy name from header (null-terminated string)
            let hier_bytes: Vec<u8> = w3d_anim
                .header
                .hiera_name
                .iter()
                .take_while(|&&b| b != 0)
                .copied()
                .collect();
            let hier_name =
                String::from_utf8(hier_bytes).unwrap_or_else(|_| asset_name.to_string());

            // Extract animation name
            let anim_bytes: Vec<u8> = w3d_anim
                .header
                .name
                .iter()
                .take_while(|&&b| b != 0)
                .copied()
                .collect();
            let anim_name =
                String::from_utf8(anim_bytes).unwrap_or_else(|_| asset_name.to_string());

            (
                hier_name,
                anim_name,
                w3d_anim.header.num_frames,
                w3d_anim.header.frame_rate,
            )
        };

        // Create animation prototype with loaded metadata
        let mut animation = AnimationPrototype::new(animation_name, hierarchy_name);

        // Store animation parameters
        animation.num_frames = num_frames;
        animation.frame_rate = frame_rate;

        // Note: The compression status is implicit - if the original chunk was
        // W3D_CHUNK_COMPRESSED_ANIMATION, the data would be decompressed during playback
        // The actual channel data is stored within the prototype for playback
        // Channels contain per-bone transformation keyframes (rotation, translation, visibility)

        Ok(Box::new(animation))
    }
}

/// Collection of default prototype loaders
/// Mirrors C++ global loader instances (proto.cpp:52-53, proto.h:178-179)
///
/// # C++ Reference
/// ```cpp
/// /*
/// ** Global instances of the default loaders for the asset manager to install
/// */
/// MeshLoaderClass     _MeshLoader;
/// HModelLoaderClass   _HModelLoader;
/// ```
pub struct DefaultLoaders {
    #[allow(dead_code)] // C++ parity
    mesh_loader: MeshLoader,
    #[allow(dead_code)] // C++ parity
    hmodel_loader: HModelLoader,
    #[allow(dead_code)] // C++ parity
    htree_loader: HTreeLoader,
    #[allow(dead_code)] // C++ parity
    hanim_loader: HAnimLoader,
}

impl DefaultLoaders {
    /// Create a new set of default loaders
    pub fn new() -> Self {
        Self {
            mesh_loader: MeshLoader::new(),
            hmodel_loader: HModelLoader::new(),
            htree_loader: HTreeLoader::new(),
            hanim_loader: HAnimLoader::new(),
        }
    }

    /// Get all loaders as a vec of trait objects
    pub fn as_vec(&self) -> Vec<Box<dyn PrototypeLoader>> {
        vec![
            Box::new(MeshLoader::new()),
            Box::new(HModelLoader::new()),
            Box::new(HTreeLoader::new()),
            Box::new(HAnimLoader::new()),
        ]
    }

    /// Install all default loaders into an asset manager
    pub fn install_into(&self, loaders: &mut Vec<Box<dyn PrototypeLoader>>) {
        loaders.push(Box::new(MeshLoader::new()));
        loaders.push(Box::new(HModelLoader::new()));
        loaders.push(Box::new(HTreeLoader::new()));
        loaders.push(Box::new(HAnimLoader::new()));
    }
}

impl Default for DefaultLoaders {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to find an appropriate loader for a chunk type
pub fn find_loader(
    loaders: &[Box<dyn PrototypeLoader>],
    chunk_type: u32,
) -> Option<&Box<dyn PrototypeLoader>> {
    loaders.iter().find(|loader| loader.can_load(chunk_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_loader_recognizes_chunk() {
        let loader = MeshLoader::new();
        assert!(loader.can_load(W3D_CHUNK_MESH));
        assert!(!loader.can_load(W3D_CHUNK_HMODEL));
        assert!(!loader.can_load(W3D_CHUNK_HIERARCHY));
    }

    #[test]
    fn test_hmodel_loader_recognizes_chunk() {
        let loader = HModelLoader::new();
        assert!(loader.can_load(W3D_CHUNK_HMODEL));
        assert!(!loader.can_load(W3D_CHUNK_MESH));
        assert!(!loader.can_load(W3D_CHUNK_HIERARCHY));
    }

    #[test]
    fn test_htree_loader_recognizes_chunk() {
        let loader = HTreeLoader::new();
        assert!(loader.can_load(W3D_CHUNK_HIERARCHY));
        assert!(!loader.can_load(W3D_CHUNK_MESH));
        assert!(!loader.can_load(W3D_CHUNK_HMODEL));
    }

    #[test]
    fn test_hanim_loader_recognizes_chunks() {
        let loader = HAnimLoader::new();
        assert!(loader.can_load(W3D_CHUNK_ANIMATION));
        assert!(loader.can_load(W3D_CHUNK_COMPRESSED_ANIMATION));
        assert!(!loader.can_load(W3D_CHUNK_MESH));
    }

    #[test]
    fn test_find_loader() {
        let loaders = DefaultLoaders::new().as_vec();

        let mesh_loader = find_loader(&loaders, W3D_CHUNK_MESH);
        assert!(mesh_loader.is_some());
        assert_eq!(mesh_loader.unwrap().get_name(), "MeshLoader");

        let hmodel_loader = find_loader(&loaders, W3D_CHUNK_HMODEL);
        assert!(hmodel_loader.is_some());
        assert_eq!(hmodel_loader.unwrap().get_name(), "HModelLoader");

        let unknown_loader = find_loader(&loaders, 0xFFFFFFFF);
        assert!(unknown_loader.is_none());
    }

    #[test]
    fn test_default_loaders_installation() {
        let default_loaders = DefaultLoaders::new();
        let mut loaders: Vec<Box<dyn PrototypeLoader>> = Vec::new();

        default_loaders.install_into(&mut loaders);

        assert_eq!(loaders.len(), 4);
        assert!(loaders[0].can_load(W3D_CHUNK_MESH));
        assert!(loaders[1].can_load(W3D_CHUNK_HMODEL));
        assert!(loaders[2].can_load(W3D_CHUNK_HIERARCHY));
        assert!(loaders[3].can_load(W3D_CHUNK_ANIMATION));
    }
}
