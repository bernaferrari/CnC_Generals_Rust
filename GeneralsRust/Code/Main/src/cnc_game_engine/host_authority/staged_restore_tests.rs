use super::*;
use crate::ai::AIDifficulty;
use crate::game_logic::{GameLogic, GameMode, Player, Team};
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType};
use std::time::SystemTime;

/// Keeps this test's thread-local presentation queues independent from
/// whatever a neighboring test had recorded before it began.
struct WorldStageLogRestore {
    state: Option<crate::game_logic::staged_world_effects::WorldStageEffectsState>,
}

impl WorldStageLogRestore {
    fn take() -> Self {
        Self {
            state: Some(
                crate::game_logic::staged_world_effects::WorldStageEffectsState::take_all_for_test(
                ),
            ),
        }
    }
}

impl Drop for WorldStageLogRestore {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            drop(
                crate::game_logic::staged_world_effects::WorldStageEffectsState::replace_all_for_test(state),
            );
        }
    }
}

/// Seed the full load-map/snapshot-restore queue closure.  The staged
/// candidate uses a distinct object/label, so equality after its forced
/// failure proves both pending queues and presentation last-drain state
/// stayed attached to the live world.
fn record_all_stage_queue_events(object: crate::game_logic::ObjectId, label: &str) {
    crate::game_logic::host_spawn_log::record(object, label.to_string(), 7, [1.0, 2.0, 3.0]);
}

fn retail_map_path_for_test() -> String {
    crate::game_logic::script_loader::find_map_file("Lone Eagle")
        .expect("licensed Lone Eagle map required for staged restore acceptance");
    "Lone Eagle".to_string()
}

// GameStateMap restores resolved initial participants before loading map sides.
// Admit a real native configuration once instead of guessing a source roster
// after map creation (which fabricates unmatched default team prototypes).
fn admit_skirmish_fixture(source: &mut GameLogic, map_name: &str, human: &str, china_ai: bool) {
    let mut config = crate::skirmish_config::golden_skirmish_config(map_name);
    config.slots[0].player_name = human.into();
    if china_ai {
        config.slots[1].player_name = "Computer".into();
        config.slots[1].faction = "China".into();
        config.slots[1].player_template =
            crate::skirmish_config::SkirmishPlayerTemplateSelection::exact_template_name(
                "FactionChina",
            );
        config.slots[1].ai_difficulty = Some("Hard".into());
    } else {
        config.slots.truncate(1);
    }
    crate::skirmish_config::apply_skirmish_config(source, &config)
        .expect("admit exact initial native Skirmish participants");
}

fn save_info(slot: &str, map_name: String) -> SaveGameInfo {
    SaveGameInfo {
        pristine_map_name: None,
        filename: slot.to_string(),
        display_name: slot.to_string(),
        description: "staged restore test".to_string(),
        map_name,
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: std::time::Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}

#[test]
fn staged_load_failure_keeps_only_an_active_match_state() {
    assert_eq!(
        CnCGameEngine::staged_load_failure_return_state(GameState::InGame),
        GameState::InGame
    );
    assert_eq!(
        CnCGameEngine::staged_load_failure_return_state(GameState::Paused),
        GameState::Paused
    );
    for shell_state in [
        GameState::Initializing,
        GameState::Menu,
        GameState::Loading,
        GameState::Victory,
        GameState::Defeat,
        GameState::Exiting,
    ] {
        assert_eq!(
            CnCGameEngine::staged_load_failure_return_state(shell_state),
            GameState::Menu
        );
    }
}

#[test]
fn staged_load_error_branch_preserves_live_world_contract() {
    assert!(!HostLoadFailure::Staging("bad map".into()).requires_match_reset());
    assert!(HostLoadFailure::Committed("bad drawable".into()).requires_match_reset());
    let source = include_str!("../host_authority.rs");
    let start = source
        .find("pub(super) fn host_load_game_from_ui")
        .expect("load UI authority");
    let body = &source[start..];
    let error = &body[body.find("Err(failure) =>").expect("load error branch")..];
    let error = &error[..error
        .find("\n            }\n        }\n    }\n}")
        .unwrap_or(error.len())];
    assert!(error.contains("failure.requires_match_reset()"));
    assert!(error.contains("if committed {"));
    assert!(error.contains("self.return_to_main_menu_after_match()"));
    assert!(error.contains("Self::staged_load_failure_return_state(prior_state)"));
    assert!(!error.contains("host_clear_match_residuals"));
    assert!(!error.contains("invalidate_world_visual_state"));
    assert!(!error.contains("invalidate_presentation_drawable_world"));
}

#[test]
fn load_failure_surfaces_gui_error_loading_game() {
    let source = include_str!("../host_authority.rs");
    let start = source
        .find("fn surface_load_game_ui_feedback")
        .expect("load error UI");
    let body = &source[start..];
    assert!(body.contains("GUI:ErrorLoadingGame") || body.contains("format_error_loading_game"));
    assert!(body.contains("message_box_ok"));
    let ui = source
        .find("pub(super) fn host_load_game_from_ui")
        .expect("load UI boundary");
    let ui_body = &source[ui..];
    let reset = ui_body
        .find("self.return_to_main_menu_after_match()")
        .expect("failed commit reset");
    let message = ui_body
        .find("Self::surface_load_game_ui_feedback(&save_path)")
        .expect("load error dialog");
    assert!(
        reset < message,
        "C++ resets before showing the failed-load dialog"
    );
}

#[test]
fn offline_restore_prefers_saved_skirmish_over_live_campaign() {
    let mode = CnCGameEngine::offline_restore_mode_for_save(
        GameMode::SinglePlayer,
        &save_info("slot", "Maps\\Alpine.map".into()),
        Some(2),
    )
    .expect("saved skirmish must restore");
    assert_eq!(mode, GameMode::Skirmish);
    let mode = CnCGameEngine::offline_restore_mode_for_save(
        GameMode::Skirmish,
        &save_info("slot", "Maps\\Alpine.map".into()),
        Some(0),
    )
    .expect("saved campaign must restore");
    assert_eq!(mode, GameMode::SinglePlayer);
}

#[test]
fn staged_restore_rejects_unavailable_map_before_snapshot_restore() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "staged_restore_rejects_unavailable_map_before_snapshot_restore",
        || {
            let temp = tempfile::tempdir().expect("temporary save directory");
            let mut saves = SaveFileManager::with_save_directory(temp.path());
            saves.init().expect("initialize temporary save directory");

            // This deliberately needs no retail assets: map identity is validated
            // before a staging world is started or any snapshot state is restored.
            let mut source = GameLogic::new();
            source.start_new_game(GameMode::SinglePlayer);
            let catalog = source.templates.clone();
            saves
                .save_game(
                    "missing_map",
                    &source,
                    &save_info("missing_map", "Not A Retail Map".to_string()),
                )
                .expect("write invalid-map save metadata");

            let err = match CnCGameEngine::stage_saved_world_for_restore(
                &mut saves,
                "missing_map",
                GameMode::Shell,
                RestoreTemplateCatalog {
                    templates: &catalog,
                    modules: &source.receiving_module_content,
                    object_definitions: &source.receiving_object_definitions,
                    baseline: &source.receiving_template_baseline,
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
            ) {
                Ok(_) => panic!("missing saved map must reject before a false InGame restore"),
                Err(err) => err,
            };
            assert!(
                err.contains("not available on disk"),
                "unexpected error: {err}"
            );
        },
    );
}

#[cfg(feature = "game_client")]
#[test]
fn malformed_client_chunk_rejects_candidate_before_touching_live_client() {
    // Core compatibility script globals remain process-owned; each fixture
    // runs independently while the parent harness stays parallel.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "malformed_client_chunk_rejects_candidate_before_touching_live_client",
        || {
            let mut live =
                Box::new(game_client::core::game_client::GameClient::new().expect("client"));
            live.mark_initialized();
            live.set_frame(777);
            let logic = GameLogic::new();
            let catalog = logic.templates.clone();
            let invalid_chunk = vec![3, 0, 0, 0, 0, 1, 0xff, 0xff, 0xff, 0xff];
            let err = CnCGameEngine::stage_decoded_saved_world_for_restore(
                &crate::save_load::WorldSnapshot::default(),
                save_info("corrupt_client", "Not A Retail Map".to_string()),
                None,
                Some(invalid_chunk),
                None, // no paired TerrainVisual in this direct staging fixture
                "corrupt_client",
                GameMode::SinglePlayer,
                RestoreTemplateCatalog {
                    templates: &catalog,
                    modules: &logic.receiving_module_content,
                    object_definitions: &logic.receiving_object_definitions,
                    baseline: &logic.receiving_template_baseline,
                },
                &logic.weapon_definition_base,
                &logic.projectile_definitions,
                &logic.ocl_application_definitions(),
                logic.ai_definitions.baseline(),
                logic.production_energy_baseline(),
                logic.physics_rules_baseline(),
                logic.effective_terrain_definitions(),
                logic.current_armor_definitions(),
                logic.session_view.camera.settings,
                logic.session_view.camera.clip_policy,
                logic.session_view.camera.shake_policy,
                logic.session_view.camera.movement_policy,
                logic.session_view.camera.input_policy,
                &logic.world_services,
                |_snapshot, _staged| panic!("invalid client chunk must fail before stage restore"),
            )
            .err()
            .expect("reject malformed client chunk");
            assert!(err.contains("invalid CHUNK_GameClient"), "{err}");
            assert_eq!(live.get_frame(), 777);
        },
    );
}

