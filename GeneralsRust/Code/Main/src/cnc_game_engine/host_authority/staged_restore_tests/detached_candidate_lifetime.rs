//! Actual decoded save/map foundation/restore boundary; no fake map or renderer.
use super::*;

fn stage_decoded(
    saves: &mut SaveFileManager,
    decoded: crate::save_load::DecodedGameSave,
    source: &GameLogic,
    slot: &str,
) -> Result<StagedRestoreWorld, RestoreStagingFailure> {
    let crate::save_load::DecodedGameSave {
        campaign_state,
        snapshot,
        save_info,
        saved_game_mode,
        game_client_xfer,
        terrain_visual_xfer,
    } = decoded;
    let catalog = source.templates.clone();
    CnCGameEngine::stage_decoded_saved_world_for_restore(
        &snapshot,
        save_info,
        saved_game_mode,
        game_client_xfer,
        terrain_visual_xfer,
        slot,
        GameMode::Shell,
        RestoreTemplateCatalog {
            templates: &catalog,
            upgrade_definition_base: &source.upgrade_definition_base,
            modules: &source.receiving_module_content,
            player_templates: &source.player_template_definitions,
            multiplayer_definitions: &source.multiplayer_definitions,
            object_definitions: &source.receiving_object_definitions,
            locomotors: &source.locomotor_definition_base,
            baseline: &source.receiving_template_baseline,
            lighting_baseline: source.lighting_rules_baseline(),
            roads_baseline: source.terrain_road_baseline(),
            combat_bonus_baseline: source.combat_bonus_baseline(),
            body_damage_baseline: source.body_damage_baseline(),
            power_bar_baseline: source.power_bar_baseline(),
            body_auto_particle_baseline: source.body_auto_particle_baseline(),
            session_sell_percentage: source.session_sell_percentage,
        },
        &source.weapon_definition_base,
        &source.projectile_definitions,
        &source.ocl_application_definitions(),
        source.ai_definitions.baseline(),
        source.production_energy_baseline(),
        source.physics_rules_baseline(),
        source.effective_terrain_definitions(),
        source.current_armor_definitions(),
        source.session_view.camera.settings,
        source.session_view.camera.clip_policy,
        source.session_view.camera.shake_policy,
        source.session_view.camera.movement_policy,
        source.session_view.camera.input_policy,
        &source.world_services,
        |saved, candidate| {
            saves
                .restore_game_snapshot(saved, candidate)
                .map_err(|error| error.to_string())
        },
    )
    .map(|mut staged| {
        staged.campaign_state = campaign_state;
        staged
    })
}

