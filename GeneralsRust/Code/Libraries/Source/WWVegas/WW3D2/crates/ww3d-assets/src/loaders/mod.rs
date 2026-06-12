//! W3D File Format Loaders
//!
//! This module provides complete W3D file format loaders for meshes,
//! hierarchies, and animations. These loaders are faithful ports of the
//! C++ implementation from meshmdlio.cpp, htree.cpp, and hcompressedanim.cpp.
//!
//! # Organization
//! - `mesh_loader` - W3D mesh loading (vertices, materials, skinning)
//! - `hierarchy_loader` - W3D skeleton hierarchy loading
//! - `animation_loader` - W3D compressed animation loading
//! - `hlod_loader` - W3D HLOD loading (hierarchical LOD definitions)
//!
//! # C++ References
//! - meshmdlio.cpp - Mesh loading implementation
//! - htree.cpp - Hierarchy loading (lines 800-1200)
//! - hcompressedanim.cpp - Animation loading (lines 650-1200)

pub mod animation_loader;
pub mod hierarchy_loader;
pub mod hlod_loader;
pub mod mesh_loader;

pub use animation_loader::AnimationLoader;
pub use hierarchy_loader::HierarchyLoader;
pub use hlod_loader::HlodLoader;
pub use mesh_loader::MeshLoader;

use crate::assets::AssetManager;
use crate::loaders::mesh_loader::W3DMesh;
use crate::prototypes::{MaterialPassInfo, MeshPrototype};
use crate::w3d_loader::{W3DLoader, W3DModel};
use std::io::{self, Read, Seek, SeekFrom};
use ww3d_core::{
    W3dMaterialInfoStruct, W3dMeshHeader3Struct, W3dRGBAStruct, W3dTexCoordStruct,
    W3dTextureInfoStruct, W3dTextureStruct, W3dTriangleStruct, W3dVectorStruct,
};

// Compatibility re-exports from old loaders module
// These are stub types to maintain compatibility with existing code

/// Animation data (compatibility stub)
#[derive(Debug, Clone)]
pub struct AnimationData {
    pub name: String,
    pub frames: u32,
}

/// Parse a W3D file and register renderable prototypes with the asset manager.
pub fn parse_w3d_file<R: Read + Seek>(
    reader: &mut R,
    asset_manager: &mut AssetManager,
) -> io::Result<()> {
    parse_w3d_file_with_asset_name(reader, asset_manager, None)
}

/// Parse a W3D file and register renderable prototypes, optionally adding a
/// file-stem alias for simple single-mesh files.
pub fn parse_w3d_file_with_asset_name<R: Read + Seek>(
    reader: &mut R,
    asset_manager: &mut AssetManager,
    asset_name: Option<&str>,
) -> io::Result<()> {
    let mut data = Vec::new();
    reader.seek(SeekFrom::Start(0))?;
    reader.read_to_end(&mut data)?;

    let model = W3DLoader::load_from_bytes(&data)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    register_loaded_model(asset_manager, model, asset_name);
    Ok(())
}

fn register_loaded_model(
    asset_manager: &mut AssetManager,
    model: W3DModel,
    asset_name: Option<&str>,
) {
    let single_mesh_alias = asset_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .filter(|_| model.meshes.len() == 1)
        .filter(|name| {
            model
                .hlods
                .iter()
                .all(|hlod| !hlod.name.eq_ignore_ascii_case(name))
        });

    for mesh in &model.meshes {
        let prototype = mesh_to_prototype(mesh, None);
        let mesh_name = prototype.name.clone();
        asset_manager.add_prototype(mesh_name, Box::new(prototype));
    }

    for hlod in model.hlods {
        asset_manager.add_prototype(hlod.name.clone(), Box::new(hlod));
    }

    for hmodel in model.hmodels {
        asset_manager.add_prototype(hmodel.name.clone(), Box::new(hmodel));
    }

    if let (Some(alias), Some(mesh)) = (single_mesh_alias, model.meshes.first()) {
        let mesh_name = normalized_mesh_name(mesh, None);
        if !mesh_name.eq_ignore_ascii_case(alias) {
            let prototype = mesh_to_prototype(mesh, Some(alias));
            asset_manager.add_prototype(alias.to_string(), Box::new(prototype));
        }
    }
}