#[test]
fn staged_restore_requires_the_saved_retail_map_before_snapshot_restore() {
    // Core compatibility script globals remain process-owned; each fixture
    // runs independently while the parent harness stays parallel.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "staged_restore_requires_the_saved_retail_map_before_snapshot_restore",
        || {
            let map_name = retail_map_path_for_test();

            let temp = tempfile::tempdir().expect("temporary save directory");
            let mut saves = SaveFileManager::with_save_directory(temp.path());
            saves.init().expect("initialize temporary save directory");

            let mut source = GameLogic::new();
            let mut ai_base = source.ai_definitions.baseline().clone();
            ai_base.resources_poor = 3217;
            source.set_ai_definition_base(ai_base);
            source.admit_production_energy_base(game_engine::common::ini::ini_game_data::ProductionEnergyRules::default().overlay_ini_text("GameData\nLowEnergyPenaltyModifier=1\nMinLowEnergyProductionSpeed=.5\nMaxLowEnergyProductionSpeed=.8\nEnd").unwrap());
            // The old70/71 rows were out-of-range lobby IDs. Use actual resolved
            // human0/ChinaAI1 while retaining Hard/activity/base/frame evidence.
            admit_skirmish_fixture(&mut source, &map_name, "Human", true);
            assert!(source.load_map(&map_name), "load source retail map");
            // Seed a save-time FOW state that differs from map-start reveals. The
            // staged candidate must carry these raw counters and the pending
            // expiry queue rather than retaining its map-load singleton state.
            let expected_shroud = {
                let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
                let mut shroud = shroud_manager.lock().expect("source shroud lock");
                shroud.do_shroud_reveal(&glam::Vec3::ZERO, 75.0, 1);
                shroud.queue_undo_shroud_reveal(&glam::Vec3::ZERO, 75.0, 1, 41, 321);
                shroud.snapshot_state()
            };
            // Initial config already admitted the actual China AI; mutate
            // that same owner rather than append a second out-of-range player.
            source.set_ai_active(1, false);
            source.relocate_host_ai_base(1, glam::Vec3::new(47.0, 0.0, -31.0));
            source.set_current_frame(321);
            let catalog = source.templates.clone();
            assert_eq!(source.host_ai_difficulty(1), Some(AIDifficulty::Hard));
            assert!(!source.is_host_ai_active(1));
            let client_drawables = crate::save_load::ClientDrawableWorldSnapshot {
                drawables: vec![crate::save_load::ClientDrawableStateSnapshot {
                    object_id: 71,
                    draw_module_index: 2,
                    source_template_name: "SavedDrawableSource".to_string(),
                    model_key: "SavedDrawableModel".to_string(),
                    selected_condition_state_index: 5,
                    last_seen_weapon_discharge_sequence: 19,
                    ..Default::default()
                }],
            };
            #[cfg(feature = "game_client")]
            let save_result = {
                let mut client =
                    game_client::core::game_client::GameClient::new().expect("source client");
                client.bind_visual_world(gamelogic::helpers::ClientVisualHandle::new(Arc::clone(
                    &source.world_services,
                )));
                client.set_frame(41);
                saves.save_game_with_client_state(
                    "valid_map",
                    &source,
                    client_drawables.clone(),
                    &mut client,
                    &save_info("valid_map", map_name.clone()),
                )
            };
            #[cfg(not(feature = "game_client"))]
            let save_result = saves.save_game_with_client_drawable_snapshot(
                "valid_map",
                &source,
                client_drawables.clone(),
                &save_info("valid_map", map_name.clone()),
            );
            save_result.expect("write valid staged restore save");

            let restored = CnCGameEngine::stage_saved_world_for_restore(
                &mut saves,
                "valid_map",
                GameMode::Shell,
                RestoreTemplateCatalog {
                    templates: &catalog,
                    modules: &source.receiving_module_content,
                    object_definitions: &source.receiving_object_definitions,
                    baseline: &source.receiving_template_baseline,
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
            )
            .expect("saved map should load before restore");
            let extracted_map = std::path::Path::new(&restored.info.map_name);
            assert_eq!(
                extracted_map.file_name().and_then(|name| name.to_str()),
                Some(map_name.as_str()),
                "C++ GameStateMap extracts the embedded map under its logical name"
            );
            assert!(extracted_map.exists(), "the saved map must exist on disk");
            assert_eq!(
                restored.shroud, expected_shroud,
                "staged restore must carry exact saved FOW rather than map-start reveals"
            );
            assert_eq!(
                restored.client_drawables, client_drawables,
                "staged restore must carry the renderer companion to the commit boundary"
            );
            #[cfg(feature = "game_client")]
            crate::save_load::snapshot::validate_game_client_xfer_bytes(
                restored
                    .game_client_xfer_bytes
                    .as_deref()
                    .expect("staged client chunk"),
            )
            .expect("staged client chunk remains valid until commit");
            assert!(restored.logic.isInGame());

            assert_eq!(
                restored.logic.ai_definitions.baseline().resources_poor,
                3217
            );
            assert_eq!(
                format!("{:?}", restored.logic.ai_definitions.data()),
                format!("{:?}", source.ai_definitions.data()),
                "saved-map reconstruction uses the source engine baseline and original map/Solo overrides"
            );
            assert_eq!(
                restored.logic.get_current_map_name(),
                restored.info.map_name
            );
            assert_eq!(restored.logic.get_current_frame(), 321);
            assert_eq!(restored.logic.host_ai_player_count(), 1);
            assert_eq!(
                restored.logic.host_ai_difficulty(1),
                Some(AIDifficulty::Hard)
            );
            assert!(!restored.logic.is_host_ai_active(1));
            let restored_ai = restored
                .logic
                .snapshot_host_ai_players_for_save()
                .into_iter()
                .find(|ai| ai.player_id == 1)
                .expect("registered host AI must survive the on-disk restore");
            assert_eq!(
                restored_ai.base_center,
                Some(glam::Vec3::new(47.0, 0.0, -31.0))
            );
        },
    );
}

