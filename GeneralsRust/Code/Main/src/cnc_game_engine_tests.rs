// Mechanical extract from cnc_game_engine.rs `mod tests`.
// Child module via `#[path]`. include_str! paths stay sibling-relative.


    #[test]
    fn presentation_path_applies_pose_without_object_registry() {
        let src = include_str!("cnc_game_engine.rs");
        assert!(
            src.contains("apply_presentation_pose_to_drawables"),
            "InGame presentation path must push pose to drawables without OBJECT_REGISTRY"
        );
        let i = src
            .find("apply_presentation_shroud_to_drawables(shroud_entries)")
            .expect("shroud apply");
        let window = &src[i..src.len().min(i + 900)];
        assert!(
            window.contains("apply_presentation_pose_to_drawables"),
            "pose apply must sit next to presentation shroud residual"
        );
    }

    #[test]
    fn ui_command_path_prefers_presentation_object_identity() {
        let eng = include_str!("cnc_game_engine.rs");
        for token in [
            "fn presentation_ro",
            "fn ui_object_alive",
            "fn ui_object_is_dozer",
            "fn ui_object_can_produce",
            "fn ui_production_queue_head",
            "fn ui_selected_ids",
            "fn ui_special_power_ready",
            "fn ui_special_power_type_if_ready",
        ] {
            assert!(
                eng.contains(token),
                "missing UI presentation helper {token}"
            );
        }
        // Producer fail-open live scan only when no presentation frame.
        assert!(
            eng.contains("last_presentation_frame.is_none()"),
            "producer fail-open must gate on missing presentation frame"
        );
        // Dozer/producer filters use presentation-first helpers.
        assert!(
            eng.contains("self.ui_object_is_dozer(id)")
                && eng.contains("self.ui_object_can_produce(id)")
                && eng.contains("self.ui_production_queue_head(id)"),
            "UI command filters must call presentation-first helpers"
        );
        // Wave 214: force-completed producer pick is presentation-only (no live classify).
        assert!(
            eng.contains("Wave 214: force-completed IDs classified from presentation freeze only")
                && eng.contains("force_completed")
                && eng.contains("can_produce")
                && eng.contains("no live GameLogic dual-read residual")
                && eng.contains(
                    "Wave 214: force-completed IDs classified from presentation freeze only"
                ),
            "force-completed producer pick must be presentation-only"
        );
    }

    #[test]
    fn sample_startup_camera_heights_prefers_presentation_height_grid() {
        let eng = include_str!("cnc_game_engine.rs");
        let idx = eng
            .find("fn sample_startup_camera_heights")
            .expect("camera height helper");
        let body = &eng[idx..idx + 1200];
        assert!(
            body.contains("presentation")
                && body.contains("sample_height")
                && body.contains("world_env"),
            "camera height helper must sample presentation world_env height grid"
        );
        assert!(
            body.contains("Option<&crate::presentation_frame::PresentationFrame>")
                || body.contains("presentation: Option"),
            "camera height helper must take optional PresentationFrame"
        );
    }

    #[test]
    fn render_ui_state_prefers_presentation_without_live_update() {
        let src = include_str!("cnc_game_engine.rs");
        // Wave 591: real consumer lives in host_build_render_ui_state_from_presentation.
        // Prefer last production def (tests may embed the signature string).
        let marker =
            "fn host_build_render_ui_state_from_presentation(&mut self) -> crate::ui::GameUIState";
        let mut i = None;
        let mut from = 0usize;
        while let Some(rel) = src[from..].find(marker) {
            i = Some(from + rel);
            from = from + rel + marker.len();
        }
        let i = i.expect("render presentation UI consumer helper");
        let window = &src[i..(i + 2000).min(src.len())];
        assert!(
            window.contains("GameUIState::default()") && window.contains("pres.apply_to_ui_state"),
            "InGame render must build UI state from PresentationFrame default+apply"
        );
        assert!(
            window.contains("Boot/loading residual only")
                && window.contains("update_ui_state(self.current_player_id)"),
            "boot residual may still call update_ui_state without presentation"
        );
        // Ensure the presentation branch does not call live update_ui_state first.
        // Comments may mention update_ui_state; require no host/live call before boot arm.
        let branch_end = window
            .find("Boot/loading residual only")
            .unwrap_or(window.len());
        let presentation_branch = &window[..branch_end];
        assert!(
            !presentation_branch.contains("host_update_ui_state(")
                && !presentation_branch.contains("self.update_ui_state("),
            "presentation branch must not call live update_ui_state"
        );
        assert!(
            src.contains("self.host_build_render_ui_state_from_presentation()"),
            "render path must call host render UI presentation helper"
        );
    }

    fn presentation_path_ticks_drawables_like_cpp() {
        let src = include_str!("cnc_game_engine.rs");
        // Build token from pieces so this test source does not self-match.
        let token = format!("// {}{}:", "PRES_SHELL_ONLY_", "DRAWABLE_TICK");
        let i = src.find(&token).expect("presentation shell token comment");
        let w = &src[i..src.len().min(i + 500)];
        assert!(
            w.contains("update_presentation_shell")
                && w.contains("update_drawables_local")
                && !w.contains("game_client.update_drawables("),
            "presentation client path must use shell-only drawable tick"
        );
    }

    #[test]
    fn show_shell_menu_sets_shell_active_for_wnd_residual() {
        let src = include_str!("cnc_game_engine.rs");
        let i = src.find("fn show_shell_menu").expect("show_shell_menu");
        let body = &src[i..src.len().min(i + 2200)];
        assert!(
            body.contains("SubsystemInterface::init"),
            "show_shell_menu must init Shell before push (TLS starts uninitialized)"
        );
        assert!(
            body.contains("set_shell_active(true)"),
            "show_shell_menu must set Shell::is_shell_active after MainMenu push"
        );
        assert!(
            body.contains("shell_menu_active = true"),
            "engine shell_menu_active residual required after successful stack push"
        );
        assert!(
            body.contains("get_screen_count()"),
            "show_shell_menu must verify screen stack before latching active"
        );
        assert!(
            !body.contains("will continue without a main menu") || body.contains("screens == 0"),
            "empty stack must not latch shell_menu_active"
        );
    }

    fn match_start_presentation_seed_uses_shadow_overlay() {
        let src = include_str!("cnc_game_engine.rs");
        // Wave 590: match-start peels through host_seed_presentation_after_match_start.
        let needle = "fn host_seed_presentation_after_match_start(";
        let i = src
            .find(needle)
            .expect("host_seed_presentation_after_match_start");
        let body = &src[i..src.len().min(i + 1400)];
        assert!(
            body.contains("host_sync_shadow_and_build_presentation"),
            "match-start seed must use presentation build boundary (Wave 926)"
        );
        assert!(
            !body.contains("build_and_apply_for_hud"),
            "seed must not skip shadow via build_and_apply_for_hud"
        );
        // Thin wrapper still exists for callers.
        assert!(
            src.contains("fn seed_presentation_after_match_start")
                && src.contains("host_seed_presentation_after_match_start()"),
            "seed_presentation_after_match_start must delegate to host helper"
        );
        // Boot/render residual seed via host_ensure_presentation_frame_for_render.
        let j = src
            .find("Boot/Menu residual: if no frame yet")
            .expect("boot residual comment");
        let boot_call = &src[j..src.len().min(j + 500)];
        assert!(
            boot_call.contains("host_ensure_presentation_frame_for_render"),
            "boot path must call host_ensure_presentation_frame_for_render"
        );
        let k = src
            .find("fn host_ensure_presentation_frame_for_render(")
            .expect("host_ensure_presentation_frame_for_render");
        let boot = &src[k..src.len().min(k + 900)];
        assert!(
            boot.contains("host_sync_shadow_and_build_presentation"),
            "boot presentation seed must use presentation build boundary (Wave 926)"
        );
    }

    #[test]
    fn apply_presentation_to_huds_dual_no_recurse_residual() {
        let src = include_str!("cnc_game_engine.rs");
        let marker = "fn apply_presentation_to_huds(";
        let i = src.find(marker).expect("dual HUD apply helper");
        let body = &src[i..src.len().min(i + 450)];
        assert!(
            body.contains("pres.apply_to_game_hud(&mut self.game_hud)"),
            "must apply presentation freeze to engine GameHUD"
        );
        assert!(
            body.contains("pres.apply_to_game_hud(self.ui_manager.game_hud_mut())"),
            "must apply presentation freeze to UIManager GameHUD"
        );
        // Body must not recurse into itself (stack overflow residual).
        let after_sig = match body.split_once('{') {
            Some((_, rest)) => rest,
            None => "",
        };
        assert!(
            !after_sig.contains("self.apply_presentation_to_huds("),
            "apply_presentation_to_huds must not call itself"
        );
    }

    use super::{
        should_exit_for_smoke_test, should_keep_logic_running_while_iconic, CnCGameEngine,
        GameMode, GameState, StartupNewGameDispatch,
    };
    use crate::command_line::CommandLineArgs;
    use game_engine::common::global_data::{
        test_isolation_lock, with_global_data_restored as with_global_data_snapshot_restored,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn with_global_and_startup_state_snapshot_restored<F: FnOnce()>(f: F) {
        let _guard = test_isolation_lock()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let global_snapshot = game_engine::common::global_data::read().clone();
        let previous_difficulty = gamelogic::helpers::TheScriptEngine::get_global_difficulty();
        let previous_rank_points =
            gamelogic::helpers::TheGameLogic::get_rank_points_to_add_at_game_start();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        *game_engine::common::global_data::write() = global_snapshot;
        gamelogic::helpers::TheScriptEngine::set_global_difficulty(previous_difficulty);
        gamelogic::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(
            previous_rank_points,
        );
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }

    fn create_temp_test_dir(prefix: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "generals_main_{prefix}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn startup_deferred_budget_is_disabled() {
        let budget = CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, None, 0);
        assert_eq!(budget, 0);
    }

    #[test]
    fn startup_deferred_budget_is_enabled_for_visible_menu_frames() {
        let budget =
            CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, Some(12), 12);
        assert_eq!(budget, 4);
    }

    #[test]
    fn smoke_test_exit_only_after_menu_startup_complete() {
        assert!(should_exit_for_smoke_test(
            true,
            GameState::Menu,
            1.0,
            false
        ));
        assert!(!should_exit_for_smoke_test(
            false,
            GameState::Menu,
            1.0,
            false
        ));
        assert!(!should_exit_for_smoke_test(
            true,
            GameState::Loading,
            1.0,
            false
        ));
        assert!(!should_exit_for_smoke_test(
            true,
            GameState::Menu,
            0.995,
            false
        ));
        assert!(!should_exit_for_smoke_test(
            true,
            GameState::Menu,
            1.0,
            true
        ));
    }

    #[test]
    fn configured_startup_shell_map_disables_missing_shell_map() {
        with_global_data_snapshot_restored(|| {
            {
                let mut global = game_engine::common::global_data::write();
                global.writable.shell_map_on = true;
                global.writable.shell_map_name = "__definitely_missing_shell_map__".to_string();
            }

            let shell_map = CnCGameEngine::configured_startup_shell_map();
            assert!(shell_map.is_none());

            let global = game_engine::common::global_data::read();
            assert!(!global.writable.shell_map_on);
        });
    }

    #[test]
    fn effective_fps_limit_prefers_script_override() {
        let limit =
            CnCGameEngine::effective_fps_limit_for_frame(Some(45), false, 30, 2.0, true, true);
        assert_eq!(limit, Some(45));
    }

    #[test]
    fn effective_fps_limit_honors_cpp_tivo_replay_rule_for_global_limit() {
        let limit = CnCGameEngine::effective_fps_limit_for_frame(None, true, 30, 1.0, true, true);
        assert_eq!(limit, None);
    }

    #[test]
    fn effective_fps_limit_disables_global_limit_for_fast_visual_multiplier() {
        let limit = CnCGameEngine::effective_fps_limit_for_frame(None, true, 30, 1.5, false, false);
        assert_eq!(limit, None);
    }

    #[test]
    fn startup_new_game_dispatch_prefers_last_queued_message() {
        use game_engine::common::message_stream::{GameMessage, GameMessageType};

        let mut first = GameMessage::new(GameMessageType::NewGame);
        first.append_integer_argument(0);
        first.append_integer_argument(0);
        first.append_integer_argument(0);

        let mut replay = GameMessage::new(GameMessageType::NewGame);
        replay.append_integer_argument(3);
        replay.append_integer_argument(1);
        replay.append_integer_argument(42);
        replay.append_integer_argument(90);

        let dispatch = CnCGameEngine::startup_new_game_dispatch_from_messages(&[
            first,
            GameMessage::new(GameMessageType::ClearGameData),
            replay,
        ])
        .expect("expected startup dispatch");

        assert_eq!(dispatch.game_mode, GameMode::Replay);
        assert_eq!(dispatch.difficulty, super::GameDifficulty::Medium);
        assert_eq!(dispatch.rank_points, 42);
        assert_eq!(dispatch.max_fps, Some(90));
    }

    #[test]
    fn startup_new_game_dispatch_applies_script_side_effects() {
        with_global_and_startup_state_snapshot_restored(|| {
            let dispatch = StartupNewGameDispatch {
                game_mode_code: 0,
                game_mode: GameMode::SinglePlayer,
                difficulty_code: 2,
                difficulty: super::GameDifficulty::Hard,
                rank_points: 77,
                max_fps: None,
            };

            let prepared_map = CnCGameEngine::apply_startup_new_game_dispatch(dispatch);
            assert!(prepared_map.is_none());
            assert_eq!(
                gamelogic::helpers::TheScriptEngine::get_global_difficulty(),
                2
            );
            assert_eq!(
                gamelogic::helpers::TheGameLogic::get_rank_points_to_add_at_game_start(),
                77
            );
        });
    }

    #[test]
    fn startup_new_game_dispatch_requires_pending_file_for_startup_map_preparation() {
        with_global_and_startup_state_snapshot_restored(|| {
            {
                let mut global = game_engine::common::global_data::write();
                global.writable.map_name = "Maps\\Unexpected\\Unexpected.map".to_string();
                global.pending_file.clear();
            }

            let dispatch = StartupNewGameDispatch {
                game_mode_code: 0,
                game_mode: GameMode::SinglePlayer,
                difficulty_code: 1,
                difficulty: super::GameDifficulty::Medium,
                rank_points: 0,
                max_fps: None,
            };

            let prepared_map = CnCGameEngine::apply_startup_new_game_dispatch(dispatch);
            assert!(prepared_map.is_none());

            let global = game_engine::common::global_data::read();
            assert_eq!(global.writable.map_name, "Maps\\Unexpected\\Unexpected.map");
            assert!(global.pending_file.is_empty());
        });
    }

    #[test]
    fn startup_new_game_dispatch_ignores_unrelated_messages() {
        use game_engine::common::message_stream::{GameMessage, GameMessageType};

        let dispatch = CnCGameEngine::startup_new_game_dispatch_from_messages(&[
            GameMessage::new(GameMessageType::Invalid),
            GameMessage::new(GameMessageType::ClearGameData),
        ]);

        assert!(dispatch.is_none());
    }

    #[test]
    fn take_new_game_dispatch_drains_stream_and_keeps_other_messages() {
        use game_engine::common::message_stream::{
            get_message_stream, GameMessage, GameMessageType,
        };

        let stream = get_message_stream();
        {
            let mut g = stream.write().unwrap_or_else(|e| e.into_inner());
            g.clear_messages();
            g.append_message(GameMessageType::ClearGameData);
            let ng = g.append_message(GameMessageType::NewGame);
            ng.append_integer_argument(2); // GAME_SKIRMISH
            ng.append_integer_argument(1);
            ng.append_integer_argument(0);
            ng.append_integer_argument(30);
            g.append_message(GameMessageType::Invalid);
        }

        let dispatch = CnCGameEngine::take_new_game_dispatch_from_common_stream()
            .expect("NewGame must be drained");
        assert_eq!(dispatch.game_mode, GameMode::Skirmish);
        assert_eq!(dispatch.max_fps, Some(30));

        let g = stream.read().unwrap_or_else(|e| e.into_inner());
        assert_eq!(g.message_count(), 2, "non-NewGame messages must remain");
        let types: Vec<_> = g
            .get_messages()
            .iter()
            .map(|m| m.get_type().clone())
            .collect();
        assert!(types
            .iter()
            .any(|t| matches!(t, GameMessageType::ClearGameData)));
        assert!(types.iter().any(|t| matches!(t, GameMessageType::Invalid)));
        assert!(!types.iter().any(|t| matches!(t, GameMessageType::NewGame)));
        // silence unused import if GameMessage only used above via type
        let _ = GameMessage::new(GameMessageType::Invalid);
    }

    #[test]
    fn startup_camera_focus_prefers_shell_metadata_before_default_seed() {
        let focus = CnCGameEngine::select_startup_camera_focus(
            true,
            Some(glam::Vec2::new(12.0, 34.0)),
            Some(glam::Vec2::new(56.0, 78.0)),
            glam::Vec2::new(90.0, 91.0),
        );

        assert_eq!(focus, glam::Vec2::new(12.0, 34.0));
    }

    #[test]
    fn startup_camera_focus_falls_back_to_shell_seed_without_metadata() {
        let focus = CnCGameEngine::select_startup_camera_focus(
            true,
            None,
            Some(glam::Vec2::new(56.0, 78.0)),
            glam::Vec2::new(90.0, 91.0),
        );

        assert_eq!(
            focus,
            glam::Vec2::new(
                87.0 * gamelogic::common::MAP_XY_FACTOR,
                77.0 * gamelogic::common::MAP_XY_FACTOR,
            )
        );
    }

    #[test]
    fn startup_camera_focus_keeps_non_shell_fallback_order() {
        let focus = CnCGameEngine::select_startup_camera_focus(
            false,
            None,
            Some(glam::Vec2::new(56.0, 78.0)),
            glam::Vec2::new(90.0, 91.0),
        );

        assert_eq!(focus, glam::Vec2::new(56.0, 78.0));
    }

    #[test]
    fn startup_mode_requires_new_game_dispatch_for_non_menu_startup() {
        let mut start_in_menu = false;
        let mut map_to_load = Some("Maps\\ShellMapMD\\ShellMapMD.map".to_string());

        let mode = CnCGameEngine::resolve_startup_mode_from_dispatch(
            &mut start_in_menu,
            &mut map_to_load,
            None,
            false,
        );

        assert_eq!(mode, GameMode::Shell);
        assert!(start_in_menu);
        assert!(map_to_load.is_none());
    }

    #[test]
    fn startup_initial_file_helper_matches_cpp_table_and_gating() {
        with_global_data_snapshot_restored(|| {
            {
                let mut global = game_engine::common::global_data::write();
                global.writable.initial_file.clear();
            }

            let replay_args = vec![
                "generals".to_string(),
                "-file".to_string(),
                "Replays\\demo.rep".to_string(),
            ];
            let replay_parsed = CommandLineArgs::parse_from_args(replay_args).unwrap();
            assert_eq!(
                CnCGameEngine::startup_initial_file_from_command_line(&replay_parsed, true),
                Some("Replays\\demo.rep".to_string())
            );
            assert_eq!(
                CnCGameEngine::startup_initial_file_from_command_line(&replay_parsed, false),
                None
            );

            let replay_alias_args = vec![
                "generals".to_string(),
                "-replay".to_string(),
                "Replays\\demo.rep".to_string(),
            ];
            let replay_alias_parsed = CommandLineArgs::parse_from_args(replay_alias_args).unwrap();
            assert_eq!(
                CnCGameEngine::startup_initial_file_from_command_line(&replay_alias_parsed, true),
                None
            );
        });
    }

    #[test]
    fn startup_initial_file_helper_prefers_runtime_initial_file_state() {
        with_global_data_snapshot_restored(|| {
            {
                let mut global = game_engine::common::global_data::write();
                global.writable.initial_file = "Replays\\runtime.rep".to_string();
            }

            let cli_args = vec![
                "generals".to_string(),
                "-file".to_string(),
                "Maps\\cli\\cli.map".to_string(),
            ];
            let parsed = CommandLineArgs::parse_from_args(cli_args).unwrap();

            assert_eq!(
                CnCGameEngine::startup_initial_file_from_command_line(&parsed, true),
                Some("Replays\\runtime.rep".to_string())
            );
        });
    }

    #[test]
    fn startup_initial_file_split_matches_cpp_suffix_rules() {
        let (map_file, replay_file) =
            CnCGameEngine::split_startup_initial_file(Some("Maps\\Test\\Test.map".to_string()));
        assert_eq!(map_file, Some("Maps\\Test\\Test.map".to_string()));
        assert!(replay_file.is_none());

        let (map_file, replay_file) =
            CnCGameEngine::split_startup_initial_file(Some("Replays\\demo.rep".to_string()));
        assert!(map_file.is_none());
        assert_eq!(replay_file, Some("Replays\\demo.rep".to_string()));
    }

    #[test]
    fn apply_command_line_overrides_keeps_initial_map_side_effects_until_startup_handling() {
        with_global_data_snapshot_restored(|| {
            let args = vec![
                "generals".to_string(),
                "-file".to_string(),
                "Maps\\Test\\Test.map".to_string(),
            ];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            CnCGameEngine::apply_command_line_overrides(&parsed);

            let global = game_engine::common::global_data::read();
            assert_eq!(global.writable.initial_file, "Maps\\Test\\Test.map");
            assert!(global.pending_file.is_empty());
            assert!(global.writable.shell_map_on);
            assert!(global.writable.play_intro);
            assert!(!global.writable.after_intro);
        });
    }

    #[test]
    fn sync_after_intro_when_intro_disabled_marks_after_intro() {
        with_global_data_snapshot_restored(|| {
            {
                let mut global = game_engine::common::global_data::write();
                global.writable.play_intro = false;
                global.writable.after_intro = false;
            }

            CnCGameEngine::sync_after_intro_when_intro_disabled();

            let global = game_engine::common::global_data::read();
            assert!(!global.writable.play_intro);
            assert!(global.writable.after_intro);
        });
    }

    #[test]
    fn game_logic_gate_without_network_matches_cpp_pause_behavior() {
        assert!(CnCGameEngine::should_update_game_logic_frame(false, None));
        assert!(!CnCGameEngine::should_update_game_logic_frame(true, None));
    }

    #[test]
    fn game_logic_gate_with_network_uses_frame_ready_only() {
        assert!(CnCGameEngine::should_update_game_logic_frame(
            false,
            Some(true)
        ));
        assert!(CnCGameEngine::should_update_game_logic_frame(
            true,
            Some(true)
        ));
        assert!(!CnCGameEngine::should_update_game_logic_frame(
            false,
            Some(false)
        ));
        assert!(!CnCGameEngine::should_update_game_logic_frame(
            true,
            Some(false)
        ));
    }

    #[test]
    fn network_gate_skips_runtime_network_lookup_until_multiplayer_exists() {
        assert_eq!(CnCGameEngine::network_frame_data_ready_gate(false), None);
    }

    #[test]
    fn iconic_minimized_mode_keeps_network_sessions_running() {
        assert!(should_keep_logic_running_while_iconic(
            GameMode::Multiplayer
        ));
        assert!(should_keep_logic_running_while_iconic(GameMode::Lan));
        assert!(should_keep_logic_running_while_iconic(GameMode::Internet));
        assert!(!should_keep_logic_running_while_iconic(
            GameMode::SinglePlayer
        ));
        assert!(!should_keep_logic_running_while_iconic(GameMode::Skirmish));
        assert!(!should_keep_logic_running_while_iconic(GameMode::Shell));
    }

    #[test]
    fn command_line_fps_order_matches_cpp_fps_then_nofpslimit() {
        let args = vec![
            "generals".to_string(),
            "-fps".to_string(),
            "60".to_string(),
            "-nofpslimit".to_string(),
        ];
        let mut writable = game_engine::common::command_line::WritableGlobalData::default();
        CnCGameEngine::apply_fps_limit_overrides_from_raw_args(&args, &mut writable);
        assert!(!writable.use_fps_limit);
        assert_eq!(writable.frames_per_second_limit, 30000);
    }

    #[test]
    fn command_line_fps_order_matches_cpp_nofpslimit_then_fps() {
        let args = vec![
            "generals".to_string(),
            "-nofpslimit".to_string(),
            "-fps".to_string(),
            "60".to_string(),
        ];
        let mut writable = game_engine::common::command_line::WritableGlobalData::default();
        CnCGameEngine::apply_fps_limit_overrides_from_raw_args(&args, &mut writable);
        assert!(!writable.use_fps_limit);
        assert_eq!(writable.frames_per_second_limit, 60);
    }

    #[test]
    fn command_line_window_resolution_overrides_sync_to_writable_globals() {
        with_global_data_snapshot_restored(|| {
            let args = vec![
                "generals".to_string(),
                "-win".to_string(),
                "-xres".to_string(),
                "1280".to_string(),
                "-yres".to_string(),
                "720".to_string(),
            ];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            CnCGameEngine::apply_command_line_overrides(&parsed);

            let global = game_engine::common::global_data::read();
            assert!(global.writable.windowed);
            assert_eq!(global.writable.x_resolution, 1280);
            assert_eq!(global.writable.y_resolution, 720);
        });
    }

    #[test]
    fn command_line_noaudio_overrides_sync_to_writable_globals() {
        with_global_data_snapshot_restored(|| {
            let args = vec!["generals".to_string(), "-noaudio".to_string()];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            CnCGameEngine::apply_command_line_overrides(&parsed);

            let global = game_engine::common::global_data::read();
            assert!(!global.writable.audio_on);
            assert!(!global.writable.speech_on);
            assert!(!global.writable.sounds_on);
            assert!(!global.writable.music_on);
        });
    }

    #[test]
    fn command_line_startup_parity_flags_apply_in_argv_order() {
        with_global_data_snapshot_restored(|| {
            let args = vec![
                "generals".to_string(),
                "-particleEdit".to_string(),
                "-fullscreen".to_string(),
                "-benchmark".to_string(),
                "9".to_string(),
                "-playStats".to_string(),
                "4".to_string(),
                "-seed".to_string(),
                "-1".to_string(),
                "-netMinPlayers".to_string(),
                "3".to_string(),
                "-forceBenchmark".to_string(),
                "-nomusic".to_string(),
                "-noshaders".to_string(),
                "-scriptDebug".to_string(),
                "-winCursors".to_string(),
                "-constantDebug".to_string(),
                "-showTeamDot".to_string(),
                "-nomovecamera".to_string(),
                "-NoShellAnim".to_string(),
            ];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            CnCGameEngine::apply_command_line_overrides(&parsed);

            let global = game_engine::common::global_data::read();
            assert!(!global.writable.windowed);
            assert!(global.writable.particle_edit);
            assert!(global.writable.script_debug);
            assert!(global.writable.win_cursors);
            assert!(!global.writable.animate_windows);
            assert!(!global.writable.music_on);
            assert!(global.writable.play_sizzle);
            assert_eq!(global.writable.chip_set_type, 1);
            assert!(global.writable.force_benchmark);
            assert!(global.writable.constant_debug_update);
            assert!(global.writable.show_team_dot);
            assert!(global.writable.disable_camera_movement);
            assert_eq!(global.writable.fixed_seed, -1);
            assert_eq!(global.writable.net_min_players, 3);
            assert_eq!(global.writable.benchmark_timer, 9);
            assert_eq!(global.writable.play_stats, 4);
        });
    }

    #[test]
    fn command_line_standalone_nosizzle_is_ignored_during_startup_overrides() {
        with_global_data_snapshot_restored(|| {
            let args = vec!["generals".to_string(), "-nosizzle".to_string()];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            CnCGameEngine::apply_command_line_overrides(&parsed);

            let global = game_engine::common::global_data::read();
            assert!(global.writable.play_sizzle);
        });
    }

    #[test]
    fn command_line_jump_to_frame_matches_cpp_no_draw_behavior() {
        with_global_data_snapshot_restored(|| {
            let args = vec![
                "generals".to_string(),
                "-jumpToFrame".to_string(),
                "240".to_string(),
            ];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            CnCGameEngine::apply_command_line_overrides(&parsed);

            let global = game_engine::common::global_data::read();
            let debug_gated = CnCGameEngine::allow_debug_startup_flags();
            assert_eq!(global.writable.no_draw, debug_gated);
            if debug_gated {
                assert!(!global.writable.use_fps_limit);
                assert_eq!(global.writable.frames_per_second_limit, 30000);
            }
        });
    }

    #[test]
    fn startup_water_weather_preload_paths_match_cpp_order() {
        assert_eq!(
            CnCGameEngine::startup_water_weather_ini_paths(),
            [
                "Data/INI/Default/Water.ini",
                "Data/INI/Water.ini",
                "Data/INI/Default/Weather.ini",
                "Data/INI/Weather.ini",
            ]
        );
    }

    #[test]
    fn startup_ai_data_preload_paths_match_cpp_order() {
        assert_eq!(
            CnCGameEngine::startup_ai_data_ini_paths(),
            ["Data/INI/Default/AIData.ini", "Data/INI/AIData.ini",]
        );
    }

    #[test]
    fn startup_audio_failure_quits_only_when_audio_is_enabled() {
        assert!(CnCGameEngine::startup_audio_should_quit(false, false));
        assert!(!CnCGameEngine::startup_audio_should_quit(true, false));
        assert!(!CnCGameEngine::startup_audio_should_quit(false, true));
    }

    #[test]
    fn debug_startup_flag_gating_matches_build_mode() {
        assert_eq!(
            CnCGameEngine::allow_debug_startup_flags(),
            cfg!(any(debug_assertions, feature = "internal"))
        );
    }

    #[test]
    fn command_line_map_override_syncs_to_writable_globals() {
        with_global_data_snapshot_restored(|| {
            let args = vec![
                "generals".to_string(),
                "-map".to_string(),
                "Maps\\ShellMap1.map".to_string(),
            ];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            CnCGameEngine::apply_command_line_overrides(&parsed);

            let global = game_engine::common::global_data::read();
            assert_eq!(global.writable.map_name, "Maps\\ShellMap1\\ShellMap1.map");
        });
    }

    #[test]
    fn command_line_mod_override_updates_active_mod_and_loads_best_effort() {
        with_global_data_snapshot_restored(|| {
            let temp_root = create_temp_test_dir("mod_override");
            let user_data_dir = temp_root.join("UserData");
            let mod_dir = user_data_dir.join("Mods").join("TestMod");
            std::fs::create_dir_all(&mod_dir).unwrap();

            {
                let mut global = game_engine::common::global_data::write();
                global.set_user_data_dir(user_data_dir.to_string_lossy().into_owned());
            }

            let args = vec![
                "generals".to_string(),
                "-mod".to_string(),
                std::path::Path::new("Mods")
                    .join("TestMod")
                    .to_string_lossy()
                    .into_owned(),
            ];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            CnCGameEngine::apply_command_line_overrides(&parsed);

            let expected = format!("{}{}", mod_dir.to_string_lossy(), std::path::MAIN_SEPARATOR);
            let global = game_engine::common::global_data::read();
            assert_eq!(global.writable.mod_dir, expected);
            assert!(global.writable.mod_big.is_empty());
            assert_eq!(
                global
                    .get_override("active_mod")
                    .and_then(|value| value.as_str()),
                Some(expected.as_str())
            );

            let _ = fs::remove_dir_all(temp_root);
        });
    }

    #[test]
    fn command_line_update_images_sets_writable_flag() {
        with_global_data_snapshot_restored(|| {
            let args = vec!["generals".to_string(), "-updateimages".to_string()];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            CnCGameEngine::apply_command_line_overrides(&parsed);

            let global = game_engine::common::global_data::read();
            assert!(global.writable.should_update_tga_to_dds);
        });
    }

    #[test]
    fn command_line_update_images_alias_is_case_insensitive() {
        with_global_data_snapshot_restored(|| {
            let args = vec!["generals".to_string(), "-UpDaTeDdS".to_string()];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            CnCGameEngine::apply_command_line_overrides(&parsed);

            let global = game_engine::common::global_data::read();
            assert!(global.writable.should_update_tga_to_dds);
        });
    }
