use super::super::*;

#[test]
fn runtime_heightmap_roundtrip_preserves_samples() {
    let hm = PresentationRuntimeHeightmap {
        width: 2,
        height: 2,
        heights: vec![0.0, 0.25, 0.5, 1.0],
        max_height: 100.0,
        scale: 10.0,
        min_height: 0.0,
        height_range: 100.0,
        border_size: 0,
        tile_ndxes: vec![],
        blend_tile_ndxes: vec![],
        extra_blend_tile_ndxes: vec![0, 5, 0, 2],
        blended_tiles: vec![PresentationBlendTileInfo {
            blend_ndx: 17,
            horiz: 1,
            vert: 2,
            right_diagonal: 3,
            left_diagonal: 4,
            inverted: 5,
            long_diagonal: 6,
            custom_blend_edge_class: 7,
        }],
        extra_blended_tiles: vec![PresentationBlendTileInfo {
            blend_ndx: 29,
            horiz: 8,
            vert: 9,
            right_diagonal: 10,
            left_diagonal: 11,
            inverted: 12,
            long_diagonal: 13,
            custom_blend_edge_class: 14,
        }],
        draw_origin_x: 0,
        draw_origin_y: 0,
        draw_width: 2,
        draw_height: 2,
    };
    assert!(hm.is_usable());
    assert!(!PresentationRuntimeHeightmap::default().is_usable());
    #[cfg(feature = "game_client")]
    {
        let back = PresentationRuntimeHeightmap::from_height_map(&hm.to_height_map());
        assert_eq!(back.heights, hm.heights);
        assert_eq!(back.width, 2);
        assert_eq!(back.extra_blend_tile_ndxes, vec![0, 5, 0, 2]);
        assert_eq!(back.blended_tiles, hm.blended_tiles);
        assert_eq!(back.extra_blended_tiles, hm.extra_blended_tiles);
    }
}

#[test]
fn runtime_heightmap_extra_blend_positions_pack_i_or_j_shift_16() {
    let hm = PresentationRuntimeHeightmap {
        width: 3,
        height: 3,
        heights: vec![0.0; 9],
        max_height: 100.0,
        scale: 10.0,
        min_height: 0.0,
        height_range: 100.0,
        border_size: 0,
        tile_ndxes: vec![0; 9],
        blend_tile_ndxes: vec![0; 9],
        extra_blend_tile_ndxes: {
            let mut extra = vec![0i16; 9];
            extra[0] = 2; // cell (0,0)
            extra[4] = 7; // cell (1,1)
            extra
        },
        blended_tiles: vec![],
        extra_blended_tiles: vec![],
        draw_origin_x: 0,
        draw_origin_y: 0,
        draw_width: 3,
        draw_height: 3,
    };
    assert_eq!(
        hm.extra_blend_tile_positions(),
        vec![0 | (0 << 16), 1 | (1 << 16)]
    );
}

#[test]
fn runtime_heightmap_old_serialized_frames_default_new_blend_metadata() {
    let mut encoded = serde_json::to_value(PresentationRuntimeHeightmap {
        width: 1,
        height: 1,
        heights: vec![0.0],
        max_height: 100.0,
        scale: 1.0,
        height_range: 100.0,
        ..Default::default()
    })
    .expect("serialize heightmap frame");
    let object = encoded
        .as_object_mut()
        .expect("heightmap frame serializes as an object");
    object.remove("extra_blend_tile_ndxes");
    object.remove("blended_tiles");
    object.remove("extra_blended_tiles");

    let decoded: PresentationRuntimeHeightmap =
        serde_json::from_value(encoded).expect("old heightmap frame remains readable");
    assert!(decoded.extra_blend_tile_ndxes.is_empty());
    assert!(decoded.blended_tiles.is_empty());
    assert!(decoded.extra_blended_tiles.is_empty());
}

#[test]
fn runtime_heightmap_arc_world_env_json_roundtrip_keeps_full_payload() {
    use std::sync::Arc;

    let payload = PresentationRuntimeHeightmap {
        width: 2,
        height: 2,
        heights: vec![0.0, 0.25, 0.5, 1.0],
        max_height: 100.0,
        scale: 10.0,
        min_height: -5.0,
        height_range: 105.0,
        border_size: 1,
        tile_ndxes: vec![1, 2, 3, 4],
        blend_tile_ndxes: vec![5, 6, 7, 8],
        extra_blend_tile_ndxes: vec![9, 10, 11, 12],
        blended_tiles: vec![PresentationBlendTileInfo {
            blend_ndx: 13,
            horiz: 1,
            vert: 2,
            right_diagonal: 3,
            left_diagonal: 4,
            inverted: 5,
            long_diagonal: 6,
            custom_blend_edge_class: 7,
        }],
        extra_blended_tiles: vec![PresentationBlendTileInfo {
            blend_ndx: 14,
            horiz: 8,
            vert: 9,
            right_diagonal: 10,
            left_diagonal: 11,
            inverted: 12,
            long_diagonal: 13,
            custom_blend_edge_class: 14,
        }],
        draw_origin_x: 3,
        draw_origin_y: 4,
        draw_width: 5,
        draw_height: 6,
    };
    let env = PresentationWorldEnv {
        runtime_heightmap: Some(Arc::new(payload.clone())),
        ..Default::default()
    };

    let decoded: PresentationWorldEnv =
        serde_json::from_value(serde_json::to_value(&env).expect("serialize world env"))
            .expect("deserialize world env");
    assert_eq!(decoded.runtime_heightmap.as_deref(), Some(&payload));
}

#[test]
fn map_load_heightmap_bake_passes_none_game_logic() {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    // Runtime terrain bake is presentation-owned: device/queue only, no GameLogic arg.
    let idx = eng
        .find("load_heightmap_from_runtime_terrain")
        .expect("call site");
    let window = &eng[idx..idx + 280];
    assert!(
        !window.contains("game_logic") && !window.contains("GameLogic"),
        "map-load height bake must not dual-read live GameLogic, got: {window}"
    );
    assert!(
        window.contains("device_arc") && window.contains("queue_arc"),
        "bake should take GPU device/queue only"
    );
}

#[test]
fn terrain_texture_classes_freeze_fields_roundtrip() {
    let c = PresentationTerrainTextureClass {
        first_tile: 1,
        num_tiles: 4,
        width: 64,
        name: "Dirt".into(),
    };
    assert_eq!(c.first_tile, 1);
    assert_eq!(c.name, "Dirt");
    let env = PresentationWorldEnv {
        terrain_texture_classes: vec![c.clone()],
        ..Default::default()
    };
    assert_eq!(env.terrain_texture_classes.len(), 1);
    assert_eq!(env.terrain_texture_classes[0], c);
}

#[test]
fn heightmap_bake_consumes_presentation_texture_classes() {
    let rp = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    // Method-call chain may be line-broken (world_env.\n.terrain_texture_classes).
    assert!(
        rp.contains("terrain_texture_classes"),
        "render pipeline must read presentation terrain_texture_classes"
    );
    let idx = rp
        .find("source_tile_classes")
        .expect("source_tile_classes site");
    let window = &rp[idx..idx + 1600];
    assert!(
        (window.contains("presentation_frame") || window.contains("pres"))
            && window.contains("terrain_texture_classes")
            && window.contains("world_env"),
        "source_tile_classes must come from presentation freeze: {window}"
    );
    // Must not dual-read live GameLogic for tile classes in this window.
    assert!(
        !window.contains("game_logic.terrain_texture") && !window.contains("logic.terrain_texture"),
        "tile classes must not dual-read live GameLogic: {window}"
    );
}
