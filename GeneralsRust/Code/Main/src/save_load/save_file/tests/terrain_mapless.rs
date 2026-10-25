//! Existing mapless logical archives retain their genuine checked raw grid.
use super::*;

#[test]
fn mapless_public_roundtrip_keeps_nonempty_raw_and_coarse_grid_in_fresh_and_foreign_receivers() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mapless_public_roundtrip_keeps_nonempty_raw_and_coarse_grid_in_fresh_and_foreign_receivers",
        || {
            let directory = tempfile::tempdir().unwrap();
            let mut manager = SaveFileManager::with_save_directory(directory.path());
            manager.init().unwrap();
            let mut source = world(40, false);
            let (width, height, _) = source.snapshot_pathfinding_passability();
            let heights = vec![17.; (width as usize) * (height as usize)];
            assert!(source.restore_terrain_heights_from_grid(width, height, &heights));
            let expected_samples = source.snapshot_terrain_heights_for_path_grid().unwrap();
            let info = fixture_save_info();
            assert!(crate::game_logic::script_loader::find_map_file(&info.map_name).is_none());
            manager.save_game("logical_raw", &source, &info).unwrap();
            let mut fresh = GameLogic::new();
            manager.load_game("logical_raw", &mut fresh).unwrap();
            assert_eq!(
                fresh
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[40; 4]
            );
            assert_eq!(
                fresh.snapshot_terrain_heights_for_path_grid().unwrap(),
                expected_samples
            );
            let mut foreign = world(90, false);
            let mut unrelated =
                gamelogic::team::TeamPrototype::new("ForeignUnrelatedDefinition".into());
            unrelated.set_id(99);
            foreign
                .team_factory
                .lock()
                .unwrap()
                .replace_team_prototype(unrelated);
            let (_, prepared) = manager
                .load_game_with_terrain_visual("logical_raw", &mut foreign)
                .unwrap();
            assert_eq!(prepared.unwrap().heights(), &[40; 4]);
            assert_eq!(
                foreign
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[40; 4]
            );
            assert_eq!(
                foreign.snapshot_terrain_heights_for_path_grid().unwrap(),
                expected_samples
            );
            assert!(
                foreign
                    .team_factory
                    .lock()
                    .unwrap()
                    .list_team_prototypes()
                    .is_empty(),
                "saved complete prototype manifest must not import a foreign definition"
            );
            assert_eq!(
                source
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes(),
                &[40; 4]
            );
        },
    );
}

fn actual_definition(flag: bool) -> gamelogic::team::TeamPrototype {
    let mut definition = gamelogic::team::TeamPrototype::new("ExistingLogicalDefinition".into());
    definition.set_id(37);
    definition.set_production_condition_always_false(flag);
    definition
}

#[test]
fn mapless_bad_tail_and_late_snapshot_failure_preserve_live_grid_factory_and_cache_owner() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mapless_bad_tail_and_late_snapshot_failure_preserve_live_grid_factory_and_cache_owner",
        || {
            let directory = tempfile::tempdir().unwrap();
            let mut manager = SaveFileManager::with_save_directory(directory.path());
            manager.init().unwrap();
            let mut source = world(40, false);
            source
                .team_factory
                .lock()
                .unwrap()
                .replace_team_prototype(actual_definition(false));
            let snapshot = SnapshotBuilder::new()
                .create_world_snapshot(&source)
                .unwrap();
            let bytes = capture_receiving_terrain_visual_xfer_bytes(&source, None).unwrap();
            let info = fixture_save_info();
            let mut running = world(90, false);
            running
                .team_factory
                .lock()
                .unwrap()
                .replace_team_prototype(actual_definition(true));
            running.set_current_frame(37);
            let retained = running
                .team_factory
                .lock()
                .unwrap()
                .find_team_prototype("ExistingLogicalDefinition")
                .unwrap();
            let revision = running
                .world_services
                .terrain()
                .read()
                .unwrap()
                .height_revision();
            for (slot, candidate, payload, expected) in [
                (
                    "bad_mapless_tail",
                    copy_snapshot(&snapshot),
                    [bytes.as_slice(), &[0]].concat(),
                    "scorch count",
                ),
                (
                    "bad_mapless_late",
                    {
                        let mut candidate = copy_snapshot(&snapshot);
                        candidate.persist_v18.radar_next_event = -2;
                        candidate
                    },
                    bytes.clone(),
                    "radar ring",
                ),
            ] {
                let archive =
                    SaveFileManager::write_common_sav_chunks_with_client_and_terrain_bytes(
                        &candidate,
                        &info,
                        &[1],
                        Some(&payload),
                        directory.path(),
                        CPP_GAME_NONE,
                    )
                    .unwrap();
                std::fs::write(manager.get_save_path(slot), archive).unwrap();
                let error = manager
                    .load_game(slot, &mut running)
                    .unwrap_err()
                    .to_string();
                assert!(error.contains(expected), "{slot}: {error}");
                assert_eq!(running.get_current_frame(), 37);
                let terrain = running.world_services.terrain().read().unwrap();
                assert_eq!(terrain.logic_height_map_bytes(), &[90; 4]);
                assert_eq!(terrain.height_revision(), revision);
                drop(terrain);
                let after = running
                    .team_factory
                    .lock()
                    .unwrap()
                    .find_team_prototype("ExistingLogicalDefinition")
                    .unwrap();
                assert!(
                    std::sync::Arc::ptr_eq(&retained, &after),
                    "failed load replaced live definition"
                );
                assert_eq!(after.get_id(), 37);
                assert!(
                    after.production_condition_always_false(),
                    "failed load changed running scalar/cache"
                );
            }
        },
    );
}

