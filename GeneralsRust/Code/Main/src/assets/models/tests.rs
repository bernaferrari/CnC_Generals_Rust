//! Mechanical split from `assets/models.rs` tests.
#![allow(dead_code, unused_imports)]
use super::prelude::*;
use super::w3d_anim::*;
use super::w3d_format::*;
use super::w3d_loader::*;
use super::w3d_loader_parse::*;
use super::w3d_mesh::*;
use super::w3d_mesh_build::*;
use super::w3d_model::*;
use super::*;

fn chunk(chunk_type: u32, payload: Vec<u8>, container: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + payload.len());
    out.extend_from_slice(&chunk_type.to_le_bytes());
    let raw_size = (payload.len() as u32) | if container { 0x8000_0000 } else { 0 };
    out.extend_from_slice(&raw_size.to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

fn fixed_name(name: &str, len: usize) -> Vec<u8> {
    let mut out = vec![0; len];
    let bytes = name.as_bytes();
    let copy_len = bytes.len().min(len);
    out[..copy_len].copy_from_slice(&bytes[..copy_len]);
    out
}

fn vertex_influence_payload(records: &[(u16, [u8; 6])], trailing: &[u8]) -> Vec<u8> {
    let mut payload =
        Vec::with_capacity(records.len() * W3D_VERTEX_INFLUENCE_RECORD_SIZE + trailing.len());
    for (bone_idx, pad) in records {
        payload.extend_from_slice(&bone_idx.to_le_bytes());
        payload.extend_from_slice(pad);
    }
    payload.extend_from_slice(trailing);
    payload
}

/// Small source-shaped mesh container for the exact C++ skin-link loader.
/// It deliberately has three vertices because the chunk reader must use
/// `NumVertices`, not derive a record count from the payload length.
fn mesh_with_vertex_influence_chunks(version: u32, influence_chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut mesh_header = vec![0; 116];
    mesh_header[0..4].copy_from_slice(&version.to_le_bytes());
    mesh_header[8..24].copy_from_slice(&fixed_name("SKIN", W3D_NAME_LEN));
    mesh_header[40..44].copy_from_slice(&1u32.to_le_bytes());
    mesh_header[44..48].copy_from_slice(&3u32.to_le_bytes());

    let mut vertices = Vec::new();
    for vertex in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        for value in vertex {
            vertices.extend_from_slice(&value.to_le_bytes());
        }
    }
    let mut triangle = Vec::with_capacity(32);
    triangle.extend_from_slice(&0u32.to_le_bytes());
    triangle.extend_from_slice(&1u32.to_le_bytes());
    triangle.extend_from_slice(&2u32.to_le_bytes());
    triangle.extend_from_slice(&[0u8; 20]);

    let mut mesh_payload = [
        chunk(W3D_CHUNK_MESH_HEADER, mesh_header, false),
        chunk(W3D_CHUNK_VERTICES, vertices, false),
        chunk(W3D_CHUNK_TRIANGLES, triangle, false),
    ]
    .concat();
    for influence_chunk in influence_chunks {
        mesh_payload.extend_from_slice(&chunk(
            W3D_CHUNK_VERTEX_INFLUENCES,
            influence_chunk.clone(),
            false,
        ));
    }
    chunk(W3D_CHUNK_MESH, mesh_payload, true)
}

fn pivot(name: &str, parent: u32, translation: [f32; 3]) -> Vec<u8> {
    let mut out = fixed_name(name, W3D_NAME_LEN);
    out.extend_from_slice(&parent.to_le_bytes());
    for value in translation {
        out.extend_from_slice(&value.to_le_bytes());
    }
    // Euler angles.
    out.extend_from_slice(&[0u8; 12]);
    // Identity quaternion [x, y, z, w].
    out.extend_from_slice(&0.0f32.to_le_bytes());
    out.extend_from_slice(&0.0f32.to_le_bytes());
    out.extend_from_slice(&0.0f32.to_le_bytes());
    out.extend_from_slice(&1.0f32.to_le_bytes());
    assert_eq!(out.len(), 60);
    out
}

fn hlod_attachment_array(chunk_type: u32, entries: &[(&str, u32)]) -> Vec<u8> {
    assert!(
        matches!(
            chunk_type,
            W3D_CHUNK_HLOD_AGGREGATE_ARRAY | W3D_CHUNK_HLOD_PROXY_ARRAY
        ),
        "synthetic attachment must be an aggregate or proxy array"
    );
    assert!(
        !entries.is_empty(),
        "C++ HLOD exporter omits empty aggregate/proxy arrays"
    );

    let mut array_header = Vec::with_capacity(8);
    array_header.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    // C++ `HLodSaveClass` writes zero here for aggregate/proxy arrays.
    // It is metadata, not an attachment-LOD threshold.
    array_header.extend_from_slice(&0.0f32.to_le_bytes());
    let mut payload = chunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, array_header, false);
    for (name, bone_index) in entries {
        let mut subobject = Vec::with_capacity(36);
        subobject.extend_from_slice(&bone_index.to_le_bytes());
        subobject.extend_from_slice(&fixed_name(name, 32));
        payload.extend_from_slice(&chunk(W3D_CHUNK_HLOD_SUB_OBJECT, subobject, false));
    }
    chunk(chunk_type, payload, true)
}

fn rigid_hlod_fixture(
    lod_count: usize,
    aggregate_entries: &[(&str, u32)],
    proxy_entries: &[(&str, u32)],
) -> Vec<u8> {
    let mut hierarchy_header = Vec::with_capacity(36);
    hierarchy_header.extend_from_slice(&W3D_CURRENT_HTREE_VERSION.to_le_bytes());
    hierarchy_header.extend_from_slice(&fixed_name("RIG_HIER", W3D_NAME_LEN));
    hierarchy_header.extend_from_slice(&2u32.to_le_bytes());
    hierarchy_header.extend_from_slice(&[0u8; 12]);
    let mut pivots = pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]);
    // Deliberately does not match the mesh name.  HLOD BoneIndex, not a
    // pivot-name heuristic, must produce this child transform.
    pivots.extend_from_slice(&pivot("AUTHORED_BONE", 0, [10.0, 20.0, 30.0]));
    let hierarchy = chunk(
        W3D_CHUNK_HIERARCHY,
        [
            chunk(W3D_CHUNK_HIERARCHY_HEADER, hierarchy_header, false),
            chunk(W3D_CHUNK_PIVOTS, pivots, false),
        ]
        .concat(),
        true,
    );

    let mut mesh_header = vec![0; 116];
    mesh_header[0..4].copy_from_slice(&1u32.to_le_bytes());
    mesh_header[8..24].copy_from_slice(&fixed_name("RIGID", W3D_NAME_LEN));
    mesh_header[24..40].copy_from_slice(&fixed_name("HLODROOT", W3D_NAME_LEN));
    mesh_header[40..44].copy_from_slice(&1u32.to_le_bytes());
    mesh_header[44..48].copy_from_slice(&3u32.to_le_bytes());
    let mut vertices = Vec::new();
    for vertex in [[1.0f32, 2.0, 3.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        for value in vertex {
            vertices.extend_from_slice(&value.to_le_bytes());
        }
    }
    let mut normals = Vec::new();
    for _ in 0..3 {
        normals.extend_from_slice(&0.0f32.to_le_bytes());
        normals.extend_from_slice(&0.0f32.to_le_bytes());
        normals.extend_from_slice(&1.0f32.to_le_bytes());
    }
    let mut triangle = Vec::with_capacity(32);
    triangle.extend_from_slice(&0u32.to_le_bytes());
    triangle.extend_from_slice(&1u32.to_le_bytes());
    triangle.extend_from_slice(&2u32.to_le_bytes());
    triangle.extend_from_slice(&[0u8; 20]);
    let mesh = chunk(
        W3D_CHUNK_MESH,
        [
            chunk(W3D_CHUNK_MESH_HEADER, mesh_header, false),
            chunk(W3D_CHUNK_VERTICES, vertices, false),
            chunk(W3D_CHUNK_VERTEX_NORMALS, normals, false),
            chunk(W3D_CHUNK_TRIANGLES, triangle, false),
        ]
        .concat(),
        true,
    );

    let mut hlod_header = Vec::with_capacity(40);
    hlod_header.extend_from_slice(&0x0001_0000u32.to_le_bytes());
    hlod_header.extend_from_slice(&(lod_count as u32).to_le_bytes());
    hlod_header.extend_from_slice(&fixed_name("HLODROOT", W3D_NAME_LEN));
    hlod_header.extend_from_slice(&fixed_name("RIG_HIER", W3D_NAME_LEN));
    let mut hlod_payload = chunk(W3D_CHUNK_HLOD_HEADER, hlod_header, false);
    for _ in 0..lod_count {
        let mut array_header = Vec::with_capacity(8);
        array_header.extend_from_slice(&1u32.to_le_bytes());
        array_header.extend_from_slice(&f32::MAX.to_le_bytes());
        let mut subobject = Vec::with_capacity(36);
        subobject.extend_from_slice(&1u32.to_le_bytes());
        subobject.extend_from_slice(&fixed_name("HLODROOT.RIGID", 32));
        let lod_payload = [
            chunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, array_header, false),
            chunk(W3D_CHUNK_HLOD_SUB_OBJECT, subobject, false),
        ]
        .concat();
        hlod_payload.extend_from_slice(&chunk(W3D_CHUNK_HLOD_LOD_ARRAY, lod_payload, true));
    }
    if !aggregate_entries.is_empty() {
        hlod_payload.extend_from_slice(&hlod_attachment_array(
            W3D_CHUNK_HLOD_AGGREGATE_ARRAY,
            aggregate_entries,
        ));
    }
    if !proxy_entries.is_empty() {
        hlod_payload.extend_from_slice(&hlod_attachment_array(
            W3D_CHUNK_HLOD_PROXY_ARRAY,
            proxy_entries,
        ));
    }

    [hierarchy, mesh, chunk(W3D_CHUNK_HLOD, hlod_payload, true)].concat()
}

/// Build one source-shaped C++ `HModelDefClass` container. The HMODEL
/// structs use the original MSVC `sizeof` payloads (40-byte header,
/// 18-byte connection) because `hmdldef.cpp` reads them directly.
fn hmodel_fixture_chunk(
    version: u32,
    model_name: &str,
    hierarchy_name: &str,
    nodes: &[(u32, &str, u16)],
) -> Vec<u8> {
    hmodel_fixture_chunk_with_trailing(version, model_name, hierarchy_name, nodes, &[])
}

fn hmodel_fixture_chunk_with_trailing(
    version: u32,
    model_name: &str,
    hierarchy_name: &str,
    nodes: &[(u32, &str, u16)],
    trailing_chunks: &[(u32, Vec<u8>)],
) -> Vec<u8> {
    let mut header = Vec::with_capacity(40);
    header.extend_from_slice(&version.to_le_bytes());
    header.extend_from_slice(&fixed_name(model_name, W3D_NAME_LEN));
    header.extend_from_slice(&fixed_name(hierarchy_name, W3D_NAME_LEN));
    header.extend_from_slice(&(nodes.len() as u16).to_le_bytes());
    header.extend_from_slice(&[0u8; 2]);
    assert_eq!(header.len(), 40);

    let mut payload = chunk(W3D_CHUNK_HMODEL_HEADER, header, false);
    for (chunk_type, leaf_name, pivot_index) in nodes {
        assert!(
            matches!(
                *chunk_type,
                W3D_CHUNK_HMODEL_NODE
                    | W3D_CHUNK_HMODEL_COLLISION_NODE
                    | W3D_CHUNK_HMODEL_SKIN_NODE
            ),
            "fixture may only create HMODEL connection chunks"
        );
        let mut node = fixed_name(leaf_name, W3D_NAME_LEN);
        node.extend_from_slice(&pivot_index.to_le_bytes());
        assert_eq!(node.len(), 18);
        payload.extend_from_slice(&chunk(*chunk_type, node, false));
    }
    for (chunk_type, trailing_payload) in trailing_chunks {
        payload.extend_from_slice(&chunk(*chunk_type, trailing_payload.clone(), false));
    }
    chunk(W3D_CHUNK_HMODEL, payload, true)
}

fn hmodel_snap_points_payload(points: &[[f32; 3]], trailing: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(points.len() * 12 + trailing.len());
    for point in points {
        for component in point {
            payload.extend_from_slice(&component.to_le_bytes());
        }
    }
    payload.extend_from_slice(trailing);
    payload
}

fn rigid_hmodel_fixture(version: u32, nodes: &[(u32, &str, u16)]) -> Vec<u8> {
    [
        rigid_hlod_fixture(1, &[], &[]),
        hmodel_fixture_chunk(version, "RIG_HMODEL", "RIG_HIER", nodes),
    ]
    .concat()
}