fn mesh_to_prototype(mesh: &W3DMesh, name_override: Option<&str>) -> MeshPrototype {
    let name = normalized_mesh_name(mesh, name_override);
    let mut prototype = MeshPrototype::new(name);

    prototype.header = Some(W3dMeshHeader3Struct {
        version: mesh.header.version,
        attrs: mesh.header.attributes,
        mesh_name: fixed_bytes::<16>(&mesh.header.mesh_name),
        container_name: fixed_bytes::<16>(&mesh.header.container_name),
        num_tris: mesh.header.num_tris,
        num_verts: mesh.header.num_vertices,
        num_materials: mesh.header.num_materials,
        num_damage_stages: mesh.header.num_damage_stages,
        sort_level: mesh.header.sort_level,
        prelit_version: mesh.header.prelit_version,
        future_counts: [mesh.header.future_count],
        vertex_channels: mesh.header.vertex_channels,
        face_channels: mesh.header.face_channels,
        bbox_min: vec3_to_w3d(mesh.header.min),
        bbox_max: vec3_to_w3d(mesh.header.max),
        sph_center: vec3_to_w3d(mesh.header.sph_center),
        sph_radius: mesh.header.sph_radius,
    });

    prototype.vertices = mesh.vertices.iter().copied().map(vec3_to_w3d).collect();
    prototype.normals = mesh.normals.iter().copied().map(vec3_to_w3d).collect();
    prototype.triangles = mesh
        .triangles
        .iter()
        .enumerate()
        .map(|(index, triangle)| W3dTriangleStruct {
            vindex: *triangle,
            attributes: mesh.triangle_attributes.get(index).copied().unwrap_or(0),
            normal: W3dVectorStruct::default(),
            distance: 0.0,
        })
        .collect();
    prototype.material_info =
        mesh.material_info
            .as_ref()
            .map(|material_info| W3dMaterialInfoStruct {
                pass_count: material_info.pass_count,
                vert_matl_count: material_info.vertex_material_count,
                shader_count: material_info.shader_count,
                texture_count: material_info.texture_count,
            });
    prototype.textures = mesh
        .textures
        .iter()
        .map(|texture| W3dTextureStruct {
            name: fixed_bytes::<256>(&texture.name),
            texture_info: W3dTextureInfoStruct {
                attributes: (texture.texture_info & 0xffff) as u16,
                animation_type: 0,
                frame_count: 1,
                frame_rate: 30.0,
            },
        })
        .collect();
    prototype.vertex_shade_indices =
        (!mesh.shade_indices.is_empty()).then(|| mesh.shade_indices.clone());
    if !mesh.tex_coords.is_empty() {
        prototype.stage_texcoords.push(
            mesh.tex_coords
                .iter()
                .map(|tex_coord| W3dTexCoordStruct {
                    u: tex_coord.x,
                    v: tex_coord.y,
                })
                .collect(),
        );
    }

    for pass in &mesh.material_passes {
        prototype.passes.push(MaterialPassInfo {
            vm_id: pass.vertex_material_ids.first().copied().unwrap_or(0),
            shader_id: pass.shader_ids.first().copied().unwrap_or(0),
            texture_count: pass.texture_stages.len() as u32,
        });
        prototype
            .per_pass_vertex_material_ids
            .push(pass.vertex_material_ids.clone());
        prototype.per_pass_shader_ids.push(pass.shader_ids.clone());
        prototype.per_pass_stage_texture_ids.push(
            pass.texture_stages
                .iter()
                .map(|stage| stage.texture_ids.clone())
                .collect(),
        );
        prototype
            .per_pass_dcg_colors
            .push(pass.dcg.iter().copied().map(vec4_to_rgba).collect());
        prototype
            .per_pass_dig_colors
            .push(pass.dig.iter().copied().map(vec4_to_rgba).collect());
        for (stage_index, stage) in pass.texture_stages.iter().enumerate() {
            if prototype.stage_texcoords.len() <= stage_index {
                prototype
                    .stage_texcoords
                    .resize_with(stage_index + 1, Vec::new);
            }
            if prototype.per_face_texcoord_ids.len() <= stage_index {
                prototype
                    .per_face_texcoord_ids
                    .resize_with(stage_index + 1, Vec::new);
            }
            if !stage.tex_coords.is_empty() {
                prototype.stage_texcoords[stage_index] = stage
                    .tex_coords
                    .iter()
                    .map(|tex_coord| W3dTexCoordStruct {
                        u: tex_coord.x,
                        v: tex_coord.y,
                    })
                    .collect();
            }
            if !stage.per_face_texcoord_ids.is_empty() {
                prototype.per_face_texcoord_ids[stage_index] = stage.per_face_texcoord_ids.clone();
            }
        }
    }

    prototype
}