#[test]
fn forced_post_restore_failure_restores_globals_and_tls_effect_queues() {
    // Core compatibility script globals remain process-owned; each fixture
    // runs independently while the parent harness stays parallel.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "forced_post_restore_failure_restores_globals_and_tls_effect_queues",
        || {
            let map_name = retail_map_path_for_test();

            let temp = tempfile::tempdir().expect("temporary save directory");
            let mut saves = SaveFileManager::with_save_directory(temp.path());
            saves.init().expect("initialize temporary save directory");

            let mut source = GameLogic::new();
            let mut ai_base = source.ai_definitions.baseline().clone();
            ai_base.resources_poor = 7654;
            source.set_ai_definition_base(ai_base);
            source.admit_production_energy_base(game_engine::common::ini::ini_game_data::ProductionEnergyRules::default().overlay_ini_text("GameData\nLowEnergyPenaltyModifier=1\nMinLowEnergyProductionSpeed=.5\nMaxLowEnergyProductionSpeed=.8\nEnd").unwrap());
            admit_skirmish_fixture(&mut source, &map_name, "Human", true);
            assert!(source.load_map(&map_name), "load source retail map");
            // Seed compatibility state independently of match setup. A failed
            // staged restore must leave that state intact; setting up the host's
            // opponents no longer initializes these separate AI representations.
            source.setup_skirmish_ai(0);
            let foreign_core = gamelogic::system::engine_stores::new_for_world();
            let (first_live_ai_group_id, second_live_ai_group_id) = {
                let ai_store = foreign_core.ai();
                let mut ai = ai_store.write().expect("lock live legacy AI");
                let first = ai.create_group();
                let first = first.read().expect("read first live AI group").get_id();
                let second = ai.create_group();
                let second = second.read().expect("read second live AI group").get_id();
                (first, second)
            };
            // C++ AIPlayer.cpp:81 reads its AI owner's teamSeconds at construction.
            // Admit this independent Core fixture under its own store, then restore
            // Main's service selection before testing the real rollback boundary.
            let live_integration_ai_player_count =
                gamelogic::system::engine_stores::with_active_stores(&foreign_core, || {
                    gamelogic::ai::integration::initialize_ai_integration()
                        .expect("initialize independent compatibility AI fixture");
                    gamelogic::ai::integration::with_ai_integration_mut(|manager| {
                        manager.ensure_ai_player(77, false);
                        manager.get_ai_player_count()
                    })
                    .expect("live AI integration manager initialized")
                });
            let catalog = source.templates.clone();
            saves
                .save_game(
                    "forced_stage_failure",
                    &source,
                    &save_info("forced_stage_failure", map_name.clone()),
                )
                .expect("write forced-failure staged restore save");
            let crate::save_load::DecodedGameSave {
                snapshot,
                save_info: info,
                saved_game_mode,
                ..
            } = saves
                .load_game_snapshot("forced_stage_failure")
                .expect("decode forced-failure staged restore save");

            // Capture every singleton family this transaction owns.  The candidate
            // map load is deliberately allowed to mutate them before the injected
            // error; equality below proves the active values were restored, not
            // merely cleared to defaults.
            let global_probe = |source: &GameLogic| {
                let terrain_owner_handle = gamelogic::terrain::get_terrain_logic();
                let terrain = terrain_owner_handle
                    .read()
                    .map(|terrain| terrain.get_source_filename().to_string())
                    .unwrap_or_default();
                let players = gamelogic::player::ThePlayerList()
                    .read()
                    .map(|players| (players.get_player_count(), players.get_local_player_index()))
                    .unwrap_or_default();
                let teams = gamelogic::team::get_team_factory()
                    .lock()
                    .map(|teams| {
                        (
                            teams.get_all_teams().len(),
                            teams.get_next_team_id(),
                            teams.get_next_team_prototype_id(),
                        )
                    })
                    .unwrap_or_default();
                let sides = (
                    source.prepared_map_sides().get_num_sides(),
                    source.prepared_map_sides().get_num_teams(),
                );
                let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
                let shroud = shroud_manager
                    .lock()
                    .map(|shroud| shroud.snapshot_state())
                    .unwrap_or_default();
                let mut named = gamelogic::scripting::engine::get_named_object_tracker()
                    .get_all_named_objects()
                    .unwrap_or_default();
                named.sort();
                let mut areas = gamelogic::scripting::engine::get_area_tracker().all_area_aabbs();
                areas.sort_by(|left, right| left.0.cmp(&right.0));
                let script_engine_present = gamelogic::scripting::engine::get_script_engine()
                    .read()
                    .map(|engine| engine.is_some())
                    .unwrap_or(false);
                let ai_store = foreign_core.ai();
                let legacy_ai_groups = ai_store
                    .read()
                    .map(|ai| {
                        (
                            ai.get_group_by_id(first_live_ai_group_id).is_some(),
                            ai.get_group_by_id(second_live_ai_group_id).is_some(),
                        )
                    })
                    .unwrap_or_default();
                let integration_group_count =
                    gamelogic::ai::integration::with_ai_integration(|manager| {
                        manager.get_ai_player_count()
                    })
                    .unwrap_or_default();
                (
                    terrain,
                    players,
                    teams,
                    sides,
                    shroud,
                    named,
                    areas,
                    script_engine_present,
                    legacy_ai_groups,
                    integration_group_count,
                )
            };
            let before = global_probe(&source);

            let _log_restore = WorldStageLogRestore::take();
            let sentinel = crate::game_logic::ObjectId(0x00ff_ee11);
            record_all_stage_queue_events(sentinel, "stage-rollback-sentinel");
            let expected =
                crate::game_logic::staged_world_effects::WorldStageEffectsState::take_all_for_test(
                );
            drop(
            crate::game_logic::staged_world_effects::WorldStageEffectsState::replace_all_for_test(
                expected.clone(),
            ),
        );

            #[cfg(feature = "game_client")]
            let mut live_client = {
                let mut client = Box::new(
                    game_client::core::game_client::GameClient::new().expect("live client"),
                );
                client.mark_initialized();
                client.set_frame(777);
                client
            };
            #[cfg(feature = "game_client")]
            let staged_client_bytes = {
                let mut saved = game_client::core::game_client::GameClient::new()
                    .expect("saved client payload");
                saved.set_frame(42);
                Some(saved.capture_xfer_bytes().expect("valid client chunk"))
            };
            #[cfg(not(feature = "game_client"))]
            let staged_client_bytes: Option<Vec<u8>> = None;

            let retained_ai_definitions = source.ai_definitions.snapshot();
            source
                .movement_events
                .record(source.frame, sentinel, Some([4.0, 5.0, 6.0]));
            let _ = source.movement_events.drain_for_host_boundary();
            source.movement_events.record(source.frame, sentinel, None);
            let retained_moves = source.movement_events.snapshot_completed().to_vec();
            let retained_pending_moves = source.movement_events.snapshot_pending().to_vec();
            let retained_move_session = source.movement_events.session().clone();
            let err = match CnCGameEngine::stage_decoded_saved_world_for_restore(
                &snapshot,
                info,
                saved_game_mode,
                staged_client_bytes,
                None, // no paired TerrainVisual in this direct staging fixture
                "forced_stage_failure",
                GameMode::Skirmish,
                RestoreTemplateCatalog {
                    templates: &catalog,
                    modules: &source.receiving_module_content,
                    object_definitions: &source.receiving_object_definitions,
                    baseline: &source.receiving_template_baseline,
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
                |_snapshot, staged| {
                    assert!(staged.isInGame(), "failure is injected after map load");
                    assert_eq!(staged.ai_definitions.baseline().resources_poor, 7654);
                    assert_eq!(staged.production_energy_rules().min_speed, 0.5);
                    assert!(gamelogic::runtime_world_transaction::world_runtime_staging_active());
                    assert!(crate::game_logic::staged_world_effects::world_stage_effects_active());
                    // The map candidate has already emitted its normal object
                    // creation events.  Emit every conditional residual family as
                    // well, so rollback proves the raw boundary discards both.
                    record_all_stage_queue_events(
                        crate::game_logic::ObjectId(0x00ff_ee12),
                        "staged-candidate-only",
                    );
                    // Deliberately collide with the live world's numeric ID.
                    assert_ne!(staged.movement_events.session(), &retained_move_session);
                    staged
                        .movement_events
                        .record(staged.frame, sentinel, Some([99.0, 98.0, 97.0]));
                    let _ = staged.movement_events.drain_for_host_boundary();
                    Err("forced failure after staged snapshot restore".to_string())
                },
            ) {
                Ok(_) => {
                    panic!("injected post-restore failure must reject the candidate world")
                }
                Err(err) => err,
            };
            assert!(err.contains("forced failure after staged snapshot restore"));
            assert_eq!(source.movement_events.session(), &retained_move_session);
            assert_eq!(
                source.movement_events.snapshot_pending(),
                retained_pending_moves
            );
            assert_eq!(
                source.movement_events.snapshot_completed(),
                retained_moves,
                "rejected candidate must leave the producing world's completed movements intact"
            );
            assert!(
                Arc::ptr_eq(&retained_ai_definitions, &source.ai_definitions.snapshot()),
                "failed candidate must leave live AI definitions unchanged"
            );

            #[cfg(feature = "game_client")]
            assert_eq!(
                live_client.get_frame(),
                777,
                "failed staging must not deserialize CHUNK_GameClient into the live client"
            );

            assert_eq!(
                global_probe(&source),
                before,
                "rollback must restore live globals"
            );
            let resumed_ai_group_id = {
                let ai_store = foreign_core.ai();
                let mut ai = ai_store.write().expect("lock restored legacy AI");
                let resumed = ai.create_group();
                let resumed_id = resumed
                    .read()
                    .expect("read post-rollback legacy AI group")
                    .get_id();
                resumed_id
            };
            assert_eq!(
                resumed_ai_group_id,
                second_live_ai_group_id.wrapping_add(1),
                "rollback must preserve the legacy AI group-ID allocator"
            );
            assert_eq!(
                gamelogic::ai::integration::with_ai_integration(|manager| {
                    manager.get_ai_player_count()
                }),
                Some(live_integration_ai_player_count),
                "rollback must preserve the live AI integration manager"
            );
            let actual =
                crate::game_logic::staged_world_effects::WorldStageEffectsState::take_all_for_test(
                );
            assert_eq!(
                actual, expected,
                "all map/snapshot-reachable TLS queues and last-drain state must survive rollback"
            );
        },
    );
}