fn visibility_hlod_fixture() -> Vec<u8> {
    let mut hierarchy_header = Vec::with_capacity(36);
    hierarchy_header.extend_from_slice(&W3D_CURRENT_HTREE_VERSION.to_le_bytes());
    hierarchy_header.extend_from_slice(&fixed_name("VIS_HIER", W3D_NAME_LEN));
    hierarchy_header.extend_from_slice(&2u32.to_le_bytes());
    hierarchy_header.extend_from_slice(&[0u8; 12]);
    let mut pivots = pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]);
    pivots.extend_from_slice(&pivot("NOT_A_MESH_NAME", 0, [3.0, 4.0, 5.0]));
    let hierarchy = chunk(
        W3D_CHUNK_HIERARCHY,
        [
            chunk(W3D_CHUNK_HIERARCHY_HEADER, hierarchy_header, false),
            chunk(W3D_CHUNK_PIVOTS, pivots, false),
        ]
        .concat(),
        true,
    );

    let mut mesh_header = vec![0; 116];
    mesh_header[0..4].copy_from_slice(&1u32.to_le_bytes());
    mesh_header[8..24].copy_from_slice(&fixed_name("body_d", W3D_NAME_LEN));
    mesh_header[24..40].copy_from_slice(&fixed_name("VIS_HLOD", W3D_NAME_LEN));
    mesh_header[40..44].copy_from_slice(&1u32.to_le_bytes());
    mesh_header[44..48].copy_from_slice(&3u32.to_le_bytes());
    let mut vertices = Vec::new();
    for vertex in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        for value in vertex {
            vertices.extend_from_slice(&value.to_le_bytes());
        }
    }
    let mut normals = Vec::new();
    for _ in 0..3 {
        normals.extend_from_slice(&0.0f32.to_le_bytes());
        normals.extend_from_slice(&0.0f32.to_le_bytes());
        normals.extend_from_slice(&1.0f32.to_le_bytes());
    }
    let mut triangle = Vec::with_capacity(32);
    triangle.extend_from_slice(&0u32.to_le_bytes());
    triangle.extend_from_slice(&1u32.to_le_bytes());
    triangle.extend_from_slice(&2u32.to_le_bytes());
    triangle.extend_from_slice(&[0u8; 20]);
    let mesh = chunk(
        W3D_CHUNK_MESH,
        [
            chunk(W3D_CHUNK_MESH_HEADER, mesh_header, false),
            chunk(W3D_CHUNK_VERTICES, vertices, false),
            chunk(W3D_CHUNK_VERTEX_NORMALS, normals, false),
            chunk(W3D_CHUNK_TRIANGLES, triangle, false),
        ]
        .concat(),
        true,
    );

    let mut animation_header = Vec::with_capacity(44);
    animation_header.extend_from_slice(&W3D_CURRENT_HANIM_VERSION.to_le_bytes());
    animation_header.extend_from_slice(&fixed_name("VIS_CLIP", W3D_NAME_LEN));
    animation_header.extend_from_slice(&fixed_name("VIS_HIER", W3D_NAME_LEN));
    animation_header.extend_from_slice(&4u32.to_le_bytes());
    animation_header.extend_from_slice(&30u32.to_le_bytes());
    // first=2, last=4, flags=BIT_CHANNEL_VIS, pivot=1, default=visible;
    // bits 0b0000_0101 make frames [2, 3, 4] => [true, false, true].
    let mut raw_bit_channel = Vec::new();
    raw_bit_channel.extend_from_slice(&2u16.to_le_bytes());
    raw_bit_channel.extend_from_slice(&4u16.to_le_bytes());
    raw_bit_channel.extend_from_slice(&0u16.to_le_bytes());
    raw_bit_channel.extend_from_slice(&1u16.to_le_bytes());
    raw_bit_channel.push(1);
    raw_bit_channel.push(0b0000_0101);
    let animation = chunk(
        W3D_CHUNK_ANIMATION,
        [
            chunk(W3D_CHUNK_ANIMATION_HEADER, animation_header, false),
            chunk(W3D_CHUNK_BIT_CHANNEL, raw_bit_channel, false),
        ]
        .concat(),
        true,
    );

    let mut hlod_header = Vec::with_capacity(40);
    hlod_header.extend_from_slice(&0x0001_0000u32.to_le_bytes());
    hlod_header.extend_from_slice(&1u32.to_le_bytes());
    hlod_header.extend_from_slice(&fixed_name("VIS_HLOD", W3D_NAME_LEN));
    hlod_header.extend_from_slice(&fixed_name("VIS_HIER", W3D_NAME_LEN));
    let mut array_header = Vec::with_capacity(8);
    array_header.extend_from_slice(&1u32.to_le_bytes());
    array_header.extend_from_slice(&f32::MAX.to_le_bytes());
    let mut subobject = Vec::with_capacity(36);
    subobject.extend_from_slice(&1u32.to_le_bytes());
    subobject.extend_from_slice(&fixed_name("VIS_HLOD.body_d", 32));
    let lod_payload = [
        chunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, array_header, false),
        chunk(W3D_CHUNK_HLOD_SUB_OBJECT, subobject, false),
    ]
    .concat();
    let hlod = chunk(
        W3D_CHUNK_HLOD,
        [
            chunk(W3D_CHUNK_HLOD_HEADER, hlod_header, false),
            chunk(W3D_CHUNK_HLOD_LOD_ARRAY, lod_payload, true),
        ]
        .concat(),
        true,
    );

    [hierarchy, mesh, animation, hlod].concat()
}

