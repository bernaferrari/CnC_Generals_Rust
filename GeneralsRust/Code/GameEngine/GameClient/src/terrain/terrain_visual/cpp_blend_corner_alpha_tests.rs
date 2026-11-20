use super::*;

#[test]
fn legacy_stretched_cliff_keeps_source_tile_and_height_diagonal() {
    let mut hm = HeightMap::new(2, 2, 255.0, 1.0);
    hm.adjust_cliff_textures = true;
    hm.tile_ndxes[0] = 4;
    hm.set_raw_height(1, 1, 100);
    let classes = [TerrainSourceTileClass {
        first_tile: 0,
        num_tiles: 4,
        width: 2,
        name: "Cliff".into(),
    }];

    // C++ adjusts the high corner, retains the source tile's atlas
    // origin, and flips when |h0-h2| > |h1-h3|.
    assert!(authored_cell_flip(&hm, &classes, 0, 0, 4, None));
    assert_eq!(
        cell_class_uv(&hm, &classes, [0.5, 0.5], [0.0, 0.0]),
        [0.5, 1.0]
    );
    assert_eq!(
        class_uv_for_tile(&hm, &classes, 0, 0, 4, 1.0, 1.0),
        [0.75, 0.0]
    );
    // A missing class does not run the old stretch or change the split.
    assert!(!authored_cell_flip(&hm, &[], 0, 0, 4, None));
    assert_eq!(class_uv_for_tile(&hm, &[], 0, 0, 4, 1.0, 1.0), [0.5, 0.5]);
    hm.adjust_cliff_textures = false;
    assert!(!authored_cell_flip(&hm, &classes, 0, 0, 4, None));
}

#[test]
fn legacy_cliff_one_low_corner_is_shifted_back_inside_class() {
    let mut hm = HeightMap::new(2, 2, 255.0, 1.0);
    hm.adjust_cliff_textures = true;
    hm.tile_ndxes[0] = 4;
    for (x, y) in [(1, 0), (1, 1), (0, 1)] {
        hm.set_raw_height(x, y, 100);
    }
    let classes = [TerrainSourceTileClass {
        first_tile: 0,
        num_tiles: 4,
        width: 2,
        name: "Cliff".into(),
    }];
    // Whole-class V extent is 1; the low corner moves to 1.75 and
    // the entire quad is translated down by 0.75.
    assert_eq!(
        class_uv_for_tile(&hm, &classes, 0, 0, 4, 0.0, 0.0),
        [0.5, 1.0]
    );
    assert_eq!(
        class_uv_for_tile(&hm, &classes, 0, 0, 4, 1.0, 0.0),
        [0.75, 0.25]
    );
    assert_eq!(
        class_uv_for_tile(&hm, &classes, 0, 0, 4, 1.0, 1.0),
        [0.75, 0.0]
    );
    assert!(authored_cell_flip(&hm, &classes, 0, 0, 4, None));
}

#[test]
fn extra_blend_cliff_uses_matching_class_for_its_triangle_split() {
    crate::test_support::ensure_ambient_host();
    let mut hm = HeightMap::new(3, 3, 255.0, 1.0);
    hm.adjust_cliff_textures = true;
    hm.set_raw_height(1, 1, 100);
    hm.cliff_info.push(crate::terrain::TCliffInfo {
        tile_index: 0,
        flip: true,
        ..Default::default()
    });
    hm.cliff_info_ndxes[0] = 1;
    let mut blend = crate::terrain::textures::BlendTileInfo::new();
    blend.blend_ndx = 4; // different tile in the same authored cliff class
    blend.horiz = 1;
    hm.blended_tiles = vec![crate::terrain::textures::BlendTileInfo::new(), blend];
    hm.extra_blend_tile_ndxes[0] = 1;
    let mut visual = TerrainVisualImpl::new();
    visual.load_heightmap_from_data(hm, None, None).unwrap();
    visual.source_tile_classes = vec![TerrainSourceTileClass {
        first_tile: 0,
        num_tiles: 4,
        width: 2,
        name: "Cliff".into(),
    }];
    let mesh = visual.build_extra_blend_draw_mesh();
    let positions: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| [v.position[0], v.position[2]])
        .collect();
    assert_eq!(
        positions,
        vec![
            [1.0, 0.0],
            [0.0, 1.0],
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
        ]
    );
}