/// GameLogic::loadMapINI uses pristineMapName for companions, while the
/// extracted GameStateMap remains the authoritative saved geometry.
#[test]
fn embedded_saved_map_restores_original_ai_companions_without_replacing_geometry() {
    // Core compatibility script globals remain process-owned; each fixture
    // runs independently while the parent harness stays parallel.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "embedded_saved_map_restores_original_ai_companions_without_replacing_geometry",
        || {
            fn map_bytes(ambient: [f32; 3]) -> Vec<u8> {
                let mut out = game_engine::common::system::DataChunkOutput::new();
                out.open_data_chunk("GlobalLighting", 1);
                out.write_int(2); // Afternoon
                for _ in 0..4 {
                    for _ in 0..2 {
                        for value in ambient {
                            out.write_real(value);
                        }
                        for value in [0.8, 0.8, 0.8, 0.0, 0.0, -1.0] {
                            out.write_real(value);
                        }
                    }
                }
                out.close_data_chunk();
                out.into_ckmp_bytes()
            }
            struct CreatedDirectories(Vec<std::path::PathBuf>);
            impl Drop for CreatedDirectories {
                fn drop(&mut self) {
                    for directory in &self.0 {
                        // Only remove directories this fixture created, and only
                        // if empty; concurrent/user content is never removed.
                        let _ = std::fs::remove_dir(directory);
                    }
                }
            }
            for bare_lookup in [false, true] {
                let _logs = WorldStageLogRestore::take();
                let temp = tempfile::tempdir().unwrap();
                let retail_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../windows_game/extracted_big_files/MapsZH/Maps");
                let mut created = Vec::new();
                if bare_lookup {
                    let mut path = retail_root.as_path();
                    while !path.exists() {
                        created.push(path.to_path_buf());
                        path = path.parent().unwrap();
                    }
                    std::fs::create_dir_all(&retail_root).unwrap();
                }
                let _created_directories = CreatedDirectories(created);
                let retail_dir = bare_lookup.then(|| {
                    tempfile::Builder::new()
                        .prefix("Hq2umeiBare")
                        .tempdir_in(&retail_root)
                        .unwrap()
                });
                let original_dir = retail_dir.as_ref().map_or_else(
                    || temp.path().join("authored"),
                    |dir| dir.path().to_path_buf(),
                );
                std::fs::create_dir_all(&original_dir).unwrap();
                let bare_name = original_dir.file_name().unwrap().to_str().unwrap();
                let original_map = original_dir.join(if bare_lookup {
                    format!("{bare_name}.map")
                } else {
                    "OwnedSavedMap.map".to_string()
                });
                let saved_geometry = map_bytes([0.1, 0.2, 0.3]);
                std::fs::write(&original_map, &saved_geometry).unwrap();
                std::fs::write(
                    original_dir.join("Map.ini"),
                    "AIData\n WallHeight = 31\n GuardEnemyScanRate = 1500\n End\nGameData\n MinLowEnergyProductionSpeed=.7\n MaxLineBuildObjects=7\n End\n",
                )
                .unwrap();
                std::fs::write(
                    original_dir.join("Solo.ini"),
                    "AIData\n GuardEnemyScanRate = 2500\n AttackUsesLineOfSight = No\n End\nGameData\n MaxLowEnergyProductionSpeed=.75\n MaxLineBuildObjects=3\n End\n",
                )
                .unwrap();
                let resolved_source = original_map.to_str().unwrap().to_string();
                let map_name = if bare_lookup {
                    bare_name.to_string()
                } else {
                    resolved_source.clone()
                };
                let resolved_source = crate::game_logic::script_loader::find_map_file(&map_name)
                    .expect("qualified and bare fixture must resolve actual authored map")
                    .to_string_lossy()
                    .into_owned();
                assert_eq!(
                    std::path::Path::new(&resolved_source)
                        .canonicalize()
                        .unwrap(),
                    original_map.canonicalize().unwrap()
                );
                let mut source = GameLogic::new();
                let mut base = source.ai_definitions.baseline().clone();
                base.resources_poor = 4321;
                base.guard_enemy_scan_rate = 15;
                base.wall_height = 21.0;
                source.set_ai_definition_base(base);
                source.admit_production_energy_base(game_engine::common::ini::ini_game_data::ProductionEnergyRules::default().overlay_ini_text("GameData\nLowEnergyPenaltyModifier=1\nMinLowEnergyProductionSpeed=.5\nMaxLowEnergyProductionSpeed=.8\nMaxLineBuildObjects=50\nEnd").unwrap());
                admit_skirmish_fixture(&mut source, &map_name, "MovementSavedOwner", false);
                assert!(source.load_map(&map_name));
                assert_eq!(
                    source.get_current_map_name(),
                    map_name,
                    "resolved companion identity must not replace the logical map name"
                );
                assert_eq!(source.host_guard_enemy_scan_rate(), 75);
                assert_eq!(source.ai_definitions.data().wall_height, 31.0);
                assert!(!source.ai_definitions.data().attack_uses_line_of_sight);
                assert_eq!(
                    source.last_parsed_map_settings().unwrap().ambient_color,
                    Some([0.1, 0.2, 0.3])
                );
                let source_definitions = source.ai_definitions.snapshot();
                let mut mover = crate::game_logic::ThingTemplate::new("StagedMovementUnit");
                mover
                    .add_kind_of(crate::game_logic::KindOf::Infantry)
                    .set_health(100.0);
                source.templates.insert(mover.name.clone(), mover);
                let mover_id = source
                    .create_object("StagedMovementUnit", Team::USA, glam::Vec3::ZERO)
                    .unwrap();
                let saved_destination = glam::Vec3::new(23.0, 0.0, 24.0);
                let (object, mut writer) = source
                    .host_object_and_movement_events_mut(mover_id)
                    .unwrap();
                object.set_destination(saved_destination, &mut writer);
                let _ = source.movement_events.drain_for_host_boundary();
                let source_movement_session = source.movement_events.session().clone();
                let source_movement_receipts = source.movement_events.snapshot_completed().to_vec();
                let catalog = source.templates.clone();
                let mut saves = SaveFileManager::with_save_directory(temp.path().join("save"));
                saves.init().unwrap();
                saves
                    .save_game(
                        "owned_companions",
                        &source,
                        &save_info("owned_companions", map_name.clone()),
                    )
                    .unwrap();
                // Original map geometry now differs. Its companions intentionally remain.
                let changed_geometry = map_bytes([0.7, 0.8, 0.9]);
                std::fs::write(&original_map, &changed_geometry).unwrap();
                std::fs::remove_file(&original_map).unwrap();
                assert!(
                    !original_map.exists(),
                    "embedded restore must not require original geometry"
                );
                let foreign = gamelogic::system::engine_stores::new_for_world();
                {
                    let mut data = foreign.ai_data().write().unwrap();
                    data.ensure_base();
                    data.get_active_mut().unwrap().wall_height = 999.0;
                    data.get_active_mut().unwrap().guard_enemy_scan_rate = 777;
                }
                gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                    let crate::save_load::DecodedGameSave {
                        snapshot,
                        save_info: info,
                        saved_game_mode,
                        ..
                    } = saves.load_game_snapshot("owned_companions").unwrap();
                    let extracted = std::path::Path::new(&info.map_name);
                    assert_ne!(extracted, original_map);
                    assert_eq!(std::fs::read(extracted).unwrap(), saved_geometry);
                    assert_ne!(std::fs::read(extracted).unwrap(), changed_geometry);
                    let pristine = info
                        .pristine_map_name
                        .as_deref()
                        .expect("native pristine identity");
                    assert_eq!(pristine, resolved_source);
                    assert_eq!(
                        source.map_definition_source.as_deref(),
                        Some(resolved_source.as_str())
                    );
                    let mut staged = CnCGameEngine::stage_decoded_saved_world_for_restore(
                        &snapshot,
                        info,
                        saved_game_mode,
                        None,
                        None, // no paired TerrainVisual in this direct staging fixture
                        "owned_companions",
                        GameMode::Skirmish,
                        RestoreTemplateCatalog {
                            templates: &catalog,
                            modules: &source.receiving_module_content,
                            object_definitions: &source.receiving_object_definitions,
                            baseline: &source.receiving_template_baseline,
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
                        |snapshot, candidate| {
                            saves
                                .restore_game_snapshot(snapshot, candidate)
                                .map_err(|err| err.to_string())
                        },
                    )
                    .unwrap();
                    assert_eq!(
                        staged
                            .logic
                            .last_parsed_map_settings()
                            .unwrap()
                            .ambient_color,
                        Some([0.1, 0.2, 0.3]),
                        "decode saved geometry, never reread changed original geometry"
                    );
                    assert_eq!(
                        format!("{:?}", staged.logic.ai_definitions.data()),
                        format!("{:?}", source_definitions),
                        "rebuild original engine + Map.ini + Solo.ini definitions"
                    );
                    assert_eq!(staged.logic.ai_definitions.baseline().resources_poor, 4321);
                    assert_eq!(staged.logic.production_energy_baseline().min_speed, 0.5);
                    assert_eq!(staged.logic.production_energy_rules().min_speed, 0.7);
                    assert_eq!(staged.logic.production_energy_rules().max_speed, 0.75);
                    assert_eq!(
                        staged
                            .logic
                            .production_energy_baseline()
                            .max_line_build_objects,
                        50
                    );
                    assert_eq!(
                        staged.logic.line_build_object_limit(),
                        3,
                        "saved-map candidate applies Map.ini then Solo.ini over its admitted baseline"
                    );
                    assert_eq!(
                        staged
                            .logic
                            .host_object(mover_id)
                            .unwrap()
                            .movement
                            .target_position,
                        Some(saved_destination),
                        "saved movement state survives without replaying transient movement observations"
                    );
                    assert_ne!(
                        staged.logic.movement_events.session(),
                        &source_movement_session
                    );
                    assert!(staged.logic.movement_events.snapshot_pending().is_empty());
                    assert!(staged.logic.movement_events.snapshot_completed().is_empty());
                    assert_eq!(
                        source.movement_events.snapshot_completed(),
                        source_movement_receipts
                    );
                    let restored_movement_session = staged.logic.movement_events.session().clone();
                    let (object, mut writer) = staged
                        .logic
                        .host_object_and_movement_events_mut(mover_id)
                        .unwrap();
                    object.set_destination(glam::Vec3::new(43.0, 0.0, 44.0), &mut writer);
                    let _ = staged.logic.movement_events.drain_for_host_boundary();
                    assert_eq!(staged.logic.movement_events.snapshot_completed().len(), 1);
                    assert_eq!(
                        staged.logic.movement_events.snapshot_completed()[0].session,
                        restored_movement_session
                    );
                    assert_eq!(
                        source.movement_events.snapshot_completed(),
                        source_movement_receipts,
                        "the same-ID restored world's first command cannot consume source output"
                    );
                    assert_eq!(
                        foreign
                            .ai_data()
                            .read()
                            .unwrap()
                            .get_active()
                            .unwrap()
                            .wall_height,
                        999.0
                    );
                    assert!(Arc::ptr_eq(
                        &source_definitions,
                        &source.ai_definitions.snapshot()
                    ));
                    // Re-save the extracted map, retaining its original companion identity.
                    saves
                        .save_game(
                            "owned_resave",
                            &staged.logic,
                            &save_info("owned_resave", staged.info.map_name.clone()),
                        )
                        .unwrap();
                    let crate::save_load::DecodedGameSave {
                        save_info: resaved_info,
                        ..
                    } = saves.load_game_snapshot("owned_resave").unwrap();
                    assert_eq!(
                        resaved_info.pristine_map_name,
                        staged.info.pristine_map_name
                    );
                    let mut ordinary = staged.logic;
                    ordinary.reset();
                    assert!(ordinary.map_definition_source.is_none());
                    std::fs::write(&original_map, &changed_geometry).unwrap();
                    assert!(ordinary.load_map(&map_name));
                    assert_eq!(
                        ordinary.map_definition_source.as_deref(),
                        Some(resolved_source.as_str())
                    );
                    assert_eq!(
                        ordinary.last_parsed_map_settings().unwrap().ambient_color,
                        Some([0.7, 0.8, 0.9])
                    );
                });
            }
        },
    );
}

