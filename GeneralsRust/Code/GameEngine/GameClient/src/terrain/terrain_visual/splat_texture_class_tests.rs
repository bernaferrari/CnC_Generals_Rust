use super::*;

fn class(first_tile: i32, num_tiles: i32, name: &str) -> TerrainSourceTileClass {
    TerrainSourceTileClass {
        first_tile,
        num_tiles,
        width: 2,
        name: name.to_string(),
    }
}

fn bound_fixture() -> (
    TerrainTextures,
    Vec<TerrainSourceTileClass>,
    [TextureId; MAX_TEXTURES_PER_CHUNK],
    HashMap<TextureId, usize>,
) {
    let mut textures = TerrainTextures::new();
    let grass = textures.register_texture(TerrainTexture::new(
        0,
        "Grass".to_string(),
        "Art/Terrain/Grass.tga".to_string(),
    ));
    let rock = textures.register_texture(TerrainTexture::new(
        0,
        "Rock".to_string(),
        "Art/Terrain/Rock.tga".to_string(),
    ));
    let snow = textures.register_texture(TerrainTexture::new(
        0,
        "Snow".to_string(),
        "Art/Terrain/Snow.tga".to_string(),
    ));
    let dirt = textures.register_texture(TerrainTexture::new(
        0,
        "Dirt".to_string(),
        "Art/Terrain/Dirt.tga".to_string(),
    ));
    let mut stable = [grass; MAX_TEXTURES_PER_CHUNK];
    stable[..4].copy_from_slice(&[grass, rock, snow, dirt]);
    let mut slot_map = HashMap::new();
    for (slot, id) in stable.iter().enumerate() {
        slot_map.entry(*id).or_insert(slot);
    }
    let classes = vec![
        class(0, 4, "Grass"),
        class(4, 4, "Rock"),
        class(8, 4, "Snow"),
        class(12, 4, "Dirt"),
    ];
    (textures, classes, stable, slot_map)
}

#[test]
fn authored_mixed_cell_carries_independent_uvs_and_corner_coverage() {
    let (textures, classes, stable, slots) = bound_fixture();
    let mut hm = HeightMap::new(3, 2, 255.0, 1.0);
    hm.tile_ndxes[0] = 0;
    let mut blend = crate::terrain::textures::BlendTileInfo::new();
    blend.blend_ndx = 5; // same class, next tile, right-hand quadrant
    blend.horiz = 1;
    hm.blended_tiles = vec![crate::terrain::textures::BlendTileInfo::new(), blend];
    hm.blend_tile_ndxes[0] = 1;

    let left = authored_blend_vertex(
        &hm,
        &classes,
        &textures,
        &stable,
        &slots,
        [0.5, 0.5],
        [0.0, 0.0],
    )
    .unwrap();
    let right = authored_blend_vertex(
        &hm,
        &classes,
        &textures,
        &stable,
        &slots,
        [0.5, 0.5],
        [1.0, 0.0],
    )
    .unwrap();
    assert_eq!(
        (left.0, left.1),
        (0, 0),
        "same-class tiles still need two UVs"
    );
    assert_eq!((left.2, right.2), (0.0, 1.0));
    assert_eq!(left.3, [0.75, 1.0]);
    assert_eq!(right.3, [1.0, 1.0]);
    assert_eq!(
        cell_class_uv(&hm, &classes, [0.5, 0.5], [0.0, 0.0]),
        [0.0, 1.0]
    );
    assert!(
        authored_blend_vertex(
            &hm,
            &classes,
            &textures,
            &stable,
            &slots,
            [1.5, 0.5],
            [1.0, 0.0]
        )
        .is_none(),
        "unblended neighbor stays opaque"
    );
}

