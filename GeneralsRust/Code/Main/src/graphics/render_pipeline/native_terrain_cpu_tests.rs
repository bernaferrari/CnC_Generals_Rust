use super::*;
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