#[test]
fn staged_native_commit_finishes_after_logic_and_camera_install() {
    let source = include_str!("../host_authority.rs");
    let start = source
        .find("fn host_replace_staged_restore_world")
        .expect("combined staged commit boundary");
    let body = &source[start
        ..source[start..]
            .find("\n    pub(super) fn host_save_game_authority")
            .map(|end| start + end)
            .expect("end of combined staged commit boundary")];
    let globals = body
        .find("runtime_world.install_globals()")
        .expect("global install");
    let logic = body
        .find("std::mem::replace(&mut self.game_logic, logic)")
        .expect("host logic install");
    let effects = body
        .find("native_commit.finish_after_logic_commit()")
        .expect("deferred team effects");
    let shadow = body.find("shadow.sync_from_host").expect("shadow rebuild");
    let camera = body
        .find("self.admit_camera_continuation(camera)")
        .expect("receiving native camera admission");
    let preflight = body
        .find("runtime_world.preflight_native()")
        .expect("native payload preflight");
    let teardown = body
        .find("self.host_advance_direct_visual_world_epoch()")
        .expect("old client teardown");
    let discard_live = body
        .find("discard_live_for_world_replace()")
        .expect("live output invalidation");
    assert!(preflight < discard_live && preflight < teardown && teardown < globals);
    assert!(globals < logic && logic < camera && camera < effects && effects < shadow);
    assert!(!body.contains("execute_after_logic_commit()"));
}

/// C++ keeps RandomValue process-static and no load path reseeds it
/// (GameState.cpp:628-741): the post-load stream continues from wherever
/// the save left it.  This is the run-to-N / save / continue / load /
/// continue comparison in miniature: branch A keeps ticking the live
/// match past the save, branch B stages the save and applies the
/// commit-time reseed, and both must draw the identical sequence.
#[test]
fn staged_restore_continues_logic_rng_stream_from_save_point() {
    // Core compatibility script globals remain process-owned; each fixture
    // runs independently while the parent harness stays parallel.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "staged_restore_continues_logic_rng_stream_from_save_point",
        || {
            let map_name = retail_map_path_for_test();
            const FRAME_TIMESTEP: f32 = 1.0 / 30.0;

            let temp = tempfile::tempdir().expect("temporary save directory");
            let mut saves = SaveFileManager::with_save_directory(temp.path());
            saves.init().expect("initialize temporary save directory");

            let mut source = GameLogic::new();
            admit_skirmish_fixture(&mut source, &map_name, "Human", false);
            assert!(source.load_map(&map_name), "load source retail map");
            // Admit the source seed explicitly, then advance its owned continuation.
            source.set_logic_random_seed(0x1BAD_B002);
            for _ in 0..8 {
                source.tick_logic_frame(FRAME_TIMESTEP, None, None);
            }
            // Deterministic draws through the driving instance — exactly what a
            // tick's published-owner scope does — so the save-point words
            // provably sit past a fresh re-derivation of the base seed.
            let draw_through_instance = |logic: &mut GameLogic, draws: usize| -> Vec<i32> {
                game_engine::common::random_value::with_session_random(&logic.logic_random, || {
                    (0..draws)
                        .map(|_| {
                            game_engine::common::random_value::get_game_logic_random_value(
                                0, 1_000_000,
                            )
                        })
                        .collect()
                })
            };
            let _ = draw_through_instance(&mut source, 16);
            let saved_words = source.logic_random.seed_words();
            let mut fresh_derivation = game_engine::common::random_value::RandomState::default();
            fresh_derivation.seed_random(0x1BAD_B002);
            assert_ne!(
                saved_words,
                fresh_derivation.seed_words(),
                "pre-save draws must move the instance stream off the fresh derivation"
            );

            let catalog = source.templates.clone();
            saves
                .save_game(
                    "rng_continue",
                    &source,
                    &save_info("rng_continue", map_name.clone()),
                )
                .expect("write RNG continuation save");

            // Branch A: the live match continues from the save point.
            let branch_a_draws = draw_through_instance(&mut source, 6);
            let branch_a_words = source.logic_random.seed_words();

            // Branch B: restore the detached candidate's selected seed and exact
            // continuation, then preserve it across presentation commit work.
            let mut staged = CnCGameEngine::stage_saved_world_for_restore(
                &mut saves,
                "rng_continue",
                GameMode::Shell,
                RestoreTemplateCatalog {
                    templates: &catalog,
                    modules: &source.receiving_module_content,
                    object_definitions: &source.receiving_object_definitions,
                    baseline: &source.receiving_template_baseline,
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
            )
            .expect("stage RNG continuation save");
            assert_eq!(
                staged.logic_rng_seed_words, saved_words,
                "the snapshot must carry the save-point driving-instance words"
            );
            CnCGameEngine::host_restore_staged_logic_rng(
                &mut staged.logic,
                staged.logic_rng_seed_words,
            );
            assert_eq!(
                staged.logic.logic_random.seed_words(),
                saved_words,
                "commit must reseed the staged instance from the saved words"
            );
            assert_eq!(
                staged.logic.logic_base_seed, 0x1BAD_B002,
                "commit must retain the saved selected seed"
            );
            let branch_b_draws = draw_through_instance(&mut staged.logic, 6);
            assert_eq!(
                branch_b_draws, branch_a_draws,
                "first post-load draws must continue the save-point stream"
            );
            assert_eq!(
                staged.logic.logic_random.seed_words(),
                branch_a_words,
                "post-load stream state must match the unreloaded branch"
            );
        },
    );
}

