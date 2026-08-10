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
        }
    }

    #[test]
    fn map_load_heightmap_bake_passes_none_game_logic() {
        let eng = include_str!("../../cnc_game_engine.rs");
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
        let rp = include_str!("../../graphics/render_pipeline.rs");
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
            !window.contains("game_logic.terrain_texture")
                && !window.contains("logic.terrain_texture"),
            "tile classes must not dual-read live GameLogic: {window}"
        );
    }
