//! Real archive pairing with a foreign ambient terrain adversary.
use super::*;
fn world(raw: u8, enabled: bool) -> GameLogic {
    let owner = GameLogic::new();
    {
        let mut terrain = owner.world_services.terrain().write().unwrap();
        terrain
            .try_load_map_geometry({
                let mut map = gamelogic::system::map_loader::MapData::new();
                map.width = 2;
                map.height = 2;
                map.heightmap = vec![raw; 4];
                map
            })
            .unwrap();
        terrain.water_grid_state_mut().enabled = enabled;
    }
    owner
}
#[test]
fn terrain_archives_use_receiving_capture_and_survive_interleaved_decode_and_direct_restore() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "terrain_archives_use_receiving_capture_and_survive_interleaved_decode_and_direct_restore",
        || {
            let directory = tempfile::tempdir().unwrap();
            let mut manager = SaveFileManager::with_save_directory(directory.path());
            manager.init().unwrap();
            let a = world(40, true);
            let b = world(90, false);
            {
                let handle = gamelogic::terrain::get_terrain_logic();
                let mut foreign = handle.write().unwrap();
                foreign.reset();
                foreign
                    .try_restore_logic_height_map_with_effects(2, 2, &[200; 4], &mut || {})
                    .unwrap();
                foreign.water_grid_state_mut().enabled = false;
            }
            manager
                .save_game("terrain_a", &a, &fixture_save_info())
                .unwrap();
            manager
                .save_game("terrain_b", &b, &fixture_save_info())
                .unwrap();
            let decoded_a = manager.load_game_snapshot("terrain_a").unwrap();
            let decoded_b = manager.load_game_snapshot("terrain_b").unwrap();
            let prepared_a =
                prepare_receiving_terrain_visual(&a, decoded_a.terrain_visual_xfer.as_deref())
                    .unwrap()
                    .unwrap();
            let prepared_b =
                prepare_receiving_terrain_visual(&b, decoded_b.terrain_visual_xfer.as_deref())
                    .unwrap()
                    .unwrap();
            assert_eq!(prepared_a.heights(), &[40; 4]);
            assert_eq!(prepared_b.heights(), &[90; 4]);
            assert!(prepared_a.grid().enabled);
            assert!(!prepared_b.grid().enabled);
            assert_eq!(prepared_a.scorches(), None);
            assert_eq!(prepared_b.scorches(), None);
            let mut receiving_a = world(1, true);
            let mut receiving_b = world(2, false);
            // Snapshot-only APIs have explicit absence; decode B cannot donate its
            // TerrainVisual to the subsequent restore of A.
            manager
                .restore_game_snapshot(&decoded_a.snapshot, &mut receiving_a)
                .unwrap();
            assert_eq!(
                receiving_a
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[40; 4]
            );
            manager
                .restore_game_snapshot(&decoded_b.snapshot, &mut receiving_b)
                .unwrap();
            assert_eq!(
                receiving_b
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[90; 4]
            );
            admit_receiving_terrain_visual(&mut receiving_b, &prepared_b).unwrap();
            admit_receiving_terrain_visual(&mut receiving_a, &prepared_a).unwrap();
            assert_eq!(
                receiving_a
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[40; 4]
            );
            assert!(
                receiving_a
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .water_grid_state()
                    .enabled
            );
            assert_eq!(
                receiving_b
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[90; 4]
            );
            assert!(
                !receiving_b
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .water_grid_state()
                    .enabled
            );
            assert_eq!(
                gamelogic::terrain::get_terrain_logic()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[200; 4]
            );
        },
    );
}

#[test]
fn prepared_terrain_candidate_changes_no_native_scorches_until_explicit_commit() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "prepared_terrain_candidate_changes_no_native_scorches_until_explicit_commit",
        || {
            use game_client::terrain::scorch_mesh::{
                ScorchMark, add_terrain_scorch, clear_terrain_scorches, terrain_scorch_marks,
            };
            clear_terrain_scorches();
            assert!(add_terrain_scorch([700., 800., 9.], 18., 1));
            let foreign_native = terrain_scorch_marks();
            let source = world(40, true);
            let receiving = world(90, true);
            let mark = ScorchMark {
                location: [10., 20., 3.],
                radius: 5.,
                scorch_type: 2,
            };
            let bytes =
                capture_receiving_terrain_visual_xfer_bytes(&source, Some(&[mark])).unwrap();
            assert!(
                prepare_receiving_terrain_visual(&receiving, Some(&bytes[..bytes.len() - 1]))
                    .is_err()
            );
            assert_eq!(terrain_scorch_marks(), foreign_native);
            assert_eq!(
                receiving
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[90; 4]
            );
            let prepared = prepare_receiving_terrain_visual(&receiving, Some(&bytes))
                .unwrap()
                .unwrap();
            assert_eq!(
                terrain_scorch_marks(),
                foreign_native,
                "successful candidate decode is also effect-free"
            );
            prepared.commit_legacy_native();
            assert_eq!(terrain_scorch_marks(), [mark]);
            let no_native = capture_receiving_terrain_visual_xfer_bytes(&source, None).unwrap();
            let prepared = prepare_receiving_terrain_visual(&receiving, Some(&no_native))
                .unwrap()
                .unwrap();
            prepared.commit_legacy_native();
            assert_eq!(
                terrain_scorch_marks(),
                [mark],
                "omitted native tail cannot invent an empty clear"
            );
        },
    );
}

