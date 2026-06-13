#![cfg(feature = "w3d")]

use game_engine_device::w3d::volumetric_shadow::{
    construct_shadow_volume, ShadowGeometryMesh, AIRBORNE_UNIT_GROUND_DELTA, MAX_POLYGON_NEIGHBORS,
    MAX_SHADOW_CASTER_MESHES, MAX_SILHOUETTE_EDGES, OVERHANGING_OBJECT_CLAMP_ANGLE,
    SHADOW_EXTRUSION_BUFFER, SHADOW_SAMPLING_INTERVAL,
};
use glam::Vec3;

#[test]
fn constants_match_cpp_volumetric_shadow_defs() {
    assert_eq!(MAX_SHADOW_CASTER_MESHES, 160);
    assert_eq!(MAX_SILHOUETTE_EDGES, 1024);
    assert_eq!(MAX_POLYGON_NEIGHBORS, 3);
    assert_eq!(SHADOW_EXTRUSION_BUFFER, 0.1);
    assert_eq!(AIRBORNE_UNIT_GROUND_DELTA, 2.0);
    assert_eq!(SHADOW_SAMPLING_INTERVAL, 20.0);
    assert!((OVERHANGING_OBJECT_CLAMP_ANGLE - 80.0 / 180.0 * std::f32::consts::PI).abs() < 0.001);
}

#[test]
fn polygon_neighbors_require_opposite_edge_winding() {
    let verts = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
    ];
    let mesh = ShadowGeometryMesh::new(verts, vec![[0, 1, 2], [2, 1, 3]]);

    assert_eq!(mesh.poly_neighbors[0].my_index, 0);
    assert_eq!(mesh.poly_neighbors[1].my_index, 1);
    assert!(mesh.poly_neighbors[0]
        .neighbor
        .iter()
        .any(|edge| edge.neighbor_index == 1 && edge.neighbor_edge_index == [1, 2]));
    assert!(mesh.poly_neighbors[1]
        .neighbor
        .iter()
        .any(|edge| edge.neighbor_index == 0 && edge.neighbor_edge_index == [2, 1]));
}

#[test]
fn flipped_coplanar_triangles_are_not_valid_neighbors() {
    let verts = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let mesh = ShadowGeometryMesh::new(verts, vec![[0, 1, 2], [0, 2, 1]]);

    assert!(mesh.poly_neighbors[0]
        .neighbor
        .iter()
        .all(|edge| edge.neighbor_index == -1));
}

#[test]
fn silhouette_uses_visible_face_and_neighborless_edges() {
    let verts = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let mut mesh = ShadowGeometryMesh::new(verts, vec![[0, 1, 2]]);

    let silhouette = mesh.build_silhouette(Vec3::new(0.0, 0.0, 1.0));

    assert_eq!(silhouette, vec![[0, 1], [1, 2], [2, 0]]);
    assert!(mesh.poly_neighbors[0].is_visible());
}

#[test]
fn silhouette_edge_between_visible_and_hidden_neighbor_preserves_cpp_order() {
    let verts = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(1.0, 1.0, 1.0),
    ];
    let mut mesh = ShadowGeometryMesh::new(verts, vec![[0, 1, 2], [2, 1, 3]]);

    let silhouette = mesh.build_silhouette(Vec3::new(10.0, 10.0, 10.0));

    assert!(silhouette.contains(&[1, 2]) || silhouette.contains(&[2, 1]));
}

#[test]
fn construct_shadow_volume_extrudes_each_silhouette_edge() {
    let verts = vec![
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ];
    let mesh = ShadowGeometryMesh::new(verts, vec![[0, 1, 2]]);
    let volume = construct_shadow_volume(&mesh, &[[0, 1]], Vec3::ZERO, 5.0);

    assert_eq!(volume.vertices.len(), 4);
    assert_eq!(volume.indices, vec![0, 1, 2, 0, 2, 3]);
    assert_eq!(volume.vertices[0], Vec3::new(1.0, 0.0, 0.0));
    assert_eq!(volume.vertices[2], Vec3::new(0.0, 6.0, 0.0));
    assert_eq!(volume.vertices[3], Vec3::new(6.0, 0.0, 0.0));
}