#[test]
fn extra_blend_gpu_mesh_uses_authored_class_uv_and_alpha() {
    crate::test_support::ensure_ambient_host();
    let (textures, classes, stable, _) = bound_fixture();
    let mut hm = HeightMap::new(3, 2, 255.0, 1.0);
    let mut blend = crate::terrain::textures::BlendTileInfo::new();
    blend.blend_ndx = 16;
    blend.horiz = 1;
    hm.blended_tiles = vec![crate::terrain::textures::BlendTileInfo::new(), blend];
    hm.extra_blend_tile_ndxes[0] = 1;
    let mesh = hm.build_extra_blend_draw_mesh(&[]);
    assert_eq!(
        mesh.tile_count, 1,
        "the unblended adjacent cell emits no overlay"
    );
    let mut visual = TerrainVisualImpl::new();
    visual.height_map = Some(hm);
    visual.texture_system = textures;
    visual.source_tile_classes = classes;
    visual.active_chunk_texture_ids = Some(stable);
    let gpu = visual.extra_blend_mesh_to_terrain_vertices(&mesh, None);
    assert_eq!(gpu.len(), 6);
    for vertex in &gpu {
        assert_eq!(vertex.blend_indices[0], 1);
        assert_eq!(vertex.color[3], vertex.position[0]);
        assert_eq!(vertex.tex_coords[0], vertex.position[0] * 0.25);
    }
}

#[test]
fn extra_blend_mesh_respects_authored_diagonal_and_window_edges() {
    let mut hm = HeightMap::new(3, 2, 255.0, 1.0);
    let mut blend = crate::terrain::textures::BlendTileInfo::new();
    blend.blend_ndx = 16;
    blend.right_diagonal = 1;
    blend.long_diagonal = 1;
    hm.blended_tiles = vec![crate::terrain::textures::BlendTileInfo::new(), blend];
    hm.extra_blend_tile_ndxes[0] = 1;

    let mesh = hm.build_extra_blend_draw_mesh_for_window(&[], 0, 0, 3, 2);
    assert_eq!(mesh.tile_count, 1);
    let corners: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| ([v.position[0], v.position[2]], v.color[3]))
        .collect();
    // C++ right long diagonal, not inverted: flipped split 1,3,0 / 1,2,3.
    assert_eq!(
        corners,
        vec![
            ([1.0, 0.0], 1.0),
            ([0.0, 1.0], 1.0),
            ([0.0, 0.0], 0.0),
            ([1.0, 0.0], 1.0),
            ([1.0, 1.0], 1.0),
            ([0.0, 1.0], 1.0),
        ]
    );
    assert!(
        hm.build_extra_blend_draw_mesh_for_window(&[], 1, 0, 2, 2)
            .is_empty()
    );
}

#[test]
fn get_texture_class_from_ndx_includes_tile_zero() {
    crate::test_support::ensure_ambient_host();
    // C++ WorldHeightMap::getTextureClassFromNdx (WorldHeightMap.cpp:2308)
    // after tileNdx>>2. firstTile==0 is a valid class.
    let classes = [class(0, 4, "Grass"), class(4, 4, "Rock")];
    assert_eq!(texture_class_index_from_ndx(0, &classes), Some(0));
    assert!(source_tile_class_contains(&classes, 0));
    assert_eq!(texture_class_index_from_ndx(3, &classes), Some(0));
    assert_eq!(texture_class_index_from_ndx(4, &classes), Some(1));
}

#[test]
fn adjacent_classes_bind_different_texture_id_slots() {
    crate::test_support::ensure_ambient_host();
    let (textures, classes, stable, slot_map) = bound_fixture();
    let grass = bound_texture_id_for_source_tile(0, &classes, &textures, &stable, &slot_map);
    let rock = bound_texture_id_for_source_tile(4, &classes, &textures, &stable, &slot_map);
    let snow = bound_texture_id_for_source_tile(8, &classes, &textures, &stable, &slot_map);
    assert_eq!(grass, Some(stable[0]));
    assert_eq!(rock, Some(stable[1]));
    assert_eq!(snow, Some(stable[2]));
    assert_ne!(grass, rock);
    assert_ne!(rock, snow);
    // tile%4 would collapse 0/4/8 onto the same slot.
    assert_ne!(slot_map[&grass.unwrap()], slot_map[&rock.unwrap()]);
    assert_ne!(slot_map[&rock.unwrap()], slot_map[&snow.unwrap()]);
}

#[test]
fn unmapped_tile_is_not_used_as_slot_index() {
    crate::test_support::ensure_ambient_host();
    let (textures, classes, stable, slot_map) = bound_fixture();
    // packed 99 is outside firstTile/numTiles — must not become 99%4.
    assert_eq!(
        bound_texture_id_for_source_tile(99, &classes, &textures, &stable, &slot_map),
        None
    );
}