#[test]
fn actual_main_holds_a_while_staging_b_and_malformed_c_then_drops_b_commits_a_and_resets() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_main_holds_a_while_staging_b_and_malformed_c_then_drops_b_commits_a_and_resets",
        || {
            let map = retail_map_path_for_test();
            let temp = tempfile::tempdir().expect("temporary real save files");
            let mut saves = SaveFileManager::with_save_directory(temp.path());
            saves.init().unwrap();
            let mut live = GameLogic::new();
            live.admit_sell_percentage(0.37);
            admit_skirmish_fixture(&mut live, &map, "CandidateLifetimeHuman", false);
            assert!(
                live.load_map(&map),
                "actual licensed map decoder and factory foundation"
            );
            live.frame = 41;
            saves
                .save_game("held_a", &live, &save_info("held_a", map.clone()))
                .unwrap();
            live.frame = 42;
            saves
                .save_game("held_b", &live, &save_info("held_b", map))
                .unwrap();
            let decoded_a = saves
                .load_game_snapshot("held_a")
                .expect("actual archive decoder A");
            let decoded_b = saves
                .load_game_snapshot("held_b")
                .expect("actual archive decoder B");
            let mut malformed_c = saves
                .load_game_snapshot("held_a")
                .expect("actual archive decoder C");
            // Exercise the real snapshot version admission after the real map load,
            // rather than injecting a closure which pretends restoration failed.
            malformed_c.snapshot.version = u32::MAX;
            live.frame = 99;
            let before = crate::authoritative_world::AuthorityProbe::capture(&live, 0);
            let live_services = Arc::clone(&live.world_services);
            let a = stage_decoded(&mut saves, decoded_a, &live, "held_a")
                .unwrap_or_else(|failure| panic!("stage A: {}", failure.reason));
            assert_eq!(a.logic.frame, 41);
            assert_eq!(a.logic.session_sell_percentage, 0.37);
            let expected_a = crate::authoritative_world::AuthorityProbe::capture(&a.logic, 0);
            assert_eq!(
                crate::authoritative_world::AuthorityProbe::capture(&live, 0),
                before
            );
            assert!(Arc::ptr_eq(&live.world_services, &live_services));
            assert!(!gamelogic::runtime_world_transaction::world_runtime_staging_active());

            // A's actual Main decoded candidate remains held during this whole call.
            let b = stage_decoded(&mut saves, decoded_b, &live, "held_b")
                .unwrap_or_else(|failure| panic!("stage B while A held: {}", failure.reason));
            assert_eq!(b.logic.frame, 42);
            assert_eq!(b.logic.session_sell_percentage, 0.37);
            assert_eq!(a.logic.frame, 41);
            assert_eq!(
                crate::authoritative_world::AuthorityProbe::capture(&live, 0),
                before
            );
            let failure = match stage_decoded(&mut saves, malformed_c, &live, "malformed_c") {
                Ok(_) => panic!("unsupported actual snapshot version must reject"),
                Err(failure) => failure,
            };
            assert!(
                failure.retained_logic.is_none(),
                "real failed-candidate cleanup must complete"
            );
            assert!(failure.reason.contains("version"), "{}", failure.reason);
            assert_eq!(
                crate::authoritative_world::AuthorityProbe::capture(&live, 0),
                before,
                "malformed C rejects before live mutation while A and B remain held"
            );
            assert!(!gamelogic::runtime_world_transaction::world_runtime_staging_active());

            let StagedRestoreWorld {
                logic: b_logic,
                runtime_world: b_runtime,
                ..
            } = b;
            drop(b_runtime);
            let discarded = CnCGameEngine::reject_logical_restore_candidate(
                &mut saves,
                b_logic,
                "discard B while A remains held".into(),
            );
            assert_eq!(discarded, "discard B while A remains held");
            assert_eq!(
                crate::authoritative_world::AuthorityProbe::capture(&live, 0),
                before
            );
            assert_eq!(a.logic.frame, 41);

            // Same logical combined-commit sequence as restore_decoded_logical_save.
            let StagedRestoreWorld {
                mut logic,
                runtime_world,
                owns_session_camera,
                ..
            } = a;
            logic.finish_saved_snapshot_admission().unwrap();
            let checked = runtime_world.preflight_native().unwrap();
            live.destroy_all_objects_immediate_with_effects_checked(&mut Default::default())
                .unwrap();
            let committed = checked.install_globals();
            admit_staged_session_view(
                &mut live.session_view,
                &mut logic.session_view,
                owns_session_camera,
            );
            let previous = std::mem::replace(&mut live, logic);
            drop(previous);
            committed.finish_after_logic_commit();
            assert_eq!(
                crate::authoritative_world::AuthorityProbe::capture(&live, 0),
                expected_a
            );
            assert!(!Arc::ptr_eq(&live.world_services, &live_services));
            assert!(!gamelogic::runtime_world_transaction::world_runtime_staging_active());
            live.destroy_all_objects_immediate_with_effects_checked(&mut Default::default())
                .unwrap();
            live.reset();
            assert_eq!(live.host_objects().registered_len(), 0);
            assert_eq!(live.frame, 0);
            assert_eq!(live.session_sell_percentage, 0.37);
            assert!(live.get_players().is_empty());
        },
    );
}