fn normalized_mesh_name(mesh: &W3DMesh, name_override: Option<&str>) -> String {
    name_override
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .or_else(|| {
            mesh.header
                .mesh_name
                .trim()
                .is_empty()
                .then_some("unnamed_mesh")
                .or(Some(mesh.header.mesh_name.trim()))
        })
        .unwrap()
        .to_string()
}

fn vec3_to_w3d(value: glam::Vec3) -> W3dVectorStruct {
    W3dVectorStruct {
        x: value.x,
        y: value.y,
        z: value.z,
    }
}

fn vec4_to_rgba(value: glam::Vec4) -> W3dRGBAStruct {
    W3dRGBAStruct {
        r: (value.x.clamp(0.0, 1.0) * 255.0).round() as u8,
        g: (value.y.clamp(0.0, 1.0) * 255.0).round() as u8,
        b: (value.z.clamp(0.0, 1.0) * 255.0).round() as u8,
        a: (value.w.clamp(0.0, 1.0) * 255.0).round() as u8,
    }
}

fn fixed_bytes<const N: usize>(value: &str) -> [u8; N] {
    let mut bytes = [0; N];
    let source = value.as_bytes();
    let copy_len = source.len().min(N);
    bytes[..copy_len].copy_from_slice(&source[..copy_len]);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prototypes::HModelPrototype;
    use std::io::Cursor;
    use ww3d_core::W3DChunkType;

    #[test]
    fn parse_w3d_file_registers_mesh_prototype() {
        let bytes = minimal_mesh_w3d("TestMesh");
        let mut reader = Cursor::new(bytes);
        let mut asset_manager = AssetManager::new();

        parse_w3d_file(&mut reader, &mut asset_manager).unwrap();

        assert!(asset_manager.is_asset_loaded("testmesh"));
        assert!(asset_manager.create_render_obj("TESTMESH").is_some());

        let mesh = asset_manager
            .get_prototype_as::<MeshPrototype>("TestMesh")
            .unwrap();
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.normals.len(), 3);
        assert_eq!(mesh.triangles.len(), 1);
        assert_eq!(mesh.triangles[0].vindex, [0, 1, 2]);
    }

    #[test]
    fn parse_w3d_file_registers_single_mesh_asset_alias() {
        let bytes = minimal_mesh_w3d("InternalMesh");
        let mut reader = Cursor::new(bytes);
        let mut asset_manager = AssetManager::new();

        parse_w3d_file_with_asset_name(&mut reader, &mut asset_manager, Some("FileAlias")).unwrap();

        assert!(asset_manager.is_asset_loaded("internalmesh"));
        assert!(asset_manager.is_asset_loaded("filealias"));
        assert!(asset_manager.create_render_obj("FileAlias").is_some());
    }

    #[test]
    fn parse_w3d_file_registers_hmodel_prototype() {
        let bytes = minimal_hmodel_w3d("TankModel", "TankTree", "Body");
        let mut reader = Cursor::new(bytes);
        let mut asset_manager = AssetManager::new();

        parse_w3d_file(&mut reader, &mut asset_manager).unwrap();

        assert!(asset_manager.is_asset_loaded("tankmodel"));
        assert!(asset_manager.create_render_obj("TANKMODEL").is_some());

        let hmodel = asset_manager
            .get_prototype_as::<HModelPrototype>("TankModel")
            .unwrap();
        assert_eq!(hmodel.hierarchy_name, "TankTree");
        assert_eq!(hmodel.nodes.len(), 1);
        assert_eq!(hmodel.nodes[0].render_obj_name, "TankModel.Body");
        assert_eq!(hmodel.nodes[0].pivot_idx, 3);
    }

    fn minimal_mesh_w3d(mesh_name: &str) -> Vec<u8> {
        let header = chunk(W3DChunkType::MeshHeader3, false, mesh_header(mesh_name));
        let vertices = chunk(
            W3DChunkType::Vertices,
            false,
            vectors(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
        );
        let normals = chunk(
            W3DChunkType::VertexNormals,
            false,
            vectors(&[[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]]),
        );
        let triangles = chunk(W3DChunkType::Triangles, false, u32s(&[0, 1, 2, 0]));
        chunk(
            W3DChunkType::Mesh,
            true,
            [header, vertices, normals, triangles].concat(),
        )
    }

    fn mesh_header(mesh_name: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_u32(&mut bytes, 0x0003_0000);
        push_u32(&mut bytes, 0);
        bytes.extend_from_slice(&fixed_bytes::<16>(mesh_name));
        bytes.extend_from_slice(&fixed_bytes::<16>(""));
        push_u32(&mut bytes, 1);
        push_u32(&mut bytes, 3);
        push_u32(&mut bytes, 0);
        push_u32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_u32(&mut bytes, 0);
        push_u32(&mut bytes, 0);
        push_u32(&mut bytes, 0x0000_0007);
        push_u32(&mut bytes, 0x0000_0001);
        push_vec3(&mut bytes, [0.0, 0.0, 0.0]);
        push_vec3(&mut bytes, [1.0, 1.0, 0.0]);
        push_vec3(&mut bytes, [0.5, 0.5, 0.0]);
        push_f32(&mut bytes, 1.0);
        bytes
    }

    fn minimal_hmodel_w3d(model_name: &str, hierarchy_name: &str, node_name: &str) -> Vec<u8> {
        let header = chunk(
            W3DChunkType::HmodelHeader,
            false,
            hmodel_header(model_name, hierarchy_name, 1),
        );
        let node = chunk(W3DChunkType::Node, false, hmodel_node(node_name, 3));
        chunk(W3DChunkType::Hmodel, true, [header, node].concat())
    }

    fn hmodel_header(model_name: &str, hierarchy_name: &str, connections: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_u32(&mut bytes, 0x0003_0000);
        bytes.extend_from_slice(&fixed_bytes::<16>(model_name));
        bytes.extend_from_slice(&fixed_bytes::<16>(hierarchy_name));
        push_u32(&mut bytes, connections);
        bytes
    }

    fn hmodel_node(render_obj_name: &str, pivot_idx: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&fixed_bytes::<16>(render_obj_name));
        push_u32(&mut bytes, pivot_idx);
        bytes
    }

    fn chunk(chunk_type: W3DChunkType, has_sub_chunks: bool, payload: Vec<u8>) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_u32(&mut bytes, chunk_type.as_u32());
        let sub_chunk_flag = if has_sub_chunks { 0x8000_0000 } else { 0 };
        push_u32(&mut bytes, payload.len() as u32 | sub_chunk_flag);
        bytes.extend_from_slice(&payload);
        bytes
    }

    fn vectors(values: &[[f32; 3]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in values {
            push_vec3(&mut bytes, *value);
        }
        bytes
    }

    fn u32s(values: &[u32]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in values {
            push_u32(&mut bytes, *value);
        }
        bytes
    }

    fn push_vec3(bytes: &mut Vec<u8>, value: [f32; 3]) {
        push_f32(bytes, value[0]);
        push_f32(bytes, value[1]);
        push_f32(bytes, value[2]);
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_f32(bytes: &mut Vec<u8>, value: f32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