fn copy_snapshot(snapshot: &WorldSnapshot) -> WorldSnapshot {
    bincode_legacy::deserialize(&bincode_legacy::serialize(snapshot).unwrap()).unwrap()
}

#[test]
fn mapless_coarse_height_public_roundtrip_needs_no_raw_or_authored_map() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mapless_coarse_height_public_roundtrip_needs_no_raw_or_authored_map",
        || {
            let directory = tempfile::tempdir().unwrap();
            let mut manager = SaveFileManager::with_save_directory(directory.path());
            manager.init().unwrap();
            let mut source = GameLogic::new();
            let (width, height, _) = source.snapshot_pathfinding_passability();
            let samples = vec![17.; (width as usize) * (height as usize)];
            assert!(source.restore_terrain_heights_from_grid(width, height, &samples));
            assert!(
                source
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes()
                    .is_empty()
            );
            let expected = source.snapshot_terrain_heights_for_path_grid().unwrap();
            manager
                .save_game("logical_coarse", &source, &fixture_save_info())
                .unwrap();
            let mut fresh = GameLogic::new();
            manager.load_game("logical_coarse", &mut fresh).unwrap();
            assert_eq!(
                fresh.snapshot_terrain_heights_for_path_grid().unwrap(),
                expected
            );
            assert!(
                fresh
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes()
                    .is_empty()
            );
        },
    );
}

#[test]
fn mapless_restore_keeps_receiving_upgrade_catalog_masks_without_mutable_center_alias() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "mapless_restore_keeps_receiving_upgrade_catalog_masks_without_mutable_center_alias",
        || {
            const NAME: &str = "Upgrade_MaplessReceivingDefinition";
            let directory = tempfile::tempdir().unwrap();
            let mut manager = SaveFileManager::with_save_directory(directory.path());
            manager.init().unwrap();
            let mut source = world(40, false);
            crate::game_logic::host_upgrade_rules::register_test_upgrade(
                &mut source,
                NAME,
                "PLAYER",
                321,
                7,
            );
            manager
                .save_game("catalog", &source, &fixture_save_info())
                .unwrap();
            let mut receiver = world(90, false);
            crate::game_logic::host_upgrade_rules::register_test_upgrade(
                &mut receiver,
                "Upgrade_ReceivingFirst",
                "PLAYER",
                1,
                1,
            );
            crate::game_logic::host_upgrade_rules::register_test_upgrade(
                &mut receiver,
                NAME,
                "OBJECT",
                654,
                19,
            );
            let retained = receiver.upgrade_template(NAME).unwrap();
            let old_center = receiver.upgrade_definitions.clone();
            let mut peer = world(80, false);
            crate::game_logic::host_upgrade_rules::register_test_upgrade(
                &mut peer, NAME, "PLAYER", 999, 23,
            );
            let peer_center = peer.upgrade_definitions.clone();
            manager.load_game("catalog", &mut receiver).unwrap();
            let admitted = receiver.upgrade_template(NAME).unwrap();
            assert_eq!(admitted.get_cost(), 654);
            assert_eq!(admitted.get_build_time(), retained.get_build_time());
            assert_eq!(admitted.get_mask(), retained.get_mask());
            assert!(
                std::sync::Arc::ptr_eq(&admitted, &retained),
                "only immutable templates may be shared"
            );
            assert_eq!(
                old_center.find_upgrade(NAME).unwrap().get_mask(),
                retained.get_mask()
            );
            crate::game_logic::host_upgrade_rules::register_test_upgrade(
                &mut receiver,
                "Upgrade_AfterRestore",
                "PLAYER",
                3,
                1,
            );
            assert!(old_center.find_upgrade("Upgrade_AfterRestore").is_none());
            assert!(peer_center.find_upgrade("Upgrade_AfterRestore").is_none());
            assert_eq!(peer_center.find_upgrade(NAME).unwrap().get_cost(), 999);
            assert_eq!(source.upgrade_template(NAME).unwrap().get_cost(), 321);
        },
    );
}