/// C++ `GameStateMap::xfer` moves the exact object-ID counter early in
/// the load (GameStateMap.cpp:372-383) so a post-load creation can never
/// alias the ID of an object destroyed before the save.  No retail map
/// needed: the allocator contract lives in the snapshot round trip.
#[test]
fn restored_object_allocator_never_reuses_a_dead_object_id() {
    // Core compatibility script globals remain process-owned; each fixture
    // runs independently while the parent harness stays parallel.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "restored_object_allocator_never_reuses_a_dead_object_id",
        || {
            let mut source = GameLogic::new();
            let mut ranger = crate::game_logic::ThingTemplate::new("USARanger");
            ranger
                .add_kind_of(crate::game_logic::KindOf::Infantry)
                .add_kind_of(crate::game_logic::KindOf::Selectable);
            source.templates.insert("USARanger".to_string(), ranger);
            source.add_player(Player::new(0, Team::USA, "USA", true));

            let mut live_ids = Vec::new();
            for index in 0..6 {
                live_ids.push(
                    source
                        .create_object(
                            "USARanger",
                            Team::USA,
                            glam::Vec3::new(10.0 + index as f32, 0.0, 8.0),
                        )
                        .expect("create ranger"),
                );
            }
            let dead_id = *live_ids.last().expect("allocated ids");
            source.destroy_object(dead_id);
            // Drain the destruction list so the dead object is gone from the
            // saved world (C++ processes destruction during the logic update).
            for _ in 0..16 {
                source.tick_logic_frame(1.0 / 30.0, None, None);
                if source.host_object(dead_id).is_none() {
                    break;
                }
            }
            assert!(
                source.host_object(dead_id).is_none(),
                "dead object must leave the world before the save"
            );

            let builder = crate::save_load::SnapshotBuilder::new();
            let mut snapshot = builder
                .create_world_snapshot(&source)
                .expect("snapshot with id gap");
            assert!(
                snapshot.next_object_id > dead_id.0,
                "snapshot must carry the exact allocator counter past the dead id"
            );
            let saved_next = snapshot.next_object_id;
            assert!(!snapshot.objects.contains_key(&dead_id));

            let restore_and_create = |snapshot: &crate::save_load::WorldSnapshot| -> u32 {
                let mut restored = GameLogic::new();
                restored.templates = source.templates.clone();
                // This allocator fixture has no map loader: admit the same
                // receiving player/default-team definition as its source.
                // Strict saved roster validation must remain enabled.
                restored.add_player(Player::new(0, Team::USA, "USA", true));
                builder
                    .restore_from_snapshot(snapshot, &mut restored)
                    .expect("restore");
                restored
                    .create_object("USARanger", Team::USA, glam::Vec3::new(60.0, 0.0, 8.0))
                    .expect("post-load creation must allocate")
                    .0
            };

            // v22 save: the exact saved counter is authoritative.
            assert_eq!(
                restore_and_create(&snapshot),
                saved_next,
                "post-load creation must continue the saved allocator"
            );
            assert_ne!(
                saved_next, dead_id.0,
                "post-load creation must never alias the destroyed object's id"
            );

            // Pre-v22 save (sentinel counter): legacy max-live-id + 1 fallback.
            snapshot.next_object_id = 0;
            let fallback_id = restore_and_create(&snapshot);
            let max_live = snapshot.objects.keys().map(|id| id.0).max().unwrap_or(0);
            assert_eq!(
                fallback_id,
                max_live.saturating_add(1),
                "pre-v22 saves keep the max-id-plus-one derivation"
            );
        },
    );
}

/// Pre-v22 saves carry the `[0; 6]` sentinel: the commit reseed must be
/// a no-op so those loads keep the staged instance's game-start stream
/// (the historical behavior).
#[test]
fn commit_logic_rng_reseed_is_a_no_op_for_pre_v22_saves() {
    // Core compatibility script globals remain process-owned; each fixture
    // runs independently while the parent harness stays parallel.
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "commit_logic_rng_reseed_is_a_no_op_for_pre_v22_saves",
        || {
            let mut logic = GameLogic::new();
            logic.start_new_game(GameMode::Skirmish);
            let before_words = logic.logic_random.seed_words();
            let before_base = logic.logic_base_seed;
            CnCGameEngine::host_restore_staged_logic_rng(&mut logic, [0u32; 6]);
            assert_eq!(logic.logic_random.seed_words(), before_words);
            assert_eq!(logic.logic_base_seed, before_base);
        },
    );
}

