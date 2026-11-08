use super::*;

#[test]
fn receiving_scorch_lighting_keeps_primary_terrain_diffuse_when_not_directionally_active() {
    let mut env = crate::presentation_frame::PresentationWorldEnv::default();
    env.terrain_global_lights[0] =
        Some(crate::presentation_frame::PresentationPrimaryGlobalLight {
            ambient: [0.1, 0.2, 0.3],
            diffuse: [0.2, 0.4, 0.6],
            light_pos: [0.0; 3],
            object_light_active: false,
        });

    let lighting = RenderPipeline::static_terrain_lighting(&env).unwrap();
    assert!(lighting.lights.is_empty());
    assert_eq!(lighting.ambient, [0.1, 0.2, 0.3]);
    assert_eq!(lighting.scorch_diffuse, [0.2, 0.4, 0.6]);

    // A later frozen time-of-day frame is re-admitted through this same owner path.
    env.terrain_global_lights[0].as_mut().unwrap().diffuse = [0.8, 0.6, 0.4];
    let changed = RenderPipeline::static_terrain_lighting(&env).unwrap();
    assert_eq!(changed.scorch_diffuse, [0.8, 0.6, 0.4]);
}

#[test]
fn detached_native_cpu_map_admission_uses_receiving_bytes_and_authored_scorches_once() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "detached_native_cpu_map_admission_uses_receiving_bytes_and_authored_scorches_once",
        || {
            let source = crate::game_logic::GameLogic::new();
            let mut map = gamelogic::system::map_loader::MapData::new();
            map.width = 8;
            map.height = 8;
            map.heightmap = vec![40; 64];
            source
                .world_services
                .terrain()
                .write()
                .unwrap()
                .try_load_map_geometry(map)
                .unwrap();
            let mut env = crate::presentation_frame::PresentationWorldEnv::from_logic(&source);
            env.authored_scorches
                .push(crate::presentation_frame::PresentationTerrainScorch {
                    location: [1., 2., 3.],
                    radius: 5.,
                    kind: 0,
                });
            let mut receiving = RenderPipeline::prepare_native_terrain_cpu(&env)
                .unwrap()
                .unwrap();
            assert_eq!(receiving.get_raw_map_height(1, 1), 40);
            assert_eq!(receiving.scorch_marks().len(), 1);
            receiving.apply_accepted_height(1, 1, 255);
            let peer = RenderPipeline::prepare_native_terrain_cpu(&env)
                .unwrap()
                .unwrap();
            assert_eq!(peer.get_raw_map_height(1, 1), 40);
            assert_eq!(peer.scorch_marks().len(), 1);
            assert_eq!(receiving.get_raw_map_height(1, 1), 255);
            assert_eq!(receiving.scorch_marks().len(), 1);
        },
    );
}

#[test]
fn native_admission_uses_each_worlds_three_way_setting() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_admission_uses_each_worlds_three_way_setting",
        || {
            use game_client::terrain::height_map::HeightMap;
            use game_client::terrain::textures::BlendTileInfo;
            let mut map = HeightMap::new(3, 3, 1.0, 10.0);
            map.tile_ndxes[0] = 1;
            map.blend_tile_ndxes[0] = 1;
            map.extra_blend_tile_ndxes[0] = 2;
            let mut blend = BlendTileInfo::new();
            blend.blend_ndx = 2;
            blend.horiz = 1;
            let mut third = BlendTileInfo::new();
            third.blend_ndx = 3;
            third.vert = 1;
            map.blended_tiles = vec![BlendTileInfo::new(), blend, third];
            let frozen = crate::presentation_frame::PresentationRuntimeHeightmap::from_height_map(&map);
            let mut off = crate::game_logic::GameLogic::new();
            let mut on = crate::game_logic::GameLogic::new();
            let mut off_rules = crate::game_logic::lighting_rules::LightingContentRules::default();
            off_rules.use_3way_terrain_blends = 0;
            off.admit_lighting_rules_base(off_rules);
            let mut on_rules = crate::game_logic::lighting_rules::LightingContentRules::default();
            on_rules.use_3way_terrain_blends = 1;
            on.admit_lighting_rules_base(on_rules);
            let mut off_env = crate::presentation_frame::PresentationWorldEnv::from_logic(&off);
            let mut on_env = crate::presentation_frame::PresentationWorldEnv::from_logic(&on);
            off_env.runtime_heightmap = Some(Arc::new(frozen.clone()));
            on_env.runtime_heightmap = Some(Arc::new(frozen));
            let disabled = RenderPipeline::prepare_native_terrain_cpu(&off_env).unwrap().unwrap();
            let enabled = RenderPipeline::prepare_native_terrain_cpu(&on_env).unwrap().unwrap();
            assert!(disabled.build_extra_blend_draw_mesh().is_empty());
            assert_eq!(enabled.build_extra_blend_draw_mesh().index_count(), 6);
            assert!(!disabled.extra_blend_draw());
            assert!(enabled.extra_blend_draw());
        },
    );
}
