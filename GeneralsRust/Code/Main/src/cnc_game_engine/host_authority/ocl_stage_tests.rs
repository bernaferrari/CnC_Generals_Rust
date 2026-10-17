//! Actual map-loading candidate uses the receiving application OCL base.
use super::*;
use crate::save_load::{GameDifficulty, SaveFileType};
fn list(model: &str) -> String {
    format!(
        "ObjectCreationList SameOCL\n CreateDebris\n ModelNames={model}\n Count=1\n End\n End\n"
    )
}
fn selected(world: &GameLogic) -> String {
    let list = world.object_creation_list_definition("SameOCL").unwrap();
    list.nuggets()[0]
        .as_any()
        .downcast_ref::<gamelogic::object_creation_list::nuggets::GenericObjectCreationNugget>()
        .unwrap()
        .names[0]
        .clone()
}
#[test]
fn actual_saved_map_ocl_candidate_uses_own_base_and_solo_without_mutating_active_or_peer() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_saved_map_ocl_candidate_uses_own_base_and_solo_without_mutating_active_or_peer",
        || {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("SavedOCL.map");
            let mut map = game_engine::common::system::DataChunkOutput::new();
            map.open_data_chunk("GlobalLighting", 1);
            map.write_int(2);
            for _ in 0..4 {
                for _ in 0..2 {
                    for value in [0.1, 0.2, 0.3, 0.8, 0.8, 0.8, 0., 0., -1.] {
                        map.write_real(value);
                    }
                }
            }
            map.close_data_chunk();
            std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
            std::fs::write(directory.path().join("Map.ini"), list("SavedMap")).unwrap();
            std::fs::write(directory.path().join("Solo.ini"), list("SavedSolo")).unwrap();
            let mut source = GameLogic::new();
            source
                .admit_ocl_application_sources(&[list("Application")])
                .unwrap();
            source.start_new_game(GameMode::Skirmish);
            assert!(source.load_map(path.to_str().unwrap()));
            assert_eq!(selected(&source), "SavedSolo");
            let builder = crate::save_load::snapshot::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let mut active = GameLogic::new();
            active
                .admit_ocl_application_sources(&[list("ActiveApplication")])
                .unwrap();
            active
                .apply_receiving_ocl_overrides(&[list("ActiveMap")])
                .unwrap();
            let mut peer = GameLogic::new();
            peer.admit_ocl_application_sources(&[list("Peer")]).unwrap();
            let active_base = active.ocl_application_definitions();
            let peer_base = peer.ocl_application_definitions();
            let info = || SaveGameInfo {
                pristine_map_name: None,
                filename: "ocl".into(),
                display_name: "ocl".into(),
                description: "owned definitions".into(),
                map_name: path.to_str().unwrap().into(),
                campaign_side: None,
                mission_number: None,
                save_date: std::time::SystemTime::now(),
                game_version: env!("CARGO_PKG_VERSION").into(),
                play_time: std::time::Duration::ZERO,
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Normal,
            };
            let failed = CnCGameEngine::stage_decoded_saved_world_for_restore(
                &snapshot,
                info(),
                None,
                "ocl",
                GameMode::Skirmish,
                &source.templates,
                &source.weapon_definition_base,
                &source.projectile_definitions,
                &active.ocl_application_definitions(),
                source.ai_definitions.baseline(),
                source.production_energy_baseline(),
                source.physics_rules_baseline(),
                active.session_view.camera.settings,
                active.session_view.camera.clip_policy,
                active.session_view.camera.shake_policy,
                &active.world_services,
                |_, candidate| {
                    assert_eq!(selected(candidate), "SavedSolo");
                    Err("after actual map admission".into())
                },
            );
            assert!(failed.err().unwrap().contains("after actual map admission"));
            assert_eq!(selected(&active), "ActiveMap");
            assert_eq!(selected(&peer), "Peer");
            assert!(Arc::ptr_eq(
                &active_base,
                &active.ocl_application_definitions()
            ));
            assert!(Arc::ptr_eq(&peer_base, &peer.ocl_application_definitions()));
            let mut staged = CnCGameEngine::stage_decoded_saved_world_for_restore(
                &snapshot,
                info(),
                None,
                "ocl",
                GameMode::Skirmish,
                &source.templates,
                &source.weapon_definition_base,
                &source.projectile_definitions,
                &active.ocl_application_definitions(),
                source.ai_definitions.baseline(),
                source.production_energy_baseline(),
                source.physics_rules_baseline(),
                active.session_view.camera.settings,
                active.session_view.camera.clip_policy,
                active.session_view.camera.shake_policy,
                &active.world_services,
                |snapshot, candidate| {
                    assert_eq!(selected(candidate), "SavedSolo");
                    builder
                        .restore_from_snapshot(snapshot, candidate)
                        .map_err(|error| error.to_string())
                },
            )
            .unwrap();
            assert_eq!(selected(&staged.logic), "SavedSolo");
            assert!(Arc::ptr_eq(
                &active.ocl_application_definitions(),
                &staged.logic.ocl_application_definitions()
            ));
            staged.logic.reset_ocl_map_definitions();
            assert_eq!(selected(&staged.logic), "ActiveApplication");
            assert_eq!(selected(&active), "ActiveMap");
            assert_eq!(selected(&peer), "Peer");
        },
    );
}