/// A source-shaped single-HLOD fixture with a parent, a descendant, and a
/// sibling.  Mesh and pivot names intentionally differ; only retained
/// `HLOD.Name.Child -> BoneIndex` records are legal visibility bindings.
fn hide_show_subobjects_hlod_model() -> W3DModel {
    let mut model = W3DModel::new("hide_show_subobjects".to_string());
    model.hierarchy = Some(W3dHierarchy {
        name: "VIS_HIER".to_string(),
        pivots: vec![
            W3dPivot {
                name: "ROOT".to_string(),
                parent_idx: u32::MAX,
                translation: [0.0; 3],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
            W3dPivot {
                name: "PARENT_BONE".to_string(),
                parent_idx: 0,
                translation: [0.0; 3],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
            W3dPivot {
                name: "CHILD_BONE".to_string(),
                parent_idx: 1,
                translation: [0.0; 3],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
            W3dPivot {
                name: "SIBLING_BONE".to_string(),
                parent_idx: 0,
                translation: [0.0; 3],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
        ],
        pivot_fixups: Vec::new(),
    });
    model.hlods.push(W3dHlod {
        version: 0x0001_0000,
        name: "VIS_HLOD".to_string(),
        hierarchy_name: "VIS_HIER".to_string(),
        lods: vec![W3dHlodLod {
            max_screen_size: f32::MAX,
            subobjects: vec![
                W3dHlodSubObject {
                    name: "VIS_HLOD.ParentMesh".to_string(),
                    bone_index: 1,
                },
                W3dHlodSubObject {
                    // C++ only hides the directly named RenderObj on a
                    // bone, not another direct sibling sharing that bone.
                    name: "VIS_HLOD.SameBoneMesh".to_string(),
                    bone_index: 1,
                },
                W3dHlodSubObject {
                    name: "VIS_HLOD.ChildMesh".to_string(),
                    bone_index: 2,
                },
                W3dHlodSubObject {
                    name: "VIS_HLOD.SiblingMesh".to_string(),
                    bone_index: 3,
                },
            ],
        }],
        aggregates: None,
        proxies: None,
        has_unrendered_aggregates: false,
        has_invalid_trailing_records: false,
    });
    model.meshes = ["ParentMesh", "SameBoneMesh", "ChildMesh", "SiblingMesh"]
        .into_iter()
        .map(|name| {
            let mut mesh = W3DMesh::new(name.to_string());
            mesh.container_name = "VIS_HLOD".to_string();
            mesh
        })
        .collect();
    model
}

/// One exact supported HLOD tree for primary-turret control tests. Mesh
/// names deliberately differ from pivot names: only the source HLOD child
/// records and exact source `Turret`/`TurretPitch` pivot names may bind.
fn primary_turret_hlod_model() -> W3DModel {
    fn test_pivot(name: &str, parent_idx: u32, translation: [f32; 3]) -> W3dPivot {
        W3dPivot {
            name: name.to_string(),
            parent_idx,
            translation,
            euler_angles: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    let mut model = W3DModel::new("primary_turret_hlod".to_string());
    model.hierarchy = Some(W3dHierarchy {
        name: "TURRET_HIER".to_string(),
        pivots: vec![
            test_pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]),
            test_pivot("HULL_PIVOT", 0, [-4.0, 0.0, 0.0]),
            test_pivot("YAW_PIVOT", 0, [0.0, 10.0, 0.0]),
            test_pivot("PITCH_PIVOT", 2, [5.0, 0.0, 0.0]),
            test_pivot("MUZZLE_PIVOT", 3, [3.0, 0.0, 0.0]),
        ],
        pivot_fixups: Vec::new(),
    });
    model.hlods.push(W3dHlod {
        version: 0x0001_0000,
        name: "TURRET_HLOD".to_string(),
        hierarchy_name: "TURRET_HIER".to_string(),
        lods: vec![W3dHlodLod {
            max_screen_size: f32::MAX,
            subobjects: vec![
                W3dHlodSubObject {
                    name: "TURRET_HLOD.ChassisMesh".to_string(),
                    bone_index: 1,
                },
                W3dHlodSubObject {
                    name: "TURRET_HLOD.GunHousingMesh".to_string(),
                    bone_index: 2,
                },
                W3dHlodSubObject {
                    name: "TURRET_HLOD.BarrelMesh".to_string(),
                    bone_index: 3,
                },
                W3dHlodSubObject {
                    name: "TURRET_HLOD.FlashMesh".to_string(),
                    bone_index: 4,
                },
            ],
        }],
        aggregates: None,
        proxies: None,
        has_unrendered_aggregates: false,
        has_invalid_trailing_records: false,
    });
    model.meshes = ["ChassisMesh", "GunHousingMesh", "BarrelMesh", "FlashMesh"]
        .into_iter()
        .map(|name| {
            let mut mesh = W3DMesh::new(name.to_string());
            mesh.container_name = "TURRET_HLOD".to_string();
            mesh
        })
        .collect();
    // A selected source HAnim changes YAW_PIVOT's X translation at frame
    // one. The turret control must apply to that sampled pose and then
    // propagate through PITCH_PIVOT and MUZZLE_PIVOT, not to a separate
    // global hull matrix.
    model.animations.push(W3dAnimation {
        name: "TURRET_POSE".to_string(),
        hierarchy_name: "TURRET_HIER".to_string(),
        num_frames: 2,
        frame_rate: 30,
        source_is_compressed: false,
        channels: vec![W3dAnimChannel {
            first_frame: 0,
            last_frame: 1,
            vector_len: 1,
            flags: 0,
            pivot: 2,
            data: vec![0.0, 2.0],
        }],
        raw_visibility_channels: Vec::new(),
        unsupported_visibility_pivots: Vec::new(),
    });
    model
}

/// The exact turret fixture with one external C++ `AdditionalModels`
/// record attached to the recoil/barrel pivot. It lets the aggregate
/// projection prove that parent mesh and external child share one HTree
/// control sequence without involving a renderer or asset lookup.
fn primary_turret_hlod_model_with_barrel_aggregate() -> W3DModel {
    let mut model = primary_turret_hlod_model();
    model.hlods[0].aggregates = Some(W3dHlodAttachmentArray {
        max_screen_size: 0.0,
        subobjects: vec![W3dHlodSubObject {
            name: "EXTERNAL_BARREL_ATTACHMENT".to_string(),
            bone_index: 3,
        }],
    });
    model.hlods[0].has_unrendered_aggregates = true;
    model
}

/// An animation-only companion W3D, shaped like the raw HAnim files C++
/// loads on a `Get_HAnim(Hierarchy.Animation)` miss.
fn companion_animation_fixture(
    hierarchy_name: &str,
    animation_name: &str,
    first_x: f32,
    last_x: f32,
) -> Vec<u8> {
    let mut animation_header = Vec::with_capacity(44);
    animation_header.extend_from_slice(&W3D_CURRENT_HANIM_VERSION.to_le_bytes());
    animation_header.extend_from_slice(&fixed_name(animation_name, W3D_NAME_LEN));
    animation_header.extend_from_slice(&fixed_name(hierarchy_name, W3D_NAME_LEN));
    animation_header.extend_from_slice(&2u32.to_le_bytes());
    animation_header.extend_from_slice(&30u32.to_le_bytes());

    // X translation, pivot 1, frames [0, 1]. The companion contains no
    // geometry or hierarchy chunk; the geometry W3D supplies the HTree.
    let mut channel = Vec::with_capacity(20);
    channel.extend_from_slice(&0u16.to_le_bytes());
    channel.extend_from_slice(&1u16.to_le_bytes());
    channel.extend_from_slice(&1u16.to_le_bytes());
    channel.extend_from_slice(&0u16.to_le_bytes());
    channel.extend_from_slice(&1u16.to_le_bytes());
    channel.extend_from_slice(&0u16.to_le_bytes());
    channel.extend_from_slice(&first_x.to_le_bytes());
    channel.extend_from_slice(&last_x.to_le_bytes());
    chunk(
        W3D_CHUNK_ANIMATION,
        [
            chunk(W3D_CHUNK_ANIMATION_HEADER, animation_header, false),
            chunk(W3D_CHUNK_ANIMATION_CHANNEL, channel, false),
        ]
        .concat(),
        true,
    )
}

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

#[test]
fn w3d_archive_path_variants_include_retail_art_w3d_casing() {
    for name in ["AmericaCommandCenter", "airanger_s"] {
        let paths = w3d_archive_path_variants(name);
        assert!(
            paths.iter().any(|p| p.contains("Art/W3D/")),
            "{name} must include Art/W3D/: {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.ends_with(".W3D")),
            "{name} must include .W3D: {paths:?}"
        );
    }

    let cmd = w3d_archive_path_variants("AmericaCommandCenter");
    assert_eq!(cmd[0], "art/w3d/AmericaCommandCenter.w3d");
    assert_eq!(cmd[1], "AmericaCommandCenter.w3d");
    assert!(
        cmd.iter().any(|p| p == "Art/W3D/ABBtCmdHQ.W3D"),
        "AmericaCommandCenter must include Art/W3D/ABBtCmdHQ.W3D: {cmd:?}"
    );
    assert!(
        cmd.iter().any(|p| p.contains("ABBtCmdHQ")),
        "AmericaCommandCenter must include retail ABBtCmdHQ: {cmd:?}"
    );

    let ranger = w3d_archive_path_variants("airanger_s");
    assert_eq!(ranger[0], "art/w3d/airanger_s.w3d");
    assert_eq!(ranger[1], "airanger_s.w3d");
    assert!(
        ranger.iter().any(|p| p == "Art/W3D/AIRanger_S.W3D"),
        "airanger_s must include Art/W3D/AIRanger_S.W3D: {ranger:?}"
    );
    assert!(
        ranger.iter().any(|p| p.contains("AIRanger_S")),
        "airanger_s must include retail AIRanger_S: {ranger:?}"
    );
}

#[test]
fn load_model_from_bytes_rejects_empty() {
    let loader = W3DLoader::new();
    assert!(loader.load_model_from_bytes(&[], "empty").is_err());
    assert!(
        loader
            .load_model_from_bytes(&[], "AmericaCommandCenter")
            .is_err()
    );
}

#[test]
fn mesh_vertex_influences_retain_exact_eight_byte_records_and_ignore_trailing_data() {
    let records = [
        (2u16, [1, 2, 3, 4, 5, 6]),
        (0u16, [7, 8, 9, 10, 11, 12]),
        (u16::MAX, [13, 14, 15, 16, 17, 18]),
    ];
    let bytes = mesh_with_vertex_influence_chunks(
        0x0004_0002,
        &[vertex_influence_payload(&records, &[0xAA, 0xBB, 0xCC])],
    );
    let model = W3DLoader::new()
        .load_model_from_bytes(&bytes, "exact_vertex_influences")
        .expect("a complete W3dVertInfStruct array with trailing bytes is source-valid");
    let influences = model.meshes[0]
        .vertex_influences
        .as_ref()
        .expect("the source SKIN chunk must survive parsing");

    assert_eq!(influences.len(), 3);
    assert_eq!(influences[0].bone_idx, 2);
    assert_eq!(influences[0].pad, [1, 2, 3, 4, 5, 6]);
    assert_eq!(influences[1].bone_idx, 0);
    assert_eq!(influences[1].pad, [7, 8, 9, 10, 11, 12]);
    assert_eq!(influences[2].bone_idx, u16::MAX);
    assert_eq!(influences[2].pad, [13, 14, 15, 16, 17, 18]);
}

#[test]
fn mesh_vertex_influences_use_last_complete_chunk_and_only_pre3_indices_wrap_forward() {
    let first = vertex_influence_payload(&[(9, [1; 6]), (9, [2; 6]), (9, [3; 6])], &[]);
    let last = vertex_influence_payload(&[(0, [4; 6]), (1, [5; 6]), (u16::MAX, [6; 6])], &[]);
    let repeated = W3DLoader::new()
        .load_model_from_bytes(
            &mesh_with_vertex_influence_chunks(0x0004_0002, &[first, last]),
            "repeated_vertex_influences",
        )
        .expect("each complete source influence chunk can overwrite the prior link array");
    let repeated = repeated.meshes[0]
        .vertex_influences
        .as_ref()
        .expect("the final complete chunk remains");
    assert_eq!(
        repeated
            .iter()
            .map(|influence| influence.bone_idx)
            .collect::<Vec<_>>(),
        vec![0, 1, u16::MAX],
        "modern W3D indices must stay exactly as written"
    );
    assert_eq!(repeated[0].pad, [4; 6]);
    assert_eq!(repeated[2].pad, [6; 6]);

    let legacy_records = [(0, [7; 6]), (1, [8; 6]), (u16::MAX, [9; 6])];
    let pre3 = W3DLoader::new()
        .load_model_from_bytes(
            &mesh_with_vertex_influence_chunks(
                W3D_HTREE_ROOT_VERSION - 1,
                &[vertex_influence_payload(&legacy_records, &[])],
            ),
            "pre3_vertex_influences",
        )
        .expect("a complete pre-3.0 source influence chunk parses");
    let pre3 = pre3.meshes[0]
        .vertex_influences
        .as_ref()
        .expect("pre-3.0 links are retained after C++ root fixup");
    assert_eq!(
        pre3.iter()
            .map(|influence| influence.bone_idx)
            .collect::<Vec<_>>(),
        vec![1, 2, 0],
        "the C++ uint16 root insertion fixup wraps after a successful load"
    );
    assert_eq!(pre3[0].pad, [7; 6]);
    assert_eq!(pre3[2].pad, [9; 6]);
}

#[test]
fn mesh_vertex_influences_short_chunk_invalidates_the_mesh_without_partial_skin_data() {
    let only_two_records =
        vertex_influence_payload(&[(0, [0; 6]), (1, [0; 6])], &[0xFE, 0xED, 0xFA, 0xCE]);
    assert!(
        W3DLoader::new()
            .load_model_from_bytes(
                &mesh_with_vertex_influence_chunks(0x0004_0002, &[only_two_records]),
                "short_vertex_influences",
            )
            .is_err(),
        "a file whose only mesh has a short W3dVertInfStruct array must fail closed instead of preserving partial links"
    );
}

#[test]
fn hlod_rigid_binding_uses_authored_bone_once_without_name_heuristics() {
    let model = W3DLoader::new()
        .load_model_from_bytes(&rigid_hlod_fixture(1, &[], &[]), "rigid_hlod")
        .expect("source-shaped HLOD fixture should parse");

    assert_eq!(model.hlods.len(), 1);
    assert_eq!(model.hlods[0].name, "HLODROOT");
    assert_eq!(model.hlods[0].hierarchy_name, "RIG_HIER");
    assert_eq!(model.hlods[0].lods.len(), 1);
    assert_eq!(model.hlods[0].lods[0].subobjects[0].name, "HLODROOT.RIGID");
    assert_eq!(model.hlods[0].lods[0].subobjects[0].bone_index, 1);
    assert_eq!(model.meshes[0].name, "RIGID");
    assert_eq!(model.meshes[0].container_name, "HLODROOT");
    assert_ne!(model.meshes[0].name, "AUTHORED_BONE");

    // Import converts source [1, 2, 3] to Main render [1, 3, 2], but it
    // must remain unbaked.  The HLOD local matrix applies source translation
    // [10, 20, 30] exactly once, becoming Main render [10, 30, 20].
    assert_eq!(model.meshes[0].vertices[0].position, [1.0, 3.0, 2.0]);
    let local = model
        .mesh_local_transform_for_animation(0, 0, 0.0)
        .expect("authored HLOD child should resolve through BoneIndex");
    let transformed =
        local.transform_point3(Vec3::from_array(model.meshes[0].vertices[0].position));
    assert!(
        (transformed - Vec3::new(11.0, 33.0, 22.0)).length() < 0.0001,
        "single HLOD transform produced {transformed:?}"
    );
}

#[test]
fn hmodel_parser_retains_exact_typed_connections_and_its_named_htree_pose() {
    let model = W3DLoader::new()
        .load_model_from_bytes(
            &rigid_hmodel_fixture(
                0x0004_0002,
                &[
                    (W3D_CHUNK_HMODEL_NODE, "RigidBody", 1),
                    (W3D_CHUNK_HMODEL_COLLISION_NODE, "Collision", 0),
                    (W3D_CHUNK_HMODEL_SKIN_NODE, "Skin", 1),
                ],
            ),
            "rigid_hmodel",
        )
        .expect("source-shaped HMODEL fixture should parse");

    assert_eq!(model.hmodels.len(), 1);
    assert_eq!(
        model.hierarchies.len(),
        1,
        "the named source HTree is retained"
    );
    let hmodel = &model.hmodels[0];
    assert_eq!(hmodel.name, "RIG_HMODEL");
    assert_eq!(hmodel.hierarchy_name, "RIG_HIER");
    assert!(!hmodel.has_invalid_records);
    assert_eq!(
        hmodel
            .nodes
            .iter()
            .map(|node| (node.name.as_str(), node.bone_index, node.kind))
            .collect::<Vec<_>>(),
        vec![
            ("RIG_HMODEL.RigidBody", 1, W3dHmodelNodeKind::Node),
            ("RIG_HMODEL.Collision", 0, W3dHmodelNodeKind::CollisionNode),
            ("RIG_HMODEL.Skin", 1, W3dHmodelNodeKind::SkinNode),
        ],
        "HModelDefClass::read_connection always forms <ModelName>.<RenderObjName>"
    );

    let poses = model
        .hmodel_rigid_node_poses(0)
        .expect("valid HMODEL uses only its explicitly named HTree");
    assert_eq!(poses.len(), 2, "SKIN_NODE must not enter the rigid path");
    assert_eq!(poses[0].name, "RIG_HMODEL.RigidBody");
    assert_eq!(poses[0].bone_index, 1);
    assert!(
        (poses[0].parent_transform.w_axis.truncate() - Vec3::new(10.0, 30.0, 20.0)).length()
            < 0.0001,
        "the HMODEL child uses its own source HTree pivot in render basis"
    );
    assert_eq!(poses[1].name, "RIG_HMODEL.Collision");
    assert_eq!(poses[1].parent_transform, Mat4::IDENTITY);

    let skin_palette = model
        .hmodel_bind_pose_palette(0)
        .expect("a valid HMODEL owns its named HTree bind palette");
    assert_eq!(skin_palette.len(), 2);
    assert_eq!(skin_palette[0], Mat4::IDENTITY);
    assert!(
        (skin_palette[1].w_axis.truncate() - Vec3::new(10.0, 30.0, 20.0)).length() < 0.0001,
        "SKIN_NODE must use the HMODEL's named HTree, not a whole-file palette"
    );
    assert_eq!(
        model
            .hmodel_skin_node_bindings(0)
            .expect("the valid skin connection has an HMODEL-local palette"),
        vec![W3dHmodelSkinNodeBinding {
            name: "RIG_HMODEL.Skin".to_string(),
            bone_index: 1,
        }]
    );
}

#[test]
fn hmodel_pre30_pivots_normalize_exactly_like_hmdldef() {
    let model = W3DLoader::new()
        .load_model_from_bytes(
            &rigid_hmodel_fixture(
                2 << 16,
                &[
                    (W3D_CHUNK_HMODEL_NODE, "LegacyRoot", u16::MAX),
                    (W3D_CHUNK_HMODEL_NODE, "LegacyChild", 0),
                ],
            ),
            "legacy_hmodel",
        )
        .expect("pre-3.0 HMODEL fixture should normalize safely");

    let hmodel = &model.hmodels[0];
    assert_eq!(hmodel.nodes[0].bone_index, 0);
    assert_eq!(hmodel.nodes[1].bone_index, 1);
    let poses = model
        .hmodel_rigid_node_poses(0)
        .expect("normalized HMODEL nodes use the corresponding HTree pivots");
    assert_eq!(poses[0].parent_transform, Mat4::IDENTITY);
    assert!(
        (poses[1].parent_transform.w_axis.truncate() - Vec3::new(10.0, 30.0, 20.0)).length()
            < 0.0001
    );
}

#[test]
fn hmodel_header_names_follow_cxx_fifteen_byte_termination() {
    let bytes = [
        rigid_hlod_fixture(1, &[], &[]),
        hmodel_fixture_chunk(
            0x0004_0002,
            "0123456789ABCDEF",
            "ABCDEFGHIJKLMNOP",
            &[(W3D_CHUNK_HMODEL_NODE, "Body", 0)],
        ),
    ]
    .concat();
    let model = W3DLoader::new()
        .load_model_from_bytes(&bytes, "truncated_hmodel")
        .expect("HMODEL header remains source-shaped when its fixed fields fill 16 bytes");

    assert_eq!(model.hmodels[0].name, "0123456789ABCDE");
    assert_eq!(model.hmodels[0].hierarchy_name, "ABCDEFGHIJKLMNO");
    assert_eq!(model.hmodels[0].nodes[0].name, "0123456789ABCDE.Body");
}

#[test]
fn hmodel_points_retain_source_vectors_without_consuming_connections() {
    let expected_points = [[1.25, -2.5, 3.75], [-4.0, 5.5, 6.25]];
    let bytes = [
        rigid_hlod_fixture(1, &[], &[]),
        hmodel_fixture_chunk_with_trailing(
            0x0004_0002,
            "TRAILING_HMODEL",
            "RIG_HIER",
            &[(W3D_CHUNK_HMODEL_NODE, "Body", 1)],
            &[
                // C++ `HModelDefClass::Load_W3D` loads snap points but
                // does not increment SubObjectCount for them.
                (
                    W3D_CHUNK_POINTS,
                    hmodel_snap_points_payload(&expected_points, &[]),
                ),
                // Both old HMODEL extension records are skipped by the
                // documented source parser and cannot become aliases.
                (W3D_CHUNK_HMODEL_OBSOLETE_AUX_DATA, vec![1, 2, 3, 4]),
                (W3D_CHUNK_HMODEL_OBSOLETE_SHADOW_NODE, vec![5, 6]),
            ],
        ),
    ]
    .concat();
    let model = W3DLoader::new()
        .load_model_from_bytes(&bytes, "trailing_hmodel")
        .expect("non-connection HMODEL chunks remain source-shaped metadata");

    assert_eq!(model.hmodels.len(), 1);
    assert!(!model.hmodels[0].has_invalid_records);
    assert_eq!(model.hmodels[0].nodes.len(), 1);
    assert_eq!(model.hmodels[0].nodes[0].name, "TRAILING_HMODEL.Body");
    assert_eq!(
        model
            .hmodel_source_snap_points(0)
            .expect("a valid HMODEL exposes its source definition points"),
        [
            W3dHmodelSnapPoint {
                source_position: expected_points[0],
            },
            W3dHmodelSnapPoint {
                source_position: expected_points[1],
            },
        ]
    );
    assert_eq!(
        model.hmodel_source_snap_point(0, 1),
        Some(W3dHmodelSnapPoint {
            source_position: expected_points[1],
        })
    );
    assert_eq!(model.hmodel_source_snap_point(0, 2), None);
}

#[test]
fn hmodel_later_points_replaces_prior_points_and_ignores_remainder() {
    let first_points = [[1.0, 2.0, 3.0]];
    let replacement_points = [[-4.0, 5.0, -6.0], [7.0, -8.0, 9.0]];
    let bytes = [
        rigid_hlod_fixture(1, &[], &[]),
        hmodel_fixture_chunk_with_trailing(
            0x0004_0002,
            "REPLACED_POINTS",
            "RIG_HIER",
            &[(W3D_CHUNK_HMODEL_NODE, "Body", 1)],
            &[
                (
                    W3D_CHUNK_POINTS,
                    hmodel_snap_points_payload(&first_points, &[]),
                ),
                (
                    W3D_CHUNK_POINTS,
                    hmodel_snap_points_payload(&replacement_points, &[0xDE, 0xAD, 0xBE]),
                ),
            ],
        ),
    ]
    .concat();
    let model = W3DLoader::new()
        .load_model_from_bytes(&bytes, "replaced_hmodel_points")
        .expect("C++ source-shaped HMODEL points should parse");

    let hmodel = &model.hmodels[0];
    assert!(!hmodel.has_invalid_records);
    assert_eq!(
        hmodel.nodes.len(),
        1,
        "POINTS records cannot consume a declared HMODEL connection"
    );
    assert_eq!(
        model
            .hmodel_source_snap_points(0)
            .expect("valid HMODEL source points"),
        [
            W3dHmodelSnapPoint {
                source_position: replacement_points[0],
            },
            W3dHmodelSnapPoint {
                source_position: replacement_points[1],
            },
        ],
        "the later C++ SnapPointsClass allocation replaces the prior definition vector"
    );
}

#[test]
fn hmodel_rejects_twenty_byte_connection_payloads_from_the_wrong_u32_layout() {
    let mut malformed_node = fixed_name("Body", W3D_NAME_LEN);
    malformed_node.extend_from_slice(&1u16.to_le_bytes());
    // This extra dword-tail shape comes from the incorrect u32 helper
    // struct, not `sizeof(W3dHModelNodeStruct)` in the original MSVC C++.
    malformed_node.extend_from_slice(&[0u8; 2]);
    assert_eq!(malformed_node.len(), 20);
    let bytes = [
        rigid_hlod_fixture(1, &[], &[]),
        hmodel_fixture_chunk_with_trailing(
            0x0004_0002,
            "BAD_NODE_LAYOUT",
            "RIG_HIER",
            &[],
            &[(W3D_CHUNK_HMODEL_NODE, malformed_node)],
        ),
    ]
    .concat();
    let model = W3DLoader::new()
        .load_model_from_bytes(&bytes, "bad_hmodel_node_layout")
        .expect("malformed HMODEL remains inspectable but cannot render");

    assert!(model.hmodels[0].has_invalid_records);
    assert!(model.hmodels[0].nodes.is_empty());
    assert!(model.hmodel_rigid_node_poses(0).is_none());
    assert!(model.hmodel_bind_pose_palette(0).is_none());
    assert!(model.hmodel_skin_node_bindings(0).is_none());
}

#[test]
fn hmodel_missing_named_htree_uses_only_default_root_and_never_an_inferred_palette() {
    let mut model = W3DModel::new("hmodel_default_root".to_string());
    model.hierarchy = Some(W3dHierarchy {
        name: "UNRELATED_TREE".to_string(),
        pivots: vec![
            W3dPivot {
                name: "ROOT".to_string(),
                parent_idx: u32::MAX,
                translation: [0.0; 3],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
            W3dPivot {
                name: "UNRELATED_BONE".to_string(),
                parent_idx: 0,
                translation: [99.0, 88.0, 77.0],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
        ],
        pivot_fixups: Vec::new(),
    });
    model.hmodels.push(W3dHmodel {
        version: 0x0004_0002,
        name: "DEFAULT_ROOT_HMODEL".to_string(),
        hierarchy_name: "MISSING_TREE".to_string(),
        nodes: vec![
            W3dHmodelNode {
                name: "DEFAULT_ROOT_HMODEL.RootChild".to_string(),
                bone_index: 0,
                kind: W3dHmodelNodeKind::Node,
            },
            W3dHmodelNode {
                name: "DEFAULT_ROOT_HMODEL.InvalidChild".to_string(),
                bone_index: 1,
                kind: W3dHmodelNodeKind::Node,
            },
            W3dHmodelNode {
                name: "DEFAULT_ROOT_HMODEL.RootSkin".to_string(),
                bone_index: 0,
                kind: W3dHmodelNodeKind::SkinNode,
            },
            W3dHmodelNode {
                name: "DEFAULT_ROOT_HMODEL.InvalidSkin".to_string(),
                bone_index: 1,
                kind: W3dHmodelNodeKind::SkinNode,
            },
        ],
        source_snap_points: Vec::new(),
        has_invalid_records: false,
    });

    let poses = model
        .hmodel_rigid_node_poses(0)
        .expect("C++ Animatable3DObjClass creates a default HTree on a miss");
    assert_eq!(poses.len(), 1);
    assert_eq!(poses[0].name, "DEFAULT_ROOT_HMODEL.RootChild");
    assert_eq!(poses[0].parent_transform, Mat4::IDENTITY);
    assert_eq!(
        model
            .hmodel_bind_pose_palette(0)
            .expect("missing named HTree uses only C++ Init_Default"),
        vec![Mat4::IDENTITY],
        "an unrelated whole-file hierarchy must never become an HMODEL skin palette"
    );
    assert_eq!(
        model
            .hmodel_skin_node_bindings(0)
            .expect("valid root skin remains independent from invalid siblings"),
        vec![W3dHmodelSkinNodeBinding {
            name: "DEFAULT_ROOT_HMODEL.RootSkin".to_string(),
            bone_index: 0,
        }],
        "the out-of-range skin connection must fail closed without a root alias"
    );
}

#[test]
fn hmodel_skin_mesh_requires_one_valid_influence_for_each_vertex() {
    let mut mesh = W3DMesh::new("strict_skin".to_string());
    mesh.vertices = vec![
        W3DVertex {
            position: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0; 2],
            color: [1.0; 4],
        },
        W3DVertex {
            position: [1.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [1.0, 0.0],
            color: [1.0; 4],
        },
    ];
    mesh.vertex_influences = Some(vec![
        W3dVertInfStruct {
            bone_idx: 0,
            pad: [0; 6],
        },
        W3dVertInfStruct {
            bone_idx: 1,
            pad: [0; 6],
        },
    ]);

    assert!(mesh.has_complete_skin_influences_for_palette(2));
    assert!(
        !mesh.has_complete_skin_influences_for_palette(1),
        "an influence outside the owning HMODEL palette is not a root fallback"
    );

    mesh.vertex_influences
        .as_mut()
        .expect("fixture influences")
        .pop();
    assert!(
        !mesh.has_complete_skin_influences_for_palette(2),
        "C++ reads exactly one W3dVertInfStruct per source vertex"
    );
}

#[test]
fn hmodel_modern_invalid_pivot_fails_closed_without_partial_rigid_rendering() {
    let model = W3DLoader::new()
        .load_model_from_bytes(
            &rigid_hmodel_fixture(3 << 16, &[(W3D_CHUNK_HMODEL_NODE, "BadPivot", u16::MAX)]),
            "invalid_hmodel",
        )
        .expect("invalid source topology remains inspectable");

    assert!(model.hmodels[0].has_invalid_records);
    assert!(
        model.hmodel_rigid_node_poses(0).is_none(),
        "modern 0xffff pivot is an assertion violation in C++; Main must not guess"
    );
    assert!(
        model.hmodel_bind_pose_palette(0).is_none(),
        "a malformed HMODEL cannot still donate a skin palette"
    );
}

#[test]
fn w3d_hlod_visibility_raw_bit_channel_uses_lsb_frames_and_default_outside_range() {
    let model = W3DLoader::new()
        .load_model_from_bytes(&visibility_hlod_fixture(), "visibility_hlod")
        .expect("source-shaped raw visibility HLOD should parse");
    assert_eq!(model.animations.len(), 1);
    let animation = &model.animations[0];
    assert_eq!(animation.raw_visibility_channels.len(), 1);
    assert_eq!(animation.raw_visibility_channels[0].pivot, 1);
    assert!(animation.raw_visibility_channels[0].visible_at(0));
    assert!(animation.raw_visibility_channels[0].visible_at(2));
    assert!(
        !animation.raw_visibility_channels[0].visible_at(3),
        "bit one is the second low-order bit, not MSB-first"
    );
    assert!(animation.raw_visibility_channels[0].visible_at(4));
    assert!(
        animation.raw_visibility_channels[0].visible_at(5),
        "frames outside [FirstFrame, LastFrame] use DefaultVal"
    );

    let at_default = model
        .mesh_local_transform_and_visibility_for_animation(0, Some(0), 0.0)
        .expect("exact HLOD child should resolve at default frame");
    assert!(at_default.1);
    let at_hidden = model
        .mesh_local_transform_and_visibility_for_animation(0, Some(0), 3.0)
        .expect("exact HLOD child should resolve at authored hidden frame");
    assert!(
        !at_hidden.1,
        "visibility follows the source BoneIndex, even though mesh name has _d"
    );
    let at_visible = model
        .mesh_local_transform_and_visibility_for_animation(0, Some(0), 4.0)
        .expect("exact HLOD child should resolve at authored visible frame");
    assert!(at_visible.1);
}

#[test]
fn w3d_hlod_visibility_hide_show_subobjects_apply_exact_child_bones_in_order() {
    let model = hide_show_subobjects_hlod_model();
    let directives = vec![
        AuthoredDrawSubobjectVisibility {
            // C++ first tries this complete retained HLOD child identity.
            name: "VIS_HLOD.ParentMesh".to_string(),
            hidden: true,
        },
        AuthoredDrawSubobjectVisibility {
            // This case-insensitive leaf form uses C++'s first-dot pass,
            // but only after Main has matched a retained HLOD record.
            name: "siblingmesh".to_string(),
            hidden: true,
        },
        AuthoredDrawSubobjectVisibility {
            // The descendant's later show must override the parent's hide.
            name: "CHILDMESH".to_string(),
            hidden: false,
        },
        AuthoredDrawSubobjectVisibility {
            // Unknown directives must not become a broad mesh-name rule.
            name: "not_a_retained_hlod_child".to_string(),
            hidden: true,
        },
    ];

    assert!(
        !model.mesh_visible_for_authored_subobject_directives(0, &directives),
        "the full HLOD child directive hides its direct target"
    );
    assert!(
        model.mesh_visible_for_authored_subobject_directives(1, &directives),
        "a same-bone sibling is not the directly named C++ RenderObj and must stay visible"
    );
    assert!(
        model.mesh_visible_for_authored_subobject_directives(2, &directives),
        "a later descendant ShowSubObject wins over its hidden ancestor"
    );
    assert!(
        !model.mesh_visible_for_authored_subobject_directives(3, &directives),
        "the exact leaf directive resolves only its retained sibling record"
    );
}

#[test]
fn w3d_hlod_turret_primary_controls_exact_bones_after_selected_animation() {
    let model = primary_turret_hlod_model();
    let binding = W3dAnimationBinding::local(0);
    let primary_turret = AuthoredDrawPrimaryTurret {
        // The synthetic HLOD child names are deliberately unrelated to
        // these exact source pivot names.
        yaw_bone: Some("yaw_pivot".to_string()),
        pitch_bone: Some("pitch_pivot".to_string()),
        yaw_art_angle_radians_bits: std::f32::consts::FRAC_PI_2.to_bits(),
        pitch_art_angle_radians_bits: 0.0f32.to_bits(),
        ..Default::default()
    };

    let normal_hull = model
        .mesh_local_transform_and_visibility_for_binding(0, Some(&binding), 1.0)
        .expect("selected raw HAnim should resolve the chassis");
    let normal_muzzle = model
        .mesh_local_transform_and_visibility_for_binding(3, Some(&binding), 1.0)
        .expect("selected raw HAnim should resolve the muzzle");
    let controlled_hull = model
        .mesh_local_transform_and_visibility_for_primary_turret(
            0,
            Some(&binding),
            1.0,
            &primary_turret,
            0.0,
            90.0,
        )
        .expect("exact HLOD chassis remains renderable");
    let controlled_barrel = model
        .mesh_local_transform_and_visibility_for_primary_turret(
            2,
            Some(&binding),
            1.0,
            &primary_turret,
            0.0,
            90.0,
        )
        .expect("exact HLOD pitch child remains renderable");
    let controlled_muzzle = model
        .mesh_local_transform_and_visibility_for_primary_turret(
            3,
            Some(&binding),
            1.0,
            &primary_turret,
            0.0,
            90.0,
        )
        .expect("exact HLOD pitch descendant remains renderable");
    let bind_controlled_hull = model
        .mesh_local_transform_and_visibility_for_primary_turret(
            0,
            None,
            1.0,
            &primary_turret,
            0.0,
            90.0,
        )
        .expect("bind-pose chassis remains renderable under the same control");
    let bind_controlled_muzzle = model
        .mesh_local_transform_and_visibility_for_primary_turret(
            3,
            None,
            1.0,
            &primary_turret,
            0.0,
            90.0,
        )
        .expect("bind-pose muzzle remains renderable under the same control");

    assert_eq!(normal_hull.1, controlled_hull.1);
    assert!(
        normal_hull
            .0
            .to_cols_array()
            .iter()
            .zip(controlled_hull.0.to_cols_array())
            .all(|(before, after)| (*before - after).abs() < 1.0e-5),
        "a source turret binding must not rotate the chassis/hull"
    );
    assert!(
        (normal_muzzle.0.w_axis.truncate() - controlled_muzzle.0.w_axis.truncate()).length()
            > 1.0e-3,
        "yaw/pitch controls must affect only their exact HTree descendant path"
    );
    assert!(
        (controlled_barrel.0.w_axis.truncate() - controlled_muzzle.0.w_axis.truncate()).length()
            > 1.0e-3,
        "the pitch control is installed before the next source descendant is evaluated"
    );
    assert!(
        (bind_controlled_hull
            .0
            .to_cols_array()
            .iter()
            .zip(controlled_hull.0.to_cols_array())
            .map(|(bind, selected)| (bind - selected).abs())
            .fold(0.0f32, f32::max))
            < 1.0e-5,
        "a selected HAnim that animates the yaw subtree must still leave its sibling hull equal to bind pose"
    );
    assert!(
        (bind_controlled_muzzle
            .0
            .to_cols_array()
            .iter()
            .zip(controlled_muzzle.0.to_cols_array())
            .map(|(bind, selected)| (bind - selected).abs())
            .fold(0.0f32, f32::max))
            > 1.0e-3,
        "the controlled descendant must retain the selected frame-one HAnim pose rather than reverting to bind pose"
    );

    let alternate_turret = AuthoredDrawPrimaryTurret {
        alternate_yaw_bone_present: true,
        ..primary_turret.clone()
    };
    let alternate_fallback = model
        .mesh_local_transform_and_visibility_for_primary_turret(
            3,
            Some(&binding),
            1.0,
            &alternate_turret,
            0.0,
            90.0,
        )
        .expect("unsupported alternate source retains normal HLOD pose");
    assert!(
        alternate_fallback
            .0
            .to_cols_array()
            .iter()
            .zip(normal_muzzle.0.to_cols_array())
            .all(|(fallback, normal)| (*fallback - normal).abs() < 1.0e-5),
        "an authored alternate turret must fail closed instead of borrowing the primary angle"
    );
}

#[test]
fn aggregate_poses_inherit_primary_turret_and_recoil_controls_in_cxx_order() {
    let model = primary_turret_hlod_model_with_barrel_aggregate();
    let binding = W3dAnimationBinding::local(0);
    let primary_turret = AuthoredDrawPrimaryTurret {
        yaw_bone: Some("yaw_pivot".to_string()),
        pitch_bone: Some("pitch_pivot".to_string()),
        yaw_art_angle_radians_bits: std::f32::consts::FRAC_PI_2.to_bits(),
        pitch_art_angle_radians_bits: 0.0f32.to_bits(),
        ..Default::default()
    };
    let weapon_controls = [W3dWeaponVisualControl {
        recoil_pivot_index: Some(3),
        recoil_shift: 2.5,
        muzzle_flash_pivot_index: None,
        muzzle_flash_visible: false,
    }];

    let parent_barrel = model
        .mesh_local_transform_and_visibility_for_primary_turret_and_weapon_controls(
            2,
            Some(&binding),
            1.0,
            &primary_turret,
            0.0,
            90.0,
            &weapon_controls,
        )
        .expect("the rigid barrel parent must retain its exact controlled pose");
    let aggregate = model
        .aggregate_attachment_poses_for_primary_turret_and_weapon_controls(
            Some(&binding),
            1.0,
            &primary_turret,
            0.0,
            90.0,
            &weapon_controls,
        )
        .expect("an aggregate on a valid parent pivot must inherit controls");
    assert_eq!(aggregate.len(), 1);
    assert_eq!(aggregate[0].name, "EXTERNAL_BARREL_ATTACHMENT");
    assert!(aggregate[0].visible);
    assert!(
        aggregate[0]
            .parent_transform
            .to_cols_array()
            .iter()
            .zip(parent_barrel.0.to_cols_array())
            .all(|(aggregate, parent)| (*aggregate - parent).abs() < 1.0e-5),
        "C++ AdditionalModels use the same final HTree transform as their parent barrel"
    );

    let bind_pose = model
        .aggregate_attachment_poses_for_binding(Some(&binding), 1.0)
        .expect("source-valid aggregate bind/HAnim pose");
    assert!(
        bind_pose[0]
            .parent_transform
            .to_cols_array()
            .iter()
            .zip(aggregate[0].parent_transform.to_cols_array())
            .any(|(before, controlled)| (*before - controlled).abs() > 1.0e-4),
        "turret/recoil controls must not be dropped from the aggregate pose"
    );
}

#[test]
fn w3d_hlod_weapon_barrel_topology_uses_all_four_bases_and_cxx_numbered_order() {
    let mut model = primary_turret_hlod_model();
    {
        let hierarchy = model
            .hierarchy
            .as_mut()
            .expect("single-HLOD fixture has an HTree");
        hierarchy.pivots[1].name = "Fx01".to_string();
        hierarchy.pivots[2].name = "Recoil01".to_string();
        hierarchy.pivots[3].name = "Muzzle01".to_string();
        hierarchy.pivots[4].name = "Launch01".to_string();
        hierarchy.pivots.push(W3dPivot {
            // A second muzzle but no second FX is the explicit C++ exception:
            // it reuses the first exact FX pivot rather than abandoning the
            // numbered barrel or inventing a bare-name lookup.
            name: "Muzzle02".to_string(),
            parent_idx: 0,
            translation: [0.0; 3],
            euler_angles: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        });
    }

    let bindings = AuthoredDrawWeaponBoneBindings {
        slots: [
            AuthoredDrawWeaponBoneSlot {
                fire_fx_bone_base: Some("fx".to_string()),
                recoil_bone_base: Some("recoil".to_string()),
                muzzle_flash_bone_base: Some("muzzle".to_string()),
                launch_bone_base: Some("launch".to_string()),
                projectile_hide_show_bone: None,
            },
            AuthoredDrawWeaponBoneSlot::default(),
            AuthoredDrawWeaponBoneSlot::default(),
        ],
        source_fields_valid: true,
    };
    let topology = model
        .weapon_barrel_topology_for_authored_bindings(&bindings)
        .expect("one exact HLOD/hierarchy accepts frozen valid source bases");
    let primary = topology.slot(0).expect("PRIMARY topology");
    assert_eq!(primary.len(), 2, "01 then 02 stop at first all-missing 03");
    assert_eq!(topology.barrel_count(0), Some(2));
    assert_eq!(primary[0].fire_fx_pivot_index, Some(1));
    assert_eq!(primary[0].recoil_pivot_index, Some(2));
    assert_eq!(primary[0].muzzle_flash_pivot_index, Some(3));
    assert_eq!(primary[0].launch_pivot_index, Some(4));
    assert!(primary[0].has_recoil_or_muzzle());
    assert_eq!(
        primary[1].fire_fx_pivot_index,
        Some(1),
        "C++ reuses the previous numbered FX bone for a later muzzle-only barrel"
    );
    assert_eq!(primary[1].muzzle_flash_pivot_index, Some(5));
    assert_eq!(primary[1].recoil_pivot_index, None);
    assert_eq!(primary[1].launch_pivot_index, None);
    assert!(primary[1].has_recoil_or_muzzle());

    // No numbered `Bare*01` pivots exist, so C++ tries unadorned names
    // exactly once. It must not mix those with the numbered sequence.
    {
        let hierarchy = model
            .hierarchy
            .as_mut()
            .expect("single-HLOD fixture retains its HTree");
        hierarchy.pivots[1].name = "BareFx".to_string();
        hierarchy.pivots[2].name = "BareRecoil".to_string();
        hierarchy.pivots[3].name = "BareMuzzle".to_string();
        hierarchy.pivots[4].name = "BareLaunch".to_string();
        hierarchy.pivots.truncate(5);
    }
    let bare_bindings = AuthoredDrawWeaponBoneBindings {
        slots: [
            AuthoredDrawWeaponBoneSlot {
                fire_fx_bone_base: Some("barefx".to_string()),
                recoil_bone_base: Some("barerecoil".to_string()),
                muzzle_flash_bone_base: Some("baremuzzle".to_string()),
                launch_bone_base: Some("barelaunch".to_string()),
                projectile_hide_show_bone: None,
            },
            AuthoredDrawWeaponBoneSlot::default(),
            AuthoredDrawWeaponBoneSlot::default(),
        ],
        source_fields_valid: true,
    };
    let bare = model
        .weapon_barrel_topology_for_authored_bindings(&bare_bindings)
        .expect("same valid single-HLOD accepts bare source bases");
    assert_eq!(bare.barrel_count(0), Some(1));
    assert_eq!(
        bare.slot(0).expect("PRIMARY bare topology")[0],
        W3dWeaponBarrelBinding {
            fire_fx_pivot_index: Some(1),
            recoil_pivot_index: Some(2),
            muzzle_flash_pivot_index: Some(3),
            launch_pivot_index: Some(4),
        }
    );
}

#[test]
fn w3d_hlod_weapon_barrel_topology_rejects_invalid_source_and_unsupported_hlods() {
    let model = primary_turret_hlod_model();
    let invalid_source = AuthoredDrawWeaponBoneBindings {
        source_fields_valid: false,
        ..Default::default()
    };
    assert!(
        model
            .weapon_barrel_topology_for_authored_bindings(&invalid_source)
            .is_none(),
        "malformed source WeaponSlotType data cannot fall through to an empty/guessed topology"
    );

    let mut multi_lod = model.clone();
    let second_lod = multi_lod.hlods[0].lods[0].clone();
    multi_lod.hlods[0].lods.push(second_lod);
    assert!(
            multi_lod
                .weapon_barrel_topology_for_authored_bindings(&AuthoredDrawWeaponBoneBindings::default())
                .is_none(),
            "until C++ LOD selection exists, even empty valid bindings may not authorize a multi-LOD model"
        );
}

#[test]
fn w3d_hlod_visibility_compressed_channel_fails_closed_for_its_authored_pivot() {
    let mut model = W3DLoader::new()
        .load_model_from_bytes(&visibility_hlod_fixture(), "visibility_hlod")
        .expect("source-shaped raw visibility HLOD should parse");
    model.animations[0]
        .unsupported_visibility_pivots
        .push(Some(1));
    assert!(
        model
            .mesh_local_transform_and_visibility_for_animation(0, Some(0), 0.0)
            .is_none(),
        "a compressed visibility source must not be rendered using a guessed value"
    );
}

#[test]
fn w3d_companion_animation_external_binding_wins_over_local_clip() {
    let loader = W3DLoader::new();
    let geometry = loader
        .load_model_from_bytes(&visibility_hlod_fixture(), "geometry")
        .expect("source-shaped geometry fixture should parse");
    let companion = loader
        .load_companion_animation_from_bytes(
            &companion_animation_fixture("VIS_HIER", "EXTERNAL", 4.0, 12.0),
            "VIS_HIER.EXTERNAL",
        )
        .expect("animation-only companion should parse through HAnim path");
    let mut compressed_companion = companion.clone();
    compressed_companion.source_is_compressed = true;
    assert!(companion.matches_draw_identity("vis_hier.external"));
    assert_eq!(
        w3d_companion_animation_filename("VIS_HIER.EXTERNAL"),
        Some("EXTERNAL.w3d".to_string())
    );

    let binding = W3dAnimationBinding::companion("VIS_HIER.EXTERNAL", Arc::new(companion));
    assert!(geometry.animation_binding_is_compatible(&binding));
    let compressed_binding =
        W3dAnimationBinding::companion("VIS_HIER.EXTERNAL", Arc::new(compressed_companion));
    assert!(
        !geometry.animation_binding_is_compatible(&compressed_binding),
        "external compressed companion channels stay fail-closed until their path is ported"
    );

    let local_x = geometry
        .sample_animation(0, 1.0)
        .expect("local source clip should sample")[1][12];
    let external_x = geometry
        .sample_animation_binding(&binding, 1.0)
        .expect("exact external binding should sample")[1][12];
    assert_eq!(local_x, 3.0, "fixture local clip is only the bind pose");
    assert_eq!(
        external_x, 12.0,
        "companion motion must override local clip"
    );

    let (transform, visible) = geometry
        .mesh_local_transform_and_visibility_for_binding(0, Some(&binding), 1.0)
        .expect("external companion must carry through the exact HLOD record");
    assert!(visible, "companion with no bit channel defaults to visible");
    assert!(
        (transform.w_axis.x - 12.0).abs() < 0.0001,
        "HLOD transform must use the external companion, got {transform:?}"
    );

    // A missing external binding becomes the source bind pose at the
    // collector boundary, not the geometry file's local animation zero.
    let bind_pose = geometry
        .mesh_local_transform_and_visibility_for_binding(0, None, 1.0)
        .expect("absent selected HAnim is a bind-pose request");
    assert!((bind_pose.0.w_axis.x - 3.0).abs() < 0.0001);
}

#[test]
fn w3d_companion_animation_retail_china_agent_asset_identity_when_available() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let candidates = [
        root.join("windows_game/extracted_big_files/W3DZH/Art/W3D/AIRNGR_ATB.W3D"),
        root.join("windows_game/extracted_big_files/W3DZH/art/w3d/AIRNGR_ATB.W3D"),
    ];
    let Some(path) = candidates.into_iter().find(|path| path.is_file()) else {
        eprintln!("skip: retail AIRNGR_ATB.W3D companion is not available on disk");
        return;
    };
    let paths = w3d_companion_animation_archive_path_variants("AIRngr_SKL.AIRngr_ATB")
        .expect("qualified retail Draw identity must yield exact companion paths");
    assert_eq!(
        w3d_companion_animation_filename("AIRngr_SKL.AIRngr_ATB"),
        Some("AIRngr_ATB.w3d".to_string())
    );
    assert!(
        paths.iter().any(|path| path == "Art/W3D/AIRNGR_ATB.W3D"),
        "case-only exact companion candidates must retain retail archive spelling: {paths:?}"
    );

    let bytes = std::fs::read(&path).expect("retail companion W3D bytes");
    let animation = W3DLoader::new()
        .load_companion_animation_from_bytes(&bytes, "AIRngr_SKL.AIRngr_ATB")
        .expect("retail China Agent companion must contain its exact HAnim");
    assert!(animation.matches_draw_identity("airngr_skl.airngr_atb"));
}

#[test]
fn htree_source_matrix_multiply_keeps_cxx_parent_local_and_capture_order() {
    let quarter_turn = std::f32::consts::FRAC_1_SQRT_2;
    let hierarchy = W3dHierarchy {
        name: "MATRIX_ORDER".to_string(),
        pivots: vec![
            W3dPivot {
                name: "ROOT".to_string(),
                parent_idx: u32::MAX,
                translation: [0.0; 3],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
            W3dPivot {
                name: "ROTATED_PARENT".to_string(),
                parent_idx: 0,
                translation: [0.0; 3],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, quarter_turn, quarter_turn],
            },
            W3dPivot {
                name: "LOCAL_CHILD".to_string(),
                parent_idx: 1,
                translation: [2.0, 0.0, 0.0],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
        ],
        pivot_fixups: Vec::new(),
    };
    let locals: Vec<_> = hierarchy.pivots.iter().map(mat4_from_pivot).collect();

    let globals = compute_htree_global_transforms_from_locals(&hierarchy, &locals)
        .expect("ordered affine HTree must evaluate");
    assert!(
        (Mat4::from_cols_array(&globals[2]).w_axis.truncate() - Vec3::new(0.0, 2.0, 0.0)).length()
            < 0.0001,
        "C++ Matrix3D::Multiply(parent, local) rotates the child's local translation"
    );

    let mut controls = vec![None; hierarchy.pivots.len()];
    controls[1] = Some(Mat4::from_translation(Vec3::X * 3.0).to_cols_array());
    let captured = compute_htree_global_transforms_from_locals_with_capture_controls(
        &hierarchy, &locals, &controls,
    )
    .expect("a valid C++ Capture_Bone control must evaluate");
    assert!(
        (Mat4::from_cols_array(&captured[2]).w_axis.truncate() - Vec3::new(0.0, 5.0, 0.0)).length()
            < 0.0001,
        "C++ Control_Bone post-multiplies the parent before descendants inherit it"
    );

    let loader_globals = W3DLoader::compute_global_transforms(&hierarchy)
        .expect("legacy HMODEL residual must use the same source matrix convention");
    assert!(
        (Mat4::from_cols_array(&loader_globals[2]).w_axis.truncate() - Vec3::new(0.0, 2.0, 0.0))
            .length()
            < 0.0001
    );
}

#[test]
fn raw_hanim_uses_cxx_integer_delta_postcomposition_and_duplicate_channel_rules() {
    let quarter_turn = std::f32::consts::FRAC_1_SQRT_2;
    let hierarchy = W3dHierarchy {
        name: "RAW_DELTA".to_string(),
        pivots: vec![
            W3dPivot {
                name: "ROOT".to_string(),
                parent_idx: u32::MAX,
                translation: [0.0; 3],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
            W3dPivot {
                name: "ANIMATED_PARENT".to_string(),
                parent_idx: 0,
                translation: [10.0, 0.0, 0.0],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, quarter_turn, quarter_turn],
            },
            W3dPivot {
                name: "CHILD".to_string(),
                parent_idx: 1,
                translation: [1.0, 0.0, 0.0],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            },
        ],
        pivot_fixups: Vec::new(),
    };
    let animation = W3dAnimation {
        name: "RAW_DELTA_ANIM".to_string(),
        hierarchy_name: "RAW_DELTA".to_string(),
        num_frames: 3,
        frame_rate: 30,
        source_is_compressed: false,
        channels: vec![
            // C++ starts its raw node-motion loop at pivot one, so this
            // malformed root channel must not poison the usable child pose.
            W3dAnimChannel {
                first_frame: 0,
                last_frame: 2,
                vector_len: 0,
                flags: 0,
                pivot: 0,
                data: Vec::new(),
            },
            W3dAnimChannel {
                first_frame: 1,
                last_frame: 1,
                vector_len: 1,
                flags: 0,
                pivot: 1,
                data: vec![2.0],
            },
            // `HRawAnimClass::add_channel` overwrites the prior X
            // pointer; the final source record is authoritative.
            W3dAnimChannel {
                first_frame: 1,
                last_frame: 1,
                vector_len: 1,
                flags: 0,
                pivot: 1,
                data: vec![3.0],
            },
            W3dAnimChannel {
                first_frame: 1,
                last_frame: 1,
                vector_len: 4,
                flags: 6,
                pivot: 1,
                data: vec![0.0, 0.0, quarter_turn, quarter_turn],
            },
        ],
        raw_visibility_channels: vec![W3dRawVisibilityChannel {
            first_frame: 1,
            last_frame: 1,
            flags: 0,
            pivot: 1,
            default_visible: true,
            bits: vec![0],
        }],
        unsupported_visibility_pivots: Vec::new(),
    };

    let raw_one = sample_animation_local_transforms(&hierarchy, &animation, 1.25)
        .expect("fractional raw frame must use the C++ rounded integer frame");
    let raw_one_globals = compute_htree_global_transforms_from_locals(&hierarchy, &raw_one)
        .expect("valid raw HAnim hierarchy");
    assert!(
        (Mat4::from_cols_array(&raw_one_globals[2]).w_axis.truncate() - Vec3::new(9.0, 3.0, 0.0))
            .length()
            < 0.0001,
        "base Rz90 * delta Tx3 * delta Rz90 must rotate the child after preserving its bind pose"
    );
    assert_eq!(
        animation.visibility_for_pivot(1, 1.25),
        Some(false),
        "specialized Generals raw update uses the same rounded frame for visibility"
    );

    let raw_two = sample_animation_local_transforms(&hierarchy, &animation, 1.5)
        .expect("half frame rounds to the even raw frame under C++ _RC_NEAR");
    let raw_two_globals = compute_htree_global_transforms_from_locals(&hierarchy, &raw_two)
        .expect("valid raw HAnim hierarchy");
    assert!(
        (Mat4::from_cols_array(&raw_two_globals[2]).w_axis.truncate() - Vec3::new(10.0, 1.0, 0.0))
            .length()
            < 0.0001,
        "outside a raw channel range C++ supplies identity deltas rather than clamping/interpolating"
    );
    assert_eq!(animation.visibility_for_pivot(1, 1.5), Some(true));

    let wrapped = sample_animation_local_transforms(&hierarchy, &animation, 2.6)
        .expect("a raw frame at NumFrames wraps to zero");
    let wrapped_globals = compute_htree_global_transforms_from_locals(&hierarchy, &wrapped)
        .expect("valid wrapped raw HAnim hierarchy");
    assert!(
        (Mat4::from_cols_array(&wrapped_globals[2]).w_axis.truncate() - Vec3::new(10.0, 1.0, 0.0))
            .length()
            < 0.0001
    );
}

#[test]
fn htree_runtime_pose_ignores_authored_pivot_zero_pose_channels_and_visibility() {
    let mut model = W3DLoader::new()
        .load_model_from_bytes(&rigid_hlod_fixture(1, &[], &[]), "root_pose")
        .expect("source-shaped rigid HLOD fixture should parse");
    let original_bounds = (model.bounding_box_min, model.bounding_box_max);
    let hierarchy = model
        .hierarchy
        .as_mut()
        .expect("rigid HLOD fixture has an HTree");
    hierarchy.pivots[0].translation = [99.0, 88.0, 77.0];
    hierarchy.pivots[0].rotation = [0.0, 0.0, 1.0, 0.0];
    model.animations.push(W3dAnimation {
        name: "ROOT_ONLY".to_string(),
        hierarchy_name: "RIG_HIER".to_string(),
        num_frames: 2,
        frame_rate: 30,
        source_is_compressed: false,
        channels: vec![
            W3dAnimChannel {
                first_frame: 0,
                last_frame: 1,
                vector_len: 1,
                flags: 0,
                pivot: 0,
                data: vec![0.0, 500.0],
            },
            W3dAnimChannel {
                first_frame: 0,
                last_frame: 1,
                vector_len: 4,
                flags: 6,
                pivot: 0,
                data: vec![0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0],
            },
        ],
        raw_visibility_channels: vec![W3dRawVisibilityChannel {
            first_frame: 0,
            last_frame: 1,
            flags: 0,
            pivot: 0,
            default_visible: false,
            bits: vec![0],
        }],
        // A compressed pivot-zero source is likewise ignored because C++
        // replaces pivot zero before it queries visibility.
        unsupported_visibility_pivots: vec![Some(0)],
    });

    let binding = W3dAnimationBinding::local(0);
    let sampled = model
        .sample_animation_binding(&binding, 1.0)
        .expect("root-local source data must not invalidate the HTree pose");
    assert_eq!(sampled[0], Mat4::IDENTITY.to_cols_array());
    assert_eq!(
        Mat4::from_cols_array(&sampled[1]).w_axis.truncate(),
        Vec3::new(10.0, 20.0, 30.0),
        "a child must inherit the external object root, not authored pivot-zero data"
    );
    assert_eq!(
        model.animations[0].visibility_for_pivot(0, 1.0),
        Some(true),
        "pivot zero is always visible even when its raw source channel is hidden or unsupported"
    );

    let (bind_transform, bind_visible) = model
        .mesh_local_transform_and_visibility_for_binding(0, None, 0.0)
        .expect("bind-pose rigid HLOD child");
    let (animated_transform, animated_visible) = model
        .mesh_local_transform_and_visibility_for_binding(0, Some(&binding), 1.0)
        .expect("animated rigid HLOD child");
    assert!(bind_visible && animated_visible);
    assert_eq!(animated_transform, bind_transform);

    let palette = model
        .animation_palette_for_binding_and_capture_controls(Some(&binding), 1.0, &[])
        .expect("selected HAnim palette must use the same HTree root rule");
    assert_eq!(palette[0], Mat4::IDENTITY);
    assert_eq!(palette[1], animated_transform);

    model.calculate_bounding_box();
    assert_eq!(
        (model.bounding_box_min, model.bounding_box_max),
        original_bounds,
        "bind-pose bounds must ignore authored pivot-zero data"
    );

    let mut root_mesh_model = model.clone();
    root_mesh_model.hlods[0].lods[0].subobjects[0].bone_index = 0;
    let (root_transform, root_visible) = root_mesh_model
        .mesh_local_transform_and_visibility_for_binding(0, Some(&binding), 1.0)
        .expect("a valid root-bound HLOD child must receive the external root");
    assert_eq!(root_transform, Mat4::IDENTITY);
    assert!(root_visible);

    let loader_globals = W3DLoader::compute_global_transforms(
        root_mesh_model
            .hierarchy
            .as_ref()
            .expect("fixture hierarchy"),
    )
    .expect("loader residual HModel path must share the HTree root rule");
    assert_eq!(loader_globals[0], Mat4::IDENTITY.to_cols_array());
    assert_eq!(
        Mat4::from_cols_array(&loader_globals[1]).w_axis.truncate(),
        Vec3::new(10.0, 20.0, 30.0)
    );
}

#[test]
fn pre30_hierarchy_and_raw_animation_insert_and_address_the_synthetic_root() {
    const PRE30_VERSION: u32 = (2 << 16) | 1;

    let mut hierarchy_header = Vec::with_capacity(36);
    hierarchy_header.extend_from_slice(&PRE30_VERSION.to_le_bytes());
    hierarchy_header.extend_from_slice(&fixed_name("LEGACY_HIER", W3D_NAME_LEN));
    hierarchy_header.extend_from_slice(&2u32.to_le_bytes());
    hierarchy_header.extend_from_slice(&[0u8; 12]);
    let hierarchy_data = [
        chunk(W3D_CHUNK_HIERARCHY_HEADER, hierarchy_header, false),
        chunk(
            W3D_CHUNK_PIVOTS,
            [
                pivot("LEGACY_ROOT", u32::MAX, [0.0, 0.0, 0.0]),
                pivot("LEGACY_CHILD", 0, [2.0, 0.0, 0.0]),
            ]
            .concat(),
            false,
        ),
    ]
    .concat();

    let mut animation_header = Vec::with_capacity(44);
    animation_header.extend_from_slice(&PRE30_VERSION.to_le_bytes());
    animation_header.extend_from_slice(&fixed_name("LEGACY_ANIM", W3D_NAME_LEN));
    animation_header.extend_from_slice(&fixed_name("LEGACY_HIER", W3D_NAME_LEN));
    animation_header.extend_from_slice(&2u32.to_le_bytes());
    animation_header.extend_from_slice(&30u32.to_le_bytes());
    let mut root_x_channel = Vec::with_capacity(20);
    root_x_channel.extend_from_slice(&0u16.to_le_bytes());
    root_x_channel.extend_from_slice(&1u16.to_le_bytes());
    root_x_channel.extend_from_slice(&1u16.to_le_bytes());
    root_x_channel.extend_from_slice(&0u16.to_le_bytes());
    root_x_channel.extend_from_slice(&0u16.to_le_bytes());
    root_x_channel.extend_from_slice(&0u16.to_le_bytes());
    root_x_channel.extend_from_slice(&0.0f32.to_le_bytes());
    root_x_channel.extend_from_slice(&7.0f32.to_le_bytes());
    let mut child_visibility = Vec::with_capacity(10);
    child_visibility.extend_from_slice(&0u16.to_le_bytes());
    child_visibility.extend_from_slice(&1u16.to_le_bytes());
    child_visibility.extend_from_slice(&0u16.to_le_bytes());
    child_visibility.extend_from_slice(&1u16.to_le_bytes());
    child_visibility.push(1);
    // Source child pivot one is visible at frame zero and hidden at one.
    child_visibility.push(0b0000_0001);
    let animation_data = [
        chunk(W3D_CHUNK_ANIMATION_HEADER, animation_header, false),
        chunk(W3D_CHUNK_ANIMATION_CHANNEL, root_x_channel, false),
        chunk(W3D_CHUNK_BIT_CHANNEL, child_visibility, false),
    ]
    .concat();

    let loader = W3DLoader::new();
    let hierarchy = loader
        .parse_hierarchy_chunk(&hierarchy_data)
        .expect("pre-3.0 hierarchy source must normalize safely");
    assert_eq!(hierarchy.pivots.len(), 3);
    assert_eq!(hierarchy.pivots[0].name, "RootTransform");
    assert_eq!(hierarchy.pivots[0].parent_idx, u32::MAX);
    assert_eq!(hierarchy.pivots[1].name, "LEGACY_ROOT");
    assert_eq!(hierarchy.pivots[1].parent_idx, 0);
    assert_eq!(hierarchy.pivots[2].name, "LEGACY_CHILD");
    assert_eq!(hierarchy.pivots[2].parent_idx, 1);

    let animation = loader
        .parse_animation_chunk(&animation_data)
        .expect("pre-3.0 raw HAnim source must normalize safely");
    assert_eq!(animation.channels[0].pivot, 1);
    assert_eq!(animation.raw_visibility_channels[0].pivot, 2);
    assert_eq!(animation.visibility_for_pivot(2, 0.0), Some(true));
    assert_eq!(animation.visibility_for_pivot(2, 1.0), Some(false));

    let mut model = W3DModel::new("pre30_pose".to_string());
    model.hierarchy = Some(hierarchy);
    model.animations.push(animation);
    let sampled = model
        .sample_animation_binding(&W3dAnimationBinding::local(0), 1.0)
        .expect("shifted raw HAnim channel must reach the normalized hierarchy");
    assert_eq!(sampled[0], Mat4::IDENTITY.to_cols_array());
    assert_eq!(
        sampled[1][12], 7.0,
        "source root channel shifts to pivot one"
    );
    assert_eq!(
        sampled[2][12], 9.0,
        "the source child must inherit the shifted original root, not the synthetic external root"
    );
}

#[test]
fn multi_lod_rigid_hlod_uses_cxx_constructor_selection_and_retains_attachment_metadata() {
    let mut multi_lod = W3DLoader::new()
        .load_model_from_bytes(&rigid_hlod_fixture(2, &[], &[]), "multi_lod")
        .expect("multi-LOD source fixture should parse and retain metadata");
    assert_eq!(multi_lod.hlods[0].lods.len(), 2);

    // C++ `Calculate_Cost_Value_Arrays(1.0f, ...)` uses a strict `<`.
    // A level whose authored maximum is exactly one pixel remains the
    // constructor-selected minimum level.
    multi_lod.hlods[0].lods[0].max_screen_size = 1.0;
    multi_lod.hlods[0].lods[1].max_screen_size = f32::MAX;
    assert_eq!(
        W3DModel::cxx_constructor_selected_hlod_lod_index(&multi_lod.hlods[0]),
        Some(0)
    );

    // Once the first level is strictly below one, C++ raises CurLod to the
    // second level. Make only that selected level name the flattened mesh
    // so the transform proof cannot accidentally draw both source groups.
    multi_lod.hlods[0].lods[0].max_screen_size = 0.999_999;
    multi_lod.hlods[0].lods[0].subobjects[0].name = "HLODROOT.LOW_ONLY".to_string();
    assert_eq!(
        W3DModel::cxx_constructor_selected_hlod_lod_index(&multi_lod.hlods[0]),
        Some(1)
    );
    assert!(
        multi_lod
            .mesh_local_transform_for_animation(0, 0, 0.0)
            .is_some(),
        "only the C++ constructor-selected HLOD level may resolve a rigid child"
    );
    assert!(
        !multi_lod.mesh_visible_for_authored_subobject_directives(
            0,
            &[AuthoredDrawSubobjectVisibility {
                name: "RIGID".to_string(),
                hidden: true,
            }],
        ),
        "HideSubObject must resolve only within that same selected source level"
    );

    // If all levels are below one pixel, C++ clamps to the final (highest
    // detail) level rather than wrapping or choosing a guessed default.
    multi_lod.hlods[0].lods[1].max_screen_size = 0.5;
    assert_eq!(
        W3DModel::cxx_constructor_selected_hlod_lod_index(&multi_lod.hlods[0]),
        Some(1)
    );

    // A malformed threshold is not a license to render an arbitrary
    // source level in Main's bounded implementation.
    multi_lod.hlods[0].lods[0].max_screen_size = f32::NAN;
    assert!(
        multi_lod
            .mesh_local_transform_for_animation(0, 0, 0.0)
            .is_none(),
        "malformed HLOD thresholds must fail closed"
    );

    let aggregate = W3DLoader::new()
        .load_model_from_bytes(
            &rigid_hlod_fixture(
                1,
                &[("ATTACHED_MODEL", 1), ("SECOND_ATTACHMENT", 0)],
                &[("SPAWN_POINT", 1), ("RALLY_PROXY", 0)],
            ),
            "aggregate_hlod",
        )
        .expect("source-shaped attachment arrays should parse");
    let aggregate_hlod = &aggregate.hlods[0];
    assert_eq!(
        aggregate_hlod.aggregates.as_ref(),
        Some(&W3dHlodAttachmentArray {
            max_screen_size: 0.0,
            subobjects: vec![
                W3dHlodSubObject {
                    name: "ATTACHED_MODEL".to_string(),
                    bone_index: 1,
                },
                W3dHlodSubObject {
                    name: "SECOND_ATTACHMENT".to_string(),
                    bone_index: 0,
                }
            ],
        })
    );
    assert_eq!(
        aggregate_hlod.proxies.as_ref(),
        Some(&W3dHlodAttachmentArray {
            max_screen_size: 0.0,
            subobjects: vec![
                W3dHlodSubObject {
                    name: "SPAWN_POINT".to_string(),
                    bone_index: 1,
                },
                W3dHlodSubObject {
                    name: "RALLY_PROXY".to_string(),
                    bone_index: 0,
                }
            ],
        })
    );
    assert!(aggregate_hlod.has_unrendered_aggregates);
    assert!(!aggregate_hlod.has_invalid_trailing_records);
    let aggregate_poses = aggregate
        .aggregate_attachment_poses_for_binding(None, 0.0)
        .expect("a source-valid aggregate HLOD must expose exact parent-bone poses");
    assert_eq!(aggregate_poses.len(), 2);
    assert_eq!(aggregate_poses[0].name, "ATTACHED_MODEL");
    assert_eq!(aggregate_poses[0].bone_index, 1);
    assert!(aggregate_poses[0].visible);
    assert_eq!(
        aggregate_poses[0].parent_transform.w_axis.truncate(),
        Vec3::new(10.0, 30.0, 20.0),
        "aggregate pose must use the parent HTree bone in the same render basis as rigid children"
    );
    assert_eq!(aggregate_poses[1].name, "SECOND_ATTACHMENT");
    assert_eq!(aggregate_poses[1].bone_index, 0);
    let controlled_poses = aggregate
        .aggregate_attachment_poses_for_binding_and_capture_controls(
            None,
            0.0,
            &[(1, Mat4::from_translation(Vec3::new(-2.0, 0.0, 0.0)))],
        )
        .expect("valid source capture controls must update aggregate parent poses");
    assert_eq!(
        controlled_poses[0].parent_transform.w_axis.truncate(),
        Vec3::new(8.0, 30.0, 20.0),
        "C++ controls post-multiply the aggregate's HTree parent pivot before child rendering"
    );
    let mut authored_root_offset = aggregate.clone();
    authored_root_offset
        .hierarchy
        .as_mut()
        .expect("synthetic aggregate hierarchy")
        .pivots[0]
        .translation = [99.0, 88.0, 77.0];
    let root_ignored_poses = authored_root_offset
        .aggregate_attachment_poses_for_binding(None, 0.0)
        .expect("HTree root handling must remain valid");
    assert_eq!(
        root_ignored_poses[0].parent_transform.w_axis.truncate(),
        Vec3::new(10.0, 30.0, 20.0),
        "HTree overwrites pivot zero with the parent object root; W3D pivot-zero bind data must not leak into the child attachment pose"
    );
    assert!(
        aggregate
            .mesh_local_transform_for_animation(0, 0, 0.0)
            .is_some(),
        "C++ keeps the selected parent LOD visible while each aggregate is resolved or skipped independently"
    );

    let proxy_only = W3DLoader::new()
        .load_model_from_bytes(
            &rigid_hlod_fixture(1, &[], &[("SPAWN_POINT", 1)]),
            "proxy_hlod",
        )
        .expect("source-shaped proxy array should parse");
    assert!(!proxy_only.hlods[0].has_unrendered_aggregates);
    assert!(!proxy_only.hlods[0].has_invalid_trailing_records);
    assert!(
        proxy_only
            .aggregate_attachment_poses_for_binding(None, 0.0)
            .is_none(),
        "proxies remain source application records, not implicit aggregate render objects"
    );
    assert!(
        proxy_only
            .mesh_local_transform_for_animation(0, 0, 0.0)
            .is_some(),
        "C++ proxies are non-rendering application records and must not hide the parent HLOD"
    );

    let mut malformed_proxy = rigid_hlod_fixture(1, &[], &[("SPAWN_POINT", 1)]);
    let proxy_chunk_type = W3D_CHUNK_HLOD_PROXY_ARRAY.to_le_bytes();
    let proxy_offset = malformed_proxy
        .windows(proxy_chunk_type.len())
        .position(|window| window == proxy_chunk_type)
        .expect("synthetic fixture must contain its proxy chunk");
    let raw_size_offset = proxy_offset + proxy_chunk_type.len();
    let raw_size = u32::from_le_bytes(
        malformed_proxy[raw_size_offset..raw_size_offset + 4]
            .try_into()
            .expect("proxy chunk must carry a raw size"),
    );
    malformed_proxy[raw_size_offset..raw_size_offset + 4]
        .copy_from_slice(&(raw_size & 0x7FFF_FFFF).to_le_bytes());
    let malformed_proxy = W3DLoader::new()
        .load_model_from_bytes(&malformed_proxy, "malformed_proxy_hlod")
        .expect("malformed trailing metadata must safely retain the outer HLOD record");
    assert!(malformed_proxy.hlods[0].has_invalid_trailing_records);
    assert!(
        malformed_proxy
            .mesh_local_transform_for_animation(0, 0, 0.0)
            .is_none(),
        "a malformed source attachment record must not be treated as safe rigid topology"
    );
}

#[test]
fn retail_america_command_center_hlod_retains_rigid_bone_records_when_available() {
    let Some(path) = crate::assets::mesh_asset_resolve::find_filesystem_w3d("ABBtCmdHQ") else {
        eprintln!("skip: retail ABBtCmdHQ.W3D is not available on disk");
        return;
    };
    let model = W3DLoader::new()
        .load_model_from_path(&path)
        .expect("retail AmericaCommandCenter W3D should parse");
    let hlod = model
        .hlods
        .iter()
        .find(|hlod| hlod.name.eq_ignore_ascii_case("ABBTCMDHQ"))
        .expect("ABBtCmdHQ must retain its HLOD header");
    let fan = hlod.lods[0]
        .subobjects
        .iter()
        .find(|subobject| subobject.name.eq_ignore_ascii_case("ABBTCMDHQ.FAN03"))
        .expect("retail Command Center HLOD must retain FAN03 source identity");
    assert_eq!(fan.bone_index, 2);

    let fan_mesh_index = model
        .meshes
        .iter()
        .position(|mesh| {
            mesh.name.eq_ignore_ascii_case("FAN03")
                && mesh.container_name.eq_ignore_ascii_case("ABBTCMDHQ")
        })
        .expect("retail Command Center must include FAN03 mesh with source ContainerName");
    assert!(
        model
            .mesh_local_transform_for_animation(fan_mesh_index, 0, 0.0)
            .is_some(),
        "retail FAN03 must bind through the authored HLOD record"
    );
}

#[test]
fn w3d_hlod_visibility_hide_show_subobjects_retail_scorpion_maps_exact_hlod_child_when_available() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let ini_path =
        root.join("windows_game/extracted_big_files/INIZH/Data/INI/Object/GC_Chem_GLAUnits.ini");
    let Some(w3d_path) = crate::assets::mesh_asset_resolve::find_filesystem_w3d("UVLiteTank")
    else {
        eprintln!("skip: retail UVLiteTank.W3D is not available on disk");
        return;
    };
    let Ok(ini_content) = std::fs::read_to_string(&ini_path) else {
        eprintln!(
            "skip: retail Scorpion Object INI is not available at {}",
            ini_path.display()
        );
        return;
    };

    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&ini_content, "GC_Chem_GLAUnits.ini")
        .expect("parse retail Scorpion source Draw state");
    let scorpion = parser
        .get_definition("GC_Chem_GLATankScorpion")
        .expect("retail Chem Scorpion definition");
    let upgrade_bit_index =
        crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
            "WEAPONSET_PLAYER_UPGRADE",
        )
        .expect("retail player-upgrade condition bit");
    let upgrade_bits = 1u128
        .checked_shl(u32::try_from(upgrade_bit_index).expect("condition bit index fits u32"))
        .expect("condition bit fits retained bank");
    let default_draw = scorpion
        .select_draw_models_for_conditions(0)
        .expect("retail pristine Scorpion draw state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
        .expect("retail pristine Scorpion selects UVLiteTank");
    let upgraded_draw = scorpion
        .select_draw_models_for_conditions(upgrade_bits)
        .expect("retail upgraded Scorpion draw state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
        .expect("retail upgraded Scorpion retains UVLiteTank");

    assert!(
        default_draw
            .subobject_visibility
            .iter()
            .any(|directive| directive.name == "misslerack01" && directive.hidden),
        "retail DefaultConditionState hides the misspelled source rack leaf"
    );
    assert!(
        upgraded_draw
            .subobject_visibility
            .iter()
            .any(|directive| directive.name == "misslerack01" && !directive.hidden),
        "retail upgrade state overwrites the inherited rack directive in place"
    );

    let model = W3DLoader::new()
        .load_model_from_path(&w3d_path)
        .expect("retail UVLiteTank W3D should parse");
    let rack_mesh_index = model
        .meshes
        .iter()
        .position(|mesh| {
            mesh.name.eq_ignore_ascii_case("MISSLERACK01")
                && mesh.container_name.eq_ignore_ascii_case("UVLITETANK")
        })
        .expect("retail UVLiteTank mesh must retain exact HLOD child identity");
    assert!(
        !model.mesh_visible_for_authored_subobject_directives(
            rack_mesh_index,
            &default_draw.subobject_visibility,
        ),
        "pristine Scorpion hides only the source-authored exact rack child"
    );
    assert!(
        model.mesh_visible_for_authored_subobject_directives(
            rack_mesh_index,
            &upgraded_draw.subobject_visibility,
        ),
        "upgrade state shows that same source-authored rack child"
    );
}

#[test]
fn w3d_hlod_turret_retail_scorpion_retains_exact_primary_turret_binding_when_available() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let ini_path =
        root.join("windows_game/extracted_big_files/INIZH/Data/INI/Object/GC_Chem_GLAUnits.ini");
    let Some(w3d_path) = crate::assets::mesh_asset_resolve::find_filesystem_w3d("UVLiteTank")
    else {
        eprintln!("skip: retail UVLiteTank.W3D is not available on disk");
        return;
    };
    let Ok(ini_content) = std::fs::read_to_string(&ini_path) else {
        eprintln!(
            "skip: retail Scorpion Object INI is not available at {}",
            ini_path.display()
        );
        return;
    };

    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&ini_content, "GC_Chem_GLAUnits.ini")
        .expect("parse retail Chem Scorpion Draw states");
    let scorpion = parser
        .get_definition("GC_Chem_GLATankScorpion")
        .expect("retail Chem Scorpion definition");
    let draw = scorpion
        .select_draw_models_for_conditions(0)
        .expect("retail pristine Scorpion draw state")
        .into_iter()
        .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
        .expect("retail pristine Scorpion selects UVLiteTank");
    assert_eq!(draw.primary_turret.yaw_bone.as_deref(), Some("turret01"));
    assert_eq!(draw.primary_turret.pitch_bone, None);
    assert!(!draw.primary_turret.has_unsupported_alternate_turret());

    let model = W3DLoader::new()
        .load_model_from_path(&w3d_path)
        .expect("retail UVLiteTank W3D should parse");
    let (hlod, hierarchy) = model
        .rigid_hlod_context()
        .expect("retail Scorpion body must use the currently supported single HLOD path");
    let turret_pivot_index = W3DModel::primary_turret_pivot_index(hierarchy, "turret01")
        .expect("retail source Turret01 must resolve to a non-root exact HTree pivot");
    let turret_child = hlod.lods[0]
        .subobjects
        .iter()
        .find(|child| child.bone_index == turret_pivot_index as u32)
        .expect("retail source HLOD must retain a child owned by Turret01");
    let turret_mesh_index = model
        .meshes
        .iter()
        .position(|mesh| {
            mesh.container_name.eq_ignore_ascii_case(hlod.name.as_str())
                && format!("{}.{}", mesh.container_name, mesh.name)
                    .eq_ignore_ascii_case(turret_child.name.as_str())
        })
        .expect("retail Turret01 HLOD child must map to an exact flattened Main mesh");
    assert!(
        model
            .mesh_local_transform_and_visibility_for_primary_turret(
                turret_mesh_index,
                None,
                0.0,
                &draw.primary_turret,
                0.0,
                0.0,
            )
            .is_some(),
        "retail Scorpion's exact Turret01 binding must carry through the active rigid-HLOD path"
    );
}

#[test]
fn w3d_hlod_visibility_retail_boss_airfield_binds_redlight_bone() {
    let Some(path) = crate::assets::mesh_asset_resolve::find_filesystem_w3d("NBAirfield_DS") else {
        eprintln!("skip: retail NBAirfield_DS.W3D is not available on disk");
        return;
    };
    let model = W3DLoader::new()
        .load_model_from_path(&path)
        .expect("retail Boss airfield W3D should parse");
    let animation_index = model
        .find_animation_index_for_draw_identity("nbairfield_ds.nbairfield_ds")
        .expect("retail Draw Animation identity must resolve exactly");
    let animation = &model.animations[animation_index];
    assert!(
        animation
            .raw_visibility_channels
            .iter()
            .any(|channel| channel.pivot == 15
                && channel.first_frame == 20
                && channel.last_frame == 79),
        "retail airfield retains raw pivot-15 visibility source"
    );
    let mesh_index = model
        .meshes
        .iter()
        .position(|mesh| {
            mesh.name.eq_ignore_ascii_case("REDLIGHT06")
                && mesh.container_name.eq_ignore_ascii_case("NBAIRFIELD_DS")
        })
        .expect("retail airfield red light retains exact source mesh identity");
    let before = model
        .mesh_local_transform_and_visibility_for_animation(mesh_index, Some(animation_index), 0.0)
        .expect("retail red light should resolve through source HLOD bone");
    let hidden = model
        .mesh_local_transform_and_visibility_for_animation(mesh_index, Some(animation_index), 20.0)
        .expect("retail red light should sample authored bit channel");
    assert!(before.1, "before FirstFrame, DefaultVal is visible");
    assert!(!hidden.1, "retail frame 20 is authored hidden for pivot 15");
}

#[test]
fn sample_w3d_still_loads_via_from_path_when_present() {
    let path = crate::assets::mesh_asset_resolve::find_filesystem_w3d("AmericaCommandCenter")
        .or_else(|| crate::assets::mesh_asset_resolve::find_filesystem_w3d("ABBtCmdHQ"))
        .or_else(|| crate::assets::mesh_asset_resolve::find_filesystem_w3d("airanger_s"));
    let Some(path) = path else {
        eprintln!("skip: no sample W3D on disk");
        // Candidate list is still the archive contract when bytes are absent.
        let paths = w3d_archive_path_variants("AmericaCommandCenter");
        assert!(paths.iter().any(|p| p == "Art/W3D/ABBtCmdHQ.W3D"));
        return;
    };
    let model = W3DLoader::new()
        .load_model_from_path(&path)
        .expect("sample W3D should parse");
    assert!(
        !model.meshes.is_empty(),
        "sample W3D at {} parsed with zero meshes",
        path.display()
    );
}