/// The commit boundary must reseed after all staged map work: globals
/// install, logic installs, the checked native commit finishes, and then
/// the RNG reseeded — before the shadow rebuild exposes the world.
#[test]
fn staged_commit_reseeds_logic_rng_after_staged_map_work() {
    let source = include_str!("../host_authority.rs");
    let start = source
        .find("fn host_replace_staged_restore_world")
        .expect("combined staged commit boundary");
    let body = &source[start
        ..source[start..]
            .find("\n    pub(super) fn host_save_game_authority")
            .map(|end| start + end)
            .expect("end of combined staged commit boundary")];
    let effects = body
        .find("native_commit.finish_after_logic_commit()")
        .expect("deferred team effects");
    let reseed = body
        .find("host_restore_staged_logic_rng(&mut self.game_logic")
        .expect("commit-time logic RNG reseed");
    let shadow = body.find("shadow.sync_from_host").expect("shadow rebuild");
    assert!(effects < reseed && reseed < shadow);
}
/// Main map/save boundaries must not borrow or replace an unrelated Core
/// AI runtime. This executes a real CKMP load and native save continuation.
#[test]
fn main_services_start_and_stage_leave_foreign_core_ai_installed() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "main_services_start_and_stage_leave_foreign_core_ai_installed",
        || {
            let _logs = WorldStageLogRestore::take();
            let temp = tempfile::tempdir().unwrap();
            let map_path = temp.path().join("OwnedServices.map");
            let mut map = game_engine::common::system::DataChunkOutput::new();
            // Real authored receiving geometry starts at raw40; saved logical
            // continuation below changes one sample to7 before encoding.
            map.open_data_chunk("HeightMapData", 3);
            for value in [8, 8, 0, 64] {
                map.write_int(value);
            }
            for _ in 0..64 {
                map.write_byte(40);
            }
            map.close_data_chunk();
            map.open_data_chunk("ObjectsList", 3);
            map.open_data_chunk("Object", 3);
            for value in [0.0, 0.0, 0.0, 0.0] {
                map.write_real(value);
            }
            map.write_int(0);
            map.write_ascii_string("Waypoint");
            let mut properties = game_engine::common::dict::Dict::new();
            properties.set_int(
                game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
                    "waypointID",
                ),
                7,
            );
            properties.set_ascii_string(
                game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
                    "waypointName",
                ),
                "WaveGuide1",
            );
            map.write_dict(&properties);
            map.close_data_chunk();
            map.close_data_chunk();
            map.open_data_chunk("GlobalLighting", 1);
            map.write_int(2);
            for _ in 0..4 {
                for _ in 0..2 {
                    for value in [0.1, 0.2, 0.3, 0.8, 0.8, 0.8, 0.0, 0.0, -1.0] {
                        map.write_real(value);
                    }
                }
            }
            map.close_data_chunk();
            std::fs::write(&map_path, map.into_ckmp_bytes()).unwrap();
            let map_name = map_path.to_str().unwrap().to_string();
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let integration_count =
                gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                    gamelogic::ai::integration::initialize_ai_integration().unwrap();
                    gamelogic::ai::integration::with_ai_integration_mut(|ai| {
                        ai.ensure_ai_player(77, false);
                        ai.get_ai_player_count()
                    })
                    .unwrap()
                });
            let mut foreign_ai = foreign.ai().write().unwrap();
            let group = foreign_ai.create_group();
            let group_id = group.read().unwrap().get_id();
            let next_id = foreign_ai.create_group().read().unwrap().get_id();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                let foreign_parser = game_engine::common::ini::ini_ai_data::get_ai_data_store();
                let mut source = GameLogic::new();
                let terrain_definitions = crate::game_logic::terrain_definitions::parse_application_terrain_sources(&[
                    ("Terrain.ini", "Terrain SavedOwnedTerrain\n Texture = owned-saved.tga\n Class = ROCK\nEnd\n".into())
                ]).unwrap();
                source
                    .admit_terrain_definitions(terrain_definitions)
                    .unwrap();
                assert!(
                    Arc::ptr_eq(&gamelogic::system::engine_stores::active(), &foreign),
                    "constructor is inert"
                );
                assert!(Arc::ptr_eq(
                    &game_engine::common::ini::ini_ai_data::get_ai_data_store(),
                    &foreign_parser
                ));
                admit_skirmish_fixture(&mut source, &map_name, "Human", false);
                game_engine::common::ini::ini_game_data::init_global_data();
                {
                    let definitions = game_engine::common::ini::get_global_data().unwrap();
                    let mut definitions = definitions.write();
                    definitions.vertex_water_available_maps[0] = map_name.clone();
                    definitions.vertex_water_x_grid_cells[0] = 1;
                    definitions.vertex_water_y_grid_cells[0] = 1;
                    definitions.vertex_water_grid_size[0] = 1.0;
                    definitions.vertex_water_attenuation_a[0] = 1.0;
                    definitions.vertex_water_attenuation_range[0] = 1.0;
                }
                assert!(
                    source.load_map(&map_name),
                    "actual owned map load under foreign Core scope"
                );
                let mut saved_heights = source
                    .world_services
                    .terrain()
                    .read()
                    .unwrap()
                    .logic_height_map_bytes()
                    .to_vec();
                assert_eq!(saved_heights[9], 40, "authored map base");
                saved_heights[9] = 7;
                source
                    .world_services
                    .terrain()
                    .write()
                    .unwrap()
                    .try_apply_logic_height_map_bytes_with_effects(&saved_heights, &mut || {})
                    .unwrap();
                source.set_logic_random_seed(0x5EED_A001);
                let _ = source.logic_random.next_u32();
                let saved_rng = source.logic_random.seed_words();
                let saved_player_count = source.get_players().len();
                let live_terrain = Arc::clone(source.world_services.terrain());
                let live_light = gamelogic::system::engine_stores::with_world_services(
                    &source.world_services,
                    gamelogic::helpers::create_scene_point_light,
                );
                let mut saves = SaveFileManager::with_save_directory(temp.path().join("saves"));
                saves.init().unwrap();
                #[cfg(feature = "game_client")]
                let saved_native_water = {
                    let mut visual = game_client::terrain::terrain_visual::TerrainVisualImpl::new();
                    visual.admit_water_state(
                        source
                            .world_services
                            .terrain()
                            .read()
                            .unwrap()
                            .water_grid_state(),
                    );
                    visual.add_water_velocity(0.0, 0.0, 3.0, 17.0);
                    let observed = visual.capture_water_state();
                    assert!(!observed.mesh_motion.is_empty());
                    assert!(
                        source
                            .world_services
                            .terrain()
                            .read()
                            .unwrap()
                            .water_grid_state()
                            .mesh_motion
                            .is_empty()
                    );
                    let mut client = game_client::core::game_client::GameClient::new().unwrap();
                    client.bind_visual_world(gamelogic::helpers::ClientVisualHandle::new(
                        Arc::clone(&source.world_services),
                    ));
                    saves
                        .save_game_with_client_save_state_and_native_terrain(
                            "owned_services",
                            &source,
                            Default::default(),
                            &mut client,
                            &save_info("owned_services", map_name.clone()),
                            Default::default(),
                            Some(&visual),
                        )
                        .unwrap();
                    observed
                };
                #[cfg(not(feature = "game_client"))]
                saves
                    .save_game(
                        "owned_services",
                        &source,
                        &save_info("owned_services", map_name.clone()),
                    )
                    .unwrap();
                let catalog = source.templates.clone();
                let crate::save_load::DecodedGameSave {
                    snapshot,
                    save_info: info,
                    saved_game_mode,
                    ..
                } = saves.load_game_snapshot("owned_services").unwrap();
                let mut failed_native =
                    super::super::map_native_admission::NativeMapCandidate::default();
                let failure = CnCGameEngine::stage_decoded_saved_world_for_restore_with_native(
                    &snapshot,
                    info,
                    saved_game_mode,
                    None,
                    None, // no paired TerrainVisual in this direct staging fixture
                    "owned_services",
                    GameMode::Skirmish,
                    RestoreTemplateCatalog {
                        templates: &catalog,
                        modules: &source.receiving_module_content,
                        object_definitions: &source.receiving_object_definitions,
                        baseline: &source.receiving_template_baseline,
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
                    |_, candidate| {
                        assert!(candidate.isInGame());
                        assert!(!Arc::ptr_eq(
                            candidate.world_services.terrain(),
                            &live_terrain
                        ));
                        Err("owned-service rollback witness".into())
                    },
                    failed_native.receiver(),
                );
                assert!(matches!(failure, Err(ref error) if error.contains("rollback witness")));
                #[cfg(feature = "game_client")]
                assert_eq!(
                    failed_native.raw_height_for_test(1, 1),
                    Some(40),
                    "rejected native candidate is detached from receiving saved raw7"
                );
                assert!(source.isInGame());
                assert!(
                    source
                        .world_services
                        .terrain()
                        .read()
                        .unwrap()
                        .water_grid_state()
                        .mesh_motion
                        .is_empty(),
                    "failed detached stage leaves receiving logical water unchanged"
                );
                assert_eq!(
                    source
                        .world_services
                        .terrain()
                        .read()
                        .unwrap()
                        .logic_height_map_bytes(),
                    saved_heights.as_slice(),
                    "failed staged restore cannot change receiving saved heights"
                );
                assert_eq!(
                    source.logic_random.seed_words(),
                    saved_rng,
                    "failed candidate leaves live continuation intact"
                );
                assert_eq!(source.logic_base_seed, 0x5EED_A001);
                assert_eq!(source.get_current_map_name(), map_name);
                assert!(Arc::ptr_eq(source.world_services.terrain(), &live_terrain));
                assert_eq!(
                    gamelogic::ai::integration::with_ai_integration(|ai| ai.get_ai_player_count()),
                    Some(integration_count)
                );
                let staged = CnCGameEngine::stage_saved_world_for_restore(
                    &mut saves,
                    "owned_services",
                    GameMode::Skirmish,
                    RestoreTemplateCatalog {
                        templates: &catalog,
                        modules: &source.receiving_module_content,
                        object_definitions: &source.receiving_object_definitions,
                        baseline: &source.receiving_template_baseline,
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
                )
                .expect("successful native owned-service staging");
                assert!(staged.logic.isInGame());
                assert_eq!(
                    &**staged.logic.effective_terrain_definitions(),
                    &**source.effective_terrain_definitions(),
                    "actual encoded save staging carries the receiving current immutable Terrain catalog"
                );
                assert_eq!(
                    staged
                        .logic
                        .world_services
                        .terrain()
                        .read()
                        .unwrap()
                        .logic_height_map_bytes(),
                    saved_heights.as_slice()
                );
                #[cfg(feature = "game_client")]
                assert_eq!(
                    staged.native_map.raw_height_for_test(1, 1),
                    Some(7),
                    "same detached native owner must contain final saved raw7, not authored raw40, before install"
                );
                #[cfg(feature = "game_client")]
                assert_eq!(
                    staged
                        .logic
                        .world_services
                        .terrain()
                        .read()
                        .unwrap()
                        .water_grid_state()
                        .mesh_motion,
                    saved_native_water.mesh_motion,
                    "actual native archive retains receiving water motion through map staging"
                );
                assert_eq!(staged.logic.logic_base_seed, 0x5EED_A001);
                assert_eq!(staged.logic.logic_random.seed_words(), saved_rng);
                assert!(!Arc::ptr_eq(
                    staged.logic.world_services.terrain(),
                    &live_terrain
                ));
                assert_eq!(
                    gamelogic::system::engine_stores::with_world_services(
                        &source.world_services,
                        gamelogic::helpers::scene_point_lights,
                    )
                    .len(),
                    1
                );
                assert_eq!(live_light, 1);
                // Match the production boundary: globals, installed host,
                // then checked native finish. No Core AI participates.
                let StagedRestoreWorld {
                    logic,
                    runtime_world,
                    ..
                } = staged;
                let effects = runtime_world.preflight_native().unwrap().install_globals();
                let old = std::mem::replace(&mut source, logic);
                drop(old);
                effects.finish_after_logic_commit();
                assert!(source.isInGame());
                assert_eq!(source.get_players().len(), saved_player_count);
                assert!(source.get_player(0).unwrap().is_human);
                assert_eq!(
                    gamelogic::ai::integration::with_ai_integration(|ai| ai.get_ai_player_count()),
                    Some(integration_count)
                );
                assert!(Arc::ptr_eq(
                    &gamelogic::system::engine_stores::active(),
                    &foreign
                ));
                assert!(Arc::ptr_eq(
                    &game_engine::common::ini::ini_ai_data::get_ai_data_store(),
                    &foreign_parser
                ));
            });
            assert!(foreign_ai.get_group_by_id(group_id).is_some());
            assert!(foreign_ai.get_group_by_id(next_id).is_some());
            let resumed = foreign_ai.create_group().read().unwrap().get_id();
            assert_eq!(resumed, next_id.wrapping_add(1));
            assert_eq!(
                gamelogic::ai::integration::with_ai_integration(|ai| ai.get_ai_player_count()),
                Some(integration_count)
            );
        },
    );
}