fn file_backed_world(path: &std::path::Path, raw: u8) -> GameLogic {
    let mut map = game_engine::common::system::DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [4, 4, 0, 16] {
        map.write_int(value);
    }
    for _ in 0..16 {
        map.write_byte(raw);
    }
    map.close_data_chunk();
    map.open_data_chunk("SidesList", 2);
    map.write_int(0);
    map.write_int(0);
    map.close_data_chunk();
    std::fs::write(path, map.into_ckmp_bytes()).unwrap();
    let mut owner = GameLogic::new();
    owner.start_new_game_for_restore(crate::game_logic::GameMode::SinglePlayer);
    assert!(owner.load_map(path.to_str().unwrap()));
    owner
}

#[test]
fn direct_archive_load_stages_actual_saved_map_for_fresh_and_foreign_receivers() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "direct_archive_load_stages_actual_saved_map_for_fresh_and_foreign_receivers",
        || {
            let directory = tempfile::tempdir().unwrap();
            let mut manager = SaveFileManager::with_save_directory(directory.path());
            manager.init().unwrap();
            let source = file_backed_world(&directory.path().join("receiving_a.map"), 40);
            let mut info = fixture_save_info();
            info.map_name = source.get_current_map_name().to_owned();
            manager.save_game("actual_terrain", &source, &info).unwrap();
            let mut fresh = GameLogic::new();
            let (_, terrain) = manager
                .load_game_with_terrain_visual("actual_terrain", &mut fresh)
                .unwrap();
            assert_eq!(terrain.unwrap().heights(), &[40; 16]);
            assert_eq!(
                fresh
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[40; 16]
            );
            let mut foreign = file_backed_world(&directory.path().join("receiving_b.map"), 90);
            foreign
                .world_services
                .terrain()
                .write()
                .unwrap()
                .water_grid_state_mut()
                .enabled = true;
            manager.load_game("actual_terrain", &mut foreign).unwrap();
            assert_eq!(
                foreign
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[40; 16]
            );
            assert!(
                !foreign
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .water_grid_state()
                    .enabled
            );
        },
    );
}

#[test]
fn direct_archive_bad_terrain_tail_preserves_running_foreign_world() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "direct_archive_bad_terrain_tail_preserves_running_foreign_world",
        || {
            let directory = tempfile::tempdir().unwrap();
            let mut manager = SaveFileManager::with_save_directory(directory.path());
            manager.init().unwrap();
            let source = file_backed_world(&directory.path().join("source.map"), 40);
            let mut info = fixture_save_info();
            info.map_name = source.get_current_map_name().to_owned();
            let snapshot = SnapshotBuilder::new()
                .create_world_snapshot(&source)
                .unwrap();
            let mut bytes = capture_receiving_terrain_visual_xfer_bytes(&source, None).unwrap();
            bytes.push(0); // incomplete optional native count
            let archive = SaveFileManager::write_common_sav_chunks_with_client_and_terrain_bytes(
                &snapshot,
                &info,
                &[1],
                Some(&bytes),
                directory.path(),
                CPP_GAME_SINGLE_PLAYER,
            )
            .unwrap();
            std::fs::write(manager.get_save_path("invalid_tail"), archive).unwrap();
            let mut running = file_backed_world(&directory.path().join("foreign.map"), 90);
            running
                .world_services
                .terrain()
                .write()
                .unwrap()
                .water_grid_state_mut()
                .enabled = true;
            let map = running.get_current_map_name().to_owned();
            let revision = running
                .world_services
                .terrain()
                .read()
                .unwrap()
                .height_revision();
            game_client::terrain::terrain_visual::init_terrain_visual().unwrap();
            {
                let mut visual =
                    game_client::terrain::terrain_visual::get_terrain_visual().unwrap();
                visual.as_mut().unwrap().enable_water_grid(true);
            }
            let native = game_client::terrain::scorch_mesh::terrain_scorch_marks();
            assert!(
                manager
                    .load_game("invalid_tail", &mut running)
                    .unwrap_err()
                    .to_string()
                    .contains("scorch count")
            );
            assert_eq!(running.get_current_map_name(), map);
            let terrain = running.world_services.terrain().read().unwrap();
            assert_eq!(terrain.logic_height_map_bytes(), &[90; 16]);
            assert_eq!(terrain.height_revision(), revision);
            assert!(terrain.water_grid_state().enabled);
            assert_eq!(
                game_client::terrain::scorch_mesh::terrain_scorch_marks(),
                native
            );
            assert!(
                game_client::terrain::terrain_visual::get_terrain_visual()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .water_grid_enabled(),
                "failed candidate map configuration cannot disable the running native water"
            );
        },
    );
}

#[path = "terrain_mapless.rs"]
mod mapless;