#[test]
fn authored_cliff_uv_uses_class_coordinates_for_base_and_blend_tiles() {
    crate::test_support::ensure_ambient_host();
    let mut hm = HeightMap::new(2, 2, 10.0, 1.0);
    hm.tile_ndxes[0] = 4;
    hm.cliff_info.push(crate::terrain::TCliffInfo {
        tile_index: 0,
        u0: 0.0,
        u1: 0.03125,
        u2: 0.0625,
        u3: 0.0,
        v0: 0.0,
        v1: -0.03125,
        v2: -0.0625,
        v3: -0.03125,
        flip: true,
        mutant: false,
    });
    hm.cliff_info_ndxes[0] = 1;
    let classes = [TerrainSourceTileClass {
        first_tile: 0,
        num_tiles: 4,
        width: 2,
        name: "Cliff".into(),
    }];
    hm.set_raw_height(1, 1, 100);
    let mut edge = crate::terrain::textures::BlendTileInfo::new();
    edge.custom_blend_edge_class = 0;
    assert!(!authored_cell_flip(&hm, &classes, 0, 0, 4, Some(&edge)));
    // The receiving GameData switch gates both base and blend remaps.
    assert_eq!(
        class_uv_for_tile(&hm, &classes, 0, 0, 4, 1.0, 1.0),
        [0.75, 0.75]
    );
    hm.adjust_cliff_textures = true;
    assert!(authored_cell_flip(&hm, &classes, 0, 0, 4, Some(&edge)));
    // A different source tile in the same class still uses the cliff map.
    assert_eq!(
        class_uv_for_tile(&hm, &classes, 0, 0, 4, 1.0, 1.0),
        [1.0, 0.0]
    );
    assert_eq!(
        cell_class_uv(&hm, &classes, [0.5, 0.5], [1.0, 1.0]),
        [1.0, 0.0]
    );
    assert_eq!(
        class_uv_for_tile(&hm, &classes, 0, 0, 4, 1.0, 0.0),
        [0.5, 0.5]
    );
}
use crate::terrain::textures::{BlendTileInfo, FLIPPED_MASK, INVERTED_MASK};

#[test]
fn horizontal_and_vertical_blends_cover_one_edge() {
    let mut b = BlendTileInfo::new();
    b.horiz = 1;
    assert_eq!(cpp_blend_corner_alpha(&b), ([0.0, 1.0, 1.0, 0.0], false));
    b.inverted = INVERTED_MASK | FLIPPED_MASK;
    assert_eq!(cpp_blend_corner_alpha(&b), ([1.0, 0.0, 0.0, 1.0], true));
    let mut v = BlendTileInfo::new();
    v.vert = 1;
    assert_eq!(cpp_blend_corner_alpha(&v), ([0.0, 0.0, 1.0, 1.0], false));
}

#[test]
fn diagonals_flip_the_cell_like_cpp() {
    let mut r = BlendTileInfo::new();
    r.right_diagonal = 1;
    assert_eq!(cpp_blend_corner_alpha(&r), ([0.0, 0.0, 1.0, 0.0], true));
    r.long_diagonal = 1;
    assert_eq!(cpp_blend_corner_alpha(&r), ([0.0, 1.0, 1.0, 1.0], true));
    let mut l = BlendTileInfo::new();
    l.left_diagonal = 1;
    l.inverted = INVERTED_MASK;
    assert_eq!(cpp_blend_corner_alpha(&l), ([1.0, 0.0, 0.0, 0.0], true));
    l.custom_blend_edge_class = 2;
    assert_eq!(cpp_blend_corner_alpha(&l), ([0.0; 4], false));
    assert_eq!(cell_corner(1.0, 0.0), 1);
    assert_eq!(cell_corner(0.0, 1.0), 3);
}