/// The real map factory emits an enabled Garrison InitialRoster cue. A valid
/// GameClient envelope may still fail its real Drawable decoder after staging;
/// the candidate's owned audio must stay unadmitted until acceptance.
#[cfg(feature = "game_client")]
#[test]
fn native_initial_roster_audio_survives_staging_and_discards_late_client_failure() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "native_initial_roster_audio_survives_staging_and_discards_late_client_failure",
        || {
            use crate::game_logic::containment_audio_test_support as audio;
            let temp = tempfile::tempdir().unwrap();
            let map_path = temp.path().join("ContainmentAudioStage.map");
            let mut map = game_engine::common::system::DataChunkOutput::new();
            map.open_data_chunk("HeightMapData", 3);
            for value in [8, 8, 0, 64] {
                map.write_int(value);
            }
            for _ in 0..64 {
                map.write_byte(40);
            }
            map.close_data_chunk();
            map.open_data_chunk("ObjectsList", 3);
            map.open_data_chunk("Object", 3);
            for value in [20.0, 20.0, 0.0, 0.0] {
                map.write_real(value);
            }
            map.write_int(0);
            map.write_ascii_string("ContainmentStageContainer");
            let properties = game_engine::common::dict::Dict::new();
            map.write_dict(&properties);
            map.close_data_chunk();
            map.close_data_chunk();
            std::fs::write(&map_path, map.into_ckmp_bytes()).unwrap();
            let map_name = map_path.to_str().unwrap().to_owned();
            let mut source = GameLogic::new();
            audio::admit(
                &mut source,
                r#"
Object ContainmentStageContainer
 KindOf = STRUCTURE
 SoundEnter = ContainmentStageAuthoredEnter
 Body = ActiveBody BodyTag
  MaxHealth = 200
  InitialHealth = 200
 End
 Behavior = GarrisonContain ContainTag
  ContainMax = 1
  AllowInsideKindOf = INFANTRY
  InitialRoster = ContainmentStagePassenger 1
 End
End
Object ContainmentStagePassenger
 KindOf = INFANTRY
 TransportSlotCount = 1
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
End
"#,
            );
            admit_skirmish_fixture(&mut source, &map_name, "Audio stage", false);
            assert!(source.load_map(&map_name));
            let container_id = source
                .get_objects()
                .iter()
                .find(|(_, object)| object.template_name == "ContainmentStageContainer")
                .map(|(id, _)| *id)
                .expect("actual placed container");
            assert_eq!(
                source
                    .host_object(container_id)
                    .unwrap()
                    .contained_units()
                    .len(),
                1,
                "actual GarrisonInitialRoster factory callback must execute"
            );
            let builder = crate::save_load::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let service = game_engine::common::audio::game_audio::initialize_global_audio_manager();
            let definitions = audio::manager(&["ContainmentStageAuthoredEnter"]);
            {
                let definition = definitions
                    .find_audio_event_info("ContainmentStageAuthoredEnter")
                    .unwrap();
                service
                    .lock()
                    .unwrap()
                    .register_audio_event_info((*definition).clone());
            }
            let assert_service_empty = || {
                assert_eq!(
                    service
                        .lock()
                        .unwrap()
                        .pending_play_request_count_for("ContainmentStageAuthoredEnter"),
                    0,
                    "detached initial roster cannot publish actual Common AR_Play requests"
                );
            };
            assert_service_empty();
            // The envelope is genuinely valid. The length-delimited Drawable
            // contains only its ObjectID; its real snapshot decoder must fail.
            const DRAWABLE: &str = "ContainmentStageLateClient";
            assert_eq!(
                game_engine::common::thing::thing_factory::load_templates_from_ini_text(
                    &format!("Object {DRAWABLE}\nEnd\n"),
                    "containment-stage-client.ini"
                ),
                1
            );
            let mut invalid_drawable = vec![3];
            invalid_drawable.extend_from_slice(&888_u32.to_le_bytes());
            invalid_drawable.push(1);
            invalid_drawable.extend_from_slice(&1_u32.to_le_bytes());
            invalid_drawable.push(DRAWABLE.len() as u8);
            invalid_drawable.extend_from_slice(DRAWABLE.as_bytes());
            invalid_drawable.extend_from_slice(&1_u16.to_le_bytes());
            invalid_drawable.extend_from_slice(&1_u16.to_le_bytes());
            invalid_drawable.extend_from_slice(&1_u16.to_le_bytes());
            invalid_drawable.extend_from_slice(&4_i32.to_le_bytes());
            invalid_drawable.extend_from_slice(&0_u32.to_le_bytes());
            invalid_drawable.extend_from_slice(&0_i32.to_le_bytes());
            crate::save_load::snapshot::validate_game_client_xfer_bytes(&invalid_drawable).unwrap();
            let stage = |native: &mut super::super::map_native_admission::NativeMapCandidate,
                         client_bytes: Option<Vec<u8>>| {
                CnCGameEngine::stage_decoded_saved_world_for_restore_with_native(
                    &snapshot,
                    save_info("containment-audio-stage", map_name.clone()),
                    None,
                    client_bytes,
                    None,
                    "containment-audio-stage",
                    GameMode::Skirmish,
                    RestoreTemplateCatalog::from_world(&source),
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
                    |snapshot, candidate| {
                        let placed = candidate
                            .get_objects()
                            .values()
                            .find(|object| object.template_name == "ContainmentStageContainer")
                            .expect("map factory ran before snapshot restore");
                        assert_eq!(placed.contained_units().len(), 1);
                        assert_service_empty();
                        builder
                            .restore_from_snapshot(snapshot, candidate)
                            .map_err(|error| error.to_string())
                    },
                    native.receiver(),
                )
            };
            let mut rejected_native =
                super::super::map_native_admission::NativeMapCandidate::default();
            let mut rejected =
                stage(&mut rejected_native, Some(invalid_drawable)).unwrap_or_else(|error| {
                    panic!(
                        "envelope/map staging must succeed before the real late decoder: {error}"
                    )
                });
            rejected.native_map = rejected_native;
            assert_service_empty();
            let mut failed_client = game_client::core::game_client::GameClient::new().unwrap();
            let error = crate::save_load::snapshot::restore_game_client_from_xfer_bytes(
                &mut failed_client,
                rejected.game_client_xfer_bytes.as_deref().unwrap(),
            )
            .expect_err("real GameClient Drawable payload rejects after successful staging");
            assert!(!error.to_string().is_empty());
            drop(rejected);
            assert_service_empty();
            let mut accepted_native =
                super::super::map_native_admission::NativeMapCandidate::default();
            let mut accepted = stage(&mut accepted_native, None)
                .unwrap_or_else(|error| panic!("accepted stage: {error}"));
            accepted.native_map = accepted_native;
            assert_service_empty();
            {
                let mut recipient = service.lock().unwrap();
                accepted.native_map.admit_audio_to(&mut *recipient);
                audio::assert_events(
                    &recipient,
                    &[("ContainmentStageAuthoredEnter", container_id)],
                );
                accepted.native_map.admit_audio_to(&mut *recipient);
                audio::assert_events(
                    &recipient,
                    &[("ContainmentStageAuthoredEnter", container_id)],
                );
            }
        },
    );
}
