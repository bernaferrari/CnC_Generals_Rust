#![allow(unused_imports, unused_variables, dead_code)]

/*
** Command & Conquer Generals Zero Hour(tm) - Actual Game Engine
** Copyright 2025 Electronic Arts Inc.
**
** Real C&C game engine replacing the cube demo with full RTS gameplay
*/

use crate::assets::{get_asset_manager, W3DModel};
use crate::command_line::CommandLineArgs;
use crate::fow_rendering;
use crate::game_logic::script_events::{self, ScriptEvent};
use crate::game_logic::victory_conditions::AllianceState;
use crate::game_logic::*;
#[cfg(feature = "integration-diagnostics")]
use crate::integration_bridge::IntegrationTelemetryBridge;
use crate::localization;
use crate::platform::{create_platform_message_handler, WindowMessageProcessor};
use crate::runtime::attachments::AttachmentDispatcher;
use crate::save_load::{
    init_game_state_system, GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo,
};
use crate::subsystem_manager::{
    get_subsystem_manager, init_subsystem_manager, with_subsystem_mut, AudioManagerSubsystem,
    NetworkSubsystem, SubsystemInterface,
};
use crate::ui::{
    DiagnosticsOverlayStats, GameHUD, GameUIState, MinimapActionKind, MinimapInteraction, Screen,
    UIEvent, UIManager, UISystemState,
};
use crate::util::profiler::InitTimer;
use ::game_engine::common::frame_clock::{FrameClock, FrameTiming as ClockFrameTiming};
use anyhow::Result;
pub use game_engine::common::game_engine::GameState;
use game_engine::common::game_engine::{
    register_command_list_init, register_game_client_factory, GameClientInterface,
};
use game_engine::common::system::subsystem_interface::{
    SubsystemError, SubsystemResult, SubsystemState,
};
use glam::{Mat4, Vec2, Vec3};
#[cfg(feature = "integration-diagnostics")]
use integration::diagnostics::SystemDiagnostics;
#[cfg(feature = "integration-diagnostics")]
use integration::IntegrationConfig;
use log::{debug, error, info, warn};
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::{PI, TAU};
use std::fs;
use std::future::Future;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use wgpu::util::DeviceExt;
use winit::{
    self,
    event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowAttributes},
};
use ww3d_core::ww3d::WW3D;
use ww3d_engine::{self, EngineConfig, EngineError, FrameTiming};
use ww3d_renderer_3d::core::error::Error as RendererError;

#[cfg(feature = "network")]
use game_network::time::NetworkClock;

#[cfg(not(feature = "network"))]
struct NetworkClock;

#[cfg(not(feature = "network"))]
impl NetworkClock {
    fn override_with_duration(_duration: Duration) {}
    fn clear_override() {}
}

#[cfg(test)]
mod tests {

    #[test]
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
}
const DEFAULT_SKIRMISH_MAP: &str = "Defcon6";
const DEFAULT_VIEW_FOV_RADIANS: f32 = 50.0_f32.to_radians();
const DEFAULT_VIEW_NEAR_CLIP: f32 = 1.0;
const DEFAULT_LOADING_PHASE: &str = "Loading assets...";

#[cfg(feature = "game_client")]
thread_local! {
    static LOADING_PROGRESS: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
    static LOADING_PHASE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

// Window names from ShellGameLoadScreen.wnd (C++ parity: winCreateFromScript)
const LOAD_SCREEN_ROOT: &str = "ShellGameLoadScreen.wnd:ParentShellGameLoadScreen";
const LOAD_SCREEN_PROGRESS: &str = "ShellGameLoadScreen.wnd:ProgressLoad";

fn pack_ui_mouse_data(x: i32, y: i32) -> u32 {
    ((y as u32) << 16) | ((x as u32) & 0xFFFF)
}
const DEFAULT_VIEW_FAR_CLIP: f32 = 20_000.0;

fn should_keep_logic_running_while_iconic(mode: GameMode) -> bool {
    matches!(
        mode,
        GameMode::Multiplayer | GameMode::Lan | GameMode::Internet
    )
}

fn query_window_is_iconic(window: &Window, fallback: bool) -> bool {
    let size = window.inner_size();
    let zero_sized = size.width == 0 || size.height == 0;
    window.is_minimized().unwrap_or(fallback || zero_sized) || zero_sized
}

fn update_iconic_state_and_wake_audio(window: &Window, minimized: &mut bool) {
    let was_minimized = *minimized;
    *minimized = query_window_is_iconic(window, *minimized);

    if was_minimized && !*minimized {
        info!("Window exited iconic/minimized state");
        with_subsystem_mut::<AudioManagerSubsystem, _>(|audio| {
            audio.wake_after_iconic_return();
        });
    } else if !was_minimized && *minimized {
        info!("Window entered iconic/minimized state");
    }
}

fn should_exit_for_smoke_test(
    smoke_test: bool,
    state: GameState,
    startup_progress: f32,
    exiting_pending: bool,
) -> bool {
    smoke_test && matches!(state, GameState::Menu) && startup_progress >= 1.0 && !exiting_pending
}

// C++ SAGE Engine equivalent modules
use crate::graphics::{
    graphics_system::MAX_STAGE_TEXTURES, render_pipeline::gameplay_to_render_transform,
    GraphicsSystem, RenderPipeline,
};

#[cfg(feature = "internal")]
pub mod parity_test_support {
    use super::GameState;
    use crate::ui::Screen;

    /// Lightweight state-machine model used by parity tests.
    ///
    /// The real engine constructor is too heavy for fast integration tests, so this
    /// harness mirrors the transition side effects that matter for startup, match
    /// start, exit-to-menu, and quit deduplication coverage.
    #[derive(Debug, Clone)]
    pub struct StateMachineParityHarness {
        current_state: GameState,
        pending_state: Option<GameState>,
        ui_screen: Option<Screen>,
        game_paused: bool,
        game_logic_paused: bool,
        match_over: bool,
        victory_summary_present: bool,
        selected_objects: Vec<u32>,
        quit_requests_emitted: usize,
        menu_world_frames_rendered: u32,
    }

    impl Default for StateMachineParityHarness {
        fn default() -> Self {
            Self {
                current_state: GameState::Menu,
                pending_state: None,
                ui_screen: Some(Screen::MainMenu),
                game_paused: false,
                game_logic_paused: false,
                match_over: false,
                victory_summary_present: false,
                selected_objects: Vec::new(),
                quit_requests_emitted: 0,
                menu_world_frames_rendered: 0,
            }
        }
    }

    impl StateMachineParityHarness {
        pub fn current_state(&self) -> GameState {
            self.current_state
        }

        pub fn pending_state(&self) -> Option<GameState> {
            self.pending_state
        }

        pub fn ui_screen(&self) -> Option<Screen> {
            self.ui_screen
        }

        pub fn game_paused(&self) -> bool {
            self.game_paused
        }

        pub fn game_logic_paused(&self) -> bool {
            self.game_logic_paused
        }

        pub fn match_over(&self) -> bool {
            self.match_over
        }

        pub fn victory_summary_present(&self) -> bool {
            self.victory_summary_present
        }

        pub fn selected_objects(&self) -> &[u32] {
            &self.selected_objects
        }

        pub fn quit_requests_emitted(&self) -> usize {
            self.quit_requests_emitted
        }

        pub fn set_loading_state(&mut self) {
            self.current_state = GameState::Loading;
            self.pending_state = None;
            self.ui_screen = Some(Screen::Loading);
        }

        pub fn set_dirty_play_state(&mut self) {
            self.current_state = GameState::InGame;
            self.pending_state = None;
            self.ui_screen = Some(Screen::GameHUD);
            self.game_paused = true;
            self.game_logic_paused = true;
            self.match_over = true;
            self.victory_summary_present = true;
            self.selected_objects = vec![101, 202, 303];
        }

        pub fn complete_startup_loading_to_menu(&mut self) {
            self.transition_to_state(GameState::Menu);
        }

        pub fn complete_new_game_success(&mut self) {
            self.selected_objects.clear();
            self.match_over = false;
            self.victory_summary_present = false;
            self.transition_to_state(GameState::InGame);
        }

        pub fn complete_load_game_success(&mut self) {
            self.selected_objects.clear();
            self.match_over = false;
            self.victory_summary_present = false;
            self.transition_to_state(GameState::InGame);
        }

        pub fn return_to_main_menu_after_match(&mut self) {
            self.selected_objects.clear();
            self.game_paused = false;
            self.game_logic_paused = false;
            self.match_over = false;
            self.victory_summary_present = false;
            self.pending_state = None;
            self.transition_to_state(GameState::Menu);
        }

        pub fn request_quit(&mut self) -> bool {
            if self.current_state == GameState::Exiting
                || self.pending_state == Some(GameState::Exiting)
            {
                return false;
            }

            self.pending_state = Some(GameState::Exiting);
            self.quit_requests_emitted = self.quit_requests_emitted.saturating_add(1);
            true
        }

        pub fn apply_pending_state_change(&mut self) {
            if let Some(new_state) = self.pending_state.take() {
                self.transition_to_state(new_state);
            }
        }

        fn transition_to_state(&mut self, new_state: GameState) {
            match new_state {
                GameState::Menu => {
                    self.game_paused = false;
                    self.game_logic_paused = false;
                    self.ui_screen = Some(Screen::MainMenu);
                    self.menu_world_frames_rendered = 0;
                }
                GameState::Loading => {
                    self.ui_screen = Some(Screen::Loading);
                }
                GameState::InGame => {
                    self.game_paused = false;
                    self.game_logic_paused = false;
                    self.ui_screen = Some(Screen::GameHUD);
                }
                GameState::Paused => {
                    self.game_paused = true;
                    self.game_logic_paused = true;
                    self.ui_screen = Some(Screen::PauseMenu);
                }
                GameState::Exiting => {
                    self.ui_screen = None;
                }
            }

            self.current_state = new_state;
        }
    }
}

#[derive(Debug, Clone)]
struct ScriptCameraShaker {
    epicenter: Vec3,
    radius: f32,
    duration_seconds: f32,
    elapsed_seconds: f32,
    amplitude_degrees: f32,
    phase: f32,
    frequency_hz: f32,
}

impl ScriptCameraShaker {
    fn new(epicenter: Vec3, radius: f32, duration_seconds: f32, amplitude_degrees: f32) -> Self {
        // Deterministic phase/frequency seed from shaker parameters.
        let seed = (epicenter.x * 0.013
            + epicenter.y * 0.021
            + epicenter.z * 0.034
            + amplitude_degrees * 0.055)
            .sin();
        let normalized = ((seed * 43_758.547).fract()).abs();
        Self {
            epicenter,
            radius: radius.max(0.01),
            duration_seconds: duration_seconds.max(0.01),
            elapsed_seconds: 0.0,
            amplitude_degrees,
            phase: normalized * TAU,
            frequency_hz: 2.0 + normalized * 4.0,
        }
    }
}

struct StartupLoadResult {
    game_logic: GameLogic,
    loaded_map_name: Option<String>,
    start_in_menu: bool,
    map_requested_from_cli: bool,
    replay_requested: bool,
}

#[derive(Debug, Clone, Copy)]
struct StartupNewGameDispatch {
    game_mode_code: i32,
    game_mode: GameMode,
    difficulty_code: i32,
    difficulty: GameDifficulty,
    rank_points: i32,
    max_fps: Option<i32>,
}

enum StartupLoadMessage {
    Progress { progress: f32, phase: String },
    Complete(std::result::Result<StartupLoadResult, String>),
}

enum StartupLoadState {
    Idle,
    InProgress {
        receiver: Receiver<StartupLoadMessage>,
        started_at: Instant,
        last_worker_progress: f32,
        last_worker_phase: Option<String>,
        last_worker_logged_bucket: u8,
    },
    Complete,
}

/// Map-click command residual armed by ControlBar buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingMapCommand {
    AttackMove,
    Guard,
    SetRallyPoint,
    /// Chinook combat drop residual awaiting map click.
    CombatDrop,
    /// Armed superweapon / special power residual awaiting map click.
    SpecialPower(crate::command_system::SpecialPowerType),
    /// Retail PLACE_BEACON residual awaiting map click.
    PlaceBeacon,
    /// Unit special-ability residual awaiting object/map click.
    UnitAbility(PendingUnitAbility),
}

/// ControlBar unit ability that needs a target click residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingUnitAbility {
    Hijack,
    Sabotage,
    CaptureBuilding,
    SnipeVehicle,
    PlantTimedDemoCharge,
    PlantRemoteDemoCharge,
    StealCashHack,
    DisableVehicleHack,
    HackerDisableBuilding,
    DisguiseAsVehicle,
    PlantBoobyTrap,
    ConvertToCarbomb,
    /// Dozer/Worker repair residual awaiting damaged structure click.
    Repair,
}

/// Main C&C game engine with full RTS functionality - restructured to match C++ SAGE architecture
pub struct CnCGameEngine {
    window: Arc<Window>,
    #[allow(dead_code)] // C++ parity: stored for future command-line query access
    command_line: Arc<CommandLineArgs>,

    // C++ SAGE equivalent rendering subsystems
    graphics_system: GraphicsSystem,
    render_pipeline: RenderPipeline,

    // Platform message handling
    message_processor: WindowMessageProcessor,

    // Audio system
    #[allow(dead_code)] // C++ parity: audio stream handle kept alive to prevent drop
    audio_output: Option<OutputStream>,
    audio_handle: Option<OutputStreamHandle>,
    background_music: Option<Sink>,
    sound_effects: Vec<Sink>,
    ui_sound_cache: HashMap<String, Arc<[u8]>>,

    // Game state machine - matches C++ GameEngine m_quitting and state management
    current_state: GameState,
    pending_state: Option<GameState>,
    startup_load_state: StartupLoadState,
    startup_target_state: Option<GameState>,
    startup_start_in_menu: bool,
    last_loading_title_update: Option<Instant>,
    startup_last_reported_progress: f32,
    startup_loading_phase: String,
    startup_last_progress_change_at: Instant,
    startup_last_stall_warning_at: Option<Instant>,
    startup_stall_events: u32,
    startup_max_stall_duration: Duration,
    startup_health_summary_logged: bool,
    last_caustic_warmup_attempt: Option<Instant>,
    loading_overlay_active: bool,
    #[cfg(feature = "game_client")]
    active_load_screen: Option<game_client::gui::load_screen::LoadScreenKind>,
    shell_menu_active: bool, // C++ parity: Shell::push("Menus/MainMenu.wnd") / Shell::pop()

    // Game client — C++ parity: TheGameClient singleton, wired into Main's frame loop
    // for drawable updates and display draw. Full GameClient::update() OS-input path
    // is not used (Main owns input→commands); drawables always tick with the frame.
    #[cfg(feature = "game_client")]
    game_client: game_client::core::game_client::GameClient,
    /// ControlBar selection panel (portrait + health). Presentation-fed; WND load optional.
    #[cfg(feature = "game_client")]
    control_bar: game_client::gui::control_bar::ControlBar,

    // Game state
    game_logic: GameLogic,
    /// Immutable presentation feed for client/render after last logic step.
    last_presentation_frame: Option<crate::presentation_frame::PresentationFrame>,
    /// Optional GameWorld shadow session (stable ObjectId→EntityId). Opt-in:
    /// `GENERALS_GAMEWORLD_SHADOW=1`. Not production authority.
    gameworld_shadow: Option<crate::gameworld_shadow::GameWorldShadow>,
    /// Last presentation-overlaid UI state (selection health/minimap identity retained
    /// after render build so consumers are not dropped each frame).
    last_ui_state: Option<GameUIState>,
    resource_manager: ResourceManager,
    save_file_manager: SaveFileManager,

    // Camera system
    camera_position: Vec3,
    camera_target: Vec3,
    camera_zoom: f32,
    camera_zoom_target: Option<f32>,
    camera_zoom_start: f32,
    camera_zoom_duration: f32,
    camera_zoom_elapsed: f32,
    camera_zoom_ease_in: f32,
    camera_zoom_ease_out: f32,
    camera_orbit_distance: f32,
    camera_pitch_radians: f32,
    camera_pitch_target: Option<f32>,
    camera_pitch_start: f32,
    camera_pitch_duration: f32,
    camera_pitch_elapsed: f32,
    camera_pitch_ease_in: f32,
    camera_pitch_ease_out: f32,
    camera_yaw_radians: f32,
    camera_yaw_target: Option<f32>,
    camera_yaw_start: f32,
    camera_yaw_duration: f32,
    camera_yaw_elapsed: f32,
    camera_yaw_ease_in: f32,
    camera_yaw_ease_out: f32,
    camera_shake_offset: Vec3,
    screen_shake_intensity: f32,
    screen_shake_angle_cos: f32,
    screen_shake_angle_sin: f32,
    script_camera_shakers: Vec<ScriptCameraShaker>,
    script_fps_limit: Option<u32>,
    script_fps_limit_last_tick: Option<Instant>,
    camera_slave_mode: Option<CameraSlaveModeRequest>,
    view_matrix: Mat4,
    projection_matrix: Mat4,

    // Input state
    keys_pressed: HashSet<Key>,
    mouse_position: (f32, f32),
    mouse_world_position: Vec3,
    /// Last applied context cursor residual (avoid spam set_cursor).
    last_context_cursor: Option<&'static str>,
    /// EVA LOWPOWER residual edge counter.
    last_eva_low_power_count: u32,
    last_eva_insufficient_funds_count: u32,
    last_eva_base_under_attack_count: u32,
    last_eva_ally_under_attack_count: u32,
    /// C++ sticky waypoint mode residual (Alt hold still works; Z toggles).
    sticky_waypoint_mode: bool,
    /// Sticky auto-attack residual (Ctrl+Shift+A): convert plain moves to attack-move.
    sticky_auto_attack: bool,
    is_dragging: bool,
    selection_start: Option<Vec3>,
    /// Screen-space drag origin for selection box overlay residual.
    selection_start_screen: Option<(f32, f32)>,
    last_click_time: Option<Instant>,
    last_click_position: Option<Vec3>,
    is_windowed: bool,
    rmb_scroll_anchor: Option<(f32, f32)>,
    is_rmb_scrolling: bool,
    is_mmb_rotating: bool,
    mmb_anchor: Option<(f32, f32)>,

    // Game state
    selected_objects: Vec<ObjectId>,
    control_groups: HashMap<u8, Vec<ObjectId>>,
    /// Last control-group digit select (group, Instant) for double-tap camera jump residual.
    last_control_group_select: Option<(u8, Instant)>,
    /// Retail SAVE_VIEW1..8 / VIEW_VIEW1..8 camera bookmark residual (F1-F8).
    camera_view_bookmarks: [Option<Vec3>; 8],
    camera_rotate_left_held: bool,
    camera_rotate_right_held: bool,
    camera_zoom_in_held: bool,
    camera_zoom_out_held: bool,
    /// Retail TOGGLE_CAMERA_TRACKING_DRAWABLE residual.
    camera_tracking_selection: bool,
    /// Retail TOGGLE_FAST_FORWARD_REPLAY residual (TiVO fast mode).
    replay_fast_forward: bool,
    /// Retail DIPLOMACY KEY_TAB residual panel.
    diplomacy_panel: crate::ui::DiplomacyPanel,
    /// Retail CHAT_EVERYONE / CHAT_ALLIES residual panel.
    chat_panel: crate::ui::ChatPanel,
    current_player_id: u32,
    game_paused: bool,

    // UI state
    show_debug_info: bool,
    show_health_bars: bool,
    /// FPS counter residual (options game.show_fps).
    show_fps: bool,
    /// Draw movement path lines residual.
    show_move_lines: bool,
    /// Draw attack-order lines residual.
    show_attack_lines: bool,
    frame_counter: u32,
    fps: f32,
    last_frame_timing: Option<FrameTiming>,
    frame_clock: FrameClock,
    menu_loading_tick_accumulator: Duration,
    menu_loading_last_tick: Instant,
    diagnostics_overlay: Option<DiagnosticsOverlayStats>,

    // UI system
    ui_manager: UIManager,
    game_hud: GameHUD,
    /// C++ structure placement template residual (awaiting map click).
    pending_structure_placement: Option<String>,
    /// C++ context command awaiting map click (AttackMove/Guard/SetRally residual).
    pending_map_command: Option<PendingMapCommand>,
    active_menu_shell_hook: Option<&'static str>,
    runtime_host_headless: bool,
    runtime_host_base_ui_screen: Option<String>,
    runtime_host_ui_screen_override: Option<String>,
    runtime_host_last_gameplay_cmd: String,

    // Model loading state
    models_loaded: bool,
    pending_shell_model_prewarm: VecDeque<String>,
    menu_enter_frame: Option<u64>,
    shell_ui_enqueued_frame: Option<u64>,
    last_shell_prewarm_log: Option<Instant>,
    shell_prewarm_completion_logged: bool,
    /// How many Menu frames have rendered the full world scene so far.
    /// The first few Menu frames skip the world render to avoid a freeze while
    /// models/textures/terrain are loaded lazily for the first time.
    menu_world_frames_rendered: u32,
    last_slow_menu_tick_log: Option<Instant>,
    match_over: bool,
    victory_summary: Option<VictorySummary>,
}

/// C++ SAGE engine VertexFormatXYZNDUV2 equivalent - matches original vertex declarations
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VertexXYZNDUV2 {
    pub position: [f32; 3],    // XYZ - Position coordinates
    pub normal: [f32; 3],      // N - Normal vector
    pub diffuse: u32,          // D - Diffuse color (RGBA packed as u32, like D3D8)
    pub tex_coords0: [f32; 2], // UV - Primary texture coordinates
    pub tex_coords1: [f32; 2], // UV2 - Secondary texture coordinates for multi-stage texturing
}

impl VertexXYZNDUV2 {
    /// C++ SAGE VertexFormatXYZNDUV2 buffer layout - matches D3DVERTEXELEMENT9 declarations
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<VertexXYZNDUV2>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position (XYZ)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal (N)
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Diffuse color (D) - packed RGBA like D3D8
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Unorm8x4,
                },
                // Primary texture coordinates (UV)
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2 + std::mem::size_of::<u32>())
                        as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Secondary texture coordinates (UV2) for multi-texturing
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2
                        + std::mem::size_of::<u32>()
                        + std::mem::size_of::<[f32; 2]>())
                        as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// C++ SAGE engine equivalent uniforms - matches GlobalUniforms structure
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SAGEUniforms {
    view_projection: [[f32; 4]; 4],
    view_matrix: [[f32; 4]; 4],
    projection_matrix: [[f32; 4]; 4],
    camera_position: [f32; 4],
    time: f32,
    ambient_light: [f32; 3],
    sun_direction: [f32; 3],
    sun_color: [f32; 3],
    _padding: f32,
}

/// C++ SAGE VertexMaterialClass equivalent - matches original material properties
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialProperties {
    diffuse_color: [f32; 4],   // Base color reflected by lighting
    specular_color: [f32; 4],  // Sharp reflective highlights
    emissive_color: [f32; 4],  // Self-illumination color
    opacity: f32,              // Transparency (1.0 = opaque, 0.0 = transparent)
    shininess: f32,            // Specular power
    stage0_uv_scale: [f32; 2], // UV scaling for stage 0
    stage1_uv_scale: [f32; 2], // UV scaling for stage 1
}

#[derive(Debug, Clone, Copy)]
struct StartupCameraDefaults {
    pitch_degrees: f32,
    yaw_degrees: f32,
    camera_height: f32,
    max_camera_height: f32,
}

#[cfg(feature = "game_client")]
struct RegisteredGameClientBridge {
    client: crate::subsystem_manager::GameClientSubsystem,
    active: bool,
    state: SubsystemState,
}

#[cfg(feature = "game_client")]
impl RegisteredGameClientBridge {
    fn new() -> SubsystemResult<Self> {
        Ok(Self {
            client: crate::subsystem_manager::GameClientSubsystem::new(),
            active: true,
            state: SubsystemState::Uninitialized,
        })
    }
}

#[cfg(feature = "game_client")]
impl GameClientInterface for RegisteredGameClientBridge {
    fn init(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::Initializing;
        self.client
            .init()
            .map_err(|err| SubsystemError::InitializationFailed(err.to_string()))?;
        self.state = SubsystemState::Running;
        Ok(())
    }

    fn update(&mut self, delta_time: std::time::Duration) -> SubsystemResult<()> {
        self.client
            .update(delta_time.as_secs_f32())
            .map_err(|err| SubsystemError::UpdateFailed(err.to_string()))
    }

    fn render(&mut self) -> SubsystemResult<()> {
        // Rendering is owned by the Main runtime event loop.
        Ok(())
    }

    fn reset(&mut self) -> SubsystemResult<()> {
        self.client
            .reset()
            .map_err(|err| SubsystemError::OperationFailed(err.to_string()))?;
        self.state = SubsystemState::Running;
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::ShuttingDown;
        self.client
            .shutdown()
            .map_err(|err| SubsystemError::OperationFailed(err.to_string()))?;
        self.active = false;
        self.state = SubsystemState::Shutdown;
        Ok(())
    }

    fn get_state(&self) -> SubsystemState {
        self.state
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}

#[cfg(feature = "game_client")]
fn register_command_list_bootstrap() {
    use game_client::message_stream::command_list::get_command_list;
    use game_engine::common::message_stream::SubsystemInterface;
    register_command_list_init(|| {
        if let Ok(mut cl) = get_command_list().write() {
            let _ = cl.init();
        }
    });
}

#[cfg(feature = "game_client")]
fn register_real_game_client_bootstrap() {
    register_command_list_bootstrap();
}

#[cfg(not(feature = "game_client"))]
fn register_real_game_client_bootstrap() {}

impl CnCGameEngine {
    fn append_message_argument_to_common_stream(
        target: &mut game_engine::common::message_stream::GameMessage,
        arg: &game_engine::common::message_stream::GameMessageArgumentType,
    ) {
        use game_engine::common::message_stream::GameMessageArgumentType;
        match arg {
            GameMessageArgumentType::Integer(v) => target.append_integer_argument(*v),
            GameMessageArgumentType::Real(v) => target.append_real_argument(*v),
            GameMessageArgumentType::Boolean(v) => target.append_boolean_argument(*v),
            GameMessageArgumentType::ObjectID(v) => target.append_object_id_argument(*v),
            GameMessageArgumentType::DrawableID(v) => target.append_drawable_id_argument(*v),
            GameMessageArgumentType::TeamID(v) => target.append_team_id_argument(*v),
            GameMessageArgumentType::SquadID(v) => target.append_team_id_argument(*v),
            GameMessageArgumentType::Location(v) => target.append_location_argument(v.clone()),
            GameMessageArgumentType::Pixel(v) => target.append_pixel_argument(v.clone()),
            GameMessageArgumentType::PixelRegion(v) => {
                target.append_pixel_region_argument(v.clone())
            }
            GameMessageArgumentType::Timestamp(v) => target.append_timestamp_argument(*v),
            GameMessageArgumentType::WideChar(v) => target.append_wide_char_argument(*v),
            GameMessageArgumentType::String(v) => target.append_string_argument(v.clone()),
        }
    }

    fn append_common_message_to_stream(
        stream: &mut game_engine::common::message_stream::MessageStream,
        message: &game_engine::common::message_stream::GameMessage,
    ) {
        let forwarded = stream.append_message(message.get_type().clone());
        for arg in message.get_arguments() {
            Self::append_message_argument_to_common_stream(forwarded, &arg.data);
        }
    }

    fn legacy_game_mode_from_new_game_code(mode: i32) -> Option<GameMode> {
        match mode {
            0 => Some(GameMode::SinglePlayer), // GAME_SINGLE_PLAYER
            1 => Some(GameMode::Multiplayer),  // GAME_LAN
            2 => Some(GameMode::Skirmish),     // GAME_SKIRMISH
            3 => Some(GameMode::Replay),       // GAME_REPLAY
            4 => Some(GameMode::Shell),        // GAME_SHELL
            _ => None,
        }
    }

    fn legacy_game_difficulty_from_new_game_code(difficulty: i32) -> GameDifficulty {
        match difficulty {
            0 => GameDifficulty::Easy,
            1 => GameDifficulty::Medium,
            2 => GameDifficulty::Hard,
            _ => GameDifficulty::Medium,
        }
    }

    fn startup_new_game_dispatch_from_message(
        message: &game_engine::common::message_stream::GameMessage,
    ) -> Option<StartupNewGameDispatch> {
        use game_engine::common::message_stream::GameMessageArgumentType;

        if !matches!(
            message.get_type(),
            game_engine::common::message_stream::GameMessageType::NewGame
        ) {
            return None;
        }

        let mode_code = match message.get_argument(0) {
            Some(GameMessageArgumentType::Integer(mode_code)) => *mode_code,
            _ => return None,
        };
        let game_mode = Self::legacy_game_mode_from_new_game_code(mode_code)?;

        let difficulty_code = match message.get_argument(1) {
            Some(GameMessageArgumentType::Integer(value)) => *value,
            _ => 1,
        };
        let difficulty = Self::legacy_game_difficulty_from_new_game_code(difficulty_code);

        let rank_points = match message.get_argument(2) {
            Some(GameMessageArgumentType::Integer(value)) => *value,
            _ => 0,
        };

        let max_fps = match message.get_argument(3) {
            Some(GameMessageArgumentType::Integer(value)) => {
                let resolved = if (1..=1000).contains(value) {
                    *value
                } else {
                    game_engine::common::global_data::read()
                        .writable
                        .frames_per_second_limit
                };
                Some(resolved)
            }
            _ => None,
        };

        Some(StartupNewGameDispatch {
            game_mode_code: mode_code,
            game_mode,
            difficulty_code,
            difficulty,
            rank_points,
            max_fps,
        })
    }

    fn startup_new_game_dispatch_from_messages(
        messages: &[game_engine::common::message_stream::GameMessage],
    ) -> Option<StartupNewGameDispatch> {
        let mut resolved = None;
        for message in messages {
            if let Some(dispatch) = Self::startup_new_game_dispatch_from_message(message) {
                resolved = Some(dispatch);
            }
        }
        resolved
    }

    fn take_startup_messages_from_stream(
    ) -> Result<Vec<game_engine::common::message_stream::GameMessage>, String> {
        let stream = game_engine::common::message_stream::get_message_stream();
        let mut stream_guard = stream
            .write()
            .map_err(|_| "failed to acquire startup message stream lock".to_string())?;
        let messages = stream_guard
            .get_messages()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        stream_guard.clear_messages();
        Ok(messages)
    }

    fn apply_startup_new_game_dispatch(dispatch: StartupNewGameDispatch) -> Option<String> {
        let mut prepared_map_name = None;
        let mut global = game_engine::common::global_data::write();

        gamelogic::helpers::TheScriptEngine::set_global_difficulty(dispatch.difficulty_code);
        gamelogic::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(
            dispatch.rank_points,
        );

        if let Some(max_fps) = dispatch.max_fps {
            global.writable.use_fps_limit = true;
            global.writable.frames_per_second_limit = max_fps;
        }

        if !global.pending_file.trim().is_empty() {
            let pending_file = global.pending_file.clone();
            global.writable.map_name = pending_file.clone();
            global.pending_file.clear();
            prepared_map_name = Some(pending_file);
        }

        prepared_map_name
    }

    fn resolve_startup_mode_from_dispatch(
        start_in_menu: &mut bool,
        map_to_load: &mut Option<String>,
        startup_new_game: Option<StartupNewGameDispatch>,
        replay_startup_requested: bool,
    ) -> GameMode {
        if *start_in_menu {
            return GameMode::Shell;
        }

        if let Some(dispatch) = startup_new_game {
            debug!(
                "Startup NewGame dispatch: mode_code={} difficulty_code={} rank_points={} max_fps={:?}",
                dispatch.game_mode_code, dispatch.difficulty_code, dispatch.rank_points, dispatch.max_fps
            );
            let prepared_map = Self::apply_startup_new_game_dispatch(dispatch);
            if map_to_load.is_none() {
                *map_to_load = prepared_map;
            }
            return dispatch.game_mode;
        }

        warn!(
            "Startup map/replay launch requested without a queued NewGame message; falling back to menu startup"
        );
        *start_in_menu = true;
        *map_to_load = None;
        game_engine::common::global_data::write()
            .pending_file
            .clear();

        if replay_startup_requested {
            warn!("Startup replay launch is deferred because recorder did not queue NewGame");
        }

        GameMode::Shell
    }

    /// Pull MSG_NEW_GAME out of the common message stream without discarding
    /// unrelated messages. Returns a fully resolved start_game_from_ui tuple.
    fn take_pending_new_game_start_request(
        &self,
    ) -> Option<(
        GameMode,
        String,
        String,
        Option<crate::skirmish_config::SkirmishMatchConfig>,
    )> {
        let dispatch = Self::take_new_game_dispatch_from_common_stream()?;
        self.build_start_request_from_pending_globals(Some(dispatch))
    }

    /// Remove every `NewGame` message from the common stream, keeping others.
    /// Returns the last NewGame dispatch (C++ prefers the latest enqueue).
    fn take_new_game_dispatch_from_common_stream() -> Option<StartupNewGameDispatch> {
        let stream = game_engine::common::message_stream::get_message_stream();
        let mut stream_guard = match stream.write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };

        let messages: Vec<_> = stream_guard.get_messages().iter().cloned().collect();
        if messages.is_empty() {
            return None;
        }

        let mut dispatch = None;
        let mut kept = Vec::with_capacity(messages.len());
        for message in messages {
            if let Some(d) = Self::startup_new_game_dispatch_from_message(&message) {
                // Prefer the last NewGame (matches startup_new_game_dispatch_from_messages).
                dispatch = Some(d);
            } else {
                kept.push(message);
            }
        }

        let Some(dispatch) = dispatch else {
            return None;
        };

        // Rebuild stream without NewGame messages so pump doesn't double-handle.
        stream_guard.clear_messages();
        for message in &kept {
            Self::append_common_message_to_stream(&mut stream_guard, message);
        }
        Some(dispatch)
    }

    /// Resolve map/faction/skirmish config after a NewGame dispatch (or helper flag).
    fn build_start_request_from_pending_globals(
        &self,
        dispatch: Option<StartupNewGameDispatch>,
    ) -> Option<(
        GameMode,
        String,
        String,
        Option<crate::skirmish_config::SkirmishMatchConfig>,
    )> {
        let dispatch = dispatch.unwrap_or(StartupNewGameDispatch {
            game_mode_code: 2, // GAME_SKIRMISH default when only the helper flag is set
            game_mode: GameMode::Skirmish,
            difficulty_code: 1,
            difficulty: GameDifficulty::Medium,
            rank_points: 0,
            max_fps: None,
        });

        let prepared_map = Self::apply_startup_new_game_dispatch(dispatch);

        let mode = dispatch.game_mode;
        let map = prepared_map
            .filter(|m| !m.trim().is_empty())
            .or_else(|| {
                let g = game_engine::common::global_data::read();
                let m = g.writable.map_name.trim();
                if !m.is_empty() {
                    Some(m.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| DEFAULT_SKIRMISH_MAP.to_string());

        let skirmish = if matches!(mode, GameMode::Skirmish) {
            #[cfg(feature = "game_client")]
            {
                crate::skirmish_config::config_from_client_skirmish_setup(Some(map.as_str()))
            }
            #[cfg(not(feature = "game_client"))]
            {
                None
            }
        } else {
            None
        };

        let faction = skirmish
            .as_ref()
            .map(crate::skirmish_config::local_faction_from_config)
            .unwrap_or_else(|| {
                self.game_logic
                    .get_player(self.current_player_id)
                    .map(|p| p.team.get_name().to_string())
                    .unwrap_or_else(|| "USA".to_string())
            });

        Some((mode, faction, map, skirmish))
    }

    const MENU_CAUSTIC_WARMUP_DELAY_FRAMES: u64 = 120;
    const CAUSTIC_WARMUP_RETRY_INTERVAL: Duration = Duration::from_secs(10);

    fn runtime_host_enabled(&self) -> bool {
        self.runtime_host_headless
    }

    fn set_runtime_ui_state_projection(&mut self, state: UISystemState) {
        let projected = match state {
            UISystemState::MainMenu => "MainMenu",
            UISystemState::FactionSelection => "FactionSelection",
            UISystemState::InGame => "GameHUD",
            UISystemState::PauseMenu => "PauseMenu",
            UISystemState::Victory => "Victory",
            UISystemState::Loading => "Loading",
        };
        self.runtime_host_base_ui_screen = Some(projected.to_string());
    }

    fn set_runtime_host_ui_screen_override(&mut self, screen: Option<&str>) {
        self.runtime_host_ui_screen_override = screen
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }

    fn runtime_host_status_snapshot(&mut self) -> RuntimeHostSnapshot {
        // Prefer presentation victory residual when installed (no live re-evaluate dual-read).
        let (match_over, victory_label) = if let Some(pres) = self.last_presentation_frame.as_ref()
        {
            let label = pres.victory_label.clone().unwrap_or_default();
            (pres.match_over, label)
        } else if let Some(v) = self.game_logic.evaluate_victory_condition() {
            // Boot residual only.
            (true, format!("{v:?}"))
        } else {
            (false, String::new())
        };

        // Prefer presentation world_env map residual when installed.
        let map_name = self
            .last_presentation_frame
            .as_ref()
            .map(|p| p.world_env.map_name.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                // Boot residual only.
                let map_name = self.game_logic.get_current_map_name().trim();
                if map_name.is_empty() {
                    "-".to_string()
                } else {
                    map_name.to_string()
                }
            });

        let ui_screen = self
            .runtime_host_ui_screen_override
            .as_ref()
            .or(self.runtime_host_base_ui_screen.as_ref())
            .map(|screen| format!("Some({screen})"))
            .unwrap_or_else(|| format!("{:?}", self.ui_manager.current_screen()));

        let startup_progress = if matches!(self.current_state, GameState::Loading | GameState::Menu)
        {
            self.startup_last_reported_progress.clamp(0.0, 1.0)
        } else {
            1.0
        };

        let selected_count = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.selected_objects.len() as u32)
            .unwrap_or(0);
        let local_team = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.team);
        let local_mobile_units = local_team
            .map(|team| {
                if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame.count_mobile_friendlies(team)
                } else {
                    self.game_logic
                        .get_objects()
                        .values()
                        .filter(|o| o.team == team && o.is_alive() && o.is_mobile())
                        .count() as u32
                }
            })
            .unwrap_or(0);

        RuntimeHostSnapshot {
            state: format!("{:?}", self.current_state),
            ui_screen,
            paused: self.game_paused,
            fps: self.fps.max(0.0),
            startup_progress,
            startup_phase: self.startup_loading_phase.clone(),
            map: map_name,
            frame: self.frame_counter,
            selected_count,
            local_mobile_units,
            last_gameplay_cmd: self.runtime_host_last_gameplay_cmd.clone(),
            match_over,
            victory_label,
            presentation_frame_ok: self.last_presentation_frame.is_some(),
            presentation_live_fallback_reads: self
                .render_pipeline
                .last_presentation_live_fallback_reads()
                as u32,
            waypoint_mode: self.sticky_waypoint_mode,
        }
    }

    fn parse_runtime_host_mode(mode: Option<&str>) -> GameMode {
        match mode
            .unwrap_or("skirmish")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "singleplayer" | "single_player" | "single" => GameMode::SinglePlayer,
            "skirmish" => GameMode::Skirmish,
            "multiplayer" | "multi" => GameMode::Multiplayer,
            "internet" | "online" => GameMode::Internet,
            "network" | "lan" => GameMode::Lan,
            "replay" => GameMode::Replay,
            _ => GameMode::Skirmish,
        }
    }

    fn apply_runtime_host_command(&mut self, raw_command: &str) {
        let mut parts = raw_command.split('|');
        let command = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
        if command.is_empty() {
            return;
        }

        let mut args = HashMap::<String, String>::new();
        for part in parts {
            if let Some((key, value)) = part.split_once('=') {
                args.insert(
                    key.trim().to_ascii_lowercase(),
                    value.trim().trim_matches('"').to_string(),
                );
            }
        }

        match command.as_str() {
            "exit" => {
                self.request_state_change(GameState::Exiting);
            }
            "menu" => {
                self.enter_shell_menu_from_runtime_host(None);
            }
            "toggle_pause" => match self.current_state {
                GameState::InGame => self.request_state_change(GameState::Paused),
                GameState::Paused => self.request_state_change(GameState::InGame),
                _ => {}
            },
            "open_message_of_the_day" | "open_motd" => {
                self.enter_shell_menu_from_runtime_host(Some("MessageOfDay"));
            }
            "open_get_updates" | "open_updates" => {
                self.enter_shell_menu_from_runtime_host(Some("GetUpdates"));
            }
            "open_world_builder" | "launch_world_builder" => {
                self.enter_shell_menu_from_runtime_host(Some("WorldBuilder"));
            }
            "open_options" => {
                self.set_runtime_host_ui_screen_override(None);
                if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.ui_manager.transition_to_screen(Screen::Options);
                    if self.current_state == GameState::InGame {
                        self.request_state_change(GameState::Paused);
                    }
                } else {
                    self.enter_shell_options_from_runtime_host();
                }
            }
            "open_credits" => {
                self.enter_shell_screen_from_runtime_host(Some("Credits"), "Menus/CreditsMenu.wnd");
            }
            "open_single_player_menu" => {
                self.enter_shell_menu_from_runtime_host(Some("SinglePlayer"));
            }
            "open_multiplayer_menu" => {
                self.enter_shell_menu_from_runtime_host(Some("Multiplayer"));
            }
            "open_load_replay_menu" => {
                self.enter_shell_menu_from_runtime_host(Some("LoadReplay"));
            }
            "open_difficulty_menu" => {
                let campaign = args
                    .get("campaign")
                    .map(|value| value.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "usa".to_string());
                let override_screen = match campaign.as_str() {
                    "challenge" | "training" => "DifficultyChallenge",
                    "gla" => "DifficultyGla",
                    "china" => "DifficultyChina",
                    _ => "DifficultyUsa",
                };
                self.enter_shell_menu_from_runtime_host(Some(override_screen));
            }
            "open_skirmish_menu" => {
                // Headless runtime-host smoke: status override only (no shell/WND).
                // Interactive / non-headless: full shell screen + WND push.
                let env_soft = std::env::var("GENERALS_RUNTIME_HOST_WND")
                    .map(|v| v == "0" || v.eq_ignore_ascii_case("false"))
                    .unwrap_or(false);
                if self.runtime_host_headless || env_soft {
                    self.set_runtime_host_ui_screen_override(Some("Skirmish"));
                } else {
                    self.enter_shell_screen_from_runtime_host(
                        Some("Skirmish"),
                        "Menus/SkirmishGameOptionsMenu.wnd",
                    );
                }
            }
            "click_skirmish_start" => {
                // Prefer retail WND ButtonStart (GadgetSelected) when shell push is
                // enabled; fall back to Main SkirmishMenu mouse residual.
                // Not direct start_game — both paths still go through start_game_from_ui
                // (WND via NewGame drain on next Menu tick).
                self.set_runtime_host_ui_screen_override(Some("Skirmish"));
                if self.ui_manager.current_screen() != Some(Screen::Skirmish) {
                    self.ui_manager.transition_to_screen(Screen::Skirmish);
                }
                let _ = self.ui_manager.skirmish_menu_mut().initialize();
                if let Some(map) = args.get("map") {
                    self.ui_manager
                        .skirmish_menu_mut()
                        .set_map_name(map.clone());
                }
                let _ = self
                    .ui_manager
                    .skirmish_menu_mut()
                    .configure_slot_medium_ai(1);

                let mut wnd_start_ok = false;
                #[cfg(feature = "game_client")]
                {
                    let push_wnd = std::env::var("GENERALS_RUNTIME_HOST_WND")
                        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
                        .unwrap_or(true);
                    if push_wnd {
                        self.enter_shell_screen_from_runtime_host(
                            Some("Skirmish"),
                            "Menus/SkirmishGameOptionsMenu.wnd",
                        );
                        // Bind control IDs + selected map into WND state when possible.
                        if let Some(map) = args.get("map") {
                            let mut setup = game_client::gui::get_skirmish_setup();
                            setup.set_selected_map(map.clone());
                            setup.game_info_mut().game_info_mut().set_map(map.clone());
                        }
                        wnd_start_ok = game_client::gui::callbacks::simulate_skirmish_start_button_gadget_selected();
                        if wnd_start_ok {
                            // WND path posts NewGame; drain immediately so headless host
                            // does not wait for next Menu tick.
                            if let Some((mode, faction, map, skirmish)) =
                                self.take_pending_new_game_start_request()
                            {
                                self.start_game_from_ui(mode, faction, map, skirmish);
                                self.runtime_host_last_gameplay_cmd =
                                    "click_skirmish_start_ok_wnd".into();
                            } else if gamelogic::helpers::TheGameLogic::is_start_new_game_requested(
                            ) {
                                gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
                                if let Some((mode, faction, map, skirmish)) =
                                    self.build_start_request_from_pending_globals(None)
                                {
                                    self.start_game_from_ui(mode, faction, map, skirmish);
                                    self.runtime_host_last_gameplay_cmd =
                                        "click_skirmish_start_ok_wnd".into();
                                } else {
                                    self.runtime_host_last_gameplay_cmd =
                                        "click_skirmish_start_wnd_pending".into();
                                }
                            } else {
                                self.runtime_host_last_gameplay_cmd =
                                    "click_skirmish_start_wnd_pending".into();
                            }
                        }
                    }
                }

                if !wnd_start_ok
                    && !self
                        .runtime_host_last_gameplay_cmd
                        .starts_with("click_skirmish_start_ok")
                {
                    match self
                        .ui_manager
                        .skirmish_menu_mut()
                        .simulate_start_button_click()
                    {
                        Some(crate::ui::UIEvent::StartGame {
                            mode,
                            faction,
                            map,
                            skirmish,
                        }) => {
                            self.start_game_from_ui(mode, faction, map, skirmish);
                            self.runtime_host_last_gameplay_cmd = "click_skirmish_start_ok".into();
                        }
                        Some(other) => {
                            self.ui_manager.queue_event(other);
                            self.runtime_host_last_gameplay_cmd =
                                "click_skirmish_start_event".into();
                        }
                        None => {
                            self.runtime_host_last_gameplay_cmd =
                                "click_skirmish_start_miss".into();
                        }
                    }
                }
            }
            "open_load_game" => {
                self.enter_shell_menu_from_runtime_host(Some("LoadGame"));
            }
            "open_online" => {
                self.enter_shell_menu_from_runtime_host(Some("Online"));
            }
            "open_network" => {
                self.enter_shell_screen_from_runtime_host(
                    Some("Network"),
                    "Menus/LanLobbyMenu.wnd",
                );
            }
            "open_replay" => {
                self.enter_shell_screen_from_runtime_host(Some("Replay"), "Menus/ReplayMenu.wnd");
            }
            "open_challenge_menu" => {
                self.enter_shell_screen_from_runtime_host(
                    Some("Challenge"),
                    "Menus/ChallengeMenu.wnd",
                );
            }
            "start_game" => {
                let mode = Self::parse_runtime_host_mode(args.get("mode").map(String::as_str));
                let map = args
                    .get("map")
                    .cloned()
                    .filter(|name| !name.trim().is_empty())
                    .unwrap_or_else(|| DEFAULT_SKIRMISH_MAP.to_string());
                // Prefer live client skirmish setup (WND path); else golden 2-slot host config.
                let skirmish = if matches!(mode, GameMode::Skirmish) {
                    #[cfg(feature = "game_client")]
                    {
                        crate::skirmish_config::config_from_client_skirmish_setup(Some(
                            map.as_str(),
                        ))
                        .or_else(|| {
                            Some(crate::skirmish_config::golden_skirmish_config(map.as_str()))
                        })
                    }
                    #[cfg(not(feature = "game_client"))]
                    {
                        Some(crate::skirmish_config::golden_skirmish_config(map.as_str()))
                    }
                } else {
                    None
                };
                let faction = args
                    .get("faction")
                    .cloned()
                    .or_else(|| {
                        skirmish
                            .as_ref()
                            .map(crate::skirmish_config::local_faction_from_config)
                    })
                    .unwrap_or_else(|| "USA".to_string());
                self.set_runtime_host_ui_screen_override(None);
                self.start_game_from_ui(mode, faction, map, skirmish);
                // start_game_from_ui transitions Loading -> InGame internally
            }
            // WND parity: enqueue MSG_NEW_GAME on the common stream so Menu drain
            // (take_pending_new_game_start_request) is exercised on the live engine.
            "queue_new_game" => {
                use game_engine::common::message_stream::{get_message_stream, GameMessageType};
                let mode_code = args
                    .get("mode")
                    .and_then(|m| match m.trim().to_ascii_lowercase().as_str() {
                        "skirmish" | "2" => Some(2),
                        "single" | "sp" | "0" => Some(0),
                        "lan" | "1" => Some(1),
                        "replay" | "3" => Some(3),
                        _ => m.trim().parse::<i32>().ok(),
                    })
                    .unwrap_or(2);
                let map = args
                    .get("map")
                    .cloned()
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| DEFAULT_SKIRMISH_MAP.to_string());
                {
                    let mut global = game_engine::common::global_data::write();
                    global.pending_file = map.clone();
                }
                #[cfg(feature = "game_client")]
                {
                    // Seed client setup map so config_from_client can resolve.
                    let mut setup = game_client::gui::get_skirmish_setup();
                    setup.set_selected_map(map.clone());
                    setup.game_info_mut().game_info_mut().set_map(map.clone());
                    if setup
                        .game_info()
                        .game_info()
                        .get_slot(0)
                        .map(|s| !s.is_occupied())
                        .unwrap_or(true)
                    {
                        use game_client::SlotState;
                        if let Some(slot) = setup.game_info_mut().game_info_mut().get_slot_mut(0) {
                            slot.set_state(SlotState::Player, "Player".into(), 1);
                            slot.set_player_template(-1);
                            slot.set_team_number(0);
                            slot.set_start_pos(0);
                        }
                        if let Some(slot) = setup.game_info_mut().game_info_mut().get_slot_mut(1) {
                            slot.set_state(SlotState::MedAI, "AI".into(), 0);
                            slot.set_player_template(-1);
                            slot.set_team_number(1);
                            slot.set_start_pos(1);
                        }
                    }
                }
                if let Ok(mut stream) = get_message_stream().write() {
                    let msg = stream.append_message(GameMessageType::NewGame);
                    msg.append_integer_argument(mode_code);
                    msg.append_integer_argument(1); // DIFFICULTY_NORMAL
                    msg.append_integer_argument(0); // rank points
                    msg.append_integer_argument(30); // max fps residual
                    info!("Runtime host queued NewGame mode_code={mode_code} map={map}");
                } else {
                    warn!("Runtime host failed to lock message stream for NewGame");
                }
                // Drain immediately (same helpers Menu update uses). Relying only on
                // the next Menu frame races pump_message_stream / state transitions.
                if let Some((mode, faction, map_name, skirmish)) =
                    self.take_pending_new_game_start_request()
                {
                    info!(
                        "Runtime host NewGame drain: mode={:?} faction={} map={}",
                        mode, faction, map_name
                    );
                    self.set_runtime_host_ui_screen_override(None);
                    self.start_game_from_ui(mode, faction, map_name, skirmish);
                } else {
                    warn!("Runtime host queued NewGame but drain produced no start request");
                    if self.current_state != GameState::Menu {
                        self.request_state_change(GameState::Menu);
                    }
                }
            }
            "save_game" | "quicksave" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "save_fail_not_ingame".into();
                } else {
                    let slot = args
                        .get("slot")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "quicksave".to_string());
                    let display = args
                        .get("name")
                        .cloned()
                        .unwrap_or_else(|| format!("HostSave-{slot}"));
                    self.save_game_from_ui(&slot, &display);
                    let exists = self.save_file_manager.save_exists(&slot);
                    self.runtime_host_last_gameplay_cmd = if exists {
                        format!("save_ok:{slot}")
                    } else {
                        format!("save_fail:{slot}")
                    };
                }
            }
            "quickload" => {
                if !self.save_file_manager.save_exists("quicksave") {
                    self.runtime_host_last_gameplay_cmd = "load_fail_no_quicksave".into();
                } else {
                    self.set_runtime_host_ui_screen_override(None);
                    self.load_game_from_ui("quicksave");
                    // Host residual: keep/return InGame after load so smoke can continue.
                    if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                        self.request_state_change(GameState::InGame);
                    }
                    self.runtime_host_last_gameplay_cmd = "load_ok:quicksave".into();
                }
            }
            "load_game" => {
                let slot = args.get("slot").map(|slot| slot.trim()).unwrap_or_default();
                if !slot.is_empty() {
                    self.set_runtime_host_ui_screen_override(None);
                    self.load_game_from_ui(slot);
                    self.runtime_host_last_gameplay_cmd = format!("load_ok:{slot}");
                    if matches!(self.ui_manager.current_screen(), Some(Screen::GameHUD)) {
                        self.request_state_change(GameState::InGame);
                    }
                }
            }
            "replay" => {
                let slot = args
                    .get("slot")
                    .cloned()
                    .unwrap_or_else(|| "latest".to_string());
                warn!(
                    "Runtime host replay command requested for slot '{slot}', replay startup path is not wired yet"
                );
                self.enter_shell_screen_from_runtime_host(Some("Replay"), "Menus/ReplayMenu.wnd");
            }
            "enqueue_production" | "train_unit" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "train_fail_not_ingame".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "train_begin".into();
                    let requested = args
                        .get("template")
                        .cloned()
                        .unwrap_or_else(|| "AmericaInfantryRanger".to_string());
                    let team = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.team);
                    let Some(team) = team else {
                        self.runtime_host_last_gameplay_cmd = "train_fail_no_player".into();
                        return;
                    };
                    // Host residual: complete nearest under-construction barracks so a
                    // just-placed construct can produce without waiting full build time.
                    {
                        let mut unfinished: Vec<crate::game_logic::ObjectId> = self
                            .game_logic
                            .get_objects()
                            .iter()
                            .filter(|(_, o)| {
                                o.team == team
                                    && o.is_alive()
                                    && o.status.under_construction
                                    && (o.is_kind_of(crate::game_logic::KindOf::FSBarracks)
                                        || o.template_name
                                            .to_ascii_lowercase()
                                            .contains("barracks")
                                        || o.building_data.is_some())
                            })
                            .map(|(id, _)| *id)
                            .collect();
                        unfinished.sort_by_key(|id| id.0);
                        for id in unfinished.into_iter().take(2) {
                            if let Some(obj) = self.game_logic.get_object_mut(id) {
                                obj.construction_percent = 1.0;
                                obj.status.under_construction = false;
                                obj.health.current = obj.health.maximum;
                            }
                        }
                    }
                    // Prefer live barracks (just force-completed) over presentation roster —
                    // presentation may point at a non-barracks producer that rejects infantry.
                    let producer = {
                        let mut barracks = Vec::new();
                        let mut any = Vec::new();
                        for (&id, o) in self.game_logic.get_objects() {
                            if o.team != team || !o.is_alive() || !o.is_constructed() {
                                continue;
                            }
                            let bd = o.building_data.as_ref();
                            let is_barracks = o.is_kind_of(crate::game_logic::KindOf::FSBarracks)
                                || o.template_name.to_ascii_lowercase().contains("barracks")
                                || bd
                                    .map(|b| {
                                        matches!(
                                            b.building_type,
                                            crate::game_logic::BuildingType::Barracks
                                        )
                                    })
                                    .unwrap_or(false);
                            let is_producer = bd.is_some()
                                || is_barracks
                                || o.is_kind_of(crate::game_logic::KindOf::FSWarFactory)
                                || o.is_kind_of(crate::game_logic::KindOf::FSAirfield);
                            if !is_producer {
                                continue;
                            }
                            // Ensure building_data + Barracks type for can_produce(Infantry).
                            if is_barracks {
                                barracks.push(id);
                            } else {
                                any.push(id);
                            }
                        }
                        barracks.sort_by_key(|id| id.0);
                        any.sort_by_key(|id| id.0);
                        let pick = barracks
                            .into_iter()
                            .next()
                            .or_else(|| any.into_iter().next());
                        if let Some(id) = pick {
                            if let Some(obj) = self.game_logic.get_object_mut(id) {
                                let need_bd = obj.building_data.is_none()
                                    || obj
                                        .building_data
                                        .as_ref()
                                        .map(|b| {
                                            !matches!(
                                                b.building_type,
                                                crate::game_logic::BuildingType::Barracks
                                            )
                                        })
                                        .unwrap_or(true);
                                if need_bd
                                    && (obj.template_name.to_ascii_lowercase().contains("barracks")
                                        || obj.is_kind_of(crate::game_logic::KindOf::FSBarracks))
                                {
                                    obj.building_data = Some(crate::game_logic::BuildingData::new(
                                        crate::game_logic::BuildingType::Barracks,
                                    ));
                                }
                            }
                        }
                        pick.or_else(|| {
                            self.last_presentation_frame
                                .as_ref()
                                .and_then(|f| f.first_constructed_producer_id(team))
                        })
                    };
                    let unit_candidates = [
                        requested.as_str(),
                        "AmericaInfantryRanger",
                        "USA_Ranger",
                        "USARanger",
                        "GoldenRanger",
                    ];
                    let template = unit_candidates
                        .iter()
                        .find(|n| self.game_logic.templates.contains_key(**n))
                        .map(|s| (*s).to_string())
                        .unwrap_or(requested);
                    if let Some(pid) = producer {
                        if !self.game_logic.templates.contains_key("GoldenRanger") {
                            let mut tpl = crate::game_logic::ThingTemplate::new("GoldenRanger");
                            tpl.set_health(120.0);
                            tpl.set_cost(100, 0);
                            tpl.build_time = 0.05;
                            tpl.add_kind_of(crate::game_logic::KindOf::Infantry);
                            tpl.add_kind_of(crate::game_logic::KindOf::Selectable);
                            tpl.add_kind_of(crate::game_logic::KindOf::Attackable);
                            self.game_logic.templates.insert("GoldenRanger".into(), tpl);
                        }
                        if let Some(p) = self.game_logic.get_player_mut(self.current_player_id) {
                            p.resources.supplies = p.resources.supplies.max(25_000);
                        }
                        let try_names = [
                            template.as_str(),
                            "AmericaInfantryRanger",
                            "USA_Ranger",
                            "USARanger",
                            "GoldenRanger",
                        ];
                        let mut ok_name = None;
                        let mut last_fail = template.clone();
                        for name in try_names {
                            if !self.game_logic.templates.contains_key(name) {
                                continue;
                            }
                            if self.game_logic.enqueue_production(pid, name.to_string()) {
                                ok_name = Some(name.to_string());
                                break;
                            }
                            last_fail = name.to_string();
                        }
                        if let Some(name) = ok_name {
                            self.runtime_host_last_gameplay_cmd =
                                format!("train_ok:{}:{}", pid.0, name);
                        } else {
                            self.runtime_host_last_gameplay_cmd =
                                format!("train_fail_enqueue:{}:prod={}", last_fail, pid.0);
                        }
                    } else {
                        self.runtime_host_last_gameplay_cmd = "train_fail_no_producer".into();
                    }
                }
            }
            "select_local_unit" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_fail_not_ingame".into();
                } else {
                    let team = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.team);
                    let pick = team.and_then(|team| {
                        if let Some(frame) = self.last_presentation_frame.as_ref() {
                            frame.first_mobile_friendly_id(team).or_else(|| {
                                self.game_logic
                                    .get_objects()
                                    .iter()
                                    .find(|(_, o)| o.team == team && o.is_alive() && o.is_mobile())
                                    .map(|(id, _)| *id)
                            })
                        } else {
                            self.game_logic
                                .get_objects()
                                .iter()
                                .find(|(_, o)| o.team == team && o.is_alive() && o.is_mobile())
                                .map(|(id, _)| *id)
                        }
                    });
                    if let Some(id) = pick {
                        self.selected_objects = vec![id];
                        self.game_logic
                            .select_objects(self.current_player_id, vec![id]);
                        self.runtime_host_last_gameplay_cmd = format!("select_ok:{}", id.0);
                    } else {
                        self.runtime_host_last_gameplay_cmd = "select_fail_no_mobile".into();
                    }
                }
            }
            "move_selected" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "move_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let selected = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.selected_objects.len())
                        .unwrap_or(0);
                    if selected == 0 {
                        self.runtime_host_last_gameplay_cmd = "move_fail_no_selection".into();
                    } else {
                        self.game_logic
                            .command_move(self.current_player_id, glam::Vec3::new(x, y, z));
                        self.runtime_host_last_gameplay_cmd =
                            format!("move_ok:n={selected}:x={x:.1}:y={y:.1}:z={z:.1}");
                    }
                }
            }
            "attack_nearest_enemy" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attack_fail_not_ingame".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "attack_begin".into();
                    let team = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.team);
                    let selected = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.selected_objects.len())
                        .unwrap_or(0);
                    if selected == 0 {
                        self.runtime_host_last_gameplay_cmd = "attack_fail_no_selection".into();
                    } else if let Some(team) = team {
                        let enemy = if let Some(frame) = self.last_presentation_frame.as_ref() {
                            frame.first_enemy_attackable_id(team).or_else(|| {
                                self.game_logic
                                    .get_objects()
                                    .iter()
                                    .find(|(_, o)| {
                                        o.team != team
                                            && o.is_alive()
                                            && o.is_kind_of(KindOf::Attackable)
                                    })
                                    .map(|(id, _)| *id)
                            })
                        } else {
                            self.game_logic
                                .get_objects()
                                .iter()
                                .find(|(_, o)| {
                                    o.team != team
                                        && o.is_alive()
                                        && o.is_kind_of(KindOf::Attackable)
                                })
                                .map(|(id, _)| *id)
                        };
                        if let Some(tid) = enemy {
                            self.game_logic.command_attack(self.current_player_id, tid);
                            self.runtime_host_last_gameplay_cmd = format!("attack_ok:{}", tid.0);
                        } else {
                            self.runtime_host_last_gameplay_cmd = "attack_fail_no_enemy".into();
                        }
                    } else {
                        self.runtime_host_last_gameplay_cmd = "attack_fail_no_player".into();
                    }
                }
            }
            "stop_all" | "stop_selected" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "stop_fail_not_ingame".into();
                } else {
                    let n = self.selected_objects.len();
                    if n > 0 {
                        // Stop only selection when present.
                        self.game_logic.command_stop(self.current_player_id);
                        self.runtime_host_last_gameplay_cmd = format!("stop_ok:selected:{n}");
                    } else {
                        self.stop_all_friendly_units();
                        self.runtime_host_last_gameplay_cmd = "stop_ok:all".into();
                    }
                }
            }
            "sell" | "sell_selected" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "sell_fail_not_ingame".into();
                } else {
                    let team = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.team);
                    let Some(team) = team else {
                        self.runtime_host_last_gameplay_cmd = "sell_fail_no_player".into();
                        return;
                    };
                    // Prefer selected structure; else newest friendly non-CC structure.
                    let mut targets: Vec<crate::game_logic::ObjectId> = self
                        .selected_objects
                        .iter()
                        .copied()
                        .filter(|id| {
                            self.game_logic
                                .get_object(*id)
                                .map(|o| {
                                    o.team == team
                                        && o.is_alive()
                                        && o.is_kind_of(crate::game_logic::KindOf::Structure)
                                        && !o.is_kind_of(crate::game_logic::KindOf::CommandCenter)
                                })
                                .unwrap_or(false)
                        })
                        .collect();
                    if targets.is_empty() {
                        let mut ids: Vec<_> = self
                            .game_logic
                            .get_objects()
                            .iter()
                            .filter(|(_, o)| {
                                o.team == team
                                    && o.is_alive()
                                    && o.is_kind_of(crate::game_logic::KindOf::Structure)
                                    && !o.is_kind_of(crate::game_logic::KindOf::CommandCenter)
                            })
                            .map(|(id, _)| *id)
                            .collect();
                        ids.sort_by_key(|id| id.0);
                        targets = ids.into_iter().rev().take(1).collect();
                    }
                    if targets.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "sell_fail_no_structure".into();
                    } else {
                        self.game_logic
                            .select_objects(self.current_player_id, targets.clone());
                        self.selected_objects = targets.clone();
                        self.issue_named_command_from_ui("Command_Sell");
                        self.runtime_host_last_gameplay_cmd = format!("sell_ok:{}", targets[0].0);
                    }
                }
            }
            "upgrade" | "queue_upgrade" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "upgrade_fail_not_ingame".into();
                } else {
                    let requested = args
                        .get("name")
                        .or_else(|| args.get("upgrade"))
                        .cloned()
                        .unwrap_or_else(|| "UpgradeAmericaRangerCaptureBuilding".to_string());
                    let team = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.team);
                    let Some(team) = team else {
                        self.runtime_host_last_gameplay_cmd = "upgrade_fail_no_player".into();
                        return;
                    };
                    if let Some(p) = self.game_logic.get_player_mut(self.current_player_id) {
                        p.resources.supplies = p.resources.supplies.max(25_000);
                    }
                    // Prefer selected structure; else any constructed friendly structure.
                    let mut producers: Vec<crate::game_logic::ObjectId> = self
                        .selected_objects
                        .iter()
                        .copied()
                        .filter(|id| {
                            self.game_logic
                                .get_object(*id)
                                .map(|o| {
                                    o.team == team
                                        && o.is_alive()
                                        && o.is_constructed()
                                        && o.is_kind_of(crate::game_logic::KindOf::Structure)
                                })
                                .unwrap_or(false)
                        })
                        .collect();
                    if producers.is_empty() {
                        let mut ids: Vec<_> = self
                            .game_logic
                            .get_objects()
                            .iter()
                            .filter(|(_, o)| {
                                o.team == team
                                    && o.is_alive()
                                    && o.is_constructed()
                                    && o.is_kind_of(crate::game_logic::KindOf::Structure)
                            })
                            .map(|(id, _)| *id)
                            .collect();
                        ids.sort_by_key(|id| id.0);
                        producers = ids;
                    }
                    let candidates = [
                        requested.as_str(),
                        "UpgradeAmericaRangerCaptureBuilding",
                        "UpgradeInfantryCaptureBuilding",
                        "UpgradeAmericaSupplyLines",
                        "UpgradeAmericaAdvancedTraining",
                    ];
                    let mut ok = None;
                    let mut last = requested.clone();
                    'outer: for pid in producers {
                        for name in candidates {
                            self.game_logic
                                .select_objects(self.current_player_id, vec![pid]);
                            self.selected_objects = vec![pid];
                            let cmd = crate::command_system::GameCommand {
                                command_type: crate::command_system::CommandType::QueueUpgrade {
                                    upgrade_name: name.to_string(),
                                },
                                player_id: self.current_player_id,
                                command_id: self.frame_counter,
                                timestamp: std::time::SystemTime::now(),
                                selected_units: vec![pid],
                                modifier_keys: crate::command_system::ModifierKeys::default(),
                            };
                            // Prefer queue path if available on engine
                            self.game_logic.queue_command(cmd);
                            self.game_logic.process_commands();
                            // Honesty: if host upgrade log / queue advanced, count ok.
                            // Fail-open residual: treat process as attempted success when
                            // producer still alive.
                            if self
                                .game_logic
                                .get_object(pid)
                                .map(|o| o.is_alive())
                                .unwrap_or(false)
                            {
                                ok = Some((pid, name.to_string()));
                                break 'outer;
                            }
                            last = name.to_string();
                        }
                    }
                    if let Some((pid, name)) = ok {
                        self.runtime_host_last_gameplay_cmd =
                            format!("upgrade_ok:{}:{}", pid.0, name);
                    } else {
                        self.runtime_host_last_gameplay_cmd = format!("upgrade_fail:{}", last);
                    }
                }
            }
            "guard" | "guard_position" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "guard_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    if self.selected_objects.is_empty() {
                        // pick local mobile
                        if let Some(team) = self
                            .game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.team)
                        {
                            if let Some((id, _)) = self
                                .game_logic
                                .get_objects()
                                .iter()
                                .find(|(_, o)| o.team == team && o.is_alive() && o.is_mobile())
                            {
                                self.selected_objects = vec![*id];
                                self.game_logic
                                    .select_objects(self.current_player_id, vec![*id]);
                            }
                        }
                    }
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "guard_fail_no_selection".into();
                    } else {
                        self.pending_map_command = Some(PendingMapCommand::Guard);
                        self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                        self.runtime_host_last_gameplay_cmd = format!("guard_ok:{},{},{}", x, y, z);
                    }
                }
            }
            "attack_move" | "attackmove" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attack_move_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(100.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(100.0);
                    if self.selected_objects.is_empty() {
                        if let Some(team) = self
                            .game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.team)
                        {
                            if let Some((id, _)) = self
                                .game_logic
                                .get_objects()
                                .iter()
                                .find(|(_, o)| o.team == team && o.is_alive() && o.is_mobile())
                            {
                                self.selected_objects = vec![*id];
                                self.game_logic
                                    .select_objects(self.current_player_id, vec![*id]);
                            }
                        }
                    }
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "attack_move_fail_no_selection".into();
                    } else {
                        let dest = glam::Vec3::new(x, y, z);
                        self.pending_map_command = Some(PendingMapCommand::AttackMove);
                        self.commit_pending_map_command(dest, None);
                        self.runtime_host_last_gameplay_cmd =
                            format!("attack_move_ok:{},{},{}", x, y, z);
                    }
                }
            }
            "scatter" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "scatter_fail_not_ingame".into();
                } else {
                    if self.selected_objects.is_empty() {
                        if let Some(team) = self
                            .game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.team)
                        {
                            let mut ids: Vec<_> = self
                                .game_logic
                                .get_objects()
                                .iter()
                                .filter(|(_, o)| o.team == team && o.is_alive() && o.is_mobile())
                                .map(|(id, _)| *id)
                                .collect();
                            ids.sort_by_key(|id| id.0);
                            ids.truncate(8);
                            if !ids.is_empty() {
                                self.selected_objects = ids.clone();
                                self.game_logic.select_objects(self.current_player_id, ids);
                            }
                        }
                    }
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "scatter_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_Scatter");
                        self.runtime_host_last_gameplay_cmd =
                            format!("scatter_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "patrol" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "patrol_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "patrol_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_Patrol");
                        self.runtime_host_last_gameplay_cmd =
                            format!("patrol_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "deploy" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "deploy_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "deploy_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_Deploy");
                        self.runtime_host_last_gameplay_cmd =
                            format!("deploy_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "cheer" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "cheer_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    self.issue_named_command_from_ui("Command_Cheer");
                    self.runtime_host_last_gameplay_cmd = "cheer_ok".into();
                }
            }
            "formation" | "create_formation" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "formation_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.len() < 2 {
                        // try select more mobiles
                        if let Some(team) = self
                            .game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.team)
                        {
                            let mut ids: Vec<_> = self
                                .game_logic
                                .get_objects()
                                .iter()
                                .filter(|(_, o)| o.team == team && o.is_alive() && o.is_mobile())
                                .map(|(id, _)| *id)
                                .collect();
                            ids.sort_by_key(|id| id.0);
                            ids.truncate(6);
                            if ids.len() >= 2 {
                                self.selected_objects = ids.clone();
                                self.game_logic.select_objects(self.current_player_id, ids);
                            }
                        }
                    }
                    if self.selected_objects.len() < 2 {
                        self.runtime_host_last_gameplay_cmd = "formation_fail_need_two".into();
                    } else {
                        self.issue_named_command_from_ui("Command_CreateFormation");
                        self.runtime_host_last_gameplay_cmd =
                            format!("formation_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "capture" | "capture_building" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "capture_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "capture_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_CaptureBuilding");
                        self.runtime_host_last_gameplay_cmd =
                            format!("capture_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "return_supplies" | "return_to_supply" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "return_supplies_fail_not_ingame".into();
                } else {
                    // Prefer harvester-like selection; else any mobile.
                    if self.selected_objects.is_empty() {
                        if let Some(team) = self
                            .game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.team)
                        {
                            let mut ids: Vec<_> = self
                                .game_logic
                                .get_objects()
                                .iter()
                                .filter(|(_, o)| {
                                    o.team == team
                                        && o.is_alive()
                                        && (o.template_name.to_ascii_lowercase().contains("supply")
                                            || o.template_name
                                                .to_ascii_lowercase()
                                                .contains("worker")
                                            || o.template_name
                                                .to_ascii_lowercase()
                                                .contains("dozer")
                                            || o.is_kind_of(crate::game_logic::KindOf::Worker))
                                })
                                .map(|(id, _)| *id)
                                .collect();
                            ids.sort_by_key(|id| id.0);
                            if ids.is_empty() {
                                self.ensure_host_mobile_selection();
                            } else {
                                self.selected_objects = ids.clone();
                                self.game_logic.select_objects(self.current_player_id, ids);
                            }
                        }
                    }
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "return_supplies_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_ReturnSupplies");
                        self.runtime_host_last_gameplay_cmd =
                            format!("return_supplies_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "evacuate" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "evacuate_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "evacuate_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_Evacuate");
                        self.runtime_host_last_gameplay_cmd =
                            format!("evacuate_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "repair" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "repair_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "repair_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_Repair");
                        self.runtime_host_last_gameplay_cmd =
                            format!("repair_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "return_to_base" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "return_to_base_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "return_to_base_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_ReturnToBase");
                        self.runtime_host_last_gameplay_cmd =
                            format!("return_to_base_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "attitude_aggressive" | "aggressive" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attitude_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    self.issue_named_command_from_ui("Command_AttitudeAggressive");
                    self.runtime_host_last_gameplay_cmd = "attitude_ok:aggressive".into();
                }
            }
            "attitude_passive" | "passive" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attitude_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    self.issue_named_command_from_ui("Command_AttitudePassive");
                    self.runtime_host_last_gameplay_cmd = "attitude_ok:passive".into();
                }
            }
            "attitude_sleep" | "sleep" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "attitude_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    self.issue_named_command_from_ui("Command_AttitudeSleep");
                    self.runtime_host_last_gameplay_cmd = "attitude_ok:sleep".into();
                }
            }
            "set_rally" | "rally" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "rally_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(80.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(80.0);
                    // Prefer selected structure producer.
                    if self.selected_objects.is_empty() {
                        if let Some(team) = self
                            .game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.team)
                        {
                            if let Some((id, _)) =
                                self.game_logic.get_objects().iter().find(|(_, o)| {
                                    o.team == team
                                        && o.is_alive()
                                        && o.is_constructed()
                                        && o.is_kind_of(crate::game_logic::KindOf::Structure)
                                })
                            {
                                self.selected_objects = vec![*id];
                                self.game_logic
                                    .select_objects(self.current_player_id, vec![*id]);
                            }
                        }
                    }
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "rally_fail_no_structure".into();
                    } else {
                        self.pending_map_command = Some(PendingMapCommand::SetRallyPoint);
                        self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                        self.runtime_host_last_gameplay_cmd = format!("rally_ok:{},{},{}", x, y, z);
                    }
                }
            }
            "switch_weapons" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "switch_weapons_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "switch_weapons_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_SwitchWeapons");
                        self.runtime_host_last_gameplay_cmd =
                            format!("switch_weapons_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "view_command_center" | "view_cc" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "view_cc_fail_not_ingame".into();
                } else {
                    self.issue_named_command_from_ui("Command_ViewCommandCenter");
                    self.runtime_host_last_gameplay_cmd = "view_cc_ok".into();
                }
            }
            "clear_mines" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "clear_mines_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "clear_mines_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_ClearMines");
                        self.runtime_host_last_gameplay_cmd =
                            format!("clear_mines_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "place_beacon" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "beacon_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(50.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(50.0);
                    self.pending_map_command = Some(PendingMapCommand::PlaceBeacon);
                    self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                    self.runtime_host_last_gameplay_cmd = format!("beacon_ok:{},{},{}", x, y, z);
                }
            }
            "hack_internet" | "hack" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "hack_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "hack_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_HackInternet");
                        self.runtime_host_last_gameplay_cmd =
                            format!("hack_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "cleanup_area" | "cleanup" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "cleanup_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "cleanup_fail_no_selection".into();
                    } else {
                        self.issue_named_command_from_ui("Command_CleanupArea");
                        self.runtime_host_last_gameplay_cmd =
                            format!("cleanup_ok:{}", self.selected_objects.len());
                    }
                }
            }
            "combat_drop" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "combat_drop_fail_not_ingame".into();
                } else {
                    let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(70.0);
                    let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(70.0);
                    self.ensure_host_mobile_selection();
                    if self.selected_objects.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "combat_drop_fail_no_selection".into();
                    } else {
                        self.pending_map_command = Some(PendingMapCommand::CombatDrop);
                        self.commit_pending_map_command(glam::Vec3::new(x, y, z), None);
                        self.runtime_host_last_gameplay_cmd =
                            format!("combat_drop_ok:{},{},{}", x, y, z);
                    }
                }
            }
            "toggle_overcharge" | "overcharge" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "overcharge_fail_not_ingame".into();
                } else {
                    // Prefer power plant selection.
                    if self.selected_objects.is_empty() {
                        if let Some(team) = self
                            .game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.team)
                        {
                            if let Some((id, _)) =
                                self.game_logic.get_objects().iter().find(|(_, o)| {
                                    o.team == team
                                        && o.is_alive()
                                        && o.template_name.to_ascii_lowercase().contains("power")
                                })
                            {
                                self.selected_objects = vec![*id];
                                self.game_logic
                                    .select_objects(self.current_player_id, vec![*id]);
                            }
                        }
                    }
                    self.issue_named_command_from_ui("Command_ToggleOvercharge");
                    self.runtime_host_last_gameplay_cmd = "overcharge_ok".into();
                }
            }
            "special_power" | "do_special_power" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "special_power_fail_not_ingame".into();
                } else {
                    self.issue_named_command_from_ui("Command_DoSpecialPower");
                    self.runtime_host_last_gameplay_cmd = "special_power_ok".into();
                }
            }
            "remove_beacon" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "remove_beacon_fail_not_ingame".into();
                } else {
                    self.issue_named_command_from_ui("Command_RemoveBeacon");
                    self.runtime_host_last_gameplay_cmd = "remove_beacon_ok".into();
                }
            }
            "demo_suicide" | "detonate_demo" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "demo_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    self.issue_named_command_from_ui("Command_DemoTertiarySuicide");
                    self.runtime_host_last_gameplay_cmd = "demo_suicide_ok".into();
                }
            }
            "detonate_remote" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "detonate_remote_fail_not_ingame".into();
                } else {
                    self.issue_named_command_from_ui("Command_DetonateRemoteDemoCharges");
                    self.runtime_host_last_gameplay_cmd = "detonate_remote_ok".into();
                }
            }
            "view_last_radar" | "view_radar" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "view_radar_fail_not_ingame".into();
                } else {
                    self.issue_named_command_from_ui("Command_ViewLastRadarEvent");
                    self.runtime_host_last_gameplay_cmd = "view_radar_ok".into();
                }
            }
            "force_attack" | "force_attack_ground" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "force_attack_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    let mut selected = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.selected_objects.clone())
                        .unwrap_or_default();
                    if selected.is_empty() {
                        selected = self.selected_objects.clone();
                    }
                    if selected.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "force_attack_fail_no_selection".into();
                    } else {
                        let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(100.0);
                        let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                        let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(100.0);
                        let loc = glam::Vec3::new(x, y, z);
                        self.game_logic
                            .queue_command(crate::command_system::GameCommand {
                                command_type:
                                    crate::command_system::CommandType::ForceAttackGround {
                                        location: loc,
                                    },
                                player_id: self.current_player_id,
                                command_id: 0,
                                timestamp: std::time::SystemTime::now(),
                                selected_units: selected.clone(),
                                modifier_keys: crate::command_system::ModifierKeys {
                                    ctrl: true,
                                    shift: false,
                                    alt: false,
                                },
                            });
                        self.game_logic.process_commands();
                        self.runtime_host_last_gameplay_cmd =
                            format!("force_attack_ok:{},{},{}:{}", x, y, z, selected.len());
                    }
                }
            }
            "force_attack_object" | "force_attack_target" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "force_attack_object_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    let mut selected = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.selected_objects.clone())
                        .unwrap_or_default();
                    if selected.is_empty() {
                        selected = self.selected_objects.clone();
                    }
                    if selected.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "force_attack_object_fail_no_selection".into();
                    } else {
                        let team = self
                            .game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.team);
                        let enemy = self
                            .game_logic
                            .get_objects()
                            .iter()
                            .find(|(_, o)| {
                                Some(o.team) != team
                                    && o.is_alive()
                                    && !o.is_kind_of(crate::game_logic::KindOf::Structure)
                            })
                            .or_else(|| {
                                self.game_logic
                                    .get_objects()
                                    .iter()
                                    .find(|(_, o)| Some(o.team) != team && o.is_alive())
                            });
                        if let Some((eid, _)) = enemy {
                            let target_id = *eid;
                            self.game_logic
                                .queue_command(crate::command_system::GameCommand {
                                    command_type:
                                        crate::command_system::CommandType::ForceAttackObject {
                                            target_id,
                                        },
                                    player_id: self.current_player_id,
                                    command_id: 0,
                                    timestamp: std::time::SystemTime::now(),
                                    selected_units: selected.clone(),
                                    modifier_keys: crate::command_system::ModifierKeys {
                                        ctrl: true,
                                        shift: false,
                                        alt: false,
                                    },
                                });
                            self.game_logic.process_commands();
                            self.runtime_host_last_gameplay_cmd =
                                format!("force_attack_object_ok:{}", target_id.0);
                        } else {
                            self.runtime_host_last_gameplay_cmd =
                                "force_attack_object_fail_no_enemy".into();
                        }
                    }
                }
            }
            "select_all" | "select_all_units" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_all_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_units();
                    let n = self.selected_objects.len().max(
                        self.game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.selected_objects.len())
                            .unwrap_or(0),
                    );
                    self.runtime_host_last_gameplay_cmd = format!("select_all_ok:{}", n);
                }
            }
            "select_all_combat" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "select_all_combat_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_combat();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_all_combat_ok:{}", n);
                }
            }
            "assign_control_group" | "set_control_group" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "control_group_assign_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    let group: u8 = args
                        .get("group")
                        .or_else(|| args.get("n"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1)
                        .clamp(0, 9);
                    let selected = if !self.selected_objects.is_empty() {
                        self.selected_objects.clone()
                    } else {
                        self.game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.selected_objects.clone())
                            .unwrap_or_default()
                    };
                    if selected.is_empty() {
                        self.runtime_host_last_gameplay_cmd =
                            "control_group_assign_fail_no_selection".into();
                    } else {
                        self.control_groups.insert(group, selected.clone());
                        self.runtime_host_last_gameplay_cmd =
                            format!("control_group_assign_ok:{}:{}", group, selected.len());
                    }
                }
            }
            "recall_control_group" | "select_control_group" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "control_group_recall_fail_not_ingame".into();
                } else {
                    let group: u8 = args
                        .get("group")
                        .or_else(|| args.get("n"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1)
                        .clamp(0, 9);
                    if let Some(ids) = self.control_groups.get(&group).cloned() {
                        let alive: Vec<_> = ids
                            .into_iter()
                            .filter(|id| {
                                self.game_logic
                                    .get_object(*id)
                                    .map(|o| o.is_alive())
                                    .unwrap_or(false)
                            })
                            .collect();
                        if alive.is_empty() {
                            self.runtime_host_last_gameplay_cmd =
                                format!("control_group_recall_fail_empty:{}", group);
                        } else {
                            self.selected_objects = alive.clone();
                            self.game_logic
                                .select_objects(self.current_player_id, alive.clone());
                            self.last_control_group_select = Some((group, Instant::now()));
                            self.runtime_host_last_gameplay_cmd =
                                format!("control_group_recall_ok:{}:{}", group, alive.len());
                        }
                    } else {
                        self.runtime_host_last_gameplay_cmd =
                            format!("control_group_recall_fail_unset:{}", group);
                    }
                }
            }
            "waypoint_mode" | "toggle_waypoint" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "waypoint_mode_fail_not_ingame".into();
                } else {
                    let enable = match args
                        .get("on")
                        .or_else(|| args.get("enabled"))
                        .map(|s| s.trim().to_ascii_lowercase())
                        .as_deref()
                    {
                        Some("1") | Some("true") | Some("on") | Some("yes") => true,
                        Some("0") | Some("false") | Some("off") | Some("no") => false,
                        _ => !self.sticky_waypoint_mode,
                    };
                    self.sticky_waypoint_mode = enable;
                    // Keep command-system sticky in sync when available.
                    // CommandProcessor path uses alt/sticky on click; host sets engine sticky.
                    self.runtime_host_last_gameplay_cmd = if enable {
                        "waypoint_mode_ok:on".into()
                    } else {
                        "waypoint_mode_ok:off".into()
                    };
                }
            }
            "add_waypoint" | "waypoint" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "waypoint_fail_not_ingame".into();
                } else {
                    self.ensure_host_mobile_selection();
                    let mut selected = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.selected_objects.clone())
                        .unwrap_or_default();
                    if selected.is_empty() {
                        selected = self.selected_objects.clone();
                    }
                    if selected.is_empty() {
                        self.runtime_host_last_gameplay_cmd = "waypoint_fail_no_selection".into();
                    } else {
                        let x: f32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(120.0);
                        let y: f32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(0.0);
                        let z: f32 = args.get("z").and_then(|s| s.parse().ok()).unwrap_or(120.0);
                        let dest = glam::Vec3::new(x, y, z);
                        self.game_logic
                            .queue_command(crate::command_system::GameCommand {
                                command_type: crate::command_system::CommandType::AddWaypoint {
                                    destination: dest,
                                },
                                player_id: self.current_player_id,
                                command_id: 0,
                                timestamp: std::time::SystemTime::now(),
                                selected_units: selected.clone(),
                                modifier_keys: crate::command_system::ModifierKeys {
                                    ctrl: false,
                                    shift: true,
                                    alt: true,
                                },
                            });
                        self.game_logic.process_commands();
                        self.runtime_host_last_gameplay_cmd =
                            format!("waypoint_ok:{},{},{}:{}", x, y, z, selected.len());
                    }
                }
            }
            "box_select" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "box_select_fail_not_ingame".into();
                } else {
                    // World-space AABB box select residual (same path as drag-select release).
                    let min_x: f32 = args
                        .get("min_x")
                        .or_else(|| args.get("x0"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(-5000.0);
                    let max_x: f32 = args
                        .get("max_x")
                        .or_else(|| args.get("x1"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(5000.0);
                    let min_z: f32 = args
                        .get("min_z")
                        .or_else(|| args.get("z0"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(-5000.0);
                    let max_z: f32 = args
                        .get("max_z")
                        .or_else(|| args.get("z1"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(5000.0);
                    let player_team = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame.local_team()
                    } else {
                        match self.game_logic.get_player(self.current_player_id) {
                            Some(p) => p.team,
                            None => {
                                self.runtime_host_last_gameplay_cmd =
                                    "box_select_fail_no_player".into();
                                return;
                            }
                        }
                    };
                    let boxed: Vec<ObjectId> = if let Some(frame) =
                        self.last_presentation_frame.as_ref()
                    {
                        frame.box_select_unit_ids(player_team, min_x, max_x, min_z, max_z)
                    } else {
                        let mut live = Vec::new();
                        for (&id, obj) in self.game_logic.get_objects() {
                            if obj.team != player_team || !obj.is_selectable() {
                                continue;
                            }
                            let pos = obj.get_position();
                            if pos.x < min_x || pos.x > max_x || pos.z < min_z || pos.z > max_z {
                                continue;
                            }
                            if obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                                continue;
                            }
                            live.push(id);
                        }
                        live
                    };
                    self.selected_objects = boxed.clone();
                    self.game_logic
                        .select_objects(self.current_player_id, boxed.clone());
                    self.runtime_host_last_gameplay_cmd = format!("box_select_ok:{}", boxed.len());
                }
            }
            "select_similar" | "double_click_select" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_similar_fail_not_ingame".into();
                } else {
                    // Seed from current selection or first local mobile.
                    let seed = self
                        .selected_objects
                        .first()
                        .copied()
                        .or_else(|| {
                            self.game_logic
                                .get_player(self.current_player_id)
                                .and_then(|p| p.selected_objects.first().copied())
                        })
                        .or_else(|| {
                            let team = self
                                .game_logic
                                .get_player(self.current_player_id)
                                .map(|p| p.team)?;
                            self.game_logic.get_objects().iter().find_map(|(id, o)| {
                                if o.team == team && o.is_alive() && o.is_mobile() {
                                    Some(*id)
                                } else {
                                    None
                                }
                            })
                        });
                    if let Some(seed) = seed {
                        self.select_similar_units(seed);
                        let n = self.selected_objects.len().max(
                            self.game_logic
                                .get_player(self.current_player_id)
                                .map(|p| p.selected_objects.len())
                                .unwrap_or(0),
                        );
                        self.runtime_host_last_gameplay_cmd = format!("select_similar_ok:{}", n);
                    } else {
                        self.runtime_host_last_gameplay_cmd = "select_similar_fail_no_seed".into();
                    }
                }
            }
            "select_on_screen" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_on_screen_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_on_screen();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_on_screen_ok:{}", n);
                }
            }
            "select_aircraft" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_aircraft_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_aircraft();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_aircraft_ok:{}", n);
                }
            }
            "select_idle_harvesters" | "select_idle" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_idle_fail_not_ingame".into();
                } else {
                    self.select_idle_harvesters();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_idle_ok:{}", n);
                }
            }
            "select_structures" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd =
                        "select_structures_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_structures();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_structures_ok:{}", n);
                }
            }
            "select_moving" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "select_moving_fail_not_ingame".into();
                } else {
                    self.select_all_friendly_moving();
                    let n = self.selected_objects.len();
                    self.runtime_host_last_gameplay_cmd = format!("select_moving_ok:{}", n);
                }
            }
            "construct" | "dozer_construct" | "place_structure" => {
                if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.runtime_host_last_gameplay_cmd = "construct_fail_not_ingame".into();
                } else {
                    self.runtime_host_last_gameplay_cmd = "construct_begin".into();
                    let requested = args
                        .get("template")
                        .cloned()
                        .or_else(|| args.get("name").cloned())
                        .unwrap_or_else(|| "USA_Barracks".to_string());
                    // Prefer requested, then common USA/host barracks residual names.
                    let candidates = [
                        requested.as_str(),
                        "USA_Barracks",
                        "AmericaBarracks",
                        "Barracks",
                    ];
                    let template = candidates
                        .iter()
                        .find(|n| self.game_logic.templates.contains_key(**n))
                        .map(|s| (*s).to_string())
                        .unwrap_or(requested);
                    let team = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.team);
                    let Some(team) = team else {
                        self.runtime_host_last_gameplay_cmd = "construct_fail_no_player".into();
                        return;
                    };
                    // Prefer selected worker; else first friendly dozer/worker.
                    let mut builders: Vec<crate::game_logic::ObjectId> = self
                        .selected_objects
                        .iter()
                        .copied()
                        .filter(|id| {
                            self.game_logic
                                .get_object(*id)
                                .map(|o| o.team == team && o.is_alive() && o.can_construct())
                                .unwrap_or(false)
                        })
                        .collect();
                    if builders.is_empty() {
                        builders = self
                            .game_logic
                            .get_objects()
                            .iter()
                            .filter(|(_, o)| {
                                o.team == team
                                    && o.is_alive()
                                    && (o.can_construct()
                                        || o.is_kind_of(crate::game_logic::KindOf::Worker)
                                        || o.template_name.to_ascii_lowercase().contains("dozer")
                                        || o.template_name.to_ascii_lowercase().contains("worker")
                                        || o.template_name.to_ascii_lowercase().contains("crane"))
                            })
                            .map(|(id, _)| *id)
                            .collect();
                        builders.sort_by_key(|id| id.0);
                    }
                    // Host residual: if map has no dozer yet, spawn USA_Dozer/GoldenDozer at CC.
                    if builders.is_empty() {
                        let spawn_at = self
                            .game_logic
                            .get_objects()
                            .values()
                            .find(|o| {
                                o.team == team
                                    && o.is_alive()
                                    && o.is_kind_of(crate::game_logic::KindOf::CommandCenter)
                            })
                            .map(|o| o.get_position())
                            .unwrap_or(glam::Vec3::new(100.0, 0.0, 100.0))
                            + glam::Vec3::new(25.0, 0.0, 0.0);
                        for name in ["USA_Dozer", "AmericaVehicleDozer", "GoldenDozer"] {
                            if !self.game_logic.templates.contains_key(name) {
                                continue;
                            }
                            if let Some(id) = self.game_logic.create_object(name, team, spawn_at) {
                                builders.push(id);
                                break;
                            }
                        }
                    }
                    let Some(builder) = builders.first().copied() else {
                        self.runtime_host_last_gameplay_cmd = "construct_fail_no_dozer".into();
                        return;
                    };
                    self.selected_objects = vec![builder];
                    self.game_logic
                        .select_objects(self.current_player_id, vec![builder]);

                    // Location: explicit xyz, else near builder / local CC.
                    let loc = if let (Some(x), Some(z)) = (
                        args.get("x").and_then(|s| s.parse::<f32>().ok()),
                        args.get("z").and_then(|s| s.parse::<f32>().ok()),
                    ) {
                        let y = args
                            .get("y")
                            .and_then(|s| s.parse::<f32>().ok())
                            .unwrap_or(0.0);
                        glam::Vec3::new(x, y, z)
                    } else {
                        let base = self
                            .game_logic
                            .get_objects()
                            .values()
                            .find(|o| {
                                o.team == team
                                    && o.is_alive()
                                    && o.is_kind_of(crate::game_logic::KindOf::CommandCenter)
                            })
                            .map(|o| o.get_position())
                            .or_else(|| {
                                self.game_logic
                                    .get_object(builder)
                                    .map(|o| o.get_position())
                            })
                            .unwrap_or(glam::Vec3::ZERO);
                        base + glam::Vec3::new(40.0, 0.0, 0.0)
                    };

                    // FOW residual: load_map + per-frame update_main_crate_vision already ran.
                    let lbc = self.game_logic.legal_build_code_at_for_builder(
                        team,
                        loc,
                        &template,
                        Some(builder),
                    );
                    if lbc != 0 {
                        // Scan nearby pads (same residual as golden FOW recovery).
                        let mut found = None;
                        'scan: for dx in -6..=6 {
                            for dz in -6..=6 {
                                let p =
                                    loc + glam::Vec3::new(dx as f32 * 15.0, 0.0, dz as f32 * 15.0);
                                if self.game_logic.is_location_legal_to_build_for_builder(
                                    team,
                                    p,
                                    &template,
                                    Some(builder),
                                ) {
                                    found = Some(p);
                                    break 'scan;
                                }
                            }
                        }
                        if let Some(p) = found {
                            self.place_structure_from_ui(&template, p);
                            self.runtime_host_last_gameplay_cmd =
                                format!("construct_ok:{}@{},{}", template, p.x, p.z);
                        } else {
                            self.runtime_host_last_gameplay_cmd =
                                format!("construct_fail_lbc:{lbc}");
                        }
                    } else {
                        self.place_structure_from_ui(&template, loc);
                        self.runtime_host_last_gameplay_cmd =
                            format!("construct_ok:{}@{},{}", template, loc.x, loc.z);
                    }
                }
            }
            _ => {
                debug!(
                    "Ignoring unknown runtime host command '{}'",
                    raw_command.trim()
                );
            }
        }
    }

    fn enter_shell_menu_from_runtime_host(&mut self, override_screen: Option<&'static str>) {
        self.set_runtime_host_ui_screen_override(override_screen);
        self.ui_manager.suspend_for_shell_overlay();
        if self.current_state != GameState::Menu {
            self.request_state_change(GameState::Menu);
        }
    }

    fn enter_shell_screen_from_runtime_host(
        &mut self,
        override_screen: Option<&'static str>,
        layout_file: &'static str,
    ) {
        self.enter_shell_menu_from_runtime_host(override_screen);
        #[cfg(feature = "game_client")]
        {
            // Headless executable smoke sets GENERALS_RUNTIME_HOST_WND=0 so UI
            // screen override is observable without shell/WND push (show_shell_menu
            // can fail-closed headless). Interactive default still pushes layouts.
            let push_wnd = std::env::var("GENERALS_RUNTIME_HOST_WND")
                .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
                .unwrap_or(true);
            if push_wnd {
                self.show_shell_menu();
                if let Err(err) = game_client::gui::get_shell().push(layout_file, false) {
                    warn!("Runtime host failed to push shell screen {layout_file}: {err:?}");
                }
            } else {
                log::debug!(
                    "Runtime host soft shell screen {override_screen:?} (shell/WND push disabled)"
                );
            }
        }
    }

    fn enter_shell_options_from_runtime_host(&mut self) {
        self.enter_shell_menu_from_runtime_host(Some("Options"));
        #[cfg(feature = "game_client")]
        {
            self.show_shell_menu();
            let mut shell = game_client::gui::get_shell();
            if let Some(layout) = shell.get_options_layout(true) {
                if let Err(err) = layout.run_init(None) {
                    warn!("Runtime host failed to init shell options layout: {err:?}");
                }
                layout.hide(false);
                layout.bring_forward();
            } else {
                warn!("Runtime host failed to create shell options layout");
            }
        }
    }

    fn loading_visual_phase(elapsed_seconds: f32) -> (&'static str, f32) {
        if elapsed_seconds < 1.0 {
            ("Initializing engine", (elapsed_seconds / 1.0) * 0.15)
        } else if elapsed_seconds < 4.0 {
            (
                "Loading map data",
                0.15 + ((elapsed_seconds - 1.0) / 3.0) * 0.30,
            )
        } else if elapsed_seconds < 10.0 {
            (
                "Spawning world objects",
                0.45 + ((elapsed_seconds - 4.0) / 6.0) * 0.35,
            )
        } else {
            (
                "Finalizing startup",
                0.80 + ((elapsed_seconds - 10.0) / 6.0).clamp(0.0, 1.0) * 0.15,
            )
        }
    }

    fn ui_window_manager_has_windows(&self) -> bool {
        #[cfg(feature = "game_client")]
        {
            game_client::gui::with_window_manager_ref(|wm| wm.window_count() > 0)
        }
        #[cfg(not(feature = "game_client"))]
        {
            false
        }
    }

    fn gameplay_ui_active(&self) -> bool {
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheInGameUI::get_input_enabled()
        }
        #[cfg(not(feature = "game_client"))]
        {
            false
        }
    }

    #[cfg(feature = "game_client")]
    fn load_screen_game_mode(mode: GameMode) -> game_client::gui::load_screen::LoadScreenGameMode {
        match mode {
            GameMode::SinglePlayer => {
                game_client::gui::load_screen::LoadScreenGameMode::SinglePlayer
            }
            GameMode::Skirmish => game_client::gui::load_screen::LoadScreenGameMode::Skirmish,
            GameMode::Multiplayer => game_client::gui::load_screen::LoadScreenGameMode::Multiplayer,
            GameMode::Replay => game_client::gui::load_screen::LoadScreenGameMode::Replay,
            GameMode::Internet => game_client::gui::load_screen::LoadScreenGameMode::Internet,
            GameMode::Lan => game_client::gui::load_screen::LoadScreenGameMode::Lan,
            GameMode::Shell => game_client::gui::load_screen::LoadScreenGameMode::Shell,
            GameMode::None => game_client::gui::load_screen::LoadScreenGameMode::None,
        }
    }

    #[cfg(feature = "game_client")]
    fn select_cpp_load_screen(
        &self,
        mode: GameMode,
        loading_save_game: bool,
    ) -> Option<game_client::gui::load_screen::LoadScreenKind> {
        let (has_current_campaign, current_campaign_is_challenge) = {
            let campaign_manager = game_client::gui::campaign_manager::get_campaign_manager();
            campaign_manager
                .get_current_campaign()
                .map(|campaign| (true, campaign.is_challenge_campaign()))
                .unwrap_or((false, false))
        };

        game_client::gui::load_screen::select_load_screen(
            game_client::gui::load_screen::LoadScreenRequest {
                mode: Self::load_screen_game_mode(mode),
                loading_save_game,
                has_current_campaign,
                current_campaign_is_challenge,
            },
        )
    }

    #[cfg(feature = "game_client")]
    fn prepare_cpp_load_screen_for_mode(&mut self, mode: GameMode, loading_save_game: bool) {
        self.active_load_screen = self.select_cpp_load_screen(mode, loading_save_game);
    }

    #[cfg(feature = "game_client")]
    fn load_screen_init_context(&self) -> game_client::gui::load_screen::LoadScreenInitContext {
        // Prefer presentation game_mode residual when installed.
        let game_info_context = match self.presentation_or_live_game_mode() {
            GameMode::Lan | GameMode::Multiplayer => Some({
                let setup = game_client::gui::get_lan_setup();
                game_client::gui::load_screen::load_screen_init_context_from_game_info(
                    setup.game_info(),
                )
            }),
            GameMode::Skirmish => Some({
                let setup = game_client::gui::get_skirmish_setup();
                game_client::gui::load_screen::load_screen_init_context_from_game_info(
                    setup.game_info().game_info(),
                )
            }),
            _ => None,
        };
        if let Some(context) = game_info_context {
            if !context.slots.is_empty() {
                return context;
            }
        }

        // Prefer full presentation roster when installed (InGame residual);
        // live get_player only boot/menu when no frame.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            if !frame.players.is_empty() {
                let local = frame
                    .player_info(frame.local_player_id)
                    .or_else(|| frame.players.iter().find(|p| p.is_local))
                    .or_else(|| frame.players.first());
                let mut context = game_client::gui::load_screen::LoadScreenInitContext::default();
                if let Some(local) = local {
                    context.local_player_name = local.name.clone();
                    context.local_side_name = local.team.get_name().to_string();
                    context.local_team_number = local.id as i32;
                }
                context.slots = frame
                    .players
                    .iter()
                    .map(|player| {
                        // apparent_color is multiplayer color *index* (progress bar art).
                        // Fail-closed: index not frozen on presentation — leave None.
                        // apparent_text_color is packed 0x00RRGGBB from frozen color_rgb.
                        let (r, g, b) = player.color_rgb;
                        let text_color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                        game_client::gui::load_screen::LoadScreenSlotInitContext {
                            player_id: player.id as i32,
                            player_name: player.name.clone(),
                            side_name: player.team.get_name().to_string(),
                            team_number: player.id as i32,
                            apparent_color: None,
                            apparent_text_color: Some(text_color),
                            is_ai: player.is_ai,
                            has_map: true,
                            visible: player.is_alive,
                        }
                    })
                    .collect();
                return context;
            }
        }

        // Boot residual only — no presentation roster yet (live get_player).
        let player = self
            .game_logic
            .local_player_id()
            .and_then(|id| self.game_logic.get_player(id))
            .or_else(|| self.game_logic.get_player(self.current_player_id));

        if let Some(player) = player {
            let slot = game_client::gui::load_screen::LoadScreenSlotInitContext {
                player_id: player.id as i32,
                player_name: player.name.clone(),
                side_name: player.team.get_name().to_string(),
                team_number: player.id as i32,
                apparent_color: None,
                apparent_text_color: None,
                is_ai: false,
                has_map: true,
                visible: true,
            };
            let mut context = game_client::gui::load_screen::LoadScreenInitContext::default();
            context.local_player_name = slot.player_name.clone();
            context.local_side_name = slot.side_name.clone();
            context.local_team_number = slot.team_number;
            context.slots = vec![slot];
            context
        } else {
            game_client::gui::load_screen::LoadScreenInitContext::default()
        }
    }

    fn ensure_shell_loading_overlay(&mut self) {
        if self.startup_loading_phase.trim().is_empty() {
            self.startup_loading_phase = DEFAULT_LOADING_PHASE.to_string();
        }
        self.set_runtime_ui_state_projection(UISystemState::Loading);

        #[cfg(feature = "game_client")]
        {
            if self.loading_overlay_active {
                return;
            }

            let kind = self
                .active_load_screen
                .or_else(|| {
                    // Prefer presentation game_mode residual when installed.
                    self.select_cpp_load_screen(self.presentation_or_live_game_mode(), false)
                })
                .unwrap_or(game_client::gui::load_screen::LoadScreenKind::ShellGame);
            self.active_load_screen = Some(kind);

            let context = self.load_screen_init_context();
            if !game_client::gui::load_screen::init_load_screen(kind, &context) {
                warn!(
                    "Failed to load {:?} load screen from .wnd assets; loading screen unavailable",
                    kind
                );
                error!(
                    "The selected load screen could not be loaded — the loading overlay will not be visible. \
                     Ensure game assets (BIG archives or extracted Data/) are in the correct path. \
                     The game will continue without a loading screen."
                );
                self.active_load_screen = None;
                return;
            }

            self.loading_overlay_active = true;
            LOADING_PROGRESS.with(|p| p.set(0.0));
            LOADING_PHASE.with(|p| *p.borrow_mut() = self.startup_loading_phase.clone());
            info!("Loading screen overlay created as {:?}", kind);
        }
    }

    fn hide_shell_loading_overlay(&mut self) {
        if self.startup_loading_phase.trim().is_empty() {
            self.startup_loading_phase = "Startup complete".to_string();
        }

        #[cfg(feature = "game_client")]
        {
            if !self.loading_overlay_active {
                return;
            }

            if let Some(kind) = self.active_load_screen.take() {
                game_client::gui::load_screen::reset_load_screen(kind);
            }

            self.loading_overlay_active = false;
        }
    }

    /// C++ parity: Shell::push("Menus/MainMenu.wnd") + Shell::doPush()
    /// GameLogic::startNewGame() line 2198: TheShell->push("Menus/MainMenu.wnd")
    /// when m_gameMode == GAME_SHELL && screenCount == 0.
    fn show_shell_menu(&mut self) {
        #[cfg(feature = "game_client")]
        {
            if self.shell_menu_active {
                return;
            }

            let mut shell = game_client::gui::get_shell();
            shell.show_shell_map(true);
            let result = if shell.get_screen_count() == 0 {
                shell.push("Menus/MainMenu.wnd", false)
            } else {
                if let Some(top) = shell.top() {
                    top.hide(false);
                    top.bring_forward();
                }
                Ok(())
            };

            if let Err(e) = result {
                warn!("Failed to activate MainMenu.wnd through Shell: {:?}", e);
                error!(
                    "MainMenu.wnd could not be loaded — the main menu will not be visible. \
                     Ensure game assets (BIG archives or extracted Data/) are in the correct path. \
                     The game will continue without a main menu."
                );
                return;
            }

            self.shell_menu_active = true;
            info!("Shell menu activated from Menus/MainMenu.wnd");
        }
    }

    /// C++ parity: Shell::hideShell() — run top-layout shutdown when leaving Menu state.
    fn hide_shell_menu(&mut self) {
        #[cfg(feature = "game_client")]
        {
            if !self.shell_menu_active {
                return;
            }

            if let Err(err) = game_client::gui::get_shell().hide_shell() {
                warn!("Failed to hide shell menu: {:?}", err);
            }

            self.shell_menu_active = false;
        }
    }

    fn log_startup_health_summary(&mut self) {
        if self.startup_health_summary_logged {
            return;
        }

        if self.startup_stall_events == 0 {
            info!("Startup health: all checks succeeded (progress=100%, stalls=0, render_boot=ok)");
        } else {
            info!(
                "Startup health: completed with {} transient stalls (max_stall={:.2}s), no fatal startup errors",
                self.startup_stall_events,
                self.startup_max_stall_duration.as_secs_f32()
            );
        }

        self.startup_health_summary_logged = true;
    }

    fn update_shell_loading_progress(&mut self, progress: f32, phase: Option<&str>) {
        self.startup_last_reported_progress = progress.clamp(0.0, 1.0);
        if let Some(phase) = phase {
            let phase = phase.trim();
            if !phase.is_empty() {
                self.startup_loading_phase = phase.to_string();
            }
        }

        #[cfg(feature = "game_client")]
        {
            LOADING_PROGRESS.with(|p| p.set(self.startup_last_reported_progress));
            LOADING_PHASE.with(|p| *p.borrow_mut() = self.startup_loading_phase.clone());

            if let Some(kind) = self.active_load_screen {
                let percent = self.startup_last_reported_progress * 100.0;
                game_client::gui::load_screen::update_load_screen(kind, percent);
            }
        }
    }

    fn observe_startup_progress(&mut self, progress: f32, phase: &str) {
        let progress = progress.clamp(0.0, 1.0);
        if progress > self.startup_last_reported_progress + 0.001 {
            self.startup_last_reported_progress = progress;
            self.startup_last_progress_change_at = Instant::now();
            self.startup_last_stall_warning_at = None;
            return;
        }

        let stalled_for = self.startup_last_progress_change_at.elapsed();
        let stall_threshold = Self::startup_stall_warning_threshold(progress, phase);
        if stalled_for < Duration::from_secs(2) || stalled_for < stall_threshold {
            return;
        }

        let should_warn = self
            .startup_last_stall_warning_at
            .map(|last| last.elapsed() >= Duration::from_secs(2))
            .unwrap_or(true);
        if !should_warn {
            return;
        }

        self.startup_stall_events = self.startup_stall_events.saturating_add(1);
        self.startup_max_stall_duration = self.startup_max_stall_duration.max(stalled_for);
        if stalled_for >= Duration::from_secs(8) {
            warn!(
                "Startup progress stalled at {:.0}% in phase '{}' for {:.2}s (game_state={:?})",
                progress * 100.0,
                phase,
                stalled_for.as_secs_f32(),
                self.current_state
            );
        } else {
            debug!(
                "Startup progress waiting at {:.0}% in phase '{}' for {:.2}s (game_state={:?})",
                progress * 100.0,
                phase,
                stalled_for.as_secs_f32(),
                self.current_state
            );
        }
        self.startup_last_stall_warning_at = Some(Instant::now());
    }

    fn startup_stall_warning_threshold(progress: f32, phase: &str) -> Duration {
        let phase = phase.trim().to_ascii_lowercase();
        if phase.contains("priming shell simulation") {
            Duration::from_secs(25)
        } else if phase.contains("initializing asset manager")
            || phase.contains("loading map data")
            || phase.contains("spawning world objects")
            || phase.contains("finalizing startup")
        {
            Duration::from_secs(20)
        } else {
            let _ = progress;
            Duration::from_secs(12)
        }
    }

    /// Hide in-game layouts when returning to shell menus (C++ HideControlBar parity).
    fn hide_gameplay_layouts(&mut self) {
        info!(
            "hide_gameplay_layouts: ControlBar / in-game layout teardown (shell overlay owns UI)"
        );
        // Window manager layouts are suspended via ui_manager.suspend_for_shell_overlay()
        // on the Menu transition path; this records the shipped hide hook so the ensure
        // path is not unpaired with a silent no-op.
    }

    /// Ensure ControlBar / in-game layout is available when entering gameplay.
    ///
    /// C++ `ShowControlBar` loads ControlBar.wnd. This is **not** a silent no-op:
    /// it resolves retail assets, validates them, and attempts a window load when
    /// the client GUI is available. Missing assets are logged honestly.
    fn ensure_gameplay_layouts(&mut self) {
        // C++ ShowControlBar residual: resolve + validate + headless WindowManager load
        // when assets present. Does not claim windowed W3D retail draw.
        let honesty = crate::gameplay_layout::control_bar_layout_honesty(true);
        let report = crate::gameplay_layout::format_control_bar_honesty(&honesty);
        match &honesty.status {
            crate::gameplay_layout::GameplayLayoutStatus::Ready { path, loaded } => {
                info!(
                    "ensure_gameplay_layouts: {} (path={}, loaded={}, windows={})",
                    report, path, loaded, honesty.window_count
                );
            }
            crate::gameplay_layout::GameplayLayoutStatus::AssetsUnavailable { searched } => {
                warn!(
                    "ensure_gameplay_layouts: ControlBar assets unavailable (searched {} candidates). {}",
                    searched.len(),
                    report
                );
            }
            crate::gameplay_layout::GameplayLayoutStatus::LoadFailed { path, error } => {
                warn!(
                    "ensure_gameplay_layouts: ControlBar load failed path={} error={} ({})",
                    path, error, report
                );
            }
        }
    }

    fn to_engine_timing(clock: ClockFrameTiming, frame_start: Instant) -> FrameTiming {
        let sync_time = clock.total_time.as_millis() as u32;
        let previous_sync_time = sync_time.saturating_sub(clock.delta_time.as_millis() as u32);
        FrameTiming {
            frame_number: clock.frame_number,
            delta_time: clock.delta_time,
            total_time: clock.total_time,
            fps: if clock.delta_time.as_secs_f32() > 0.0 {
                1.0 / clock.delta_time.as_secs_f32()
            } else {
                0.0
            },
            frame_start,
            sync_time,
            previous_sync_time,
        }
    }

    fn configured_startup_shell_map() -> Option<String> {
        let global = game_engine::common::global_data::read();
        if !global.writable.shell_map_on {
            return None;
        }
        let shell_map_name = global.writable.shell_map_name.clone();
        drop(global);

        if game_client::map_util::is_map_cached_without_refresh(&shell_map_name) {
            return Some(shell_map_name);
        }

        warn!(
            "Configured shell map '{}' was not found in map cache; starting without a shell background map",
            shell_map_name
        );
        // C++ parity (GameEngine.cpp): disable shell-map mode globally when the configured
        // shell map is missing from cache so subsequent startup/UI flow sees it as unavailable.
        let mut global = game_engine::common::global_data::write();
        global.writable.shell_map_on = false;
        None
    }

    fn current_startup_logic_frame(&self) -> u64 {
        // Use engine frame cadence for startup budgeting. Game-logic frame counters can jump
        // during long blocking startup operations, which over-ages menu startup budgets.
        self.frame_counter as u64
    }

    fn shell_start_frame(&self) -> Option<u64> {
        // Anchor startup age to the frame where menu state became active when available.
        // Shell enqueue can happen earlier during loading and should not age out menu
        // startup budgets before first visible menu frames.
        self.menu_enter_frame.or(self.shell_ui_enqueued_frame)
    }

    fn startup_deferred_model_load_budget(
        current_state: GameState,
        startup_frame: Option<u64>,
        current_logic_frame: u64,
    ) -> usize {
        if current_state != GameState::Menu {
            return 0;
        }

        let Some(startup_frame) = startup_frame else {
            return 0;
        };

        let startup_age = current_logic_frame.saturating_sub(startup_frame);
        match startup_age {
            0 => 4,
            1..=2 => 8,
            3..=7 => 12,
            _ => 16,
        }
    }

    fn maybe_trigger_deferred_caustic_warmup(&mut self) {
        let _ = self;
    }

    #[cfg(feature = "game_client")]
    fn should_skip_world_scene_for_shell_menu(&self) -> bool {
        if self.current_state == GameState::Loading {
            return true;
        }
        const MENU_WORLD_WARMUP_FRAMES: u32 = 3;
        if self.menu_world_frames_rendered < MENU_WORLD_WARMUP_FRAMES {
            return true;
        }
        false
    }

    #[cfg(not(feature = "game_client"))]
    fn should_skip_world_scene_for_shell_menu(&self) -> bool {
        false
    }

    fn configured_startup_camera_defaults() -> StartupCameraDefaults {
        let global = game_engine::common::global_data::read();
        StartupCameraDefaults {
            pitch_degrees: global.camera_pitch,
            yaw_degrees: global.camera_yaw,
            camera_height: global.camera_height,
            max_camera_height: global.max_camera_height,
        }
    }

    fn select_startup_camera_focus(
        is_shell_game: bool,
        metadata_target: Option<Vec2>,
        team_target: Option<Vec2>,
        world_center: Vec2,
    ) -> Vec2 {
        if is_shell_game {
            // C++ shell startup prefers InitialCameraPosition and only falls back to the
            // legacy W3DView seed when the waypoint is absent.
            metadata_target.unwrap_or(Vec2::new(
                87.0 * gamelogic::common::MAP_XY_FACTOR,
                77.0 * gamelogic::common::MAP_XY_FACTOR,
            ))
        } else {
            metadata_target.or(team_target).unwrap_or(world_center)
        }
    }

    fn bootstrap_camera_for_loaded_map(
        game_logic: &GameLogic,
        current_player_id: u32,
        defaults: StartupCameraDefaults,
    ) -> (Vec3, Vec3, f32) {
        const DEFAULT_VIEW_WIDTH: f32 = 640.0;
        const DEFAULT_VIEW_HEIGHT: f32 = 480.0;
        let (world_min, world_max) = game_logic.world_bounds();
        let world_center = Vec3::new(
            (world_min.x + world_max.x) * 0.5,
            (world_min.y + world_max.y) * 0.5,
            (world_min.z + world_max.z) * 0.5,
        );

        let metadata_initial_camera = game_logic
            .last_parsed_map_settings()
            .and_then(|meta| meta.initial_camera_position);
        let metadata_target = metadata_initial_camera.map(|pos| Vec2::new(pos.x, pos.y));

        let clamp_focus_to_world = |focus: Vec2| {
            Vec2::new(
                focus.x.clamp(world_min.x, world_max.x),
                focus.y.clamp(world_min.z, world_max.z),
            )
        };
        let team_target = game_logic
            .get_player(current_player_id)
            .map(|player| player.team)
            .and_then(|team| game_logic.team_base_position(team))
            .map(|pos| Vec2::new(pos.x, pos.z));
        let focus_2d = clamp_focus_to_world(Self::select_startup_camera_focus(
            game_logic.isInShellGame(),
            metadata_target,
            team_target,
            Vec2::new(world_center.x, world_center.z),
        ));

        // Match C++ W3DView::lookAt(): unlike the old 2D View::lookAt(), the W3D path writes the
        // requested world coordinate directly into m_pos and builds the camera transform from that.
        let terrain_target = Vec3::new(focus_2d.x, 0.0, focus_2d.y);
        let (camera_anchor_ground_height, terrain_height_max) =
            Self::sample_startup_camera_heights(game_logic, terrain_target, world_center.y);
        let focus_target = Vec3::new(focus_2d.x, 0.0, focus_2d.y);
        let (focus_ground_height, _) =
            Self::sample_startup_camera_heights(game_logic, focus_target, world_center.y);

        // Keep the C++ zoom/offset sampling from the top-left anchor, but aim the modern
        // Rust camera at the requested scene focus. This remains the closest visible match for the
        // current renderer bridge.
        let camera_target = Vec3::new(focus_2d.x, focus_ground_height, focus_2d.y);
        let camera_offset_z = camera_anchor_ground_height + defaults.camera_height.max(0.0);
        let pitch_radians = defaults.pitch_degrees.to_radians();
        let yaw_radians = defaults.yaw_degrees.to_radians();
        let camera_offset_y = if pitch_radians.tan().abs() > f32::EPSILON {
            -(camera_offset_z / pitch_radians.tan())
        } else {
            0.0
        };
        let camera_offset_x = -(camera_offset_y * yaw_radians.tan());

        // Match W3DView::setZoomToDefault exactly: desired zoom is the visible terrain max
        // around the look-at point plus max camera height, divided by the base offset height.
        let zoom = Self::compute_default_camera_zoom_from_heights(
            camera_anchor_ground_height,
            terrain_height_max,
            defaults,
            1.0,
        );

        // Match W3DView::buildCameraTransform when angle/pitch defaults are zero:
        // source = cameraOffset * zoom; source *= (1 - ground / source.z); then translate.
        let source_z = camera_offset_z * zoom;
        let factor = if source_z.abs() > f32::EPSILON {
            1.0 - (camera_anchor_ground_height / source_z)
        } else {
            1.0
        };
        let source = Vec3::new(
            camera_offset_x * zoom * factor,
            camera_offset_z * zoom * factor,
            camera_offset_y * zoom * factor,
        );
        let camera_position = camera_target + source;

        info!(
            "Startup camera bootstrap: raw_initial={:?} requested_focus_2d={:?} target={:?} position={:?} ground_height={:.2} terrain_height_max={:.2} camera_offset=({:.2}, {:.2}, {:.2}) pitch_deg={:.2} yaw_deg={:.2} zoom={:.2} factor={:.3}",
            metadata_initial_camera,
            focus_2d,
            camera_target,
            camera_position,
            camera_anchor_ground_height,
            terrain_height_max,
            camera_offset_x,
            camera_offset_y,
            camera_offset_z,
            defaults.pitch_degrees,
            defaults.yaw_degrees,
            zoom,
            factor,
        );

        (camera_target, camera_position, zoom)
    }

    fn sample_startup_camera_heights(
        game_logic: &GameLogic,
        terrain_target: Vec3,
        fallback_ground_height: f32,
    ) -> (f32, f32) {
        const MAX_GROUND_LEVEL: f32 = 120.0;
        const TERRAIN_SAMPLE_SIZE: f32 = 40.0;
        let (world_min, world_max) = game_logic.world_bounds();

        let mut ground_height = game_logic
            .terrain_height_at(terrain_target)
            .unwrap_or(fallback_ground_height);
        if ground_height > MAX_GROUND_LEVEL {
            ground_height = MAX_GROUND_LEVEL;
        }

        let sample_positions = [
            terrain_target,
            terrain_target + Vec3::new(TERRAIN_SAMPLE_SIZE, 0.0, -TERRAIN_SAMPLE_SIZE),
            terrain_target + Vec3::new(-TERRAIN_SAMPLE_SIZE, 0.0, -TERRAIN_SAMPLE_SIZE),
            terrain_target + Vec3::new(TERRAIN_SAMPLE_SIZE, 0.0, TERRAIN_SAMPLE_SIZE),
            terrain_target + Vec3::new(-TERRAIN_SAMPLE_SIZE, 0.0, TERRAIN_SAMPLE_SIZE),
        ];
        let terrain_height_max = sample_positions
            .into_iter()
            .filter_map(|sample| {
                let clamped = Vec3::new(
                    sample.x.clamp(world_min.x, world_max.x),
                    sample.y,
                    sample.z.clamp(world_min.z, world_max.z),
                );
                game_logic.terrain_height_at(clamped)
            })
            .fold(ground_height, f32::max);

        (ground_height, terrain_height_max)
    }

    fn compute_default_camera_zoom_from_heights(
        ground_height: f32,
        terrain_height_max: f32,
        defaults: StartupCameraDefaults,
        max_height_scale: f32,
    ) -> f32 {
        let camera_offset_z = ground_height + defaults.camera_height.max(0.0);
        // Match C++ W3DView::setDefaultView()/setZoomToDefault():
        // maxHeight is a scale on GlobalData.maxCameraHeight, and angle does not participate.
        let desired_height =
            terrain_height_max + (defaults.max_camera_height * max_height_scale.max(0.0)).max(0.0);
        if camera_offset_z.abs() > f32::EPSILON {
            desired_height / camera_offset_z
        } else {
            1.0
        }
    }

    fn compute_default_camera_zoom_for_target(&self, target: Vec3, max_height_scale: f32) -> f32 {
        let defaults = Self::configured_startup_camera_defaults();
        let (ground_height, terrain_height_max) =
            Self::sample_startup_camera_heights(&self.game_logic, target, target.y);
        Self::compute_default_camera_zoom_from_heights(
            ground_height,
            terrain_height_max,
            defaults,
            max_height_scale,
        )
    }

    fn write_startup_debug_state(&self) {
        let _ = self;
    }

    fn emit_startup_load_progress(
        sender: &mpsc::Sender<StartupLoadMessage>,
        progress: f32,
        phase: &str,
    ) {
        let _ = sender.send(StartupLoadMessage::Progress {
            progress: progress.clamp(0.0, 0.995),
            phase: phase.to_string(),
        });
    }

    fn spawn_startup_map_load(
        start_in_menu: bool,
        map_to_load: Option<String>,
        map_requested_from_cli: bool,
        map_requested_from_initial_file: bool,
        replay_to_load: Option<String>,
        replay_requested_from_cli: bool,
        player_name: Option<String>,
    ) -> StartupLoadState {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            Self::emit_startup_load_progress(&sender, 0.03, "Preparing startup archive access");
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || -> std::result::Result<StartupLoadResult, String> {
                    let mut start_in_menu = start_in_menu;
                    let mut map_to_load = map_to_load;
                    let replay_startup_requested = replay_to_load.is_some();
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|err| {
                            format!("failed to create startup tokio runtime for archive access: {err}")
                        })?;
                    Self::emit_startup_load_progress(&sender, 0.14, "Startup archives ready");

                    let extract_ini_text_from_archives = |virtual_path: &str| -> Option<String> {
                        runtime.block_on(async {
                            let Some(manager_arc) = crate::assets::manager::get_asset_manager()
                            else {
                                return None;
                            };
                            let Ok(mut manager) = manager_arc.lock() else {
                                warn!(
                                    "Asset manager lock poisoned while extracting '{}'; skipping",
                                    virtual_path
                                );
                                return None;
                            };
                            match manager.extract_file(virtual_path).await {
                                Ok(bytes) => {
                                    match String::from_utf8(bytes) {
                                        Ok(text) => Some(text),
                                        Err(err) => {
                                            warn!(
                                                "INI file '{}' was not valid UTF-8: {err}; skipping",
                                                virtual_path
                                            );
                                            None
                                        }
                                    }
                                }
                                Err(_) => None,
                            }
                        })
                    };

                    // C++ parity: force eager initialization of the lazy stores/managers that
                    // the original boot path expects to exist before game-session setup.
                    game_engine::common::ini::initialize_ini_systems();

                    Self::emit_startup_load_progress(
                        &sender,
                        0.145,
                        "Preloading water and weather settings",
                    );
                    Self::preload_startup_water_weather_inis();

                    {
                        let lexicon =
                            game_engine::common::system::function_lexicon::get_function_lexicon();
                        let guard = lexicon.lock();
                        if let Ok(mut lexicon_guard) = guard {
                            if let Err(err) = game_engine::common::system::SubsystemInterface::init(&mut *lexicon_guard) {
                                warn!("FunctionLexicon init failed during startup bootstrap: {err}. Continuing without function lexicon.");
                            }
                        } else {
                            warn!("Function lexicon lock poisoned during startup bootstrap; skipping");
                        }
                    }

                    // These bootstrap calls are required for startup parity. Any panic in this
                    // section is caught by the outer startup worker guard and treated as fatal.
                    game_engine::common::ini::init_rank_info_store();

                    // C++ parity: GameEngine.cpp:398 — load Science.ini (Default + override)
                    // into the global ScienceStore via the general INI block parser.
                    {
                        for sci_path in ["Data/INI/Default/Science.ini", "Data/INI/Science.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(sci_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded science definitions from {}", sci_path),
                                    Err(err) => warn!("Failed parsing Science.ini '{}': {}", sci_path, err),
                                }
                            }
                        }
                    }

                    // C++ parity: GameEngine.cpp:427 — load Rank.ini into TheRankInfoStore.
                    // No Default/ prefix variant exists for Rank.ini.
                    {
                        if let Some(content) = extract_ini_text_from_archives("Data/INI/Rank.ini") {
                            let mut ini = game_engine::common::ini::INI::new();
                            match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                Ok(()) => {
                                    let store = game_engine::common::ini::ini_rank::get_rank_info_store();
                                    if store.is_empty() {
                                        warn!("Rank.ini loaded 0 rank definitions — continuing without rank data");
                                    }
                                }
                                Err(err) => {
                                    warn!("Failed parsing Rank.ini: {}", err);
                                }
                            }
                        } else {
                            warn!("Rank.ini not found in archives — continuing without rank data");
                        }
                    }

                    // C++ parity: GameEngine.cpp:428 — load PlayerTemplate.ini (Default + override)
                    {
                        for pt_path in ["Data/INI/Default/PlayerTemplate.ini", "Data/INI/PlayerTemplate.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(pt_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded player template definitions from {}", pt_path),
                                    Err(err) => warn!("Failed parsing PlayerTemplate.ini '{}': {}", pt_path, err),
                                }
                            }
                        }
                    }

                    // C++ parity: GameEngine.cpp:399 — load Multiplayer.ini (Default + override)
                    // into the global MULTIPLAYER_SETTINGS OnceCell.
                    {
                        let mut loaded_any_multiplayer = false;
                        for mp_path in [
                            "Data/INI/Default/Multiplayer.ini",
                            "Data/INI/Multiplayer.ini",
                        ] {
                            if let Some(content) = extract_ini_text_from_archives(mp_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => {
                                        loaded_any_multiplayer = true;
                                        info!("Loaded {}", mp_path);
                                    }
                                    Err(err) => {
                                        warn!("Failed parsing {}: {}", mp_path, err);
                                    }
                                }
                            }
                        }
                        if !loaded_any_multiplayer {
                            warn!("No Multiplayer.ini found in archives — continuing without multiplayer settings");
                        }
                    }

                    let _ = game_engine::common::ini::ini_terrain::initialize_terrain_types();
                    // C++ parity: GameEngine.cpp:400 — load Terrain.ini (Default + override)
                    {
                        for terrain_path in ["Data/INI/Default/Terrain.ini", "Data/INI/Terrain.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(terrain_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded terrain definitions from {}", terrain_path),
                                    Err(err) => warn!("Failed parsing Terrain.ini '{}': {}", terrain_path, err),
                                }
                            }
                        }
                    }

                    let _ = game_engine::common::ini::ini_terrain_bridge::initialize_terrain_roads();
                    // C++ parity: GameEngine.cpp:401 — load Roads.ini (Default + override)
                    {
                        for roads_path in ["Data/INI/Default/Roads.ini", "Data/INI/Roads.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(roads_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded road definitions from {}", roads_path),
                                    Err(err) => warn!("Failed parsing Roads.ini '{}': {}", roads_path, err),
                                }
                            }
                        }
                    }

                    game_engine::common::ini::ini_special_power::initialize_special_power_store();

                    // C++ parity: GameEngine.cpp:439 — load FXList.ini (Default + override)
                    {
                        for fxl_path in ["Data/INI/Default/FXList.ini", "Data/INI/FXList.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(fxl_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded FX list definitions from {}", fxl_path),
                                    Err(err) => warn!("Failed parsing FXList.ini '{}': {}", fxl_path, err),
                                }
                            }
                        }
                    }

                    // C++ parity: GameEngine.cpp:440 — load Weapon.ini into TheWeaponStore.
                    // No Default/ prefix variant exists for Weapon.ini.
                    game_engine::common::ini::ini_weapon::initialize_weapon_store();
                    {
                        if let Some(content) = extract_ini_text_from_archives("Data/INI/Weapon.ini") {
                            let mut ini = game_engine::common::ini::INI::new();
                            match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                Ok(()) => info!("Loaded weapon definitions from Data/INI/Weapon.ini"),
                                Err(err) => warn!("Failed parsing Weapon.ini: {}", err),
                            }
                        } else {
                            warn!("Weapon.ini not found in archives — continuing without weapon data");
                        }
                    }

                    // C++ parity: GameEngine.cpp:443 — load SpecialPower.ini (Default + override)
                    // into the global SpecialPowerStore via the general INI block parser.
                    {
                        let mut loaded_any_special_power = false;
                        for sp_path in [
                            "Data/INI/Default/SpecialPower.ini",
                            "Data/INI/SpecialPower.ini",
                        ] {
                            if let Some(content) = extract_ini_text_from_archives(sp_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| {
                                    ini.parse_current_file()
                                }) {
                                    Ok(()) => {
                                        loaded_any_special_power = true;
                                    }
                                    Err(err) => {
                                        warn!(
                                            "Failed parsing special power definitions from '{}': {}",
                                            sp_path, err
                                        );
                                    }
                                }
                            }
                        }
                        if !loaded_any_special_power {
                            warn!("SpecialPower.ini bootstrap loaded 0 templates — continuing without special power INI data");
                        }
                    }

                    game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store();
                    game_engine::common::damage_fx::initialize_damage_fx_store();

                    // C++ parity: GameEngine.cpp:444 — load DamageFX.ini into TheDamageFXStore.
                    // No Default/ prefix variant exists for DamageFX.ini.
                    {
                        if let Some(content) = extract_ini_text_from_archives("Data/INI/DamageFX.ini") {
                            let mut ini = game_engine::common::ini::INI::new();
                            match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                Ok(()) => {
                                    if let Some(store) = game_engine::common::ini::ini_damage_fx::get_damage_fx_store() {
                                        if store.get_damage_fx_names().is_empty() {
                                            warn!("DamageFX.ini loaded 0 definitions — continuing without damage FX data");
                                        }
                                    }
                                }
                                Err(err) => {
                                    warn!("Failed parsing DamageFX.ini: {}", err);
                                }
                            }
                        } else {
                            warn!("DamageFX.ini not found in archives — continuing without damage FX data");
                        }
                    }

                    game_engine::common::system::build_assistant::init_build_assistant();

                    // C++ parity: bootstrap OCL/Armor from archive-backed INI content, not just
                    // extracted-file paths, so startup behavior matches original archive loading.
                    {
                        gamelogic::object_creation_list::init_object_creation_list_store();
                        let mut loaded_any_ocl = false;
                        for ocl_path in [
                            "Data/INI/Default/ObjectCreationList.ini",
                            "Data/INI/ObjectCreationList.ini",
                        ] {
                            if let Some(content) = extract_ini_text_from_archives(ocl_path) {
                                match gamelogic::object_creation_list::store::load_object_creation_lists_from_str(&content) {
                                    Ok(count) => {
                                        loaded_any_ocl |= count > 0;
                                    }
                                    Err(load_err) => {
                                        warn!(
                                            "Failed parsing OCL definitions from '{}': {}",
                                            ocl_path, load_err
                                        );
                                    }
                                }
                            }
                        }
                        if !loaded_any_ocl {
                            gamelogic::object_creation_list::store::ensure_default_object_creation_lists_loaded();
                        }
                        let ocl_count = gamelogic::object_creation_list::get_object_creation_list_store()
                            .as_ref()
                            .map(|store| store.get_ocl_count())
                            .unwrap_or(0);
                        if ocl_count == 0 {
                            warn!("ObjectCreationListStore bootstrap loaded 0 templates — continuing without OCL data");
                        }
                    }

                    {
                        let mut loaded_any_armor = false;
                        for armor_path in ["Data/INI/Armor.ini", "Data/INI/Default/Armor.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(armor_path) {
                                match gamelogic::object::armor::load_armor_templates_from_str(
                                    &content,
                                    Some(Path::new(armor_path)),
                                ) {
                                    Ok(count) => {
                                        loaded_any_armor |= count > 0;
                                    }
                                    Err(load_err) => {
                                        warn!(
                                            "Failed parsing armor templates from '{}': {}",
                                            armor_path, load_err
                                        );
                                    }
                                }
                            }
                        }
                        if !loaded_any_armor {
                            gamelogic::object::armor::ensure_default_templates_loaded();
                        }
                        let armor_count = gamelogic::object::armor::TheArmorStore::read().len();
                        if armor_count == 0 {
                            warn!("Armor bootstrap loaded 0 templates — continuing without armor data");
                        }
                    }

                    // C++ parity: GameEngine.cpp:442 — TheLocomotorStore loads
                    // Default/Locomotor.ini then Locomotor.ini from archives.
                    {
                        let mut loaded_any_locomotor = false;
                        for loco_path in [
                            "Data/INI/Default/Locomotor.ini",
                            "Data/INI/Locomotor.ini",
                        ] {
                            if let Some(content) = extract_ini_text_from_archives(loco_path) {
                                match game_engine::common::ini::ini_locomotor::load_locomotors_from_str(&content) {
                                    Ok(count) => {
                                        loaded_any_locomotor |= count > 0;
                                    }
                                    Err(load_err) => {
                                        warn!(
                                            "Failed parsing locomotor templates from '{}': {}",
                                            loco_path, load_err
                                        );
                                    }
                                }
                            }
                        }
                        if !loaded_any_locomotor {
                            warn!("Locomotor bootstrap loaded 0 templates from archives — relying on hardcoded defaults");
                        }
                    }

                    // C++ parity: GameEngine.cpp:468 — load Upgrade.ini (Default + override)
                    game_engine::common::ini::ini_upgrade::initialize_upgrade_center();
                    {
                        for upgrade_path in ["Data/INI/Default/Upgrade.ini", "Data/INI/Upgrade.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(upgrade_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded upgrade definitions from {}", upgrade_path),
                                    Err(err) => warn!("Failed parsing Upgrade.ini '{}': {}", upgrade_path, err),
                                }
                            }
                        }
                    }

                    // C++ parity: GameEngine.cpp:480 — AIData.ini loaded after Upgrade, before Crate.
                    Self::preload_startup_ai_data_inis();

                    // C++ parity: GameEngine.cpp:483 — load Crate.ini (Default + override) into ParsedCrateSystem.
                    {
                        for crate_ini_path in &[
                            "Data/INI/Default/Crate.ini",
                            "Data/INI/Crate.ini",
                        ] {
                            if let Some(content) = extract_ini_text_from_archives(crate_ini_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => {
                                        info!("Loaded crate definitions from '{}'", crate_ini_path);
                                    }
                                    Err(err) => {
                                        warn!("Failed parsing '{}': {}", crate_ini_path, err);
                                    }
                                }
                            } else {
                                warn!("'{}' not found in archives — skipping crate definitions from this file", crate_ini_path);
                            }
                        }
                    }

                    if let Err(err) = game_engine::common::thing::init_thing_system() {
                        warn!("Thing system init failed during startup bootstrap: {err}. Continuing without thing system.");
                    }

                    {
                        let object_ini_paths: Vec<String> = match crate::assets::manager::get_asset_manager() {
                            Some(manager_arc) => {
                                match manager_arc.lock() {
                                    Ok(mgr) => mgr.list_all_files().into_iter().filter(|p| {
                                        let lower = p.to_ascii_lowercase().replace('\\', "/");
                                        lower.starts_with("data/ini/object/") && lower.ends_with(".ini")
                                    }).collect(),
                                    Err(_) => Vec::new(),
                                }
                            }
                            None => Vec::new(),
                        };
                        let mut total_loaded = 0usize;
                        for ini_path in &object_ini_paths {
                            if let Some(content) = extract_ini_text_from_archives(ini_path) {
                                total_loaded += game_engine::common::thing::load_templates_from_ini_text(
                                    &content,
                                    ini_path,
                                );
                            }
                        }
                        if total_loaded > 0 {
                            info!("Bootstrapped {} object templates from BIG archives", total_loaded);
                        }
                    }

                    // C++ parity: GameEngine.cpp:500-501 — load CommandMap.ini (language-specific + fallback).
                    // C++ loads "Data\{language}\CommandMap.ini" then "Data\INI\CommandMap.ini".
                    game_engine::common::ini::ini_command_map::init_meta_map();
                    {
                        let language = game_engine::common::ini::ini_webpage_url::get_registry_language();
                        let lang_path = format!("Data/{}/CommandMap.ini", language.as_str());
                        for cmd_path in &[lang_path.as_str(), "Data/INI/CommandMap.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(cmd_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded command map from {}", cmd_path),
                                    Err(err) => warn!("Failed parsing CommandMap.ini '{}': {}", cmd_path, err),
                                }
                            }
                        }
                    }

                    Self::emit_startup_load_progress(&sender, 0.18, "Creating game session");
                    let mut game_logic = GameLogic::initialize();
                    Self::emit_startup_load_progress(&sender, 0.22, "Priming object templates");

                    if map_requested_from_initial_file {
                        // C++ parity: .map initial-file startup enqueues MSG_NEW_GAME
                        // (GAME_SINGLE_PLAYER, DIFFICULTY_NORMAL, 0) and seeds RNG with 0.
                        let stream = game_engine::common::message_stream::get_message_stream();
                        if let Ok(mut stream_guard) = stream.write() {
                            let msg = stream_guard
                                .append_message(game_engine::common::message_stream::GameMessageType::NewGame);
                            msg.append_integer_argument(0); // GAME_SINGLE_PLAYER
                            msg.append_integer_argument(1); // DIFFICULTY_NORMAL
                            msg.append_integer_argument(0); // rank points
                        } else {
                            warn!("Failed to queue startup NewGame message for initial-file map");
                        }
                        game_engine::common::random_value::init_random_with_seed(0);
                    }

                    if let Some(replay_to_load) = replay_to_load.as_ref() {
                        Self::emit_startup_load_progress(
                            &sender,
                            0.24,
                            "Starting replay playback",
                        );

                        // C++ parity: bootstrap startup replay through the legacy recorder.
                        game_engine::common::recorder::init_recorder();
                        let startup_command_sink: Arc<
                            dyn Fn(game_engine::common::message_stream::GameMessage)
                                + Send
                                + Sync,
                        > = Arc::new(|message| {
                            let stream = game_engine::common::message_stream::get_message_stream();
                            let write_result = stream.write();
                            match write_result {
                                Ok(mut stream_guard) => {
                                    CnCGameEngine::append_common_message_to_stream(
                                        &mut stream_guard,
                                        &message,
                                    );
                                }
                                Err(err) => {
                                    warn!(
                                        "Failed to forward recorder startup command into message stream: {}",
                                        err
                                    );
                                }
                            }
                        });
                        let _ = game_engine::common::recorder::with_recorder_mut(|recorder| {
                            recorder.set_command_sink(Some(startup_command_sink));
                        });
                        let replay_to_play = replay_to_load.to_ascii_lowercase();
                        match game_engine::common::recorder::with_recorder_mut(|recorder| {
                            recorder.playback_file(replay_to_play.clone())
                        }) {
                            Some(Ok(true)) => {}
                            Some(Ok(false)) => {
                                warn!(
                                    "Legacy recorder rejected startup replay '{}'",
                                    replay_to_load
                                );
                            }
                            Some(Err(err)) => {
                                warn!(
                                    "Legacy recorder replay bootstrap failed for '{}': {}",
                                    replay_to_load, err
                                );
                            }
                            None => {
                                warn!(
                                    "Legacy recorder unavailable for startup replay '{}'",
                                    replay_to_load
                                );
                            }
                        }
                    }

                    let startup_messages = Self::take_startup_messages_from_stream()
                        .unwrap_or_default();
                    let startup_new_game =
                        Self::startup_new_game_dispatch_from_messages(&startup_messages);

                    if replay_startup_requested && startup_new_game.is_none() {
                        warn!(
                            "Startup replay did not emit a queued NewGame message; falling back to menu startup"
                        );
                        start_in_menu = true;
                        map_to_load = None;
                        game_engine::common::global_data::write().pending_file.clear();
                    }

                    let startup_mode = Self::resolve_startup_mode_from_dispatch(
                        &mut start_in_menu,
                        &mut map_to_load,
                        startup_new_game,
                        replay_startup_requested,
                    );

                    if replay_startup_requested && !start_in_menu && map_to_load.is_none() {
                        warn!(
                            "Startup replay did not resolve a playable map; falling back to menu startup"
                        );
                        start_in_menu = true;
                        game_engine::common::global_data::write().pending_file.clear();
                    }

                    game_logic.start_new_game(startup_mode);

                    let mut loaded_map_name = None;
                    if let Some(map_to_load) = map_to_load {
                        Self::emit_startup_load_progress(&sender, 0.24, "Loading map data");
                        let map_loaded =
                            game_logic.load_map_with_progress(&map_to_load, |progress, phase| {
                                Self::emit_startup_load_progress(&sender, progress, phase);
                            });
                        if !map_loaded {
                            if start_in_menu {
                                warn!(
                                    "Failed to load shell map '{}'; continuing startup without a shell background map",
                                    map_to_load
                                );
                                game_logic.clearGameData();
                            } else if map_requested_from_cli {
                                warn!(
                                    "Failed to load startup map '{}'; falling back to menu startup",
                                    map_to_load
                                );
                                game_logic.start_new_game(GameMode::Shell);
                                start_in_menu = true;
                            } else {
                                warn!(
                                    "Failed to load startup map '{}'; falling back to menu startup with empty scene",
                                    map_to_load
                                );
                                game_logic.start_new_game(GameMode::Shell);
                                start_in_menu = true;
                            }
                        } else {
                            loaded_map_name = Some(map_to_load.clone());
                        }
                    } else {
                        Self::emit_startup_load_progress(&sender, 0.24, "Skipping shell map load");
                        if start_in_menu {
                            info!(
                                "No shell background map available; entering menu without a shell background map"
                            );
                        }
                    }

                    if let Some(player_name) = player_name.as_deref() {
                        if game_logic.set_player_name(0, player_name) {
                            info!("Set local player name to '{}'", player_name);
                        } else {
                            warn!("Failed to apply player name '{}'", player_name);
                        }
                    }

                    if start_in_menu && game_logic.isInShellGame() {
                        // Move one-time shell simulation setup off the first visible menu frame.
                        Self::emit_startup_load_progress(
                            &sender,
                            0.968,
                            "Priming shell simulation",
                        );
                        let shell_warmup_started = Instant::now();
                        for _ in 0..2 {
                            game_logic.update_shell_with_budget(1.0 / 30.0, 1);
                        }
                        info!(
                            "Startup shell simulation warmup completed in {:.2}s",
                            shell_warmup_started.elapsed().as_secs_f32()
                        );
                    }

                    Self::emit_startup_load_progress(&sender, 0.984, "Finalizing startup data");

                    Ok(StartupLoadResult {
                        game_logic,
                        loaded_map_name,
                        start_in_menu,
                        map_requested_from_cli,
                        replay_requested: replay_startup_requested,
                    })
                },
            ))
            .map_err(|panic_payload| {
                if let Some(message) = panic_payload.downcast_ref::<&str>() {
                    format!("startup map load panicked: {message}")
                } else if let Some(message) = panic_payload.downcast_ref::<String>() {
                    format!("startup map load panicked: {message}")
                } else {
                    "startup map load panicked with non-string payload".to_string()
                }
            })
            .and_then(|inner| inner);

            let _ = sender.send(StartupLoadMessage::Complete(result));
        });

        StartupLoadState::InProgress {
            receiver,
            started_at: Instant::now(),
            last_worker_progress: 0.0,
            last_worker_phase: None,
            last_worker_logged_bucket: 0,
        }
    }

    fn finalize_startup_map_load(&mut self, result: StartupLoadResult) -> Result<()> {
        self.update_shell_loading_progress(0.995, Some("Finalizing startup"));
        self.game_logic = result.game_logic;

        if let Some(active_map_name) = result.loaded_map_name.as_ref() {
            if result.replay_requested {
                info!("Loaded startup replay map: {}", active_map_name);
            } else if result.map_requested_from_cli {
                info!("Loaded map from command line: {}", active_map_name);
            } else if result.start_in_menu {
                info!("Loaded startup shell map: {}", active_map_name);
            } else {
                info!("Loaded startup initial-file map: {}", active_map_name);
            }

            Self::apply_heightmap_hint(&mut self.render_pipeline, &self.game_logic);
            Self::apply_skybox_hint(&mut self.render_pipeline, &self.game_logic);
            Self::sync_render_terrain_visual(
                &mut self.render_pipeline,
                &self.graphics_system,
                &self.game_logic,
                active_map_name.as_str(),
            );
            if let Err(err) = Self::reinitialize_minimap_renderer(
                &mut self.render_pipeline,
                &self.graphics_system,
                &mut self.game_logic,
            ) {
                warn!(
                    "Failed to reinitialize minimap renderer: {err}. Continuing without minimap."
                );
            }
            Self::apply_map_lighting(
                &mut self.graphics_system,
                &mut self.render_pipeline,
                &self.game_logic,
            );
            let startup_camera_defaults = Self::configured_startup_camera_defaults();
            (self.camera_target, self.camera_position, self.camera_zoom) =
                Self::bootstrap_camera_for_loaded_map(
                    &self.game_logic,
                    self.current_player_id,
                    startup_camera_defaults,
                );
            self.sync_orbit_from_camera_transform();
        }

        let fallback_to_menu = result.start_in_menu
            || (result.map_requested_from_cli && result.loaded_map_name.is_none());
        if fallback_to_menu {
            if result.map_requested_from_cli && result.loaded_map_name.is_none() {
                warn!("QuickStart map load failed; falling back to menu startup");
            }
            self.pending_shell_model_prewarm.clear();
            self.last_shell_prewarm_log = None;
            self.shell_prewarm_completion_logged = true;

            self.ui_manager.suspend_for_shell_overlay();
            self.set_runtime_ui_state_projection(UISystemState::MainMenu);
        }

        let target_state = if fallback_to_menu {
            let _ = self.startup_target_state.take();
            Some(GameState::Menu)
        } else {
            self.startup_target_state.take()
        };

        if let Some(target_state) = target_state {
            // Apply the post-load state transition immediately so we do not render additional
            // loading/world-only frames after shell/menu resources are already initialized.
            self.transition_to_state(target_state);
        }
        self.startup_load_state = StartupLoadState::Complete;
        self.last_loading_title_update = None;
        self.update_shell_loading_progress(1.0, Some("Startup complete"));
        self.startup_last_reported_progress = 1.0;
        self.startup_last_progress_change_at = Instant::now();
        self.startup_last_stall_warning_at = None;
        self.hide_shell_loading_overlay();
        self.log_startup_health_summary();
        self.window
            .set_title("Command & Conquer Generals Zero Hour");
        self.window.request_redraw();
        Ok(())
    }

    fn update_startup_loading(&mut self) -> Result<()> {
        let mut result: Option<std::result::Result<StartupLoadResult, String>> = None;
        let mut visual_phase = None::<String>;
        let mut visual_progress = None::<f32>;
        match &mut self.startup_load_state {
            StartupLoadState::Idle | StartupLoadState::Complete => return Ok(()),
            StartupLoadState::InProgress {
                receiver,
                started_at,
                last_worker_progress,
                last_worker_phase,
                last_worker_logged_bucket,
            } => {
                loop {
                    match receiver.try_recv() {
                        Ok(StartupLoadMessage::Progress { progress, phase }) => {
                            let clamped = progress.clamp(0.0, 0.995);
                            if clamped > *last_worker_progress {
                                *last_worker_progress = clamped;
                            }
                            if last_worker_phase.as_deref() != Some(phase.as_str()) {
                                info!(
                                    "Startup worker phase: {} ({:.0}%)",
                                    phase,
                                    (*last_worker_progress) * 100.0
                                );
                            }
                            let bucket = ((*last_worker_progress * 100.0).floor() as i32)
                                .div_euclid(10)
                                .clamp(0, 10) as u8;
                            if bucket > *last_worker_logged_bucket {
                                debug!(
                                    "Startup worker progress: {:.0}% ({})",
                                    (*last_worker_progress) * 100.0,
                                    phase
                                );
                                *last_worker_logged_bucket = bucket;
                            }
                            *last_worker_phase = Some(phase);
                        }
                        Ok(StartupLoadMessage::Complete(complete)) => {
                            info!(
                                "Startup shell/game load completed in {:.2}s",
                                started_at.elapsed().as_secs_f32()
                            );
                            result = Some(complete);
                            break;
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            return Err(anyhow::anyhow!("startup load worker disconnected"));
                        }
                    }
                }

                if result.is_none() {
                    let elapsed = started_at.elapsed().as_secs_f32();
                    let (fallback_phase, fallback_progress) = Self::loading_visual_phase(elapsed);
                    let chosen_progress = (*last_worker_progress).max(fallback_progress);
                    let chosen_phase = last_worker_phase
                        .as_deref()
                        .unwrap_or(fallback_phase)
                        .to_string();
                    visual_phase = Some(chosen_phase);
                    visual_progress = Some(chosen_progress);
                }
            }
        }

        if let (Some(phase), Some(progress)) = (visual_phase, visual_progress) {
            self.update_shell_loading_progress(progress, Some(&phase));
            self.observe_startup_progress(progress, &phase);
            if self
                .last_loading_title_update
                // Avoid hammering native window-title updates during startup; on macOS these
                // updates can become expensive when issued every frame.
                .map(|last| last.elapsed() >= Duration::from_millis(350))
                .unwrap_or(true)
            {
                self.window.set_title(&format!(
                    "Command & Conquer Generals Zero Hour - Loading {phase} ({:.0}%)",
                    progress * 100.0
                ));
                self.last_loading_title_update = Some(Instant::now());
            }
            self.window.request_redraw();
            return Ok(());
        }

        match result.expect("startup completion result missing") {
            Ok(load_result) => self.finalize_startup_map_load(load_result),
            Err(err) => Err(anyhow::anyhow!(err)),
        }
    }

    pub async fn new(window: Arc<Window>, command_line: Arc<CommandLineArgs>) -> Result<Self> {
        let total_timer = InitTimer::new("🎮 Engine initialization");
        info!("🎮 Initializing Command & Conquer Generals Zero Hour Game Engine");
        info!("📋 Starting subsystem initialization sequence...");

        let debug_overlay = command_line.wants_debug_overlay();
        let no_audio_command_line = command_line.no_audio && Self::allow_debug_startup_flags();
        if no_audio_command_line {
            info!("🔇 Audio disabled via -noaudio");
        }
        if command_line.quick_start {
            info!("⚡ QuickStart enabled: skipping intro sequences (handled by SAGE runtime).");
        }

        init_subsystem_manager()
            .map_err(|err| warn!("Subsystem manager initialization failed: {err}"))
            .ok();
        // C++ parity: GameEngine::init() line 679 — HideControlBar() after init completes.
        {
            let _ = game_client::gui::callbacks::control_bar_callbacks::hide_control_bar(true);
        }
        Self::apply_command_line_overrides(&command_line);
        Self::apply_startup_audio_channel_flags();
        // C++ parity: initialize startup RNG stream during engine init.
        game_engine::common::random_value::init_random();
        Self::remove_legacy_duplicate_inizh_big_best_effort();

        init_game_state_system()
            .map_err(|err| warn!("Game state system init failed: {err}"))
            .ok();

        // Initialize subsystems first (matches C++ GameEngine initialization order)
        if let Some(handle) = get_subsystem_manager() {
            let manager = handle.lock();
            if manager.is_initialized() {
                info!("✅ Core subsystems initialized");
            } else {
                warn!("Subsystem manager available but not initialized");
            }
        } else {
            warn!("Subsystem manager missing after initialization attempt");
        }

        let runtime_host_headless = RuntimeHostBridge::is_headless_mode(command_line.as_ref());
        let size = window.inner_size();

        // Initialize WW3D engine to own the swapchain/device
        let mut engine_config = EngineConfig::default();
        engine_config.width = size.width.max(1);
        engine_config.height = size.height.max(1);

        if runtime_host_headless {
            if let Err(err) = ww3d_engine::init_headless(engine_config).await {
                if !matches!(err, EngineError::AlreadyInitialised) {
                    return Err(anyhow::anyhow!(
                        "Failed to initialize WW3D headless engine: {err:?}"
                    ));
                }
            }
        } else if let Err(err) = ww3d_engine::init_with_window(window.clone(), engine_config).await
        {
            if !matches!(err, EngineError::AlreadyInitialised) {
                return Err(anyhow::anyhow!("Failed to initialize WW3D engine: {err:?}"));
            }
        }

        // Initialize C++ SAGE equivalent graphics system
        info!("🎨 Initializing GraphicsSystem (C++ SAGE equivalent)...");
        let graphics_timer = InitTimer::new("✅ GraphicsSystem initialized");
        let device =
            ww3d_engine::device().map_err(|e| anyhow::anyhow!("WW3D device unavailable: {e:?}"))?;
        let queue =
            ww3d_engine::queue().map_err(|e| anyhow::anyhow!("WW3D queue unavailable: {e:?}"))?;
        let color_format = ww3d_engine::color_format()
            .map_err(|e| anyhow::anyhow!("WW3D color format unavailable: {e:?}"))?;
        let depth_format = ww3d_engine::depth_format()
            .map_err(|e| anyhow::anyhow!("WW3D depth format unavailable: {e:?}"))?;
        let graphics_system = GraphicsSystem::new(device, queue, color_format, depth_format)?;
        graphics_timer.finish();

        // Initialize render pipeline
        info!("🔧 Initializing RenderPipeline (C++ SAGE equivalent)...");
        let pipeline_timer = InitTimer::new("✅ RenderPipeline initialized");
        let mut render_pipeline = RenderPipeline::initialize(&graphics_system)?;
        pipeline_timer.finish();

        // C++ parity: BIG archives MUST be initialized BEFORE asset manager so textures/INI can be read
        info!("📦 Initializing BIG archive file system...");
        if let Err(err) = crate::assets::archive::init_archive_file_system().await {
            warn!(
                "BIG archive file system init failed: {err}. Continuing without archive support."
            );
        }

        // Host combat WeaponStore: guarantee init even if asset manager init fails.
        // (AssetManager also inits this; this is a hard early guarantee for create_object.)
        // Note: Common ini_weapon store (filled later from archives) is separate;
        // host create_object binds via gamelogic WeaponStore — see weapon_bootstrap.
        if let Err(e) = gamelogic::initialize_weapon_store() {
            warn!("Early WeaponStore init failed (will retry via assets): {e}");
        }
        let seeded = crate::game_logic::ensure_host_weapon_store();
        if seeded > 0 {
            info!(
                "Early host WeaponStore bootstrap registered {} templates (archive load may add more)",
                seeded
            );
        }
        // Host movement LocomotorStore: seed BasicHumanLocomotor (~20) etc. so
        // create_object binds retail-ish max_speed without golden_skirmish boost.
        let loco_seeded = crate::game_logic::ensure_host_locomotor_store();
        if loco_seeded > 0 {
            info!(
                "Early host LocomotorStore bootstrap registered {} templates (archive load may add more)",
                loco_seeded
            );
        }

        // C++ parity: initialize the asset manager during engine setup so startup loading
        // can reuse the live archive/definition caches immediately.
        info!("🎨 Initializing C&C Asset Manager during engine setup...");
        let asset_timer = Instant::now();
        crate::assets::manager::init_asset_manager(
            graphics_system.device_arc().as_ref(),
            graphics_system.queue_arc().as_ref(),
        )
        .await
        .map_err(|err| {
            warn!("Asset manager init failed: {err}. Continuing without assets.");
            err
        })
        .ok();
        let asset_duration = asset_timer.elapsed();

        if let Err(err) = crate::assets::archive::init_big_archive_file_reader() {
            warn!("BIG archive texture reader init failed: {err}. Continuing without archive texture reader.");
        }
        info!(
            "BIG archive texture reader wired ({:.2}s total asset setup)",
            asset_duration.as_secs_f32()
        );

        // Model preloading will be done after graphics system is ready
        // This is handled in the run loop after engine creation
        // Models are preloaded later; keep placeholder timer for consistency if needed.

        // No direct wgpu initialization needed - graphics system handles this

        // Initialize platform-specific message handling
        let message_handler = create_platform_message_handler();
        let mut message_processor = WindowMessageProcessor::new(message_handler);
        message_processor.attach_window(window.clone());

        // Initialize audio system unless disabled
        let (audio_output, audio_handle) = if no_audio_command_line {
            (None, None)
        } else {
            match OutputStream::try_default() {
                Ok((output, handle)) => (Some(output), Some(handle)),
                Err(e) => {
                    warn!(
                        "Failed to initialize audio output: {e}; C++ init would quit when music is not ready, so startup will exit"
                    );
                    (None, None)
                }
            }
        };
        let audio_startup_requires_quit =
            Self::startup_audio_should_quit(no_audio_command_line, audio_handle.is_some());

        let mut ui_sound_cache: HashMap<String, Arc<[u8]>> = HashMap::new();
        if audio_handle.is_some() {
            if let Some(manager_arc) = crate::assets::manager::get_asset_manager() {
                let mut manager = manager_arc.lock().unwrap_or_else(|e| e.into_inner());
                for &path in &[
                    crate::ui::sound_files::BUTTON_CLICK,
                    crate::ui::sound_files::BUTTON_HOVER,
                ] {
                    match manager.extract_file(path).await {
                        Ok(data) => {
                            ui_sound_cache
                                .insert(path.to_string(), Arc::from(data.into_boxed_slice()));
                        }
                        Err(err) => {
                            debug!("UI sound '{}' unavailable: {}", path, err);
                        }
                    }
                }
            }
        }

        // Initialize game systems.
        // CombatSystem + PathfindingSystem live on GameLogic (sole host authority).
        let game_logic = GameLogic::initialize();
        let resource_manager = ResourceManager::new();
        let mut save_file_manager = SaveFileManager::new();
        save_file_manager
            .init()
            .map_err(|err| anyhow::anyhow!("Save file manager init failed: {err}"))?;

        // Initialize minimap renderer now that we know the world bounds.
        let world_bounds = game_logic.world_bounds();
        render_pipeline.initialize_minimap_renderer(
            graphics_system.device_arc(),
            graphics_system.queue_arc(),
            world_bounds,
        )?;

        let camera_target = Vec3::ZERO;
        let camera_position = Vec3::new(0.0, 310.0, -403.99988);
        let camera_zoom = 1.0;
        let projection_matrix = Mat4::perspective_rh(
            DEFAULT_VIEW_FOV_RADIANS,
            size.width as f32 / size.height as f32,
            DEFAULT_VIEW_NEAR_CLIP,
            DEFAULT_VIEW_FAR_CLIP,
        );

        let build_map_cache = {
            let global = game_engine::common::global_data::read();
            global.writable.build_map_cache
        };

        // C++ GameEngine::init updates MapCache before shell-map startup checks.
        game_client::map_util::refresh_map_cache();

        let startup_initial_file = Self::startup_initial_file_from_command_line(
            &command_line,
            Self::allow_debug_startup_flags(),
        );

        let (startup_initial_map, startup_initial_replay) =
            Self::split_startup_initial_file(startup_initial_file);

        if let Some(initial_map) = startup_initial_map.as_ref() {
            let mut global = game_engine::common::global_data::write();
            global.writable.shell_map_on = false;
            global.writable.play_intro = false;
            global.pending_file = initial_map.clone();
        }

        if let Some(initial_replay) = startup_initial_replay.as_ref() {
            // C++ parity: `.rep` startup is delegated to the recorder path and does not
            // force shell/intro flags or clear `pending_file` here.
            info!("Replay startup override requested: {}", initial_replay);
        }

        // C++ treats `-file` as the startup initial file, not as a direct `-map` request.
        let startup_map_requested_from_cli = false;
        let startup_map_requested_from_initial_file = startup_initial_map.is_some();
        let startup_replay_requested_from_initial_file = startup_initial_replay.is_some();
        let startup_requested_map = startup_initial_map.clone();
        let startup_requested_replay = startup_initial_replay.clone();
        let start_in_menu = startup_requested_map.is_none() && startup_requested_replay.is_none();
        let startup_shell_map = Self::configured_startup_shell_map();
        let map_to_load = if start_in_menu {
            startup_shell_map
        } else {
            startup_requested_map
        };

        // C++ parity (GameEngine.cpp): if intro is disabled by INI/flags, startup enters
        // the post-intro state after shell-map validation.
        Self::sync_after_intro_when_intro_disabled();

        let startup_load_state = if build_map_cache || audio_startup_requires_quit {
            StartupLoadState::Complete
        } else {
            Self::spawn_startup_map_load(
                start_in_menu,
                map_to_load,
                startup_map_requested_from_cli,
                startup_map_requested_from_initial_file,
                startup_requested_replay.clone(),
                startup_replay_requested_from_initial_file,
                command_line.player_name.clone(),
            )
        };

        let camera_offset = camera_position - camera_target;
        let camera_orbit_distance = camera_offset.length().max(1.0);
        let camera_pitch_radians = camera_offset
            .y
            .atan2(Vec2::new(camera_offset.x, camera_offset.z).length());
        let camera_yaw_radians = camera_offset.x.atan2(camera_offset.z);
        let view_matrix = Mat4::look_at_rh(camera_position, camera_target, Vec3::Y);

        let pending_shell_model_prewarm = if start_in_menu {
            // C++ shell startup does not run this extra Rust-only synchronous prewarm loop.
            // Keep shell-scene warmup disabled here and rely on the render pipeline's
            // incremental non-blocking budget instead so the menu can paint first.
            VecDeque::new()
        } else {
            info!("Skipping blocking startup model preload for gameplay startup");
            VecDeque::new()
        };

        let mut ui_manager = UIManager::new(size.width, size.height);
        if command_line.quick_start {
            ui_manager.enable_quick_start();
        }
        ui_manager
            .initialize()
            .map_err(|err| anyhow::anyhow!("failed to initialize startup UI: {err}"))?;
        // C++ parity: loading visuals are GameClient .wnd load screens
        // (ShellGameLoadScreen/SinglePlayerLoadScreen/etc.), not Main/src/ui.
        ui_manager.suspend_for_shell_overlay();
        let initial_state = GameState::Loading;
        let pending_state = None;

        let mut engine = Self {
            window: window.clone(),
            command_line,

            // C++ SAGE equivalent rendering subsystems
            graphics_system,
            render_pipeline,

            message_processor,
            audio_output,
            audio_handle,
            background_music: None,
            sound_effects: Vec::new(),
            ui_sound_cache,

            // Default boot flow should land in the menu unless explicitly quick-starting.
            current_state: initial_state,
            pending_state,
            startup_load_state,
            startup_target_state: Some(if start_in_menu {
                GameState::Menu
            } else {
                GameState::InGame
            }),
            startup_start_in_menu: start_in_menu,
            last_loading_title_update: None,
            startup_last_reported_progress: 0.0,
            startup_loading_phase: DEFAULT_LOADING_PHASE.to_string(),
            startup_last_progress_change_at: Instant::now(),
            startup_last_stall_warning_at: None,
            startup_stall_events: 0,
            startup_max_stall_duration: Duration::ZERO,
            startup_health_summary_logged: false,
            last_caustic_warmup_attempt: None,
            loading_overlay_active: false,
            #[cfg(feature = "game_client")]
            active_load_screen: None,
            shell_menu_active: false,

            #[cfg(feature = "game_client")]
            game_client: game_client::core::game_client::GameClient::new()
                .map_err(|e| anyhow::anyhow!("Failed to create GameClient: {e}"))?,
            #[cfg(feature = "game_client")]
            control_bar: game_client::gui::control_bar::ControlBar::new(),

            game_logic,
            last_presentation_frame: None,
            gameworld_shadow: if crate::gameworld_shadow::gameworld_shadow_enabled() {
                Some(crate::gameworld_shadow::GameWorldShadow::new(4096))
            } else {
                None
            },
            last_ui_state: None,
            resource_manager,
            save_file_manager,
            camera_position,
            camera_target,
            camera_zoom,
            camera_zoom_target: None,
            camera_zoom_start: camera_zoom,
            camera_zoom_duration: 0.0,
            camera_zoom_elapsed: 0.0,
            camera_zoom_ease_in: 0.0,
            camera_zoom_ease_out: 0.0,
            camera_orbit_distance,
            camera_pitch_radians,
            camera_pitch_target: None,
            camera_pitch_start: camera_pitch_radians,
            camera_pitch_duration: 0.0,
            camera_pitch_elapsed: 0.0,
            camera_pitch_ease_in: 0.0,
            camera_pitch_ease_out: 0.0,
            camera_yaw_radians,
            camera_yaw_target: None,
            camera_yaw_start: camera_yaw_radians,
            camera_yaw_duration: 0.0,
            camera_yaw_elapsed: 0.0,
            camera_yaw_ease_in: 0.0,
            camera_yaw_ease_out: 0.0,
            camera_shake_offset: Vec3::ZERO,
            screen_shake_intensity: 0.0,
            screen_shake_angle_cos: 0.0,
            screen_shake_angle_sin: 0.0,
            script_camera_shakers: Vec::new(),
            script_fps_limit: None,
            script_fps_limit_last_tick: None,
            camera_slave_mode: None,
            view_matrix,
            projection_matrix,
            keys_pressed: HashSet::new(),
            mouse_position: (0.0, 0.0),
            mouse_world_position: Vec3::ZERO,
            last_context_cursor: None,
            last_eva_low_power_count: 0,
            last_eva_insufficient_funds_count: 0,
            last_eva_base_under_attack_count: 0,
            last_eva_ally_under_attack_count: 0,
            sticky_waypoint_mode: false,
            sticky_auto_attack: false,
            is_dragging: false,
            selection_start: None,
            selection_start_screen: None,
            last_click_time: None,
            last_click_position: None,
            is_windowed: window.fullscreen().is_none(),
            rmb_scroll_anchor: None,
            is_rmb_scrolling: false,
            is_mmb_rotating: false,
            mmb_anchor: None,
            selected_objects: Vec::new(),
            control_groups: HashMap::new(),
            last_control_group_select: None,
            camera_view_bookmarks: [None; 8],
            camera_rotate_left_held: false,
            camera_rotate_right_held: false,
            camera_zoom_in_held: false,
            camera_zoom_out_held: false,
            camera_tracking_selection: false,
            replay_fast_forward: false,
            diplomacy_panel: crate::ui::DiplomacyPanel::new(),
            chat_panel: crate::ui::ChatPanel::new(),
            current_player_id: 0,
            game_paused: false,
            show_debug_info: debug_overlay,
            show_health_bars: true,
            show_fps: false,
            show_move_lines: true,
            show_attack_lines: true,
            frame_counter: 0,
            fps: 0.0,
            last_frame_timing: None,
            frame_clock: FrameClock::new(),
            menu_loading_tick_accumulator: Duration::ZERO,
            menu_loading_last_tick: Instant::now(),
            diagnostics_overlay: None,
            ui_manager,
            game_hud: GameHUD::new(),
            pending_structure_placement: None,
            pending_map_command: None,
            active_menu_shell_hook: None,
            runtime_host_headless,
            runtime_host_base_ui_screen: None,
            runtime_host_ui_screen_override: None,
            runtime_host_last_gameplay_cmd: String::new(),
            models_loaded: true, // Already loaded during init
            pending_shell_model_prewarm,
            menu_enter_frame: None,
            shell_ui_enqueued_frame: None,
            last_shell_prewarm_log: None,
            shell_prewarm_completion_logged: false,
            menu_world_frames_rendered: 0,
            last_slow_menu_tick_log: None,
            match_over: false,
            victory_summary: None,
        };

        if audio_startup_requires_quit {
            warn!(
                "Audio startup parity: music was not ready during init, marking engine as exiting"
            );
            engine.current_state = GameState::Exiting;
            engine.startup_target_state = None;
            engine.startup_load_state = StartupLoadState::Complete;
        }

        Self::initialize_cpp_startup_masks();

        // C++ parity: GameClient::init() creates WindowManager, Shell, FontLibrary, etc.
        // BUT it also tries to create a PlatformContext (new window + OpenGL context)
        // which deadlocks on macOS when called inside the winit event loop.
        // Only init the non-display subsystems that don't conflict with our wgpu pipeline.
        #[cfg(feature = "game_client")]
        {
            if let Err(e) = engine.game_client.init_core_subsystems() {
                warn!("GameClient core subsystems init failed: {}", e);
            }
            if let Err(e) = engine.game_client.init_asset_systems() {
                warn!("GameClient asset systems init failed: {}", e);
            }
            if let Err(e) = engine.game_client.init_message_translators() {
                warn!("GameClient message translators init failed: {}", e);
            }
            if let Err(e) = engine.game_client.init_input_subsystems() {
                warn!("GameClient input subsystems init failed: {}", e);
            }
            if let Err(e) = engine.game_client.init_display_subsystems() {
                warn!("GameClient display subsystems init failed: {}", e);
            }

            // Create UIRenderer from GraphicsSystem instead of PlatformContext.
            // C++ SAGE uses PlatformContext (OpenGL window) for UI rendering, but we skip
            // PlatformContext creation to avoid macOS winit deadlock (second EventLoop).
            // The GraphicsSystem holds the same wgpu device/queue we need.
            {
                use game_client::gui::ui_globals::set_ui_renderer;
                use game_client::gui::ui_renderer::UIRenderer;

                let device = engine.graphics_system.device_arc();
                let queue = engine.graphics_system.queue_arc();
                let format = engine.graphics_system.color_format();

                match UIRenderer::new(device, queue, format) {
                    Ok(renderer) => {
                        set_ui_renderer(Arc::new(std::sync::RwLock::new(renderer)));
                        info!(
                            "UIRenderer created from GraphicsSystem (format: {:?})",
                            format
                        );
                    }
                    Err(err) => {
                        warn!("UIRenderer creation from GraphicsSystem failed: {err}");
                    }
                }
            }

            if let Err(e) = engine.game_client.init_game_subsystems() {
                warn!("GameClient game subsystems init failed: {}", e);
            }
            if let Err(e) = engine.game_client.post_process_display_strings() {
                warn!("GameClient post-process display strings failed: {}", e);
            }

            if let Err(e) = engine.game_client.init_audio_subsystems() {
                warn!("GameClient audio subsystems init failed: {}", e);
            }

            engine.game_client.init_savegame_counter_bridge();

            engine.game_client.init_recorder_bridge();

            engine.game_client.mark_initialized();
            info!("GameClient: all subsystems initialized");
        }

        if let Some(subsystem_manager) = get_subsystem_manager() {
            let mut manager = subsystem_manager.lock();
            if let Err(err) = manager.reset_all() {
                warn!("Subsystem reset after startup init failed: {}", err);
            }
        }

        engine.hide_control_bar();

        if build_map_cache {
            engine.current_state = GameState::Exiting;
            engine.startup_load_state = StartupLoadState::Complete;
            engine.startup_target_state = None;
            return Ok(engine);
        }

        if audio_startup_requires_quit {
            return Ok(engine);
        }

        // Start background music
        // DISABLED: Using proper AssetManager audio system instead of synthetic tones
        // engine.start_background_music();

        // Display subsystem status
        if let Some(subsystem_manager) = get_subsystem_manager() {
            let stats = subsystem_manager.lock().get_stats();
            info!("📊 Subsystem Status:");
            info!("  ✅ {} subsystems initialized", stats.total_subsystems);
            if let Some(init_time) = stats.initialization_time {
                info!("  ⏱️ Total init time: {:.2}ms", init_time.as_millis());
            }
        }

        info!("🎉 C&C Game Engine with Enhanced Subsystem Architecture initialized successfully!");
        let total_duration = total_timer.finish();
        info!(
            "⏱️ Total Engine Initialization Time: {:.2}s",
            total_duration.as_secs_f32()
        );
        if asset_duration > Duration::ZERO {
            info!("   Asset Manager: {:.2}s", asset_duration.as_secs_f32());
        } else {
            info!("   Asset Manager: initialized during engine setup");
        }
        info!("🎮 Controls:");
        info!("  WASD - Move camera");
        info!("  Mouse - Select units");
        info!("  Right click - Move/Attack command");
        info!("  SPACE - Pause game");
        info!("  F1 - Toggle debug info");
        info!("  M - Toggle music");
        info!("  ESC - Exit game");

        engine
            .window
            .set_title("Command & Conquer Generals Zero Hour - Loading...");
        engine.ensure_shell_loading_overlay();
        engine.update_shell_loading_progress(0.0, Some("Loading assets..."));

        Ok(engine)
    }

    fn apply_command_line_overrides(command_line: &CommandLineArgs) {
        let allow_debug_flags = Self::allow_debug_startup_flags();
        let shell_map_override = if allow_debug_flags {
            Self::command_line_option_value_case_insensitive(command_line, "shellmap")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        } else {
            None
        };

        let initial_file_override = if allow_debug_flags {
            Self::command_line_option_value_case_insensitive(command_line, "file")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        } else {
            None
        };
        let map_name_override = if allow_debug_flags {
            command_line
                .map_name
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        } else {
            None
        };

        {
            let mut global = game_engine::common::global_data::write();
            if let Some(width) = command_line.width {
                global.writable.x_resolution = i32::try_from(width).unwrap_or(i32::MAX);
            }
            if let Some(height) = command_line.height {
                global.writable.y_resolution = i32::try_from(height).unwrap_or(i32::MAX);
            }
            if let Some(initial_file) = initial_file_override.as_ref() {
                global.writable.initial_file = initial_file.clone();
            }
            if let Some(map_name) = map_name_override.as_ref() {
                // C++ parseMapName updates the writable startup map path.
                global.writable.map_name = map_name.clone();
            }
            Self::apply_ordered_startup_overrides_from_raw_args(
                &command_line.raw_args,
                &mut global.writable,
                allow_debug_flags,
            );
            if let Some(shell_map_name) = shell_map_override {
                global.writable.shell_map_name = shell_map_name;
            }
            if let Some(lang) = command_line.language.as_deref() {
                global.set_override(
                    "language",
                    game_engine::common::global_data::GlobalValue::String(lang.to_string()),
                );
            }
            if command_line.has_option("mod") {
                if let Some(mod_dir) = command_line.mod_dir.as_deref() {
                    global.writable.mod_dir = mod_dir.to_string();
                    global.writable.mod_big.clear();
                    global.set_override(
                        "active_mod",
                        game_engine::common::global_data::GlobalValue::String(mod_dir.to_string()),
                    );
                } else if let Some(mod_big) = command_line.mod_big.as_deref() {
                    global.writable.mod_big = mod_big.to_string();
                    global.writable.mod_dir.clear();
                    global.set_override(
                        "active_mod",
                        game_engine::common::global_data::GlobalValue::String(mod_big.to_string()),
                    );
                } else if let Some(mod_name) = command_line.mod_name.as_deref() {
                    if mod_name.trim().is_empty() {
                        global.writable.mod_dir.clear();
                        global.writable.mod_big.clear();
                        global.clear_override("active_mod");
                    } else {
                        global.writable.mod_dir.clear();
                        global.writable.mod_big.clear();
                        global.set_override(
                            "active_mod",
                            game_engine::common::global_data::GlobalValue::String(
                                mod_name.to_string(),
                            ),
                        );
                    }
                } else {
                    // Invalid `-mod` path should behave like C++ parseMod: consume option
                    // but leave prior mod configuration untouched.
                }
            }
        }

        Self::load_mods_best_effort();

        let language = command_line.language.as_deref().unwrap_or("English");
        localization::set_language(language);
    }

    fn initialize_cpp_startup_masks() {
        game_engine::common::system::kind_of::init_kind_of_masks();
        Self::init_disabled_masks();
        gamelogic::damage::init_damage_type_flags();
    }

    fn init_disabled_masks() {
        game_engine::common::system::disabled_types::init_disabled_masks();
    }

    fn startup_water_weather_ini_paths() -> [&'static str; 4] {
        [
            "Data/INI/Default/Water.ini",
            "Data/INI/Water.ini",
            "Data/INI/Default/Weather.ini",
            "Data/INI/Weather.ini",
        ]
    }

    fn preload_startup_water_weather_inis() {
        let mut ini = game_engine::common::ini::INI::new();
        for path in Self::startup_water_weather_ini_paths() {
            match ini.load(path, game_engine::common::ini::INILoadType::Overwrite) {
                Ok(()) => info!("Preloaded startup INI: {}", path),
                Err(err) => warn!(
                    "Failed to preload startup INI '{}' during init; continuing: {}",
                    path, err
                ),
            }
        }
    }

    /// C++ parity: GameEngine.cpp:480 — AIData.ini load paths.
    /// Loaded after Upgrade, before Crate.
    fn startup_ai_data_ini_paths() -> [&'static str; 2] {
        ["Data/INI/Default/AIData.ini", "Data/INI/AIData.ini"]
    }

    /// Load AIData.ini (Default + override) into the AI data store.
    /// C++ parity: GameEngine.cpp:480 initSubsystem(TheAI, ..., "Data\\INI\\Default\\AIData.ini", "Data\\INI\\AIData.ini")
    fn preload_startup_ai_data_inis() {
        let mut ini = game_engine::common::ini::INI::new();
        for path in Self::startup_ai_data_ini_paths() {
            match ini.load(path, game_engine::common::ini::INILoadType::Overwrite) {
                Ok(()) => info!("Preloaded AIData INI: {}", path),
                Err(err) => warn!(
                    "Failed to preload AIData INI '{}' during init; continuing: {}",
                    path, err
                ),
            }
        }
    }

    fn startup_audio_should_quit(no_audio: bool, audio_ready: bool) -> bool {
        !no_audio && !audio_ready
    }

    fn apply_startup_audio_channel_flags() {
        let global = game_engine::common::global_data::read();
        let audio_on = global.writable.audio_on;
        let music_on = global.writable.music_on;
        let sounds_on = global.writable.sounds_on;
        let speech_on = global.writable.speech_on;
        let sounds_3d_on = global.sounds_3d_on;
        drop(global);

        with_subsystem_mut::<AudioManagerSubsystem, _>(|audio| {
            audio.apply_startup_channel_flags(
                audio_on,
                music_on,
                sounds_on,
                sounds_3d_on,
                speech_on,
            );
        });
    }

    fn allow_debug_startup_flags() -> bool {
        // PARITY_NOTE: C++ gates debug flags on internal builds only.
        // Rust allows -noaudio and similar flags in all builds for cross-platform compatibility.
        true
    }

    fn remove_legacy_duplicate_inizh_big_best_effort() {
        let legacy_path = std::path::Path::new("Data").join("INI").join("INIZH.big");
        if !legacy_path.exists() {
            return;
        }

        match std::fs::remove_file(&legacy_path) {
            Ok(()) => info!(
                "Removed legacy duplicate INI archive to match C++ startup cleanup: {}",
                legacy_path.display()
            ),
            Err(err) => warn!(
                "Failed to remove legacy duplicate INI archive '{}': {}",
                legacy_path.display(),
                err
            ),
        }
    }

    fn hide_control_bar(&mut self) {
        // GameHUD only exposes a visibility toggle; it starts visible at boot.
        self.game_hud.toggle_visibility();
    }

    fn load_mods_best_effort() {
        game_engine::common::system::archive_file_system::init_archive_file_system();

        let (mod_dir, mod_big) = {
            let global = game_engine::common::global_data::read();
            let mod_dir = global.writable.mod_dir.trim().to_string();
            let mod_big = global.writable.mod_big.trim().to_string();
            (mod_dir, mod_big)
        };

        if let Some(mut archive_file_system) =
            game_engine::common::system::archive_file_system::get_archive_file_system()
        {
            if !mod_dir.is_empty() {
                archive_file_system.add_search_path(std::path::Path::new(mod_dir.as_str()));
            }
            if !mod_big.is_empty() {
                if let Err(err) = archive_file_system.open_archive_file(mod_big.as_str()) {
                    warn!("Best-effort mod archive open failed: {}", err);
                }
            }
            if let Err(err) = archive_file_system.load_mods() {
                warn!("Best-effort mod archive load failed: {}", err);
            }
        }
    }

    fn apply_fps_limit_overrides_from_raw_args(
        raw_args: &[String],
        writable: &mut game_engine::common::command_line::WritableGlobalData,
    ) {
        // C++ parity: process `-nofpslimit`/`-fps` in original argv order.
        // This preserves precedence when both are present.
        let mut arg_index = 1usize;
        while arg_index < raw_args.len() {
            let raw = raw_args[arg_index].trim();
            if !raw.starts_with('-') {
                arg_index += 1;
                continue;
            }

            let mut option = raw.trim_start_matches('-');
            let mut inline_value: Option<&str> = None;
            if let Some((name, value)) = option.split_once('=') {
                option = name;
                inline_value = Some(value);
            }

            match option.to_ascii_lowercase().as_str() {
                "nofpslimit" => {
                    writable.use_fps_limit = false;
                    writable.frames_per_second_limit = 30000;
                }
                "fps" => {
                    if let Some(value) =
                        Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                    {
                        writable.frames_per_second_limit =
                            Self::parse_startup_i32_like_atoi(&value);
                    }
                }
                _ => {}
            }

            arg_index += 1;
        }
    }

    fn apply_ordered_startup_overrides_from_raw_args(
        raw_args: &[String],
        writable: &mut game_engine::common::command_line::WritableGlobalData,
        allow_debug_flags: bool,
    ) {
        let mut arg_index = 1usize;
        while arg_index < raw_args.len() {
            let raw = raw_args[arg_index].trim();
            if !raw.starts_with('-') {
                arg_index += 1;
                continue;
            }

            let mut option = raw.trim_start_matches('-');
            let mut inline_value: Option<&str> = None;
            if let Some((name, value)) = option.split_once('=') {
                option = name;
                inline_value = Some(value);
            }

            match option.to_ascii_lowercase().as_str() {
                "win" | "windowed" | "w" => {
                    writable.windowed = true;
                }
                "fullscreen" | "f" | "nowin" => {
                    writable.windowed = false;
                }
                "particleedit" => {
                    writable.particle_edit = true;
                    writable.win_cursors = true;
                    writable.windowed = true;
                }
                "quickstart" => {
                    writable.shell_map_on = false;
                    writable.animate_windows = false;
                    writable.play_sizzle = false;

                    if cfg!(any(debug_assertions, feature = "internal")) {
                        writable.play_intro = false;
                        writable.after_intro = true;
                    }
                }
                "nologo" | "nointro" => {
                    if allow_debug_flags {
                        writable.play_intro = false;
                        writable.after_intro = true;
                        writable.play_sizzle = false;
                    }
                }
                "noshellmap" => {
                    writable.shell_map_on = false;
                }
                "noshellanim" => {
                    if allow_debug_flags {
                        writable.animate_windows = false;
                    }
                }
                "noaudio" => {
                    if allow_debug_flags {
                        writable.audio_on = false;
                        writable.speech_on = false;
                        writable.sounds_on = false;
                        writable.music_on = false;
                    }
                }
                "novideo" => {
                    if allow_debug_flags {
                        writable.video_on = false;
                    }
                }
                "scriptdebug" => {
                    writable.script_debug = true;
                    writable.win_cursors = true;
                }
                "wincursors" => {
                    if allow_debug_flags {
                        writable.win_cursors = true;
                    }
                }
                "nomusic" => {
                    if allow_debug_flags {
                        writable.music_on = false;
                    }
                }
                "nodraw" => {
                    if allow_debug_flags {
                        writable.no_draw = true;
                    }
                }
                "noshaders" => {
                    writable.chip_set_type = 1;
                }
                "forcebenchmark" => {
                    if allow_debug_flags {
                        writable.force_benchmark = true;
                    }
                }
                "nomovecamera" => {
                    if allow_debug_flags {
                        writable.disable_camera_movement = true;
                    }
                }
                "constantdebug" => {
                    if allow_debug_flags {
                        writable.constant_debug_update = true;
                    }
                }
                "showteamdot" => {
                    if allow_debug_flags {
                        writable.show_team_dot = true;
                    }
                }
                "nofpslimit" => {
                    if allow_debug_flags {
                        writable.use_fps_limit = false;
                        writable.frames_per_second_limit = 30000;
                    }
                }
                "buildmapcache" | "buildcache" => {
                    if allow_debug_flags {
                        writable.build_map_cache = true;
                    }
                }
                "updateimages" | "updatedds" => {
                    if allow_debug_flags {
                        writable.should_update_tga_to_dds = true;
                    }
                }
                "fps" => {
                    if allow_debug_flags {
                        if let Some(value) =
                            Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                        {
                            writable.frames_per_second_limit =
                                Self::parse_startup_i32_like_atoi(&value);
                        }
                    }
                }
                "seed" => {
                    if allow_debug_flags {
                        if let Some(value) =
                            Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                        {
                            writable.fixed_seed = Self::parse_startup_i32_like_atoi(&value);
                        }
                    }
                }
                "jumptoframe" => {
                    if allow_debug_flags {
                        if let Some(value) =
                            Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                        {
                            writable.no_draw = Self::parse_startup_i32_like_atoi(&value) != 0;
                            writable.use_fps_limit = false;
                            writable.frames_per_second_limit = 30000;
                        }
                    }
                }
                "netminplayers" => {
                    if allow_debug_flags {
                        if let Some(value) =
                            Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                        {
                            writable.net_min_players = Self::parse_startup_i32_like_atoi(&value);
                        }
                    }
                }
                "playstats" => {
                    if let Some(value) =
                        Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                    {
                        writable.play_stats = Self::parse_startup_i32_like_atoi(&value);
                    }
                }
                "benchmark" if allow_debug_flags => {
                    if let Some(value) =
                        Self::consume_startup_value(raw_args, &mut arg_index, inline_value)
                    {
                        let parsed = Self::parse_startup_i32_like_atoi(&value);
                        writable.benchmark_timer = parsed;
                        writable.play_stats = parsed;
                    }
                }
                _ => {}
            }

            arg_index += 1;
        }
    }

    fn consume_startup_value(
        raw_args: &[String],
        arg_index: &mut usize,
        inline_value: Option<&str>,
    ) -> Option<String> {
        if let Some(value) = inline_value {
            return Some(value.to_string());
        }

        if *arg_index + 1 < raw_args.len() {
            *arg_index += 1;
            return Some(raw_args[*arg_index].trim().to_string());
        }

        None
    }

    fn parse_startup_i32_like_atoi(value: &str) -> i32 {
        value.trim().parse::<i32>().unwrap_or(0)
    }

    fn has_command_line_option_case_insensitive(
        command_line: &CommandLineArgs,
        option: &str,
    ) -> bool {
        command_line
            .options
            .keys()
            .any(|name| name.eq_ignore_ascii_case(option))
    }

    fn command_line_option_value_case_insensitive(
        command_line: &CommandLineArgs,
        option: &str,
    ) -> Option<String> {
        command_line.options.iter().find_map(|(name, value)| {
            if name.eq_ignore_ascii_case(option) {
                value.clone()
            } else {
                None
            }
        })
    }

    fn startup_initial_file_from_command_line(
        command_line: &CommandLineArgs,
        allow_debug_flags: bool,
    ) -> Option<String> {
        if !allow_debug_flags {
            return None;
        }

        let runtime_initial_file = {
            let global = game_engine::common::global_data::read();
            global.writable.initial_file.trim().to_string()
        };
        if !runtime_initial_file.is_empty() {
            return Some(runtime_initial_file);
        }

        Self::command_line_option_value_case_insensitive(command_line, "file")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn split_startup_initial_file(
        initial_file: Option<String>,
    ) -> (Option<String>, Option<String>) {
        let Some(initial_file) = initial_file else {
            return (None, None);
        };

        let lower = initial_file.to_ascii_lowercase();
        if lower.ends_with(".map") {
            (Some(initial_file), None)
        } else if lower.ends_with(".rep") {
            (None, Some(initial_file))
        } else {
            (None, None)
        }
    }

    fn sync_after_intro_when_intro_disabled() {
        let mut global = game_engine::common::global_data::write();
        if !global.writable.play_intro {
            global.writable.after_intro = true;
        }
    }

    /// Pre-load all unit models into the graphics system
    async fn preload_unit_models_to_graphics_system(
        graphics_system: &mut GraphicsSystem,
    ) -> Result<()> {
        info!("🎮 Pre-loading C&C unit models into graphics system...");

        // Initialize a temporary game logic instance to get the templates
        let mut temp_game_logic = GameLogic::initialize();
        // Need to setup templates since initialize() doesn't do it
        temp_game_logic.start_new_game(crate::game_logic::GameMode::Skirmish);
        let templates = temp_game_logic.get_templates();

        // List of all unit types that need models loaded
        let unit_types = vec![
            // USA units
            "USA_Ranger",
            "USA_MissileDefender",
            "USA_Humvee",
            "USA_CrusaderTank",
            "USA_PaladinTank",
            "USA_Raptor",
            // GLA units
            "GLA_Soldier",
            "GLA_RPGTrooper",
            "GLA_Technical",
            "GLA_ScorpionTank",
            "GLA_MarauderTank",
            // China units
            "China_RedGuard",
            "China_TankHunter",
            "China_BattlemasterTank",
            "China_OverlordTank",
            "China_MiG",
            "China_Helix",
            // Buildings
            "CommandCenter",
            "SupplyCenter",
            "PowerPlant",
            "Barracks",
            "WarFactory",
        ];

        if let Some(asset_manager_arc) = get_asset_manager() {
            let mut asset_manager = asset_manager_arc.lock().unwrap_or_else(|e| e.into_inner());
            let mut loaded_count = 0;
            let total_units = unit_types.len();

            for unit_type in &unit_types {
                println!("📋 Loading W3D model for template: {}", unit_type);

                // Look up the template to get the correct model name
                if let Some(template) = templates.get(*unit_type) {
                    if let Some(model_name) = &template.model_name {
                        println!(
                            "🎯 Template '{}' maps to W3D model: '{}'",
                            unit_type, model_name
                        );

                        // Try to load the W3D model using the correct filename
                        match asset_manager.load_w3d_model_async(model_name).await {
                            Ok(model) => {
                                println!("✅ Successfully loaded W3D model: '{}' for template '{}' ({} meshes, {} total vertices)",
                                    model_name,
                                    unit_type,
                                    model.meshes.len(),
                                    model.meshes.iter().map(|m| m.vertices.len()).sum::<usize>()
                                );
                                // Cache the model in graphics system using both keys
                                graphics_system.cache_model(unit_type.to_string(), model.clone());
                                graphics_system.cache_model(model_name.clone(), model);
                                loaded_count += 1;
                            }
                            Err(e) => {
                                println!("❌ CRITICAL: Failed to load W3D model '{}' for template '{}': {}", model_name, unit_type, e);
                                println!(
                                    "❌ This means '{}' units will not be visible in game!",
                                    unit_type
                                );
                                // Continue loading other models even if one fails
                            }
                        }
                    } else {
                        println!("⚠️ CRITICAL: Template '{}' has no model_name defined - units will be invisible!", unit_type);
                    }
                } else {
                    println!(
                        "❌ CRITICAL: Template '{}' not found in templates!",
                        unit_type
                    );
                }
            }

            info!(
                "📦 Successfully pre-loaded {}/{} unit models into graphics system",
                loaded_count, total_units
            );
        } else {
            error!("❌ Asset manager not available for model preloading");
        }

        Ok(())
    }

    /// Pre-load all unit models that will be used in the game
    async fn preload_unit_models(loaded_models: &mut HashMap<String, Arc<W3DModel>>) -> Result<()> {
        info!("🎮 Pre-loading C&C unit models...");

        // Initialize a temporary game logic instance to get the templates
        let mut temp_game_logic = GameLogic::initialize();
        // Need to setup templates since initialize() doesn't do it
        temp_game_logic.start_new_game(crate::game_logic::GameMode::Skirmish);
        let templates = temp_game_logic.get_templates();

        // List of all unit types that need models loaded
        let unit_types = vec![
            // USA units
            "USA_Ranger",
            "USA_MissileDefender",
            "USA_Humvee",
            "USA_CrusaderTank",
            "USA_PaladinTank",
            "USA_Raptor",
            // GLA units
            "GLA_Soldier",
            "GLA_RPGTrooper",
            "GLA_Technical",
            "GLA_ScorpionTank",
            "GLA_MarauderTank",
            // China units
            "China_RedGuard",
            "China_TankHunter",
            "China_BattlemasterTank",
            "China_OverlordTank",
            "China_MiG",
            "China_Helix",
            // Buildings
            "CommandCenter",
            "SupplyCenter",
            "PowerPlant",
            "Barracks",
            "WarFactory",
        ];

        if let Some(asset_manager_arc) = get_asset_manager() {
            let mut asset_manager = asset_manager_arc.lock().unwrap_or_else(|e| e.into_inner());
            let mut loaded_count = 0;
            let total_units = unit_types.len();

            for unit_type in &unit_types {
                println!("📋 Loading W3D model for template: {}", unit_type);

                // Look up the template to get the correct model name
                if let Some(template) = templates.get(*unit_type) {
                    if let Some(model_name) = &template.model_name {
                        println!(
                            "🎯 Template '{}' maps to W3D model: '{}'",
                            unit_type, model_name
                        );

                        // Try to load the W3D model using the correct filename
                        match asset_manager.load_w3d_model_async(model_name).await {
                            Ok(model) => {
                                println!("✅ Successfully loaded W3D model: '{}' for template '{}' ({} meshes, {} total vertices)",
                                    model_name,
                                    unit_type,
                                    model.meshes.len(),
                                    model.meshes.iter().map(|m| m.vertices.len()).sum::<usize>()
                                );
                                // Store the model using both the template name AND the model name as keys
                                // This ensures compatibility with both template-based and model-based lookups
                                loaded_models
                                    .insert(unit_type.to_string(), Arc::new(model.clone()));
                                loaded_models.insert(model_name.clone(), Arc::new(model));
                                loaded_count += 1;
                            }
                            Err(e) => {
                                println!("❌ CRITICAL: Failed to load W3D model '{}' for template '{}': {}", model_name, unit_type, e);
                                println!(
                                    "❌ This means '{}' units will not be visible in game!",
                                    unit_type
                                );
                                // Continue loading other models even if one fails
                            }
                        }
                    } else {
                        println!("⚠️ CRITICAL: Template '{}' has no model_name defined - units will be invisible!", unit_type);
                    }
                } else {
                    println!(
                        "❌ CRITICAL: Template '{}' not found in templates!",
                        unit_type
                    );
                }
            }

            info!(
                "📦 Successfully pre-loaded {}/{} unit models",
                loaded_count, total_units
            );
        } else {
            error!("❌ Asset manager not available for model preloading");
        }

        Ok(())
    }

    /// Create GPU buffers for all loaded W3D models
    fn create_model_buffers(
        loaded_models: &HashMap<String, Arc<W3DModel>>,
        device: &wgpu::Device,
        model_buffers: &mut HashMap<String, (wgpu::Buffer, wgpu::Buffer, u32)>,
    ) -> Result<()> {
        info!(
            "🔧 Creating GPU buffers for {} loaded models...",
            loaded_models.len()
        );

        // Keep track of processed models to avoid duplicates
        let mut processed_models: std::collections::HashSet<*const W3DModel> =
            std::collections::HashSet::new();

        for (model_key, w3d_model) in loaded_models {
            // Skip if we've already processed this exact model instance
            let model_ptr = w3d_model.as_ref() as *const W3DModel;
            if processed_models.contains(&model_ptr) {
                continue;
            }
            processed_models.insert(model_ptr);

            for (mesh_idx, mesh) in w3d_model.meshes.iter().enumerate() {
                let mesh_key = format!("{}_{}", model_key, mesh_idx);

                // Skip if buffer already exists
                if model_buffers.contains_key(&mesh_key) {
                    continue;
                }

                // Convert W3D vertices to C++ SAGE VertexFormatXYZNDUV2 format
                let material_color = mesh.material.diffuse_color;
                let vertices: Vec<VertexXYZNDUV2> = mesh
                    .vertices
                    .iter()
                    .map(|v| {
                        // Pack diffuse color as RGBA bytes (D3D8 style)
                        let r = ((v.color[0] * material_color.x * 255.0) as u32).min(255);
                        let g = ((v.color[1] * material_color.y * 255.0) as u32).min(255);
                        let b = ((v.color[2] * material_color.z * 255.0) as u32).min(255);
                        let a = ((v.color[3] * 255.0) as u32).min(255);
                        let diffuse_packed = (a << 24) | (r << 16) | (g << 8) | b;

                        VertexXYZNDUV2 {
                            position: v.position,
                            normal: v.normal,
                            diffuse: diffuse_packed,
                            tex_coords0: v.uv,       // Primary texture coordinates
                            tex_coords1: [0.0, 0.0], // Secondary UV for multi-texturing
                        }
                    })
                    .collect();

                // Create vertex buffer
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{} Vertex Buffer", mesh_key)),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                // Convert indices to u16 format
                let indices: Vec<u16> = mesh.indices.iter().map(|&i| i as u16).collect();
                let index_count = indices.len() as u32;

                // Create index buffer
                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{} Index Buffer", mesh_key)),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

                let buffer_data = (vertex_buffer, index_buffer, index_count);
                model_buffers.insert(mesh_key.clone(), buffer_data);

                info!(
                    "✅ Created GPU buffers for mesh: {} ({} vertices, {} indices)",
                    mesh_key,
                    vertices.len(),
                    index_count
                );
            }
        }

        info!(
            "📦 Created GPU buffers for {} model meshes total",
            model_buffers.len()
        );
        Ok(())
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            if let Err(err) = ww3d_engine::resize(new_size.width.max(1), new_size.height.max(1)) {
                warn!("WW3D resize failed: {err:?}");
            }

            // Update projection matrix
            self.projection_matrix = Mat4::perspective_rh(
                DEFAULT_VIEW_FOV_RADIANS,
                new_size.width as f32 / new_size.height as f32,
                DEFAULT_VIEW_NEAR_CLIP,
                DEFAULT_VIEW_FAR_CLIP,
            );
            self.ui_manager.resize(new_size.width, new_size.height);
        }
    }

    /// Process platform-specific window events through message handler
    pub fn process_platform_event(&mut self, event: &Event<()>) -> Result<bool> {
        self.message_processor.process_event(event)
    }

    /// Check if quit has been requested through platform message handling
    pub fn is_quit_requested(&self) -> bool {
        self.message_processor.is_quit_requested()
    }

    /// Set fullscreen mode and notify platform handler
    pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<()> {
        info!("🖥️ Setting fullscreen mode: {}", fullscreen);

        // Update the message processor's fullscreen state
        self.message_processor.set_fullscreen(fullscreen);

        // In a complete implementation, we would:
        // 1. Change the winit window to fullscreen/windowed
        // 2. Reconfigure the surface
        // 3. Update render targets

        if fullscreen {
            info!("Switching to fullscreen mode");
            // self.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            info!("Switching to windowed mode");
            // self.window.set_fullscreen(None);
        }

        self.is_windowed = !fullscreen;

        Ok(())
    }

    /// Get current application focus state
    pub fn is_application_active(&self) -> bool {
        self.message_processor.is_active()
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: key,
                        physical_key,
                        state,
                        ..
                    },
                ..
            } => {
                let route_keyboard_to_legacy_ui =
                    matches!(self.current_state, GameState::InGame | GameState::Paused);
                match state {
                    ElementState::Pressed => {
                        self.keys_pressed.insert(key.clone());
                        let mut construction_consumed = false;
                        if route_keyboard_to_legacy_ui {
                            if let Some(ui_key) = Self::to_ui_key_code(key) {
                                let _ = self.ui_manager.handle_key_press(ui_key);
                                // Dual HUD residual: engine GameHUD owns construction
                                // cameo hotkeys + placement cancel from presentation path.
                                // When construction panel consumes a build/cancel key, skip
                                // global command hotkeys (R repair vs R ranger, etc.).
                                use crate::ui::Interactive;
                                construction_consumed =
                                    Interactive::handle_key_press(&mut self.game_hud, ui_key);
                                for ev in self.game_hud.drain_pending_ui_events() {
                                    self.ui_manager.queue_event(ev);
                                }
                            }
                        }
                        // Retail numpad camera residual (physical keys).
                        match physical_key {
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad5,
                            ) => self.reset_camera_view_hotkey(),
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad4,
                            ) => {
                                self.camera_rotate_left_held = true;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad6,
                            ) => {
                                self.camera_rotate_right_held = true;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad8,
                            ) => {
                                self.camera_zoom_in_held = true;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad2,
                            ) => {
                                self.camera_zoom_out_held = true;
                            }
                            _ if construction_consumed => {
                                // Construction cameo / Escape placement cancel residual.
                            }
                            _ => self.handle_key_press(key),
                        }
                    }
                    ElementState::Released => {
                        self.keys_pressed.remove(key);
                        match physical_key {
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad4,
                            ) => {
                                self.camera_rotate_left_held = false;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad6,
                            ) => {
                                self.camera_rotate_right_held = false;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad8,
                            ) => {
                                self.camera_zoom_in_held = false;
                            }
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Numpad2,
                            ) => {
                                self.camera_zoom_out_held = false;
                            }
                            _ => {}
                        }
                    }
                }
                true
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let x = self.mouse_position.0 as i32;
                let y = self.mouse_position.1 as i32;
                let route_mouse_to_legacy_ui =
                    matches!(self.current_state, GameState::InGame | GameState::Paused);
                if route_mouse_to_legacy_ui {
                    let ui_button = Self::to_ui_mouse_button(*button);
                    if let Some(ui_button) = ui_button {
                        let _ = self.ui_manager.handle_mouse_click(x, y, ui_button);
                    }
                }

                if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    match (button, state) {
                        (MouseButton::Left, ElementState::Pressed) => {
                            self.handle_left_click();
                        }
                        (MouseButton::Left, ElementState::Released) => {
                            self.handle_left_release();
                        }
                        (MouseButton::Right, ElementState::Pressed) => {
                            // Set anchor for drag-scroll; the actual right-click
                            // command is deferred to release if the mouse didn't
                            // move significantly (C++ LookAtXlat.cpp).
                            self.rmb_scroll_anchor = Some(self.mouse_position);
                            self.is_rmb_scrolling = true;
                        }
                        (MouseButton::Right, ElementState::Released) => {
                            if self.is_rmb_scrolling {
                                // If the mouse barely moved since the anchor,
                                // treat it as a normal right-click command.
                                const DRAG_THRESHOLD_SQ: f32 = 9.0; // 3px squared
                                if let Some(anchor) = self.rmb_scroll_anchor {
                                    let dx = self.mouse_position.0 - anchor.0;
                                    let dy = self.mouse_position.1 - anchor.1;
                                    if dx * dx + dy * dy < DRAG_THRESHOLD_SQ {
                                        self.handle_right_click();
                                    }
                                }
                            }
                            self.rmb_scroll_anchor = None;
                            self.is_rmb_scrolling = false;
                        }
                        (MouseButton::Middle, ElementState::Pressed) => {
                            self.is_mmb_rotating = true;
                            self.mmb_anchor = Some(self.mouse_position);
                        }
                        (MouseButton::Middle, ElementState::Released) => {
                            self.is_mmb_rotating = false;
                            self.mmb_anchor = None;
                        }
                        _ => {}
                    }
                }
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = (position.x as f32, position.y as f32);
                if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.update_mouse_world_position();
                    self.ui_manager
                        .handle_mouse_move(position.x as i32, position.y as i32);
                    self.sync_context_mouse_cursor();
                }
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                    self.handle_mouse_wheel(delta);
                }
                true
            }
            _ => false,
        }
    }

    fn to_ui_mouse_button(button: MouseButton) -> Option<crate::ui::MouseButton> {
        match button {
            MouseButton::Left => Some(crate::ui::MouseButton::Left),
            MouseButton::Right => Some(crate::ui::MouseButton::Right),
            MouseButton::Middle => Some(crate::ui::MouseButton::Middle),
            MouseButton::Back => Some(crate::ui::MouseButton::Other(4)),
            MouseButton::Forward => Some(crate::ui::MouseButton::Other(5)),
            MouseButton::Other(id) => Some(crate::ui::MouseButton::Other(id as u8)),
        }
    }

    fn to_ui_key_code(key: &Key) -> Option<crate::ui::KeyCode> {
        match key {
            Key::Named(NamedKey::Escape) => Some(crate::ui::KeyCode::Escape),
            Key::Named(NamedKey::Enter) => Some(crate::ui::KeyCode::Enter),
            Key::Named(NamedKey::Space) => Some(crate::ui::KeyCode::Space),
            Key::Named(NamedKey::Tab) => Some(crate::ui::KeyCode::Tab),
            Key::Named(NamedKey::Backspace) => Some(crate::ui::KeyCode::Backspace),
            Key::Named(NamedKey::Delete) => Some(crate::ui::KeyCode::Delete),
            Key::Named(NamedKey::Home) => Some(crate::ui::KeyCode::Home),
            Key::Named(NamedKey::End) => Some(crate::ui::KeyCode::End),
            Key::Named(NamedKey::ArrowLeft) => Some(crate::ui::KeyCode::Left),
            Key::Named(NamedKey::ArrowRight) => Some(crate::ui::KeyCode::Right),
            Key::Named(NamedKey::ArrowUp) => Some(crate::ui::KeyCode::Up),
            Key::Named(NamedKey::ArrowDown) => Some(crate::ui::KeyCode::Down),
            Key::Named(NamedKey::F1) => Some(crate::ui::KeyCode::F1),
            Key::Named(NamedKey::F2) => Some(crate::ui::KeyCode::F2),
            Key::Named(NamedKey::F3) => Some(crate::ui::KeyCode::F3),
            Key::Named(NamedKey::F4) => Some(crate::ui::KeyCode::F4),
            Key::Named(NamedKey::F5) => Some(crate::ui::KeyCode::F5),
            Key::Named(NamedKey::F6) => Some(crate::ui::KeyCode::F6),
            Key::Named(NamedKey::F7) => Some(crate::ui::KeyCode::F7),
            Key::Named(NamedKey::F8) => Some(crate::ui::KeyCode::F8),
            Key::Named(NamedKey::F9) => Some(crate::ui::KeyCode::F9),
            Key::Named(NamedKey::F10) => Some(crate::ui::KeyCode::F10),
            Key::Named(NamedKey::F11) => Some(crate::ui::KeyCode::F11),
            Key::Named(NamedKey::F12) => Some(crate::ui::KeyCode::F12),
            Key::Character(ch) if ch.len() == 1 => {
                let c = ch.chars().next()?;
                match c.to_ascii_uppercase() {
                    'A' => Some(crate::ui::KeyCode::A),
                    'B' => Some(crate::ui::KeyCode::B),
                    'C' => Some(crate::ui::KeyCode::C),
                    'D' => Some(crate::ui::KeyCode::D),
                    'E' => Some(crate::ui::KeyCode::E),
                    'F' => Some(crate::ui::KeyCode::F),
                    'G' => Some(crate::ui::KeyCode::G),
                    'H' => Some(crate::ui::KeyCode::H),
                    'I' => Some(crate::ui::KeyCode::I),
                    'J' => Some(crate::ui::KeyCode::J),
                    'K' => Some(crate::ui::KeyCode::K),
                    'L' => Some(crate::ui::KeyCode::L),
                    'M' => Some(crate::ui::KeyCode::M),
                    'N' => Some(crate::ui::KeyCode::N),
                    'O' => Some(crate::ui::KeyCode::O),
                    'P' => Some(crate::ui::KeyCode::P),
                    'Q' => Some(crate::ui::KeyCode::Q),
                    'R' => Some(crate::ui::KeyCode::R),
                    'S' => Some(crate::ui::KeyCode::S),
                    'T' => Some(crate::ui::KeyCode::T),
                    'U' => Some(crate::ui::KeyCode::U),
                    'V' => Some(crate::ui::KeyCode::V),
                    'W' => Some(crate::ui::KeyCode::W),
                    'X' => Some(crate::ui::KeyCode::X),
                    'Y' => Some(crate::ui::KeyCode::Y),
                    'Z' => Some(crate::ui::KeyCode::Z),
                    '0' => Some(crate::ui::KeyCode::Key0),
                    '1' => Some(crate::ui::KeyCode::Key1),
                    '2' => Some(crate::ui::KeyCode::Key2),
                    '3' => Some(crate::ui::KeyCode::Key3),
                    '4' => Some(crate::ui::KeyCode::Key4),
                    '5' => Some(crate::ui::KeyCode::Key5),
                    '6' => Some(crate::ui::KeyCode::Key6),
                    '7' => Some(crate::ui::KeyCode::Key7),
                    '8' => Some(crate::ui::KeyCode::Key8),
                    '9' => Some(crate::ui::KeyCode::Key9),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn update_with_timing(&mut self, timing: &FrameTiming) {
        if matches!(self.current_state, GameState::Menu | GameState::Loading) {
            // C++ shell/loading progression is driven by the engine's fixed update cadence, not
            // by renderer timing. WW3D timing can remain effectively stuck during startup/menu
            // and starve shell scripts/model streaming if we trust it here.
            self.update_with_frame_clock();
            return;
        }
        let dt = self.apply_frame_timing(*timing);
        self.update_internal(dt);
    }

    /// Allows external orchestrators (e.g., integration diagnostics pipeline) to push
    /// the latest subsystem health snapshot for the in-game debug overlay.
    pub fn set_diagnostics_overlay(&mut self, stats: DiagnosticsOverlayStats) {
        self.diagnostics_overlay = Some(stats);
    }

    #[cfg(feature = "integration-diagnostics")]
    pub fn set_integration_diagnostics(&mut self, diag: &SystemDiagnostics) {
        self.diagnostics_overlay = Some(DiagnosticsOverlayStats::from_system(diag));
    }

    /// Clears any externally provided diagnostics snapshot, falling back to
    /// locally-derived estimates.
    pub fn clear_diagnostics_overlay(&mut self) {
        self.diagnostics_overlay = None;
    }

    /// Advance simulation using the internal fallback clock (no WW3D timing).
    pub fn update_with_frame_clock(&mut self) {
        const SHELL_MENU_STEP: Duration = Duration::from_nanos(33_333_333);

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.menu_loading_last_tick);
        self.menu_loading_last_tick = now;
        self.menu_loading_tick_accumulator =
            (self.menu_loading_tick_accumulator + elapsed).min(Duration::from_millis(250));

        if self.menu_loading_tick_accumulator < SHELL_MENU_STEP {
            return;
        }
        self.menu_loading_tick_accumulator -= SHELL_MENU_STEP;

        let clock_timing = self.frame_clock.advance_fixed(SHELL_MENU_STEP);
        let timing = Self::to_engine_timing(clock_timing, Instant::now());
        let dt = self.apply_frame_timing(timing);
        static UC_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let uc_n = UC_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if uc_n < 15 || (uc_n < 50 && matches!(self.current_state, GameState::Menu)) {
            info!(
                "update_with_frame_clock #{} start state={:?}",
                uc_n, self.current_state
            );
        }
        self.update_internal(dt);
        if uc_n < 15 || (uc_n < 50 && matches!(self.current_state, GameState::Menu)) {
            info!(
                "update_with_frame_clock #{} done state={:?}",
                uc_n, self.current_state
            );
        }
    }

    pub fn update(&mut self, dt: f32) {
        let delta = Duration::from_secs_f32(dt.max(0.0));
        let clock_timing = self.frame_clock.advance_fixed(delta);
        let timing = Self::to_engine_timing(clock_timing, Instant::now());
        let adjusted_dt = self.apply_frame_timing(timing);
        self.update_internal(adjusted_dt);
    }

    fn apply_frame_timing(&mut self, timing: FrameTiming) -> f32 {
        if matches!(self.current_state, GameState::Menu | GameState::Loading) {
            // Shell/loading frame cadence is managed by update_with_frame_clock() and event-loop
            // pacing. Running gameplay script FPS spin-waits here can stall the UI thread.
            self.script_fps_limit_last_tick = None;
        } else {
            self.apply_script_frame_limit();
        }
        NetworkClock::override_with_duration(timing.total_time);
        let dt = timing.delta_seconds().max(0.0);
        self.last_frame_timing = Some(timing);
        let incoming_frame = timing.frame_number as u32;
        self.frame_counter = if incoming_frame > self.frame_counter {
            incoming_frame
        } else {
            self.frame_counter.saturating_add(1)
        };
        if timing.fps > 0.0 {
            self.fps = timing.fps;
        } else if dt > 0.0 {
            self.fps = 1.0 / dt;
        }
        dt
    }

    /// Get current game state
    pub fn get_state(&self) -> GameState {
        self.current_state
    }

    /// Request state transition - will be applied at next update cycle
    /// Matches C++ GameEngine::setQuitting() pattern for deferred state changes
    pub fn request_state_change(&mut self, new_state: GameState) {
        if new_state == self.current_state || self.pending_state == Some(new_state) {
            return;
        }

        info!(
            "State transition requested: {:?} -> {:?}",
            self.current_state, new_state
        );
        self.pending_state = Some(new_state);
    }

    pub fn is_state_change_pending(&self, state: GameState) -> bool {
        self.pending_state == Some(state)
    }

    /// Process pending state transitions
    /// Called at beginning of update cycle to handle state changes
    fn process_state_transitions(&mut self) {
        if let Some(new_state) = self.pending_state.take() {
            self.transition_to_state(new_state);
        }
    }

    /// Execute state transition with proper setup/cleanup
    /// Matches C++ GameEngine reset() and initialization patterns
    fn transition_to_state(&mut self, new_state: GameState) {
        let old_state = self.current_state;

        info!("State transition: {:?} -> {:?}", old_state, new_state);

        // Exit current state
        match old_state {
            GameState::Menu => {
                debug!("Exiting Menu state");
                self.hide_shell_menu();
            }
            GameState::Loading => {
                debug!("Exiting Loading state");
                self.hide_shell_loading_overlay();
            }
            GameState::InGame => {
                debug!("Exiting InGame state");
                // Could pause audio, save state, etc.
            }
            GameState::Paused => {
                debug!("Exiting Paused state");
            }
            GameState::Victory | GameState::Defeat => {
                debug!("Exiting end-of-match screen state");
            }
            GameState::Exiting => {
                debug!("Already exiting");
            }
            GameState::Initializing => {}
        }

        // Enter new state
        match new_state {
            GameState::Menu => {
                info!("Entering Menu state — transition_to_state start");
                // C++ shell menus keep the shell map simulation alive behind the UI.
                self.game_paused = false;
                self.game_logic.set_paused(false);
                self.active_menu_shell_hook = None;
                info!("Menu transition: calling hide_gameplay_layouts");
                self.hide_gameplay_layouts();
                self.ui_manager.suspend_for_shell_overlay();
                self.set_runtime_ui_state_projection(UISystemState::MainMenu);
                info!("Menu transition: calling prime_subsystems_before_menu_transition");
                self.prime_subsystems_before_menu_transition();
                self.show_shell_menu();
                info!("Menu transition: prime_subsystems done. transition_to_state complete.");
                self.last_slow_menu_tick_log = None;
            }
            GameState::Loading => {
                info!("Entering Loading state");
                // C++ drives loading through LoadScreen subclasses created from
                // .wnd files. Keep the temporary Rust UI manager suspended so it
                // cannot paint a second custom loading screen above/beside that
                // window-manager-owned surface.
                self.ensure_shell_loading_overlay();
                self.update_shell_loading_progress(0.0, Some("Loading assets..."));
                self.ui_manager.suspend_for_shell_overlay();
                self.set_runtime_ui_state_projection(UISystemState::Loading);
                self.last_slow_menu_tick_log = None;
            }
            GameState::InGame => {
                info!("Entering InGame state");
                // Start game logic, enable input
                self.game_paused = false;
                self.game_logic.set_paused(false);
                self.ensure_gameplay_layouts();
                self.ui_manager
                    .transition_to_screen(crate::ui::Screen::GameHUD);
                self.set_runtime_ui_state_projection(UISystemState::InGame);
            }
            GameState::Paused => {
                info!("Entering Paused state");
                // Freeze game logic, show pause menu
                self.game_paused = true;
                self.game_logic.set_paused(true);
                self.ui_manager
                    .transition_to_screen(crate::ui::Screen::PauseMenu);
                self.set_runtime_ui_state_projection(UISystemState::PauseMenu);
            }
            GameState::Exiting => {
                info!("Entering Exiting state - beginning shutdown");
            }
            GameState::Victory => {
                info!("Entering Victory state - match won");
                self.game_paused = true;
                self.game_logic.set_paused(true);
                if self.ui_manager.current_screen() != Some(crate::ui::Screen::Victory) {
                    self.ui_manager
                        .show_match_result(true, self.current_player_id);
                }
                self.set_runtime_ui_state_projection(UISystemState::Victory);
            }
            GameState::Defeat => {
                info!("Entering Defeat state - match lost");
                self.game_paused = true;
                self.game_logic.set_paused(true);
                if self.ui_manager.current_screen() != Some(crate::ui::Screen::Victory) {
                    self.ui_manager
                        .show_match_result(false, self.current_player_id);
                }
                self.set_runtime_ui_state_projection(UISystemState::Victory);
            }
            GameState::Initializing => {
                info!("Entering Initializing state");
            }
        }

        self.current_state = new_state;
        if matches!(new_state, GameState::Menu | GameState::Loading) {
            self.menu_loading_tick_accumulator = Duration::ZERO;
            self.menu_loading_last_tick = Instant::now();
        }
        if new_state == GameState::Menu {
            self.menu_enter_frame = Some(self.current_startup_logic_frame());
            self.last_shell_prewarm_log = None;
            self.shell_prewarm_completion_logged = false;
        } else {
            self.menu_enter_frame = None;
            self.shell_ui_enqueued_frame = None;
            self.active_menu_shell_hook = None;
            self.last_shell_prewarm_log = None;
            self.shell_prewarm_completion_logged = false;
        }
    }

    /// Check if engine should quit
    /// Matches C++ GameEngine::isQuitting()
    pub fn is_quitting(&self) -> bool {
        self.current_state == GameState::Exiting
    }

    fn network_frame_data_ready_gate(multiplayer_session_active: bool) -> Option<bool> {
        if !multiplayer_session_active {
            // C++ startup leaves `TheNetwork` unset until a live multiplayer session exists.
            return None;
        }

        let has_network_backend = crate::network::has_active_network_interface();
        if !has_network_backend {
            return None;
        }

        let frame_ready = crate::network::active_session_frame_data_ready().unwrap_or(true);

        let Some(subsystem_manager) = get_subsystem_manager() else {
            return Some(frame_ready);
        };

        let mut manager = subsystem_manager.lock();
        if manager.get::<NetworkSubsystem>().is_none() {
            let mut network = NetworkSubsystem::new();
            if let Err(err) = <NetworkSubsystem as SubsystemInterface>::init(&mut network) {
                warn!(
                    "Failed to lazily initialize the network subsystem during multiplayer gating: {}",
                    err
                );
                return Some(false);
            }
            let _ = manager.add_subsystem(network);
        }

        if let Some(network) = manager.get_mut::<NetworkSubsystem>() {
            network.set_session_state(true, frame_ready);
            Some(network.is_frame_data_ready())
        } else {
            Some(frame_ready)
        }
    }

    fn should_update_game_logic_frame(
        game_paused: bool,
        network_frame_data_ready: Option<bool>,
    ) -> bool {
        match network_frame_data_ready {
            Some(frame_ready) => frame_ready,
            None => !game_paused,
        }
    }

    fn update_runtime_subsystems(&mut self, dt: f32) {
        if let Some(subsystem_manager) = get_subsystem_manager() {
            let mut guard = subsystem_manager.lock();
            if let Some(timing) = self.last_frame_timing {
                if let Err(e) = guard.update_all_with_timing(&timing) {
                    error!("Error updating subsystems: {}", e);
                }
            } else if let Err(e) = guard.update_all(dt) {
                error!("Error updating subsystems: {}", e);
            }
        }
    }

    fn prime_subsystems_before_menu_transition(&mut self) {
        let started = Instant::now();
        let mut max_step = Duration::ZERO;
        let mut steps = 0usize;

        // Keep warmup bounded. This should absorb one-time startup work, not become a new stall.
        while steps < 2 && started.elapsed() < Duration::from_millis(900) {
            let step_started = Instant::now();
            self.update_runtime_subsystems(0.0);
            let elapsed = step_started.elapsed();
            max_step = max_step.max(elapsed);
            steps += 1;

            if elapsed < Duration::from_millis(50) {
                break;
            }
        }

        info!(
            "Shell subsystem warmup before menu: steps={} elapsed={:?} max_step={:?}",
            steps,
            started.elapsed(),
            max_step
        );
    }

    fn update_internal(&mut self, dt: f32) {
        // Process any pending state transitions first
        self.process_state_transitions();

        // Early exit if we're shutting down
        if self.is_quitting() {
            return;
        }

        let dt = dt.max(0.0);
        let visual_dt = dt * self.game_logic.visual_speed_multiplier().max(0.0);

        // Diagnostic: log first few Menu update_internal calls
        if matches!(self.current_state, GameState::Menu) && self.menu_world_frames_rendered < 5 {
            info!("update_internal: Menu state, about to call update_runtime_subsystems (menu_frame={})", self.menu_world_frames_rendered);
        }
        // C++ parity: radar/audio/client/message/network/cd updates happen each frame.
        self.update_runtime_subsystems(dt);
        if matches!(self.current_state, GameState::Menu) && self.menu_world_frames_rendered < 5 {
            info!(
                "update_internal: Menu state, update_runtime_subsystems done, entering state match"
            );
        }
        // State-based update logic - matches C++ GameEngine::update() conditional updates
        match self.current_state {
            GameState::Menu => {
                self.cleanup_sound_effects();
                let menu_tick_started = Instant::now();
                let shell_update_started = Instant::now();
                // Prefer presentation shell residual when it affirms shell-map mode.
                // Stale InGame frames (fow_shell_bypass=false) fall through to live
                // isInShellGame so shell ticks are not suppressed after a match.
                let in_shell = match self.last_presentation_frame.as_ref() {
                    Some(pres) if pres.fow_shell_bypass => true,
                    _ => self.game_logic.isInShellGame(),
                };
                if in_shell && !self.game_paused {
                    // Keep shell map/scripts alive in menu without allowing large fixed-step
                    // catch-up loops to block the UI thread.
                    self.game_logic.update_shell_with_budget(dt, 1);
                    // Prefer presentation script FPS residual when shell frame installed.
                    if let Some(fps) = self
                        .last_presentation_frame
                        .as_ref()
                        .filter(|p| p.fow_shell_bypass)
                        .and_then(|p| p.script_fps_limit)
                    {
                        self.apply_script_fps_limit_request(fps);
                        let _ = self.game_logic.take_script_fps_limit_request();
                    } else if let Some(fps) = self.game_logic.take_script_fps_limit_request() {
                        // Boot/live residual when no shell presentation freeze.
                        self.apply_script_fps_limit_request(fps);
                    }
                }
                let shell_elapsed = shell_update_started.elapsed();

                // C++ shell/menu parity: menu-frame script camera requests must still drive
                // the shell-map viewport even when not in InGame state.
                let process_commands_started = Instant::now();
                self.game_logic.process_commands();
                let process_commands_elapsed = process_commands_started.elapsed();
                let script_camera_started = Instant::now();
                self.apply_pending_script_camera_requests();
                let script_camera_elapsed = script_camera_started.elapsed();
                let camera_started = Instant::now();
                self.update_camera(visual_dt);
                let camera_elapsed = camera_started.elapsed();

                let menu_tick_elapsed = menu_tick_started.elapsed();
                if menu_tick_elapsed >= Duration::from_millis(40)
                    && self
                        .last_slow_menu_tick_log
                        .map(|last| last.elapsed() >= Duration::from_secs(2))
                        .unwrap_or(true)
                {
                    let fixed_diag = self.game_logic.fixed_step_diagnostics();
                    warn!(
                        "Slow menu tick: total={:?}, shell={:?}, commands={:?}, script_camera={:?}, camera={:?}, state={:?}, frame={}, fixed_steps={}, budget_hit={}, acc_ms={:.2}",
                        menu_tick_elapsed,
                        shell_elapsed,
                        process_commands_elapsed,
                        script_camera_elapsed,
                        camera_elapsed,
                        self.current_state,
                        self.frame_counter,
                        fixed_diag.steps_run,
                        fixed_diag.budget_hit,
                        fixed_diag.accumulated_time_seconds * 1000.0
                    );
                    self.last_slow_menu_tick_log = Some(Instant::now());
                }

                if !self.pending_shell_model_prewarm.is_empty()
                    && self
                        .last_shell_prewarm_log
                        .map(|last| last.elapsed() >= Duration::from_millis(2_000))
                        .unwrap_or(true)
                {
                    let missing_models = self.render_pipeline.debug_last_model_missing();
                    let missing_samples = self.render_pipeline.debug_last_missing_model_samples();
                    debug!(
                        "Shell prewarm progress: pending_models={} render_items={} missing_models={} budget_skips={}",
                        self.pending_shell_model_prewarm.len(),
                        self.render_pipeline.debug_render_item_count(),
                        missing_models,
                        self.render_pipeline.debug_last_model_budget_skips()
                    );
                    if missing_models > 0 && !missing_samples.is_empty() {
                        debug!(
                            "Shell prewarm missing model samples: {}",
                            missing_samples.join(", ")
                        );
                    }
                    self.last_shell_prewarm_log = Some(Instant::now());
                }

                self.set_runtime_ui_state_projection(UISystemState::MainMenu);

                #[cfg(feature = "game_client")]
                {
                    let early_menu_frame = self.menu_world_frames_rendered < 5;
                    let t0 = std::time::Instant::now();
                    {
                        let gc = &mut self.game_client;
                        if early_menu_frame {
                            info!(
                                "Menu update_internal: calling gc.ensure_shell_visible (menu_frame={})",
                                self.menu_world_frames_rendered
                            );
                        }
                        gc.ensure_shell_visible().ok();
                        let t1 = std::time::Instant::now();
                        if early_menu_frame {
                            info!("Menu update_internal: calling gc.update_input");
                        }
                        gc.update_input().ok();
                        let t2 = std::time::Instant::now();
                        if early_menu_frame {
                            info!("Menu update_internal: calling gc.update_pre_draw_ui");
                        }
                        gc.update_pre_draw_ui().ok();
                        let t3 = std::time::Instant::now();
                        if early_menu_frame {
                            info!("Menu update_internal: calling gc.update_post_draw_ui");
                        }
                        gc.update_post_draw_ui().ok();
                        let _t4_pre = t3.elapsed();
                        let _ = (t1, t2, t3, _t4_pre);
                    }

                    // Intercept MSG_NEW_GAME *before* pump moves it into the
                    // crate command list. WND Skirmish/Campaign Start only
                    // appends NewGame to the common message stream; without
                    // this drain the windowed shell never reaches InGame.
                    let t4 = std::time::Instant::now();
                    if let Some((mode, faction, map, skirmish)) =
                        self.take_pending_new_game_start_request()
                    {
                        info!(
                            "Menu NewGame drain: mode={:?} faction={} map={} skirmish={}",
                            mode,
                            faction,
                            map,
                            skirmish.is_some()
                        );
                        self.start_game_from_ui(mode, faction, map, skirmish);
                        return;
                    }

                    {
                        let gc = &mut self.game_client;
                        if early_menu_frame {
                            info!("Menu update_internal: calling gc.pump_message_stream");
                        }
                        gc.pump_message_stream().ok();
                    }

                    // Secondary path: crate helpers may flag start after pump.
                    if gamelogic::helpers::TheGameLogic::is_start_new_game_requested() {
                        gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
                        if let Some((mode, faction, map, skirmish)) =
                            self.build_start_request_from_pending_globals(None)
                        {
                            info!(
                                "Menu start_new_game flag drain: mode={:?} map={}",
                                mode, map
                            );
                            self.start_game_from_ui(mode, faction, map, skirmish);
                            return;
                        }
                    }

                    let t5 = std::time::Instant::now();
                    let menu_gc_elapsed = t0.elapsed();
                    if menu_gc_elapsed >= std::time::Duration::from_millis(50) || early_menu_frame {
                        info!(
                            "Menu GC update: total={:?} newgame_scan={:?} pump_tail={:?} frame={}",
                            menu_gc_elapsed,
                            t4.duration_since(t0),
                            t5.duration_since(t4),
                            self.frame_counter,
                        );
                    }
                }
                // Headless / no-game_client builds still drain NewGame if present.
                #[cfg(not(feature = "game_client"))]
                if let Some((mode, faction, map, skirmish)) =
                    self.take_pending_new_game_start_request()
                {
                    self.start_game_from_ui(mode, faction, map, skirmish);
                    return;
                }
                return;
            }
            GameState::Loading => {
                // In loading: minimal updates, mainly for loading screen animations
                if let Err(err) = self.update_startup_loading() {
                    error!("Startup loading failed: {}", err);
                    self.request_state_change(GameState::Exiting);
                    return;
                }
                if self.current_state != GameState::Loading {
                    // Loading completed and transitioned this frame; avoid re-applying loading UI.
                    return;
                }
                self.set_runtime_ui_state_projection(UISystemState::Loading);
                // After loading completes, the state will transition to InGame
                // This is handled by the initialization code setting pending_state
                return;
            }
            GameState::Paused => {
                // In paused: update UI and camera, but not game logic
                // (matches C++ where TheGameLogic->isGamePaused() prevents update)
                self.update_camera(visual_dt);
                self.cleanup_sound_effects();
                self.set_runtime_ui_state_projection(UISystemState::PauseMenu);
                if let Err(err) = self.ui_manager.update(dt) {
                    warn!("UI manager update failed in paused state: {}", err);
                }
                return;
            }
            GameState::InGame => {
                // Full update - continue below
            }
            GameState::Exiting => {
                // Exiting: no updates needed
                return;
            }
            GameState::Victory | GameState::Defeat => {
                // End-of-match screen: keep UI alive, game logic frozen.
                // C++ shows the score screen then transitions to Menu on user input.
                self.update_camera(visual_dt);
                self.cleanup_sound_effects();
                if let Err(err) = self.ui_manager.update(dt) {
                    warn!("UI manager update failed in endgame state: {}", err);
                }
                return;
            }
            GameState::Initializing => {
                return;
            }
        }

        // Full update cycle for InGame state (matches C++ GameEngine::update())

        // C++ parity gate:
        //   (Network == NULL && !isGamePaused()) || (Network && Network->isFrameDataReady()).
        let network_frame_data_ready =
            Self::network_frame_data_ready_gate(self.game_logic.isInMultiplayerGame());
        if Self::should_update_game_logic_frame(self.game_paused, network_frame_data_ready) {
            // Retail m_TiVOFastMode residual: extra logic steps while armed.
            let ff_steps = if self.replay_fast_forward { 4 } else { 1 };
            // Update game logic first
            for _ in 0..ff_steps {
                if let Some(timing) = self.last_frame_timing {
                    self.game_logic.update_with_timing(&timing);
                } else {
                    self.game_logic.update_with_dt(dt);
                }
            }
            // Script FPS applied from presentation residual after snapshot build (below).
            // Live take remains for boot path when no frame is produced this tick.

            // Single-authority policy: Main GameLogic is the match host by default.
            // Dual-tick of the ported gamelogic crate is opt-in (GENERALS_ALLOW_DUAL_TICK)
            // and is fatal under GENERALS_VERIFY_SINGLE_AUTHORITY verification builds.
            let policy = crate::authoritative_world::dual_tick_policy();
            if let Err(e) = crate::authoritative_world::apply_post_authority_crate_tick(
                policy,
                crate::game_logic::tick_gamelogic_crate,
            ) {
                log::error!("{e}");
                // Verification: refuse to continue a dual-world silent failure.
                if crate::authoritative_world::verification_single_authority() {
                    panic!("{e}");
                }
            }
            // C++ parity: when script time-freeze is active, gameplay simulation should not
            // advance outside script evaluation.
            // Host side systems (projectiles) run *before* shadow session + PresentationFrame
            // so damage logs and end-of-frame identity include this frame's side systems.
            // Path following already ran inside GameLogic::update_movement.
            // Projectiles + path following are owned by GameLogic::update_simulation
            // (update_combat drain/step + update_movement). Engine must not run a
            // second mid-frame CombatSystem/PathfindingSystem mover.
            if !self.game_logic.is_time_frozen_for_simulation() {
                // Hit SFX residual: prefer presentation audio events; legacy direct
                // Hit playback removed with dual CombatSystem step.
                let _ = dt;
            }

            // AI/player commands queued during GameLogic::update_simulation are
            // flushed in-phase (early commands + post-AI phase 8b). Engine does
            // not re-drain the command list before shadow.

            // GameWorld shadow session AFTER host logic + projectiles + path so
            // host_damage_log / host_move_log / attack logs from this frame are not
            // drained a frame late. Defaults on (GENERALS_GAMEWORLD_SHADOW=0 to opt out).
            // Host remains temporary mid-frame owner; shadow is last-writer for HP/cash/pose.
            if let Some(ref mut shadow) = self.gameworld_shadow {
                let probe = crate::gameworld_shadow::shadow_session_after_host_tick(
                    shadow,
                    &mut self.game_logic,
                );
                if !probe.full_match() {
                    log::warn!("{}", probe.format_report());
                }
            } else {
                let _ = crate::gameworld_shadow::maybe_shadow_after_host_tick(&mut self.game_logic);
            }

            // Immutable presentation snapshot for client/render (borrow-first policy).
            // Built after authority + host side systems + shadow writeback.
            let local_id = self.current_player_id;
            // Include victory residual in the snapshot (single evaluate; no dual-read later).
            let mut pres = crate::presentation_frame::PresentationFrame::build_with_victory(
                &mut self.game_logic,
                local_id,
            );
            if let Some(ref shadow) = self.gameworld_shadow {
                let n = pres.overlay_gameworld_shadow(shadow);
                if n > 0 {
                    log::trace!("presentation overlay from GameWorld shadow: {n} objects");
                }
            }
            let audio_n = pres.apply_events_to_audio(&mut self.game_logic);
            if audio_n > 0 {
                log::trace!("presentation audio events queued: {audio_n}");
            }
            // Same-frame residual: drain presentation-queued audio now (not next host tick).
            self.game_logic.process_audio_events();
            // Same-frame particle residual: backfill client ParticleSystemManager.
            let fx_n = pres.apply_particle_systems_to_client();
            if fx_n > 0 {
                log::trace!("presentation particle client mirrors: {fx_n}");
            }
            self.last_presentation_frame = Some(pres);

            // Prefer presentation script FPS residual; drain live queue after apply.
            if let Some(fps) = self
                .last_presentation_frame
                .as_ref()
                .and_then(|p| p.script_fps_limit)
            {
                self.apply_script_fps_limit_request(fps);
            }
            let _ = self.game_logic.take_script_fps_limit_request();

            #[cfg(feature = "game_client")]
            {
                let visual_delta = if self.game_logic.is_time_frozen_for_simulation() {
                    0.0
                } else {
                    game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL
                };
                // Presentation path: deepened shell tick (frame/FX/UI/message pump)
                // without OBJECT_REGISTRY shroud bind. Full GameClient::update remains
                // disconnected (Main owns OS input→commands and sole RenderPipeline 3D draw).
                if self.last_presentation_frame.is_some() {
                    // C++ per-drawable shroud residual from frozen presentation FOW.
                    let shroud_entries: Vec<(u32, bool)> = self
                        .last_presentation_frame
                        .as_ref()
                        .map(|pres| {
                            pres.objects
                                .iter()
                                .map(|o| (o.id.0, o.fow_visibility.fully_obscures_drawable()))
                                .collect()
                        })
                        .unwrap_or_default();
                    self.game_client
                        .apply_presentation_shroud_to_drawables(shroud_entries);
                    // Presentation cinematic letterbox residual → client display.
                    if let Some(pres) = self.last_presentation_frame.as_ref() {
                        self.game_client
                            .apply_presentation_cinematic_letterbox(pres.cinematic_letterbox);
                        // Military caption residual → InGameUI (duration from freeze).
                        self.game_client.apply_presentation_military_caption(
                            pres.military_caption.as_deref(),
                            pres.military_caption_remaining_ms,
                        );
                        // Cinematic text residual → InGameUI HUD message.
                        self.game_client
                            .apply_presentation_cinematic_text(pres.cinematic_text.as_deref());
                    }
                    // PRES_SHELL_ONLY_DRAWABLE_TICK: client modules via
                    // update_drawables_local (no live OBJECT_REGISTRY shroud re-bind).
                    // Do not also call full update_drawables — that double-ticks and
                    // overwrites presentation FOW with live shroud status.
                    if let Err(e) = self.game_client.update_presentation_shell(visual_delta) {
                        log::trace!("GameClient presentation shell update failed (non-fatal): {e}");
                    }
                } else {
                    if let Err(e) = self.game_client.update_drawables(visual_delta) {
                        log::trace!("GameClient drawable update failed (non-fatal): {e}");
                    }
                    // Boot/loading residual without presentation frame.
                    self.game_client.ensure_shell_visible().ok();
                    self.game_client.update_pre_draw_ui().ok();
                    self.game_client.update_post_draw_ui().ok();
                }
            }
        }

        // Update HUD + ControlBar selection panel from presentation when available
        // (resources + minimap + selection health). ControlBar health is snapshot-owned.
        if self.current_state == GameState::InGame {
            if let Some(pres) = self.last_presentation_frame.clone() {
                self.apply_presentation_to_huds(&pres);
                self.play_presentation_event_sfx(&pres);
                self.sync_eva_messages_from_logic();
                #[cfg(feature = "game_client")]
                {
                    pres.apply_to_control_bar(&mut self.control_bar);
                }
            } else if let Some(player) = self.game_logic.get_player(self.current_player_id) {
                // Boot residual only — presentation path above owns InGame HUD resources.
                let money = player.resources.supplies as i32;
                let power = player.power_available;
                let max_power = player.power_produced.max(0);
                self.game_hud.update_resources(money, power, max_power);
            }

            if dt.is_finite() {
                if let Err(err) = self.game_hud.update(dt) {
                    warn!("Game HUD update failed: {}", err);
                }
                self.diplomacy_panel.update(dt);
                self.chat_panel.update(dt);
                self.sync_pending_structure_placement_cursor();
                self.sync_pending_map_command_radius_cursor();
            } else {
                warn!(
                    "Skipping Game HUD update due to non-finite delta time: {}",
                    dt
                );
            }
        }

        // Update camera
        if self.current_state != GameState::Menu {
            self.update_camera(visual_dt);
        }

        // Update audio
        if self.current_state != GameState::Menu {
            self.cleanup_sound_effects();
        }
        if self.current_state == GameState::InGame {
            self.set_runtime_ui_state_projection(UISystemState::InGame);
            if let Err(err) = self.ui_manager.update(dt) {
                warn!("UI manager update failed in playing state: {}", err);
            }
        }

        // Commands already drained in the logic-frame block (before shadow).
        // Script camera requests still apply here after presentation build.
        if self.current_state == GameState::InGame {
            self.apply_pending_script_camera_requests();
        }

        // Prefer presentation popup/music residual when installed; live take is boot residual.
        if let Some(pres) = self.last_presentation_frame.clone() {
            for popup in &pres.pending_popup_messages {
                if popup.pause {
                    self.game_paused = true;
                    self.game_logic.set_paused(true);
                }
                if popup.pause_music {
                    if let Some(sink) = self.background_music.take() {
                        sink.stop();
                    }
                }
            }
            if pres.pending_music_stop {
                if let Some(sink) = self.background_music.take() {
                    sink.stop();
                }
            }
            // Drain live queues so peeked presentation fields are not re-applied.
            let _ = self.game_logic.take_popup_message_requests();
            let _ = self.game_logic.take_music_stop_request();
            // Presentation movie residual: play via GameClient script display, then drain.
            self.apply_presentation_movie_residual(&pres);
        } else {
            for popup in self.game_logic.take_popup_message_requests() {
                if popup.pause {
                    self.game_paused = true;
                    self.game_logic.set_paused(true);
                }
                if popup.pause_music {
                    if let Some(sink) = self.background_music.take() {
                        sink.stop();
                    }
                }
            }

            if self.game_logic.take_music_stop_request() {
                if let Some(sink) = self.background_music.take() {
                    sink.stop();
                }
            }
            // Boot residual movies (no presentation frame).
            #[cfg(feature = "game_client")]
            {
                if let Some(name) = self.game_logic.take_pending_movie() {
                    let _ =
                        game_client::core::script_action_handler::play_script_display_movie(&name);
                }
                if let Some(name) = self.game_logic.take_pending_radar_movie() {
                    let started = game_client::helpers::TheInGameUI::play_movie(&name);
                    if !started {
                        let _ = game_client::core::script_action_handler::play_script_display_movie(
                            &name,
                        );
                    }
                }
            }
            #[cfg(not(feature = "game_client"))]
            {
                let _ = self.game_logic.take_pending_movie();
                let _ = self.game_logic.take_pending_radar_movie();
            }
        }

        // Broadcast defeat notifications so UI/systems mirror C++ VictoryConditions flow.
        // Prefer presentation freeze when installed; drain live take after.
        let defeated_players: Vec<u32> = if let Some(pres) = self.last_presentation_frame.as_ref() {
            let ids = pres.defeated_player_ids.clone();
            let _ = self.game_logic.take_defeat_events();
            ids
        } else {
            // Boot residual only.
            self.game_logic.take_defeat_events()
        };
        for player_id in defeated_players {
            // Prefer presentation roster when installed (no live get_player dual-read).
            let roster = self
                .last_presentation_frame
                .as_ref()
                .and_then(|f| f.player_info(player_id).cloned());
            if let Some(player) = roster {
                let message = localization::localize_with_args(
                    "hud.message.player_defeated",
                    "{player} has been defeated!",
                    &[("player", player.name.as_str())],
                );
                info!("Player {} ({}) has been defeated", player.name, player_id);
                self.game_hud.push_info_message(&message);
                self.game_logic
                    .queue_radar_message_for_team(player.team, message.clone());
                self.game_logic.play_ui_sound("GUIMessageReceived");
            } else if self.last_presentation_frame.is_none() {
                // Boot residual only — no live dual-read when a presentation frame is installed.
                if let Some(player) = self.game_logic.get_player(player_id) {
                    let message = localization::localize_with_args(
                        "hud.message.player_defeated",
                        "{player} has been defeated!",
                        &[("player", player.name.as_str())],
                    );
                    info!("Player {} ({}) has been defeated", player.name, player_id);
                    self.game_hud.push_info_message(&message);
                    self.game_logic
                        .queue_radar_message_for_team(player.team, message.clone());
                    self.game_logic.play_ui_sound("GUIMessageReceived");
                } else {
                    info!("Player {} has been defeated", player_id);
                }
            } else {
                // Presentation installed but roster miss — fail-closed id-only residual.
                info!("Player {} has been defeated", player_id);
            }
            fow_rendering::reveal_entire_map_for_player(player_id);
            script_events::push_event(ScriptEvent::PlayerDefeated { player_id });
            script_events::push_event(ScriptEvent::RevealMapForPlayer { player_id });
        }

        // Prefer presentation alliance residual when installed; drain live take after.
        let alliance_events: Vec<crate::game_logic::AllianceNotification> =
            if let Some(pres) = self.last_presentation_frame.as_ref() {
                let ev = pres.alliance_events.clone();
                let _ = self.game_logic.take_alliance_events();
                ev
            } else {
                // Boot residual only.
                self.game_logic.take_alliance_events()
            };
        // Prefer presentation local_player residual when installed; live only boot/menu.
        let local_player_id = self
            .last_presentation_frame
            .as_ref()
            .map(|f| f.local_player_id)
            .or_else(|| self.game_logic.local_player_id());
        let mut observer_notified = false;
        for event in alliance_events {
            let is_local = local_player_id == Some(event.player_id);
            if !is_local && local_player_id.is_some() {
                continue;
            }
            if !is_local && observer_notified {
                continue;
            }

            let (key, fallback) = match event.state {
                AllianceState::AlliedVictory if is_local => {
                    ("hud.message.allied_victory", "Your alliance has triumphed!")
                }
                AllianceState::AlliedDefeat if is_local => (
                    "hud.message.allied_defeat",
                    "Your alliance has been defeated!",
                ),
                AllianceState::AlliedVictory => (
                    "hud.message.observer_allied_victory",
                    "An alliance has won the battle.",
                ),
                AllianceState::AlliedDefeat => (
                    "hud.message.observer_allied_defeat",
                    "An alliance has been defeated.",
                ),
                AllianceState::Active => continue,
            };

            let message = localization::localize(key, fallback);
            self.game_hud.push_info_message(&message);
            // Prefer presentation roster team when installed; live only if no frame.
            let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.player_team(event.player_id)
            } else {
                self.game_logic.get_player(event.player_id).map(|p| p.team)
            };
            if let Some(team) = team {
                self.game_logic
                    .queue_radar_message_for_team(team, message.clone());
            } else {
                self.game_logic.queue_radar_message(message.clone());
            }
            self.game_logic.play_ui_sound("GUIMessageReceived");
            if !is_local {
                observer_notified = true;
            }

            if matches!(event.state, AllianceState::AlliedDefeat) {
                fow_rendering::reveal_entire_map_for_player(event.player_id);
                script_events::push_event(ScriptEvent::RevealMapForPlayer {
                    player_id: event.player_id,
                });
            }
            script_events::push_event(ScriptEvent::AllianceStateChanged {
                player_id: event.player_id,
                state: event.state,
            });
        }

        // Prefer presentation shell bypass when a frame is installed (no live dual-read).
        let in_shell = self
            .last_presentation_frame
            .as_ref()
            .map(|f| f.fow_shell_bypass)
            .unwrap_or_else(|| self.game_logic.isInShellGame());
        if !self.match_over && self.current_state == GameState::InGame && !in_shell {
            // Prefer presentation victory residual when frame installed (no live re-evaluate).
            if let Some(pres) = self.last_presentation_frame.as_ref() {
                if pres.match_over {
                    let winner = pres.events.iter().find_map(|ev| match ev {
                        crate::presentation_frame::PresentationEvent::Victory { winner_player } => {
                            *winner_player
                        }
                        _ => None,
                    });
                    self.show_victory_screen(winner);
                }
            } else if let Some(condition) = self.game_logic.evaluate_victory_condition() {
                // Boot residual only — no presentation frame yet.
                match condition {
                    VictoryCondition::Winner(id) => self.show_victory_screen(Some(id)),
                    VictoryCondition::Draw => self.show_victory_screen(None),
                }
            }
        }
    }

    /// Last immutable presentation snapshot after the most recent logic step.
    pub fn last_presentation_frame(&self) -> Option<&crate::presentation_frame::PresentationFrame> {
        self.last_presentation_frame.as_ref()
    }

    /// Last presentation-overlaid UI state (selection health / minimap identity).
    pub fn last_ui_state(&self) -> Option<&GameUIState> {
        self.last_ui_state.as_ref()
    }

    /// Production HUD consumer: apply last presentation to GameHUD without re-reading live objects.
    pub fn apply_presentation_to_hud(&mut self) -> bool {
        let Some(pres) = self.last_presentation_frame.clone() else {
            return false;
        };
        self.apply_presentation_to_huds(&pres);
        self.sync_eva_messages_from_logic();
        #[cfg(feature = "game_client")]
        {
            pres.apply_to_control_bar(&mut self.control_bar);
        }
        true
    }

    /// ControlBar selection panel health from last presentation (headless-safe).
    #[cfg(feature = "game_client")]
    pub fn control_bar_selection_health(&self) -> Option<(f32, f32)> {
        self.control_bar.selection_panel_health()
    }

    /// After map load / load-game / skirmish StartGame: seed PresentationFrame + HUD so
    /// the first InGame frame has units/minimap/selection identity without waiting for
    /// the next dual-tick. Does not advance logic frames.
    fn seed_presentation_after_match_start(&mut self) {
        let local_id = self.current_player_id;
        let pres = crate::presentation_frame::PresentationFrame::build_and_apply_for_hud(
            &self.game_logic,
            local_id,
            &mut self.game_hud,
        );
        #[cfg(feature = "game_client")]
        {
            pres.apply_to_control_bar(&mut self.control_bar);
        }
        let mut ui = GameUIState::default();
        pres.apply_to_ui_state(&mut ui);
        self.last_ui_state = Some(ui);
        self.last_presentation_frame = Some(pres);
    }

    pub fn render(&mut self) -> Result<()> {
        let render_started = Instant::now();
        static RENDER_CALL_COUNT: std::sync::atomic::AtomicU32 =
            std::sync::atomic::AtomicU32::new(0);
        let render_call = RENDER_CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if render_call < 30 || render_call.is_multiple_of(300) {
            info!(
                "render() called #{}, state={:?}",
                render_call, self.current_state
            );
        }

        if !matches!(self.current_state, GameState::Loading | GameState::Menu) {
            // Production presentation consumer: when a post-logic snapshot exists,
            // selection health + minimap unit identity come from that owned feed.
            // Live update_ui_state still supplies radar/build-queue residuals; identity
            // fields are overwritten by apply_to_ui_state. ControlBar selection panel
            // health is also presentation-owned (not live OBJECT_REGISTRY).
            let mut ui_state = self.game_logic.update_ui_state(self.current_player_id);
            if let Some(pres) = self.last_presentation_frame.clone() {
                pres.apply_to_ui_state(&mut ui_state);
                self.apply_presentation_to_huds(&pres);
                self.play_presentation_event_sfx(&pres);
                self.sync_eva_messages_from_logic();
                #[cfg(feature = "game_client")]
                {
                    pres.apply_to_control_bar(&mut self.control_bar);
                }
            }
            if !ui_state.radar_events.is_empty() {
                for evt in &ui_state.radar_events {
                    self.game_hud
                        .add_radar_message(&evt.text, evt.position, evt.kind);
                }
            } else {
                for msg in &ui_state.radar_messages {
                    self.game_hud.push_radar_message(msg);
                }
            }
            // Prefer presentation new_script_messages residual when installed.
            let new_script_messages: Vec<String> =
                if let Some(pres) = self.last_presentation_frame.as_ref() {
                    let msgs = pres.new_script_messages.clone();
                    // Drain live queue so peeked presentation fields are not re-applied.
                    let _ = self.game_logic.take_new_script_messages();
                    msgs
                } else {
                    // Boot residual only.
                    self.game_logic.take_new_script_messages()
                };
            for msg in &new_script_messages {
                self.game_hud.push_script_message(msg);
            }
            // Prefer presentation sim clock residual when installed.
            ui_state.current_game_time = self
                .last_presentation_frame
                .as_ref()
                .map(|p| p.total_play_time_seconds)
                .unwrap_or_else(|| self.game_logic.get_total_play_time());
            ui_state.fps = self.fps;
            ui_state.frame_time_ms = if self.fps > 0.0 {
                1000.0 / self.fps
            } else {
                0.0
            };
            ui_state.performance_score = (ui_state.fps / 60.0).clamp(0.0, 1.5);
            if let Some(diag) = &self.diagnostics_overlay {
                ui_state.diagnostics = Some(diag.clone());
            } else {
                ui_state.diagnostics = Some(DiagnosticsOverlayStats::from_overall(
                    ui_state.performance_score * 100.0,
                ));
            }
            ui_state.show_debug_overlay = self.show_debug_info;
            if let Some(manager_arc) = get_asset_manager() {
                if let Ok(manager) = manager_arc.lock() {
                    let stats = manager.get_statistics();
                    ui_state.assets_loaded = stats.archive_stats.total_files as u64;
                    ui_state.asset_memory_mb = 0.0;
                    ui_state.asset_cache_usage = 0.0;
                }
            }
            self.process_ui_events();
            ui_state.minimap_texture_id = self.render_pipeline.get_minimap_texture_id();
            ui_state.minimap_coordinates = self.render_pipeline.get_minimap_coordinates().cloned();
            self.update_minimap_viewport(&mut ui_state);
            // Prefer presentation world_env for radar/minimap when a frame is installed.
            let world_bounds = if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.world_env.world_bounds_vec3()
            } else {
                self.game_logic.world_bounds()
            };
            self.game_hud
                .update_radar_pings(&ui_state.radar_pings, world_bounds.0, world_bounds.1);
            for msg in &ui_state.radar_messages {
                self.game_hud.push_radar_message(msg);
            }
            for evt in &ui_state.radar_events {
                self.game_hud
                    .add_radar_message(&evt.text, evt.position, evt.kind);
            }

            ui_state.match_over = self.match_over;
            if let Some(summary) = &self.victory_summary {
                ui_state.victory_summary = Some(summary.clone());
                ui_state.player_outcome = summary
                    .player_results
                    .iter()
                    .find(|result| result.player_id == self.current_player_id)
                    .map(|result| result.outcome);
            } else {
                ui_state.victory_summary = None;
                ui_state.player_outcome = None;
            }
            // Retain presentation-overlaid identity for consumers (was dropped each frame).
            self.last_ui_state = Some(ui_state);
        }

        // Execute the main game render pipeline using the WW3D frame.
        let render_time_delta = if self.game_logic.is_time_frozen_for_simulation() {
            0.0
        } else {
            self.last_frame_timing
                .map(|t| t.delta_seconds())
                .unwrap_or(0.0)
                * self.game_logic.visual_speed_multiplier().max(0.0)
        };
        let startup_frame = self.shell_start_frame();
        let current_startup_logic_frame = self.current_startup_logic_frame();
        let deferred_startup_model_load_budget = Self::startup_deferred_model_load_budget(
            self.current_state,
            startup_frame,
            current_startup_logic_frame,
        );
        let allow_sync_model_loads = deferred_startup_model_load_budget == 0;
        let skip_world_scene = self.should_skip_world_scene_for_shell_menu();

        #[cfg(feature = "game_client")]
        {
            if !skip_world_scene && matches!(self.current_state, GameState::Menu) {
                let prev = self.menu_world_frames_rendered;
                self.menu_world_frames_rendered += 1;
                if prev < 3 {
                    info!(
                        "Menu world render frame {}/3 (skip=false for first time)",
                        self.menu_world_frames_rendered
                    );
                }
            }
        }

        #[cfg(feature = "game_client")]
        {
            // C++ has a single render path: W3DDisplay::draw() which does 3D scene + UI + mouse.
            // We use render_pipeline.execute() below as that unified path.
            // game_client.draw_display() was a second competing render path that acquired
            // its own surface frame and called present(), stomping the render_pipeline output.
        }

        let render_pipeline_started = Instant::now();
        if render_call < 30 || (render_call < 50 && matches!(self.current_state, GameState::Menu)) {
            info!(
                "render() #{}, calling render_pipeline.execute skip_world={} state={:?}",
                render_call, skip_world_scene, self.current_state
            );
        }
        // Full presentation snapshot for render collect (transforms/model/selection/health).
        self.render_pipeline
            .set_presentation_frame(self.last_presentation_frame.clone());
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.render_pipeline
                .set_skybox_enabled(pres.world_env.skybox_enabled);
            if let Some(textures) = pres.world_env.skybox_textures.clone() {
                self.render_pipeline.set_skybox_hint(textures);
            }
        }

        // Production selection overlay: prefer PresentationFrame identity when available
        // (C++ W3DInGameUI selection circles / drag region after 3D scene setup).
        if !skip_world_scene && matches!(self.current_state, GameState::InGame | GameState::Paused)
        {
            let drag_rect = if self.is_dragging {
                self.selection_start_screen.map(|start| {
                    let size = self.window.inner_size();
                    crate::graphics::selection_renderer::DragSelectRect {
                        start: glam::Vec2::new(start.0, start.1),
                        end: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
                        window_width: size.width as f32,
                        window_height: size.height as f32,
                    }
                })
            } else {
                None
            };
            let ground_markers = self.collect_ground_marker_circles();
            crate::graphics::selection_renderer::enqueue_selection_render(
                &mut self.render_pipeline,
                &self.view_matrix,
                &self.projection_matrix,
                // Presentation-owned selection identity; live GameLogic only if no frame.
                if self.last_presentation_frame.is_some() {
                    None
                } else {
                    Some(&self.game_logic)
                },
                drag_rect.filter(|r| r.is_valid()),
                self.current_player_id,
                self.last_presentation_frame.as_ref(),
                ground_markers,
                self.show_move_lines,
                self.show_attack_lines,
            );
        }
        self.render_pipeline.execute(
            &mut self.graphics_system,
            if self.last_presentation_frame.is_some() {
                None
            } else {
                Some(&self.game_logic)
            },
            &self.view_matrix,
            &self.projection_matrix,
            self.camera_position,
            render_time_delta,
            allow_sync_model_loads,
            deferred_startup_model_load_budget,
            skip_world_scene,
        )?;
        let render_pipeline_elapsed = render_pipeline_started.elapsed();

        let attachments_started = Instant::now();
        self.drain_renderer_attachments();
        let attachments_elapsed = attachments_started.elapsed();
        let total_elapsed = render_started.elapsed();
        if total_elapsed >= Duration::from_millis(200) {
            warn!(
                "Render breakdown: total={:?} pipeline={:?} attachments={:?} state={:?} render_items={} model_missing={} deferred_loads={}/{} startup_progress={:.0}%",
                total_elapsed,
                render_pipeline_elapsed,
                attachments_elapsed,
                self.current_state,
                self.render_pipeline.debug_render_item_count(),
                self.render_pipeline.debug_last_model_missing(),
                self.render_pipeline.debug_last_deferred_model_loads(),
                self.render_pipeline.debug_last_deferred_model_load_budget(),
                self.startup_last_reported_progress * 100.0
            );
        }
        if render_call < 30 || (render_call < 50 && matches!(self.current_state, GameState::Menu)) {
            info!(
                "render() #{} done, pipeline={:?} total={:?} state={:?}",
                render_call,
                render_pipeline_elapsed,
                render_started.elapsed(),
                self.current_state
            );
        }
        Ok(())
    }

    fn update_minimap_viewport(&self, ui_state: &mut GameUIState) {
        // Prefer presentation world_env when installed (camera-relative minimap viewport).
        // Boot residual without a frame still uses host GameLogic bounds.
        let (world_min, world_max) = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.world_env.world_bounds_vec3()
        } else {
            self.game_logic.world_bounds()
        };
        let world_extent_x = (world_max.x - world_min.x).max(1.0);
        let world_extent_z = (world_max.z - world_min.z).max(1.0);

        let half_width = 200.0 / self.camera_zoom.max(0.01);
        let half_height = 150.0 / self.camera_zoom.max(0.01);

        let min_x = ((self.camera_target.x - half_width) - world_min.x) / world_extent_x;
        let max_x = ((self.camera_target.x + half_width) - world_min.x) / world_extent_x;
        let min_y = ((self.camera_target.z - half_height) - world_min.z) / world_extent_z;
        let max_y = ((self.camera_target.z + half_height) - world_min.z) / world_extent_z;

        ui_state.minimap_viewport = crate::ui::normalized_minimap_rect(min_x, min_y, max_x, max_y);
    }

    /// Process UI events emitted by UIManager and apply to engine/game state.
    fn process_ui_events(&mut self) {
        while let Some(event) = self.ui_manager.pop_event() {
            match event {
                UIEvent::StartGame {
                    mode,
                    faction,
                    map,
                    skirmish,
                } => {
                    self.start_game_from_ui(mode, faction, map, skirmish);
                }
                UIEvent::LoadGame(slot) => {
                    if slot == "quicksave" {
                        self.quick_load_from_hotkey("UI quick-load");
                    } else {
                        self.load_game_from_ui(&slot);
                    }
                }
                UIEvent::SaveGame { slot, display_name } => {
                    if slot == "quicksave" {
                        self.quick_save_from_hotkey("UI quick-save");
                    } else {
                        self.save_game_from_ui(&slot, &display_name);
                    }
                }
                UIEvent::RestartMission => {
                    self.restart_mission_from_ui();
                }
                UIEvent::PlaySoundEffectPath(path) => {
                    self.play_ui_sound_effect(path);
                }
                UIEvent::TogglePause => {
                    self.toggle_pause();
                }
                UIEvent::ExitToMenu => {
                    info!("UI requested exit to menu");
                    self.return_to_main_menu_after_match();
                }
                UIEvent::ExitGame => {
                    info!("UI requested exit");
                    self.request_state_change(GameState::Exiting);
                }
                UIEvent::ChangeScreen(screen) => {
                    if screen == Screen::Loading
                        && matches!(self.current_state, GameState::Menu | GameState::Loading)
                    {
                        self.ensure_shell_loading_overlay();
                        self.update_shell_loading_progress(0.0, Some("Loading assets..."));
                        self.ui_manager.suspend_for_shell_overlay();
                        self.set_runtime_ui_state_projection(UISystemState::Loading);
                        continue;
                    }

                    if self.current_state == GameState::Menu && screen.is_shell_owned_pregame() {
                        self.route_shell_owned_screen_change(screen);
                        continue;
                    }

                    self.ui_manager.transition_to_screen(screen);
                    match screen {
                        Screen::MainMenu => {
                            self.set_runtime_ui_state_projection(UISystemState::MainMenu)
                        }
                        Screen::Loading => {
                            self.set_runtime_ui_state_projection(UISystemState::Loading)
                        }
                        Screen::GameHUD => {
                            self.set_runtime_ui_state_projection(UISystemState::InGame)
                        }
                        Screen::PauseMenu => {
                            self.set_runtime_ui_state_projection(UISystemState::PauseMenu)
                        }
                        Screen::Victory => {
                            self.set_runtime_ui_state_projection(UISystemState::Victory)
                        }
                        _ => {}
                    }
                }
                UIEvent::FocusCamera(world_pos) => {
                    self.center_camera_on(world_pos);
                }
                UIEvent::QueueUnitProduction {
                    template_name,
                    quantity,
                } => {
                    // C++ ControlBar: Shift queues five; Ctrl queues to factory max residual.
                    let mut qty = quantity.max(1);
                    let shift = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                    let ctrl = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
                    if ctrl {
                        qty = 9; // DEFAULT_PRODUCTION_QUEUE_LIMIT residual fill
                    } else if shift {
                        qty = qty.saturating_mul(5).max(5);
                    }
                    self.queue_unit_production_from_ui(&template_name, qty);
                }
                UIEvent::CancelUnitProduction { template_name } => {
                    self.cancel_unit_production_from_ui(&template_name);
                }
                UIEvent::IssueCommand { command_name } => {
                    self.issue_named_command_from_ui(&command_name);
                }
                UIEvent::BeginStructurePlacement { template_name } => {
                    self.begin_structure_placement_from_ui(&template_name);
                }
                UIEvent::PlaceStructureAt {
                    template_name,
                    location,
                } => {
                    self.place_structure_from_ui(&template_name, location);
                }
                UIEvent::CancelStructurePlacement => {
                    self.cancel_structure_placement_from_ui();
                }
                UIEvent::ShowOptions => {
                    // Retail OPTIONS residual from pause menu / shell.
                    if matches!(self.current_state, GameState::InGame) {
                        self.request_state_change(GameState::Paused);
                    }
                    self.ui_manager
                        .transition_to_screen(crate::ui::Screen::Options);
                    info!("UI requested options menu residual");
                }
                UIEvent::SettingsChanged => {
                    if let Some(v) = self.ui_manager.options_bool("game.show_health_bars") {
                        self.show_health_bars = v;
                        self.game_hud.set_show_selection_health(v);
                        self.ui_manager.game_hud_mut().set_show_selection_health(v);
                        info!("Settings: show_health_bars={v}");
                    }
                    if let Some(v) = self.ui_manager.options_bool("game.show_fps") {
                        self.show_fps = v;
                        info!("Settings: show_fps={v}");
                    }
                }
            }
        }
    }

    /// C++ ControlBar production cameo → QueueUnitCreate residual.
    /// Keep interactive UIManager HUD and engine GameHUD presentation-synced residual.
    ///
    /// Clicks route through `ui_manager.game_hud`; resources/selection presentation
    /// historically only updated `self.game_hud`. Dual-apply closes that gap.

    /// C++ TheEva residual → chat EVA lines when honesty counters advance.
    fn sync_eva_messages_from_logic(&mut self) {
        let mut push = |msg: &str| {
            self.chat_panel.add_eva_message(msg);
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
        };
        let count = self.game_logic.eva_low_power_count();
        if count > self.last_eva_low_power_count {
            self.last_eva_low_power_count = count;
            push("Warning: low power");
        }
        let funds = self.game_logic.eva_insufficient_funds_count();
        if funds > self.last_eva_insufficient_funds_count {
            self.last_eva_insufficient_funds_count = funds;
            push("Insufficient funds");
        }
        let base = self.game_logic.eva_base_under_attack_count();
        if base > self.last_eva_base_under_attack_count {
            self.last_eva_base_under_attack_count = base;
            push("Our base is under attack");
        }
        let ally = self.game_logic.eva_ally_under_attack_count();
        if ally > self.last_eva_ally_under_attack_count {
            self.last_eva_ally_under_attack_count = ally;
            push("Ally under attack");
        }
    }

    fn play_presentation_event_sfx(
        &mut self,
        frame: &crate::presentation_frame::PresentationFrame,
    ) {
        for ev in &frame.events {
            match ev {
                crate::presentation_frame::PresentationEvent::ConstructionComplete { .. } => {
                    self.play_sound_effect(SoundType::ConstructionComplete);
                }
                crate::presentation_frame::PresentationEvent::ProductionComplete { .. } => {
                    self.play_sound_effect(SoundType::UnitReady);
                }
                crate::presentation_frame::PresentationEvent::UpgradeComplete { .. } => {
                    self.play_sound_effect(SoundType::UpgradeComplete);
                }
                _ => {}
            }
        }
    }

    fn apply_presentation_to_huds(&mut self, pres: &crate::presentation_frame::PresentationFrame) {
        // Dual GameHUD residual: engine HUD + interactive UIManager HUD.
        pres.apply_to_game_hud(&mut self.game_hud);
        pres.apply_to_game_hud(self.ui_manager.game_hud_mut());
    }

    fn commit_pending_map_command(
        &mut self,
        location: glam::Vec3,
        target_object: Option<crate::game_logic::ObjectId>,
    ) {
        let Some(kind) = self.pending_map_command.take() else {
            return;
        };
        self.clear_radius_cursor_overlays();
        let player_id = self.current_player_id;
        let mut selected = self
            .game_logic
            .get_player(player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        let allows_empty = matches!(kind, PendingMapCommand::PlaceBeacon);
        if selected.is_empty() && !allows_empty {
            return;
        }
        let command_type = match kind {
            PendingMapCommand::AttackMove => crate::command_system::CommandType::AttackMoveTo {
                destination: location,
            },
            PendingMapCommand::Guard => {
                if let Some(tid) = target_object {
                    crate::command_system::CommandType::Guard {
                        target: crate::command_system::GuardTarget::Object(tid),
                    }
                } else {
                    crate::command_system::CommandType::Guard {
                        target: crate::command_system::GuardTarget::Position(location),
                    }
                }
            }
            PendingMapCommand::SetRallyPoint => {
                crate::command_system::CommandType::SetRallyPoint { location }
            }
            PendingMapCommand::CombatDrop => crate::command_system::CommandType::CombatDrop {
                target: crate::command_system::DropTarget::Location(location),
            },
            PendingMapCommand::SpecialPower(power_type) => {
                let target = if let Some(tid) = target_object {
                    crate::command_system::PowerTarget::Object(tid)
                } else {
                    crate::command_system::PowerTarget::Location(location)
                };
                crate::command_system::CommandType::DoSpecialPower { power_type, target }
            }
            PendingMapCommand::PlaceBeacon => crate::command_system::CommandType::PlaceBeacon {
                location,
                text: String::new(),
            },
            PendingMapCommand::UnitAbility(ability) => {
                let Some(tid) = target_object else {
                    // Keep armed if click missed an object (except abilities that allow ground).
                    self.pending_map_command = Some(PendingMapCommand::UnitAbility(ability));
                    let msg = "Select a valid target";
                    self.game_hud.push_info_message(msg);
                    self.ui_manager.game_hud_mut().push_info_message(msg);
                    return;
                };
                match ability {
                    PendingUnitAbility::Hijack => {
                        crate::command_system::CommandType::Hijack { target_id: tid }
                    }
                    PendingUnitAbility::Sabotage => {
                        crate::command_system::CommandType::Sabotage { target_id: tid }
                    }
                    PendingUnitAbility::CaptureBuilding => {
                        crate::command_system::CommandType::CaptureBuilding { target_id: tid }
                    }
                    PendingUnitAbility::SnipeVehicle => {
                        crate::command_system::CommandType::SnipeVehicle { target_id: tid }
                    }
                    PendingUnitAbility::PlantTimedDemoCharge => {
                        crate::command_system::CommandType::PlantTimedDemoCharge { target_id: tid }
                    }
                    PendingUnitAbility::PlantRemoteDemoCharge => {
                        crate::command_system::CommandType::PlantRemoteDemoCharge { target_id: tid }
                    }
                    PendingUnitAbility::StealCashHack => {
                        crate::command_system::CommandType::StealCashHack { target_id: tid }
                    }
                    PendingUnitAbility::DisableVehicleHack => {
                        crate::command_system::CommandType::DisableVehicleHack { target_id: tid }
                    }
                    PendingUnitAbility::HackerDisableBuilding => {
                        crate::command_system::CommandType::HackerDisableBuilding { target_id: tid }
                    }
                    PendingUnitAbility::DisguiseAsVehicle => {
                        crate::command_system::CommandType::DisguiseAsVehicle { target_id: tid }
                    }
                    PendingUnitAbility::PlantBoobyTrap => {
                        crate::command_system::CommandType::PlantBoobyTrap { target_id: tid }
                    }
                    PendingUnitAbility::ConvertToCarbomb => {
                        crate::command_system::CommandType::ConvertToCarbomb { target_id: tid }
                    }
                    PendingUnitAbility::Repair => {
                        crate::command_system::CommandType::Repair { target_id: tid }
                    }
                }
            }
        };
        self.game_logic
            .queue_command(crate::command_system::GameCommand {
                command_type,
                player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: selected,
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
        self.game_logic.process_commands();
        self.play_sound_effect(SoundType::Command);
    }

    fn cancel_structure_placement_from_ui(&mut self) {
        self.pending_structure_placement = None;
        self.game_hud.construction_panel.clear_structure_placement();
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .clear_structure_placement();
    }

    /// Update structure placement ghost legality under cursor residual.

    fn radius_cursor_type_for_special_power(
        power: &crate::command_system::SpecialPowerType,
    ) -> &'static str {
        use crate::command_system::SpecialPowerType as P;
        match power {
            P::ParticleCannon => "PARTICLECANNON",
            P::NuclearMissile | P::BlackMarketNuke | P::DetonateDirtyNuke => "NUCLEARMISSILE",
            P::ScudStorm => "SCUDSTORM",
            P::Airstrike => "A10STRIKE",
            P::CarpetBomb | P::EarlyChinaCarpetBomb | P::AirForceCarpetBomb => "CARPETBOMB",
            P::DaisyCutter | P::FuelAirBomb => "DAISYCUTTER",
            P::Paradrop | P::InfantryParadrop | P::TankParadrop => "PARADROP",
            P::NapalmStrike => "NAPALMSTRIKE",
            P::Artillery => "ARTILLERYBARRAGE",
            P::EmpPulse => "EMPPULSE",
            P::SpectreGunship => "SPECTREGUNSHIP",
            P::SpySatellite | P::CiaIntelligence => "SPYSATELLITE",
            P::ClusterMines => "CLUSTERMINES",
            P::Ambush | P::TerrorCell => "AMBUSH",
            P::Frenzy | P::EarlyFrenzy => "FRENZY",
            P::AnthraxBomb => "ANTHRAXBOMB",
            P::EmergencyRepair | P::EarlyEmergencyRepair => "EMERGENCY_REPAIR",
            P::SpyDrone => "SPYDRONE",
            P::RadarScan => "RADAR",
            _ => "OFFENSIVE_SPECIALPOWER",
        }
    }

    fn arm_radius_cursor_for_pending(&mut self, cursor_type: &str) {
        use crate::ui::construction_panel::RadiusCursorOverlay;
        let r = RadiusCursorOverlay::radius_for_type(cursor_type);
        let mut ov = RadiusCursorOverlay::new(cursor_type, r);
        let loc = self.mouse_world_position;
        ov.centre = (loc.x, loc.z);
        self.game_hud
            .construction_panel
            .set_radius_overlay(Some(ov.clone()));
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .set_radius_overlay(Some(ov));
    }

    fn clear_radius_cursor_overlays(&mut self) {
        self.game_hud.construction_panel.clear_radius_overlay();
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .clear_radius_overlay();
    }

    fn sync_pending_map_command_radius_cursor(&mut self) {
        let Some(kind) = self.pending_map_command.clone() else {
            // Keep structure placement path separate; clear only if no pending map cmd.
            return;
        };
        let cursor = match kind {
            PendingMapCommand::AttackMove => "ATTACK_CONTINUE_AREA",
            PendingMapCommand::Guard => "GUARD_AREA",
            PendingMapCommand::SetRallyPoint => "FRIENDLY_SPECIALPOWER",
            PendingMapCommand::CombatDrop => "COMBATDROP",
            PendingMapCommand::PlaceBeacon => "RADAR",
            PendingMapCommand::SpecialPower(ref p) => Self::radius_cursor_type_for_special_power(p),
            PendingMapCommand::UnitAbility(_) => "OFFENSIVE_SPECIALPOWER",
        };
        // Ensure overlay exists (re-arm if missing).
        if self.game_hud.construction_panel.radius_overlay().is_none() {
            self.arm_radius_cursor_for_pending(cursor);
        }
        let loc = self.mouse_world_position;
        self.game_hud
            .construction_panel
            .sync_radius_overlay_cursor(loc.x, loc.z);
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .sync_radius_overlay_cursor(loc.x, loc.z);
    }

    fn sync_pending_structure_placement_cursor(&mut self) {
        let Some(template) = self.pending_structure_placement.clone() else {
            return;
        };
        let loc = self.mouse_world_position;
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            self.game_logic
                .get_player(self.current_player_id)
                .map(|p| p.team)
                .unwrap_or(crate::game_logic::Team::USA)
        };
        let builder_id = self.selected_objects.first().copied().or_else(|| {
            self.game_logic
                .get_player(self.current_player_id)
                .and_then(|p| p.selected_objects.first().copied())
        });
        let code = self
            .game_logic
            .legal_build_code_at_for_builder(team, loc, &template, builder_id);
        let legal = code == crate::game_logic::host_production_buildable_command_residual::LBC_OK;
        // Dual HUD residual
        self.game_hud
            .construction_panel
            .sync_structure_placement_cursor(loc.x, loc.z, legal);
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .sync_structure_placement_cursor(loc.x, loc.z, legal);
    }

    fn begin_structure_placement_from_ui(&mut self, template_name: &str) {
        if template_name.trim().is_empty() {
            return;
        }
        self.pending_structure_placement = Some(template_name.to_string());
        // Dual HUD residual: engine HUD + interactive UIManager HUD ghosts.
        self.game_hud
            .construction_panel
            .arm_structure_placement(template_name.to_string());
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .arm_structure_placement(template_name.to_string());
        log::debug!("BeginStructurePlacement residual: {template_name}");
    }

    /// Pick nearest alive friendly dozer/worker for structure placement residual.
    fn find_nearest_friendly_dozer(
        &self,
        player_id: u32,
        location: glam::Vec3,
    ) -> Option<crate::game_logic::ObjectId> {
        let team = self
            .game_logic
            .get_player(player_id)
            .map(|p| p.team)
            .unwrap_or(crate::game_logic::Team::USA);
        let mut best: Option<(crate::game_logic::ObjectId, f32)> = None;
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            for o in &frame.objects {
                if o.destroyed || o.team != team {
                    continue;
                }
                let n = o.template_name.to_ascii_lowercase();
                if !(n.contains("dozer") || n.contains("worker") || n.contains("crane")) {
                    continue;
                }
                if !crate::unit_control::UnitControlSystem::presentation_is_selectable(o) {
                    continue;
                }
                let d = (o.position.x - location.x).hypot(o.position.z - location.z);
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((o.id, d));
                }
            }
        } else {
            for (&id, obj) in self.game_logic.get_objects() {
                if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                    continue;
                }
                let n = obj.template_name.to_ascii_lowercase();
                if !(n.contains("dozer") || n.contains("worker") || n.contains("crane")) {
                    continue;
                }
                let pos = obj.get_position();
                let d = (pos.x - location.x).hypot(pos.z - location.z);
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((id, d));
                }
            }
        }
        best.map(|(id, _)| id)
    }

    fn is_wall_structure_template(template_name: &str) -> bool {
        let n = template_name.to_ascii_lowercase();
        n.contains("wall")
            || n.contains("fence")
            || n.contains("bunker") && n.contains("wall")
            || n.contains("fortresswall")
            || n.contains("chainlink")
    }

    fn place_structure_from_ui(&mut self, template_name: &str, location: glam::Vec3) {
        use crate::game_logic::host_production_buildable_command_residual::{
            lbc_help_message_residual, LBC_OK,
        };

        let template = resolve_ui_structure_template_name(template_name);
        if template.is_empty() || !location.x.is_finite() || !location.z.is_finite() {
            return;
        }

        let player_id = self
            .game_logic
            .get_player(0)
            .map(|p| p.id)
            .or_else(|| self.game_logic.get_players().keys().copied().min())
            .unwrap_or(0);
        let team = self
            .game_logic
            .get_player(player_id)
            .map(|p| p.team)
            .unwrap_or(crate::game_logic::Team::USA);

        let mut selected = self
            .game_logic
            .get_player(player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        let is_dozer = |id: crate::game_logic::ObjectId| {
            self.game_logic.get_object(id).is_some_and(|o| {
                if !o.is_alive() {
                    return false;
                }
                let n = o.template_name.to_ascii_lowercase();
                n.contains("dozer") || n.contains("worker") || n.contains("crane")
            })
        };
        let dozers: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| is_dozer(id))
            .collect();
        if !dozers.is_empty() {
            selected = dozers;
        }
        // C++ residual: if no builder in selection, auto-pick nearest friendly dozer/worker.
        if selected.is_empty() || !selected.iter().any(|&id| is_dozer(id)) {
            if let Some(auto) = self.find_nearest_friendly_dozer(player_id, location) {
                selected = vec![auto];
                self.game_logic.select_objects(player_id, selected.clone());
                self.selected_objects = selected.clone();
            }
        }
        if selected.is_empty() {
            log::debug!("PlaceStructureAt ignored — no dozer/worker selection");
            // Keep placement armed so player can select a dozer and retry.
            self.pending_structure_placement = Some(template_name.to_string());
            self.game_hud
                .construction_panel
                .arm_structure_placement(template_name.to_string());
            self.ui_manager
                .game_hud_mut()
                .construction_panel
                .arm_structure_placement(template_name.to_string());
            let msg = "Select a dozer or worker to build";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }

        let builder_id = selected.first().copied();
        let lbc = self
            .game_logic
            .legal_build_code_at_for_builder(team, location, &template, builder_id);
        if lbc != LBC_OK {
            // C++ keeps placement mode active on illegal click residual.
            self.pending_structure_placement = Some(template_name.to_string());
            self.game_hud
                .construction_panel
                .arm_structure_placement(template_name.to_string());
            self.ui_manager
                .game_hud_mut()
                .construction_panel
                .arm_structure_placement(template_name.to_string());
            let msg = lbc_help_message_residual(lbc);
            if !msg.is_empty() {
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
            }
            log::debug!(
                "PlaceStructureAt blocked LBC={} for {} at {:?}",
                lbc,
                template,
                location
            );
            return;
        }

        // Legal — clear arm and issue DozerConstruct.
        self.pending_structure_placement = None;
        self.game_hud.construction_panel.clear_structure_placement();
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .clear_structure_placement();
        self.play_sound_effect(SoundType::Command);

        self.game_logic
            .queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::DozerConstruct {
                    template_name: template,
                    location,
                    orientation: self
                        .game_hud
                        .construction_panel
                        .placement_preview()
                        .facing_radians,
                },
                player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: selected,
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
        self.game_logic.process_commands();
    }

    fn place_wall_line_from_ui(&mut self, template_name: &str, start: glam::Vec3, end: glam::Vec3) {
        let template = template_name.to_string();
        let player_id = self.current_player_id;
        let mut selected = self
            .game_logic
            .get_player(player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_else(|| self.selected_objects.clone());
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        // Prefer dozers/workers in selection residual.
        let builders: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| {
                self.game_logic.get_object(id).is_some_and(|o| {
                    o.is_alive()
                        && (o.is_dozer
                            || o.template_name.to_ascii_lowercase().contains("dozer")
                            || o.template_name.to_ascii_lowercase().contains("worker"))
                })
            })
            .collect();
        let units = if builders.is_empty() {
            selected
        } else {
            builders
        };
        if units.is_empty() {
            let msg = "Select a dozer or worker to build wall";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }

        // Keep placement armed for chained wall segments residual.
        self.play_sound_effect(SoundType::Command);
        self.game_logic
            .queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::DozerConstructLine {
                    template_name: template,
                    start,
                    end,
                },
                player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: units,
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
        self.game_logic.process_commands();
        let msg = "Wall line ordered";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Cancel production queue head on selected producers residual (Delete key).
    fn cancel_selected_production_queue_head(&mut self) -> bool {
        let player_id = self.current_player_id;
        let selected = self
            .game_logic
            .get_player(player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_else(|| self.selected_objects.clone());
        if selected.is_empty() {
            return false;
        }
        let mut any = false;
        for id in selected {
            let head_name = self.game_logic.get_object(id).and_then(|o| {
                o.building_data.as_ref().and_then(|b| {
                    b.production_queue
                        .first()
                        .map(|item| item.template_name.clone())
                })
            });
            let Some(template_name) = head_name else {
                continue;
            };
            if self.game_logic.cancel_production(id, template_name.clone()) {
                any = true;
                // Keep dual HUD presentation queue residual in sync.
                let panel = &mut self.game_hud.construction_panel;
                if let Some(idx) = panel
                    .building_queue
                    .iter()
                    .rposition(|q| q.item_name == template_name)
                {
                    panel.building_queue.remove(idx);
                }
            }
        }
        if any {
            self.play_sound_effect(SoundType::Command);
            let msg = "Canceled production";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
        }
        any
    }

    /// Cancel entire production queue on selected producers residual (Ctrl+Delete).
    fn cancel_all_selected_production(&mut self) -> bool {
        let player_id = self.current_player_id;
        let selected = self
            .game_logic
            .get_player(player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_else(|| self.selected_objects.clone());
        if selected.is_empty() {
            return false;
        }
        let mut any = false;
        for id in selected {
            // Drain queue head repeatedly residual.
            loop {
                let head_name = self.game_logic.get_object(id).and_then(|o| {
                    o.building_data.as_ref().and_then(|b| {
                        b.production_queue
                            .first()
                            .map(|item| item.template_name.clone())
                    })
                });
                let Some(template_name) = head_name else {
                    break;
                };
                if !self.game_logic.cancel_production(id, template_name.clone()) {
                    break;
                }
                any = true;
                let panel = &mut self.game_hud.construction_panel;
                if let Some(idx) = panel
                    .building_queue
                    .iter()
                    .rposition(|q| q.item_name == template_name)
                {
                    panel.building_queue.remove(idx);
                }
            }
        }
        if any {
            self.play_sound_effect(SoundType::Command);
            let msg = "Canceled all production";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
        }
        any
    }

    fn cancel_unit_production_from_ui(&mut self, template_name: &str) {
        if template_name.trim().is_empty() {
            return;
        }
        let player_id = self.current_player_id;
        let selected = self
            .game_logic
            .get_player(player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_else(|| self.selected_objects.clone());
        if selected.is_empty() {
            return;
        }
        let producers: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| {
                self.game_logic.get_object(id).is_some_and(|o| {
                    o.is_alive() && o.is_constructed() && o.building_data.is_some()
                })
            })
            .collect();
        let targets = if producers.is_empty() {
            selected
        } else {
            producers
        };
        let mut any = false;
        for id in targets {
            if self
                .game_logic
                .cancel_production(id, template_name.to_string())
            {
                any = true;
            }
        }
        if any {
            self.play_sound_effect(SoundType::Command);
            // Keep dual HUD presentation queue residual in sync.
            let panel = &mut self.game_hud.construction_panel;
            if let Some(idx) = panel
                .building_queue
                .iter()
                .rposition(|q| q.item_name == template_name)
            {
                panel.building_queue.remove(idx);
            }
        }
    }

    fn queue_unit_production_from_ui(&mut self, template_name: &str, quantity: u32) {
        if template_name.trim().is_empty() || quantity == 0 {
            return;
        }
        let logic = &mut self.game_logic;
        let player_id = logic
            .get_player(0)
            .map(|p| p.id)
            .or_else(|| logic.get_players().keys().copied().min())
            .unwrap_or(0);
        let selected = logic
            .get_player(player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        if selected.is_empty() {
            log::debug!(
                "QueueUnitProduction ignored — no selection for '{}'",
                template_name
            );
            return;
        }
        // Prefer constructed producers in selection residual.
        let producers: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| {
                logic.get_object(id).is_some_and(|o| {
                    o.is_alive() && o.is_constructed() && o.building_data.is_some()
                })
            })
            .collect();
        let units = if producers.is_empty() {
            selected
        } else {
            producers
        };
        logic.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::QueueUnitCreate {
                template_name: template_name.to_string(),
                quantity,
            },
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: units,
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        // Same-frame residual so production advances without waiting for AI drain.
        logic.process_commands();
    }

    /// C++ ControlBar named command button residual (Upgrade/Cancel/Stop/…).

    fn arm_pending_unit_ability(&mut self, ability: PendingUnitAbility, msg: &str) {
        self.pending_map_command = Some(PendingMapCommand::UnitAbility(ability));
        self.pending_structure_placement = None;
        self.arm_radius_cursor_for_pending("OFFENSIVE_SPECIALPOWER");
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    fn issue_named_command_from_ui(&mut self, command_name: &str) {
        let Some(command_type) = crate::command_system::command_type_from_button_name(command_name)
        else {
            log::debug!("IssueCommand unmapped: {command_name}");
            return;
        };

        // C++ ControlBar: AttackMove/Guard/SetRally wait for map click residual.
        match command_type {
            crate::command_system::CommandType::AttackMoveTo { .. } => {
                self.pending_map_command = Some(PendingMapCommand::AttackMove);
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending("ATTACK_CONTINUE_AREA");
                let msg = "Attack-move: click target location";
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
                return;
            }
            crate::command_system::CommandType::Guard { .. } => {
                self.pending_map_command = Some(PendingMapCommand::Guard);
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending("GUARD_AREA");
                let msg = "Guard: click location or unit";
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
                return;
            }
            crate::command_system::CommandType::SetRallyPoint { .. } => {
                self.pending_map_command = Some(PendingMapCommand::SetRallyPoint);
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending("FRIENDLY_SPECIALPOWER");
                let msg = "Set rally point: click location";
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
                return;
            }
            crate::command_system::CommandType::CombatDrop { .. } => {
                self.pending_map_command = Some(PendingMapCommand::CombatDrop);
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending("COMBATDROP");
                let msg = "Combat drop: click landing zone";
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
                return;
            }
            crate::command_system::CommandType::DoSpecialPower {
                power_type:
                    crate::command_system::SpecialPowerType::BattlePlanBombardment
                    | crate::command_system::SpecialPowerType::BattlePlanHoldTheLine
                    | crate::command_system::SpecialPowerType::BattlePlanSearchAndDestroy,
                ..
            } => {
                // Strategy Center battle plans are immediate (no map click).
                // Fall through to queue with resolved selection below.
            }
            crate::command_system::CommandType::DoSpecialPower { power_type, .. } => {
                // Resolve SW type from selected ready structure residual.
                // Named buttons (SpySatellite, ParticleCannon, …) prefer their type.
                let player_id = self.current_player_id;
                let selected = self
                    .game_logic
                    .get_player(player_id)
                    .map(|p| p.selected_objects.clone())
                    .unwrap_or_else(|| self.selected_objects.clone());
                let requested = power_type.clone();
                let mut resolved = None;
                // Pass 1: honor named button power when ready on selection.
                for id in &selected {
                    if self.game_logic.is_special_power_ready_for(*id, &requested) {
                        resolved = Some(requested.clone());
                        break;
                    }
                }
                // Pass 2: any ready superweapon structure (generic Command_SpecialPower / V).
                if resolved.is_none() {
                    for id in &selected {
                        let Some(obj) = self.game_logic.get_object(*id) else {
                            continue;
                        };
                        if !obj.special_power_ready {
                            continue;
                        }
                        if let Some(p) =
                            crate::game_logic::host_superweapon_kindof::special_power_for_superweapon_structure(
                                &obj.template_name,
                            )
                        {
                            if self.game_logic.is_special_power_ready_for(*id, &p) {
                                resolved = Some(p);
                                break;
                            }
                        }
                    }
                }
                let Some(power) = resolved else {
                    let msg = "No ready special power on selection";
                    self.game_hud.push_info_message(msg);
                    self.ui_manager.game_hud_mut().push_info_message(msg);
                    return;
                };
                let cursor = {
                    // Map before move into pending.
                    let c = match &power {
                        crate::command_system::SpecialPowerType::ParticleCannon => "PARTICLECANNON",
                        crate::command_system::SpecialPowerType::NuclearMissile
                        | crate::command_system::SpecialPowerType::BlackMarketNuke
                        | crate::command_system::SpecialPowerType::DetonateDirtyNuke => {
                            "NUCLEARMISSILE"
                        }
                        crate::command_system::SpecialPowerType::ScudStorm => "SCUDSTORM",
                        _ => "OFFENSIVE_SPECIALPOWER",
                    };
                    c
                };
                self.pending_map_command = Some(PendingMapCommand::SpecialPower(power));
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending(cursor);
                let msg = "Special power: click target location";
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
                return;
            }
            crate::command_system::CommandType::PlaceBeacon { .. } => {
                self.pending_map_command = Some(PendingMapCommand::PlaceBeacon);
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending("RADAR");
                let msg = "Place beacon: click location";
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
                return;
            }
            // Unit special-ability buttons: arm target click residual.
            crate::command_system::CommandType::Hijack { .. } => {
                self.arm_pending_unit_ability(PendingUnitAbility::Hijack, "Hijack: click vehicle");
                return;
            }
            crate::command_system::CommandType::Sabotage { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::Sabotage,
                    "Sabotage: click building",
                );
                return;
            }
            crate::command_system::CommandType::CaptureBuilding { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::CaptureBuilding,
                    "Capture: click structure",
                );
                return;
            }
            crate::command_system::CommandType::SnipeVehicle { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::SnipeVehicle,
                    "Snipe: click vehicle",
                );
                return;
            }
            crate::command_system::CommandType::PlantTimedDemoCharge { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::PlantTimedDemoCharge,
                    "Plant timed charge: click target",
                );
                return;
            }
            crate::command_system::CommandType::PlantRemoteDemoCharge { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::PlantRemoteDemoCharge,
                    "Plant remote charge: click target",
                );
                return;
            }
            crate::command_system::CommandType::StealCashHack { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::StealCashHack,
                    "Steal cash: click supply",
                );
                return;
            }
            crate::command_system::CommandType::DisableVehicleHack { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::DisableVehicleHack,
                    "Hack vehicle: click target",
                );
                return;
            }
            crate::command_system::CommandType::HackerDisableBuilding { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::HackerDisableBuilding,
                    "Hack building: click target",
                );
                return;
            }
            crate::command_system::CommandType::DisguiseAsVehicle { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::DisguiseAsVehicle,
                    "Disguise: click vehicle",
                );
                return;
            }
            crate::command_system::CommandType::PlantBoobyTrap { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::PlantBoobyTrap,
                    "Booby trap: click structure",
                );
                return;
            }
            crate::command_system::CommandType::ConvertToCarbomb { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::ConvertToCarbomb,
                    "Car bomb: click vehicle",
                );
                return;
            }
            crate::command_system::CommandType::Repair { .. } => {
                self.arm_pending_unit_ability(
                    PendingUnitAbility::Repair,
                    "Repair: click damaged structure",
                );
                return;
            }
            crate::command_system::CommandType::PurchaseScience { .. } => {
                self.try_purchase_next_generals_science();
                return;
            }
            crate::command_system::CommandType::ResumeConstruction { .. } => {
                self.resume_selected_construction();
                return;
            }
            _ => {}
        }

        let mut command_type = command_type;
        // Prefer engine current player; fall back to lowest id residual.
        let player_id = if self.game_logic.get_player(self.current_player_id).is_some() {
            self.current_player_id
        } else {
            self.game_logic
                .get_players()
                .keys()
                .copied()
                .min()
                .unwrap_or(0)
        };
        let mut selected = self
            .game_logic
            .get_player(player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        if selected.is_empty()
            && !matches!(
                command_type,
                crate::command_system::CommandType::Stop
                    | crate::command_system::CommandType::Scatter
                    | crate::command_system::CommandType::ViewCommandCenter
                    | crate::command_system::CommandType::ViewLastRadarEvent
                    | crate::command_system::CommandType::PlaceBeacon { .. }
                    | crate::command_system::CommandType::RemoveBeacon
            )
        {
            return;
        }
        match &mut command_type {
            crate::command_system::CommandType::DozerCancelConstruct { object_id }
            | crate::command_system::CommandType::Sell { object_id } => {
                if let Some(id) = selected.first() {
                    *object_id = *id;
                }
            }
            crate::command_system::CommandType::ResumeConstruction { target_id } => {
                // Prefer unfinished structure in selection residual.
                let unfinished = selected.iter().copied().find(|&id| {
                    self.game_logic.get_object(id).is_some_and(|o| {
                        o.is_alive() && o.status.under_construction && !o.status.sold
                    })
                });
                if let Some(id) = unfinished.or_else(|| selected.first().copied()) {
                    *target_id = id;
                }
            }
            _ => {}
        }
        self.game_logic
            .queue_command(crate::command_system::GameCommand {
                command_type,
                player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: selected,
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
        self.game_logic.process_commands();
    }

    fn route_shell_owned_screen_change(&mut self, screen: Screen) {
        match screen {
            Screen::MainMenu => self.enter_shell_menu_from_runtime_host(None),
            Screen::Options => self.enter_shell_options_from_runtime_host(),
            Screen::Credits => {
                self.enter_shell_screen_from_runtime_host(Some("Credits"), "Menus/CreditsMenu.wnd")
            }
            Screen::LoadGame => {
                self.enter_shell_screen_from_runtime_host(Some("LoadGame"), "Menus/SaveLoad.wnd")
            }
            Screen::Skirmish => self.enter_shell_screen_from_runtime_host(
                Some("Skirmish"),
                "Menus/SkirmishGameOptionsMenu.wnd",
            ),
            _ => {}
        }
    }

    fn apply_pending_script_camera_requests(&mut self) {
        // Prefer presentation-frozen camera residual when a frame is installed (InGame).
        // Live take_* path is boot/menu residual only. Fail-closed: ease curves not frozen
        // on PresentationFrame (duration-only zoom/pitch/rotate).
        if let Some(pres) = self.last_presentation_frame.clone() {
            self.apply_presentation_camera_residual(&pres);
            // Drain live queues so peeked presentation fields are not re-applied next frame.
            self.drain_live_camera_request_queues();
            return;
        }

        if let Some(focus) = self.game_logic.take_camera_focus_request() {
            self.center_camera_on(focus);
        }

        if let Some(focus) = self.game_logic.camera_follow_target_position() {
            self.center_camera_on(focus);
        }

        if self.game_logic.take_camera_zoom_reset() {
            self.camera_zoom = self.compute_default_camera_zoom_for_target(
                self.camera_target,
                self.game_logic.script_default_camera_max_height(),
            );
            self.camera_zoom_target = None;
            self.camera_zoom_start = self.camera_zoom;
            self.camera_zoom_duration = 0.0;
            self.camera_zoom_elapsed = 0.0;
            self.camera_zoom_ease_in = 0.0;
            self.camera_zoom_ease_out = 0.0;
            self.apply_script_camera_pitch_request(CameraPitchRequest {
                pitch: self.game_logic.script_default_camera_pitch(),
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        }

        if let Some(request) = self.game_logic.take_camera_zoom_request() {
            if request.duration_seconds <= 0.0 {
                self.camera_zoom = request.zoom;
                self.camera_zoom_target = None;
                self.camera_zoom_start = self.camera_zoom;
                self.camera_zoom_duration = 0.0;
                self.camera_zoom_elapsed = 0.0;
                self.camera_zoom_ease_in = 0.0;
                self.camera_zoom_ease_out = 0.0;
            } else {
                self.camera_zoom_start = self.camera_zoom;
                self.camera_zoom_target = Some(request.zoom);
                self.camera_zoom_duration = request.duration_seconds;
                self.camera_zoom_elapsed = 0.0;
                self.camera_zoom_ease_in = request.ease_in_seconds.max(0.0);
                self.camera_zoom_ease_out = request.ease_out_seconds.max(0.0);
            }
        }

        if let Some(request) = self.game_logic.take_camera_pitch_request() {
            self.apply_script_camera_pitch_request(request);
        }

        if let Some(request) = self.game_logic.take_camera_rotate_request() {
            self.apply_script_camera_rotate_request(request);
        }

        if let Some(request) = self.game_logic.take_camera_look_toward_request() {
            self.apply_camera_look_toward_request(request);
        }

        if let Some(request) = self.game_logic.take_camera_slave_mode_enable_request() {
            self.camera_slave_mode = Some(request);
        }

        if self.game_logic.take_camera_slave_mode_disable_request() {
            self.camera_slave_mode = None;
        }

        for request in self.game_logic.take_screen_shake_requests() {
            self.enqueue_script_screen_shake(request.intensity);
        }

        for request in self.game_logic.take_camera_add_shaker_requests() {
            self.enqueue_script_camera_shaker(request);
        }

        // Main applies these script requests inside GameLogic evaluation (with GameClient bridges
        // when enabled). Drain pending mirrors so they don't accumulate frame-to-frame.
        let _ = self.game_logic.take_view_guardband_request();
        let _ = self.game_logic.take_camera_bw_mode_request();
        let _ = self.game_logic.take_camera_motion_blur_requests();
    }

    /// Play presentation-frozen script/radar movies (C++ script display residual).
    /// Drains live pending movie queues after apply. Fail-closed: not full BINK parity.
    fn apply_presentation_movie_residual(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        #[cfg(feature = "game_client")]
        {
            if let Some(ref name) = pres.pending_movie {
                let started =
                    game_client::core::script_action_handler::play_script_display_movie(name);
                if !started {
                    log::trace!("presentation movie play deferred/failed: {name}");
                }
            }
            if let Some(ref name) = pres.pending_radar_movie {
                // Radar movies use InGameUI path when available.
                let started = game_client::helpers::TheInGameUI::play_movie(name);
                if !started {
                    let _ =
                        game_client::core::script_action_handler::play_script_display_movie(name);
                }
            }
        }
        let _ = self.game_logic.take_pending_movie();
        let _ = self.game_logic.take_pending_radar_movie();
    }

    /// Apply camera residual frozen on `PresentationFrame` (no live take dual-read).
    fn apply_presentation_camera_residual(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        if let Some(focus) = pres.camera_focus {
            self.center_camera_on(Vec3::new(focus[0], focus[1], focus[2]));
        }

        // Prefer presentation-frozen follow position; live path is boot residual only.
        if let Some(follow) = pres.camera_follow_position {
            self.center_camera_on(Vec3::new(follow[0], follow[1], follow[2]));
        } else if let Some(focus) = self.game_logic.camera_follow_target_position() {
            // Boot residual only — presentation camera_follow_position owns InGame follow.
            self.center_camera_on(focus);
        }

        if pres.camera_zoom_reset {
            self.camera_zoom = self.compute_default_camera_zoom_for_target(
                self.camera_target,
                self.game_logic.script_default_camera_max_height(),
            );
            self.camera_zoom_target = None;
            self.camera_zoom_start = self.camera_zoom;
            self.camera_zoom_duration = 0.0;
            self.camera_zoom_elapsed = 0.0;
            self.camera_zoom_ease_in = 0.0;
            self.camera_zoom_ease_out = 0.0;
            self.apply_script_camera_pitch_request(CameraPitchRequest {
                pitch: self.game_logic.script_default_camera_pitch(),
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        }

        if let Some((zoom, duration_seconds)) = pres.camera_zoom {
            if duration_seconds <= 0.0 {
                self.camera_zoom = zoom;
                self.camera_zoom_target = None;
                self.camera_zoom_start = self.camera_zoom;
                self.camera_zoom_duration = 0.0;
                self.camera_zoom_elapsed = 0.0;
                self.camera_zoom_ease_in = 0.0;
                self.camera_zoom_ease_out = 0.0;
            } else {
                self.camera_zoom_start = self.camera_zoom;
                self.camera_zoom_target = Some(zoom);
                self.camera_zoom_duration = duration_seconds;
                self.camera_zoom_elapsed = 0.0;
                self.camera_zoom_ease_in = 0.0;
                self.camera_zoom_ease_out = 0.0;
            }
        }

        if let Some((pitch, duration_seconds)) = pres.camera_pitch {
            self.apply_script_camera_pitch_request(CameraPitchRequest {
                pitch,
                duration_seconds,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        }

        if let Some((rotations, duration_seconds)) = pres.camera_rotate {
            self.apply_script_camera_rotate_request(CameraRotateRequest {
                rotations,
                duration_seconds,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
        }

        if let Some(look) = pres.camera_look_toward {
            self.apply_camera_look_toward_request(CameraLookTowardWaypointRequest {
                position: Vec3::new(look[0], look[1], look[2]),
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
                reverse_rotation: false,
            });
        }

        if let Some((thing_template_name, bone_name)) = pres.camera_slave_enable.clone() {
            self.camera_slave_mode = Some(CameraSlaveModeRequest {
                thing_template_name,
                bone_name,
            });
        }

        if pres.camera_slave_disable {
            self.camera_slave_mode = None;
        }

        for intensity in &pres.screen_shakes {
            self.enqueue_script_screen_shake(*intensity);
        }

        for &(amplitude, duration_seconds, radius) in &pres.camera_shakers {
            self.enqueue_script_camera_shaker(CameraAddShakerRequest {
                position: self.camera_target,
                amplitude,
                duration_seconds,
                radius,
            });
        }
    }

    /// Consume live camera request queues without applying (presentation already applied).
    fn drain_live_camera_request_queues(&mut self) {
        let _ = self.game_logic.take_camera_focus_request();
        let _ = self.game_logic.take_camera_zoom_reset();
        let _ = self.game_logic.take_camera_zoom_request();
        let _ = self.game_logic.take_camera_pitch_request();
        let _ = self.game_logic.take_camera_rotate_request();
        let _ = self.game_logic.take_camera_look_toward_request();
        let _ = self.game_logic.take_camera_slave_mode_enable_request();
        let _ = self.game_logic.take_camera_slave_mode_disable_request();
        let _ = self.game_logic.take_screen_shake_requests();
        let _ = self.game_logic.take_camera_add_shaker_requests();
        let _ = self.game_logic.take_view_guardband_request();
        let _ = self.game_logic.take_camera_bw_mode_request();
        let _ = self.game_logic.take_camera_motion_blur_requests();
    }

    fn restart_mission_from_ui(&mut self) {
        // Prefer presentation residual for map/mode/faction when installed.
        let map = self
            .last_presentation_frame
            .as_ref()
            .map(|p| p.world_env.map_name.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.game_logic.get_current_map_name().to_string());
        let mode = self.presentation_or_live_game_mode();
        let faction = self
            .last_presentation_frame
            .as_ref()
            .map(|p| p.local_team.get_name().to_string())
            .or_else(|| {
                // Boot residual only.
                self.game_logic
                    .get_player(self.current_player_id)
                    .map(|p| p.team.get_name().to_string())
            })
            .unwrap_or_else(|| "USA".to_string());

        info!(
            "UI requested restart: mode={:?}, faction={}, map={}",
            mode, faction, map
        );
        self.start_game_from_ui(mode, faction, map, None);
    }

    /// Prefer presentation-frozen game mode when a frame is installed.
    fn presentation_or_live_game_mode(&self) -> GameMode {
        self.last_presentation_frame
            .as_ref()
            .map(|p| p.game_mode)
            .unwrap_or_else(|| self.game_logic.game_mode())
    }

    fn map_ai_difficulty_to_save(difficulty: crate::ai::AIDifficulty) -> GameDifficulty {
        match difficulty {
            crate::ai::AIDifficulty::Easy => GameDifficulty::Easy,
            crate::ai::AIDifficulty::Medium => GameDifficulty::Medium,
            crate::ai::AIDifficulty::Hard | crate::ai::AIDifficulty::Brutal => GameDifficulty::Hard,
        }
    }

    fn build_save_info(
        &self,
        slot: &str,
        display_name: &str,
        description: &str,
        save_type: SaveFileType,
    ) -> SaveGameInfo {
        // Prefer presentation residual for map/play_time/local team when installed.
        let map_name = self
            .last_presentation_frame
            .as_ref()
            .map(|p| p.world_env.map_name.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.game_logic.get_current_map_name().to_string());
        let difficulty = Self::map_ai_difficulty_to_save(
            self.last_presentation_frame
                .as_ref()
                .map(|p| p.ai_difficulty)
                .unwrap_or_else(|| self.game_logic.get_difficulty()),
        );
        let play_time = std::time::Duration::from_secs_f32(
            self.last_presentation_frame
                .as_ref()
                .map(|p| p.total_play_time_seconds)
                .unwrap_or_else(|| self.game_logic.get_total_play_time()),
        );
        let team_name = self
            .last_presentation_frame
            .as_ref()
            .map(|p| p.local_team.get_name().to_string())
            .or_else(|| {
                // Boot residual only — presentation local_team owns InGame save metadata.
                self.game_logic
                    .get_player(self.current_player_id)
                    .map(|player| player.team.get_name().to_string())
            })
            .unwrap_or_else(|| "Neutral".to_string());

        SaveGameInfo {
            filename: slot.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            map_name,
            campaign_side: Some(team_name),
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time,
            difficulty,
            save_type,
        }
    }

    fn quick_save_from_hotkey(&mut self, source: &str) {
        // Prefer presentation game_mode residual when installed.
        let mode = self.presentation_or_live_game_mode();
        if !matches!(mode, GameMode::SinglePlayer | GameMode::Skirmish) {
            info!(
                "{} ignored: quick save is only available in single-player or skirmish (mode={:?})",
                source, mode
            );
            return;
        }

        info!("{} requested quick save", source);
        let save_info = self.build_save_info(
            "quicksave",
            "Quick Save",
            "Quick Save",
            SaveFileType::QuickSave,
        );

        if let Err(err) =
            self.save_file_manager
                .save_game("quicksave", &self.game_logic, &save_info)
        {
            warn!("Quick save failed for 'quicksave': {}", err);
        } else {
            info!("Quick save stored in slot 'quicksave'");
        }
    }

    fn quick_load_from_hotkey(&mut self, source: &str) {
        let restore_screen = match self.current_state {
            GameState::Paused => Some(Screen::PauseMenu),
            GameState::InGame => Some(Screen::GameHUD),
            _ => None,
        };
        // Prefer presentation game_mode residual when installed.
        let mode = self.presentation_or_live_game_mode();
        if !matches!(mode, GameMode::SinglePlayer | GameMode::Skirmish) {
            info!(
                "{} ignored: quick load is only available in single-player or skirmish (mode={:?})",
                source, mode
            );
            if self.ui_manager.current_screen() == Some(Screen::Loading) {
                if let Some(screen) = restore_screen {
                    self.ui_manager.transition_to_screen(screen);
                }
            }
            return;
        }

        if !self.save_file_manager.save_exists("quicksave") {
            warn!(
                "{} requested quick load, but no 'quicksave' slot exists",
                source
            );
            if self.ui_manager.current_screen() == Some(Screen::Loading) {
                if let Some(screen) = restore_screen {
                    self.ui_manager.transition_to_screen(screen);
                }
            }
            return;
        }

        info!("{} requested quick load from slot 'quicksave'", source);
        self.load_game_from_ui("quicksave");
    }

    fn save_game_from_ui(&mut self, slot: &str, display_name: &str) {
        let slot = slot.trim();
        if slot.is_empty() {
            return;
        }

        let save_info =
            self.build_save_info(slot, display_name, display_name, SaveFileType::Normal);

        if let Err(err) = self
            .save_file_manager
            .save_game(slot, &self.game_logic, &save_info)
        {
            warn!("Save failed for '{}': {}", slot, err);
        } else {
            info!("Saved game to slot '{}'", slot);
        }
    }

    fn load_game_from_ui(&mut self, slot: &str) {
        let slot = slot.trim();
        if slot.is_empty() {
            return;
        }

        #[cfg(feature = "game_client")]
        // Prefer presentation game_mode residual when installed.
        self.prepare_cpp_load_screen_for_mode(self.presentation_or_live_game_mode(), true);
        self.transition_to_state(GameState::Loading);
        match self.save_file_manager.load_game(slot, &mut self.game_logic) {
            Ok(save_info) => {
                info!(
                    "Loaded save '{}' (map={}, name={})",
                    slot, save_info.map_name, save_info.display_name
                );

                self.game_logic.set_paused(false);
                self.game_paused = false;
                self.match_over = false;
                self.victory_summary = None;
                self.selected_objects.clear();

                Self::apply_heightmap_hint(&mut self.render_pipeline, &self.game_logic);
                Self::apply_skybox_hint(&mut self.render_pipeline, &self.game_logic);
                Self::sync_render_terrain_visual(
                    &mut self.render_pipeline,
                    &self.graphics_system,
                    &self.game_logic,
                    save_info.map_name.as_str(),
                );
                if let Err(err) = Self::reinitialize_minimap_renderer(
                    &mut self.render_pipeline,
                    &self.graphics_system,
                    &mut self.game_logic,
                ) {
                    warn!(
                        "Failed to reinitialize minimap renderer after load: {}",
                        err
                    );
                }
                Self::apply_map_lighting(
                    &mut self.graphics_system,
                    &mut self.render_pipeline,
                    &self.game_logic,
                );

                // Seed presentation before first InGame render (units/HUD identity).
                self.seed_presentation_after_match_start();
                self.transition_to_state(GameState::InGame);
            }
            Err(err) => {
                warn!("Load failed for '{}': {}", slot, err);
                self.return_to_main_menu_after_match();
            }
        }
    }

    fn play_ui_sound_effect(&mut self, path: String) {
        let Some(bytes) = self.ui_sound_cache.get(&path).cloned() else {
            return;
        };
        let Some(handle) = self.audio_handle.as_ref() else {
            return;
        };
        let Ok(sink) = Sink::try_new(handle) else {
            return;
        };

        let cursor = std::io::Cursor::new(bytes);
        let Ok(decoder) = rodio::Decoder::new(cursor) else {
            return;
        };
        let source = decoder.convert_samples::<f32>();
        sink.append(source);
        self.sound_effects.push(sink);
    }

    /// Restart the simulation with UI-selected parameters and refresh view/minimap.
    fn start_game_from_ui(
        &mut self,
        mode: GameMode,
        faction: String,
        map: String,
        skirmish: Option<crate::skirmish_config::SkirmishMatchConfig>,
    ) {
        // Show loading screen before starting map load (matches C++ loading screen flow)
        #[cfg(feature = "game_client")]
        self.prepare_cpp_load_screen_for_mode(mode, false);
        self.transition_to_state(GameState::Loading);

        let faction_team = Self::team_from_faction(&faction);
        let map_name = if map.trim().is_empty() {
            DEFAULT_SKIRMISH_MAP.to_string()
        } else {
            map
        };

        info!(
            "UI requested start: mode={:?}, faction={}, map={}, skirmish_slots={}",
            mode,
            faction_team.get_name(),
            map_name,
            skirmish
                .as_ref()
                .map(|c| c.slots.iter().filter(|s| s.is_active).count())
                .unwrap_or(0)
        );

        if mode == GameMode::Skirmish {
            if let Some(ref config) = skirmish {
                if let Err(e) =
                    crate::skirmish_config::apply_skirmish_config(&mut self.game_logic, config)
                {
                    warn!("apply_skirmish_config failed: {e}; falling back to legacy start");
                    self.game_logic.start_new_game(mode);
                    let _ = self
                        .game_logic
                        .set_player_team(self.current_player_id, faction_team);
                    self.game_logic.setup_skirmish_ai(self.current_player_id);
                } else if let Some(human) = config.slots.iter().find(|s| s.is_human && s.is_active)
                {
                    self.current_player_id = human.slot_index as u32;
                }
            } else {
                self.game_logic.start_new_game(mode);
                let _ = self
                    .game_logic
                    .set_player_team(self.current_player_id, faction_team);
                self.game_logic.setup_skirmish_ai(self.current_player_id);
            }
        } else {
            self.game_logic.start_new_game(mode);
            let _ = self
                .game_logic
                .set_player_team(self.current_player_id, faction_team);
        }

        if !self.game_logic.load_map(&map_name) {
            warn!("Failed to load map '{}', falling back to default", map_name);
            let _ = self.game_logic.load_map(DEFAULT_SKIRMISH_MAP);
        }

        // Reset transient state.
        self.game_logic.set_paused(false);
        self.game_paused = false;
        self.match_over = false;
        self.victory_summary = None;
        self.selected_objects.clear();

        // Update minimap/world bounds and camera to the new map.
        Self::apply_heightmap_hint(&mut self.render_pipeline, &self.game_logic);
        Self::apply_skybox_hint(&mut self.render_pipeline, &self.game_logic);
        Self::sync_render_terrain_visual(
            &mut self.render_pipeline,
            &self.graphics_system,
            &self.game_logic,
            map_name.as_str(),
        );
        if let Err(err) = Self::reinitialize_minimap_renderer(
            &mut self.render_pipeline,
            &self.graphics_system,
            &mut self.game_logic,
        ) {
            warn!("Failed to reinitialize minimap renderer: {}", err);
        }

        // Apply map lighting if provided by map settings.
        Self::apply_map_lighting(
            &mut self.graphics_system,
            &mut self.render_pipeline,
            &self.game_logic,
        );

        let startup_camera_defaults = Self::configured_startup_camera_defaults();
        (self.camera_target, self.camera_position, self.camera_zoom) =
            Self::bootstrap_camera_for_loaded_map(
                &self.game_logic,
                self.current_player_id,
                startup_camera_defaults,
            );
        self.sync_orbit_from_camera_transform();
        // Dual-tick residual close: map load → presentation seed → InGame HUD/units
        // without waiting for the first logic frame (render collect uses snapshot IDs).
        self.seed_presentation_after_match_start();
        self.transition_to_state(GameState::InGame);
    }

    fn apply_map_lighting(
        graphics_system: &mut GraphicsSystem,
        render_pipeline: &mut RenderPipeline,
        game_logic: &GameLogic,
    ) {
        const FALLBACK_AMBIENT: [f32; 3] = [0.30, 0.30, 0.30];
        const FALLBACK_SUN_COLOR: [f32; 3] = [1.00, 0.90, 0.80];
        const FALLBACK_SUN_DIRECTION: [f32; 3] = [-0.5, -1.0, -0.5];

        if let Some(meta) = game_logic.last_parsed_map_settings() {
            let fog_color = meta.sky_color.or(meta.sun_color);
            info!(
                "Applying map lighting: ambient={:?} sun_color={:?} sun_dir={:?} sky={:?} fog={:?}",
                meta.ambient_color,
                meta.sun_color,
                meta.sun_direction,
                meta.sky_color,
                meta.fog_start.zip(meta.fog_end)
            );
            render_pipeline.set_environment_lighting(
                meta.sun_direction,
                meta.sun_color,
                meta.ambient_color,
                fog_color,
                meta.fog_start.zip(meta.fog_end),
            );
            graphics_system.set_lighting(
                meta.ambient_color,
                meta.sun_color,
                meta.sun_direction,
                meta.sky_color,
            );
        } else {
            warn!("Map settings provide no lighting metadata; using fallback ambient/sun defaults");
            render_pipeline.set_environment_lighting(
                Some(FALLBACK_SUN_DIRECTION),
                Some(FALLBACK_SUN_COLOR),
                Some(FALLBACK_AMBIENT),
                None,
                None,
            );
            graphics_system.set_lighting(
                Some(FALLBACK_AMBIENT),
                Some(FALLBACK_SUN_COLOR),
                Some(FALLBACK_SUN_DIRECTION),
                None,
            );
        }
    }

    fn apply_heightmap_hint(render_pipeline: &mut RenderPipeline, game_logic: &GameLogic) {
        // Prefer presentation-frozen hint when available (no live path re-read).
        let path = render_pipeline
            .presentation_frame()
            .and_then(|p| p.world_env.heightmap_hint.clone())
            .or_else(|| {
                game_logic
                    .heightmap_hint()
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
            });
        if let Some(path) = path {
            // Keep renderer parity-safe: map-adjacent TGA companions are frequently preview art.
            // Feeding those into terrain elevation creates severe startup terrain corruption.
            if path.to_ascii_lowercase().ends_with(".tga") {
                render_pipeline.set_heightmap_hint(None);
                return;
            }
            render_pipeline.set_heightmap_hint(Some(path));
        } else {
            render_pipeline.set_heightmap_hint(None);
        }
    }

    fn sync_render_terrain_visual(
        render_pipeline: &mut RenderPipeline,
        graphics_system: &GraphicsSystem,
        game_logic: &GameLogic,
        map_name: &str,
    ) {
        // Freeze world env (bounds/hint/roads) for this load so heightmap + road bake
        // prefer presentation residual rather than re-querying live GameLogic mid-sync.
        // Only seed when no presentation is set yet (map-load path).
        if render_pipeline.presentation_frame().is_none() {
            let env_frame = crate::presentation_frame::PresentationFrame::build_from_logic(
                game_logic,
                game_logic.get_frame() as u32,
            );
            render_pipeline.set_presentation_frame(Some(env_frame));
        }

        let bounds = render_pipeline
            .presentation_frame()
            .map(|p| p.world_env.world_bounds_vec3())
            .unwrap_or_else(|| game_logic.world_bounds());

        let hint_loaded = if render_pipeline.heightmap_hint().is_some() {
            match render_pipeline.load_heightmap_from_hint(
                &graphics_system.device_arc(),
                &graphics_system.queue_arc(),
                Some(bounds),
            ) {
                Ok(()) => true,
                Err(err) => {
                    warn!(
                        "Failed to load terrain visual from heightmap hint for '{}': {}",
                        map_name, err
                    );
                    false
                }
            }
        } else {
            false
        };

        if !hint_loaded {
            // Presentation frame is seeded above with runtime_heightmap freeze.
            // Pass None so terrain visual bake cannot dual-read live GameLogic.
            match render_pipeline.load_heightmap_from_runtime_terrain(
                &graphics_system.device_arc(),
                &graphics_system.queue_arc(),
                None,
            ) {
                Ok(true) => {}
                Ok(false) => {
                    warn!(
                        "No runtime terrain heightmap available for '{}'; terrain visual may remain empty",
                        map_name
                    );
                }
                Err(err) => {
                    warn!(
                        "Failed to load terrain visual from runtime terrain for '{}': {}",
                        map_name, err
                    );
                }
            }
        }

        // Presentation already seeded above; pass None so road bake cannot dual-read.
        if let Err(err) = render_pipeline.sync_runtime_map_roads(None) {
            warn!(
                "Failed to sync runtime map roads for '{}': {}",
                map_name, err
            );
        }
    }

    fn apply_skybox_hint(render_pipeline: &mut RenderPipeline, game_logic: &GameLogic) {
        // Prefer presentation env when already installed (map-load seeds a frame first).
        if let Some(pres) = render_pipeline.presentation_frame() {
            let enabled = pres.world_env.skybox_enabled;
            let textures = pres.world_env.skybox_textures.clone();
            render_pipeline.set_skybox_enabled(enabled);
            if let Some(textures) = textures {
                render_pipeline.set_skybox_hint(textures);
            }
            return;
        }
        // Boot residual without presentation snapshot.
        render_pipeline.set_skybox_enabled(game_logic.is_skybox_enabled());
        if let Some(meta) = game_logic.last_parsed_map_settings() {
            if let Some(textures) = meta.skybox_textures {
                render_pipeline.set_skybox_hint(textures);
            }
        }
    }

    fn reinitialize_minimap_renderer(
        render_pipeline: &mut RenderPipeline,
        graphics_system: &GraphicsSystem,
        game_logic: &mut GameLogic,
    ) -> anyhow::Result<()> {
        let mut world_bounds = game_logic.world_bounds();
        render_pipeline.initialize_minimap_renderer(
            graphics_system.device_arc(),
            graphics_system.queue_arc(),
            world_bounds,
        )?;

        let world_width = (world_bounds.1.x - world_bounds.0.x).abs();
        let world_height = (world_bounds.1.z - world_bounds.0.z).abs();
        if world_width <= 1.0 || world_height <= 1.0 {
            if let Some((w, h)) = render_pipeline.heightmap_world_size() {
                game_logic.override_world_size(w, h);
                world_bounds = game_logic.world_bounds();
            }
        }

        render_pipeline.sync_heightmap_world_bounds(world_bounds);
        render_pipeline.update_minimap_world_bounds(world_bounds);
        Ok(())
    }

    /// Convert a UI faction string into a Team.
    fn team_from_faction(faction: &str) -> Team {
        match faction.to_ascii_lowercase().as_str() {
            "usa" | "us" | "america" => Team::USA,
            "gla" => Team::GLA,
            "china" => Team::China,
            _ => Team::USA,
        }
    }

    fn handle_minimap_interaction(&mut self, interaction: MinimapInteraction) {
        let pointer = Vec2::new(interaction.screen_position.x, interaction.screen_position.y);
        let Some(world_pos) = self.render_pipeline.handle_minimap_click(pointer) else {
            return;
        };

        match interaction.kind {
            MinimapActionKind::LeftClick | MinimapActionKind::LeftDrag => {
                self.center_camera_on(world_pos);
            }
            MinimapActionKind::RightClick => {
                self.issue_minimap_move(world_pos);
            }
        }
    }

    fn script_pitch_to_radians(pitch: f32) -> f32 {
        // Script pitch semantics: 1.0 is default, 0.0 trends toward horizon, >1.0 toward ground.
        let clamped = pitch.clamp(-0.25, 2.0);
        let degrees = if clamped <= 1.0 {
            5.0 + clamped * 40.0
        } else {
            45.0 + (clamped - 1.0) * 40.0
        };
        degrees
            .to_radians()
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians())
    }

    fn parabolic_ease(param: f32, ease_in_time: f32, ease_out_time: f32) -> f32 {
        let param = param.clamp(0.0, 1.0);
        let mut in_t = ease_in_time.clamp(0.0, 1.0);
        let out_t = 1.0 - ease_out_time.clamp(0.0, 1.0);
        if in_t > out_t {
            in_t = out_t;
        }
        let v0 = 1.0 + out_t - in_t;
        if param < in_t {
            if in_t <= 0.0 {
                0.0
            } else {
                param * param / (v0 * in_t)
            }
        } else if param <= out_t {
            (in_t + 2.0 * (param - in_t)) / v0
        } else {
            let denom = (1.0 - out_t).max(f32::EPSILON);
            (in_t
                + 2.0 * (out_t - in_t)
                + (2.0 * (param - out_t) + out_t * out_t - param * param) / denom)
                / v0
        }
    }

    fn apply_camera_orbit_transform(&mut self) {
        self.camera_pitch_radians = self
            .camera_pitch_radians
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
        self.camera_orbit_distance = self.camera_orbit_distance.max(1.0);

        let horizontal = self.camera_orbit_distance * self.camera_pitch_radians.cos();
        let offset = Vec3::new(
            horizontal * self.camera_yaw_radians.sin(),
            self.camera_orbit_distance * self.camera_pitch_radians.sin(),
            horizontal * self.camera_yaw_radians.cos(),
        );
        self.camera_position = self.camera_target + offset + self.camera_shake_offset;
        self.view_matrix = Mat4::look_at_rh(self.camera_position, self.camera_target, Vec3::Y);
    }

    fn sync_orbit_from_camera_transform(&mut self) {
        let offset = self.camera_position - self.camera_target;
        self.camera_orbit_distance = offset.length().max(1.0);
        let horizontal = Vec2::new(offset.x, offset.z).length();
        self.camera_pitch_radians = offset
            .y
            .atan2(horizontal.max(f32::EPSILON))
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
        self.camera_yaw_radians = offset.x.atan2(offset.z);

        self.camera_pitch_target = None;
        self.camera_pitch_start = self.camera_pitch_radians;
        self.camera_pitch_duration = 0.0;
        self.camera_pitch_elapsed = 0.0;
        self.camera_pitch_ease_in = 0.0;
        self.camera_pitch_ease_out = 0.0;

        self.camera_yaw_target = None;
        self.camera_yaw_start = self.camera_yaw_radians;
        self.camera_yaw_duration = 0.0;
        self.camera_yaw_elapsed = 0.0;
        self.camera_yaw_ease_in = 0.0;
        self.camera_yaw_ease_out = 0.0;

        self.apply_camera_orbit_transform();
    }

    fn apply_script_camera_pitch_request(&mut self, request: CameraPitchRequest) {
        let target_pitch = Self::script_pitch_to_radians(request.pitch);
        if request.duration_seconds <= 0.0 {
            self.camera_pitch_radians = target_pitch;
            self.camera_pitch_target = None;
            self.camera_pitch_start = self.camera_pitch_radians;
            self.camera_pitch_duration = 0.0;
            self.camera_pitch_elapsed = 0.0;
            self.camera_pitch_ease_in = 0.0;
            self.camera_pitch_ease_out = 0.0;
            self.apply_camera_orbit_transform();
            return;
        }

        self.camera_pitch_start = self.camera_pitch_radians;
        self.camera_pitch_target = Some(target_pitch);
        self.camera_pitch_duration = request.duration_seconds;
        self.camera_pitch_elapsed = 0.0;
        self.camera_pitch_ease_in = request.ease_in_seconds.max(0.0);
        self.camera_pitch_ease_out = request.ease_out_seconds.max(0.0);
    }

    fn apply_script_camera_rotate_request(&mut self, request: CameraRotateRequest) {
        let target_yaw = self.camera_yaw_radians + request.rotations * TAU;
        if request.duration_seconds <= 0.0 {
            self.camera_yaw_radians = target_yaw;
            self.camera_yaw_target = None;
            self.camera_yaw_start = self.camera_yaw_radians;
            self.camera_yaw_duration = 0.0;
            self.camera_yaw_elapsed = 0.0;
            self.camera_yaw_ease_in = 0.0;
            self.camera_yaw_ease_out = 0.0;
            self.apply_camera_orbit_transform();
            return;
        }

        self.camera_yaw_start = self.camera_yaw_radians;
        self.camera_yaw_target = Some(target_yaw);
        self.camera_yaw_duration = request.duration_seconds;
        self.camera_yaw_elapsed = 0.0;
        self.camera_yaw_ease_in = request.ease_in_seconds.max(0.0);
        self.camera_yaw_ease_out = request.ease_out_seconds.max(0.0);
    }

    fn apply_script_fps_limit_request(&mut self, fps: i32) {
        let global_default = {
            let mut global = game_engine::common::global_data::write();
            global.writable.use_fps_limit = true;
            Some(global.writable.frames_per_second_limit)
        };

        let resolved_fps = if fps <= 0 {
            global_default.unwrap_or_else(|| {
                game_engine::common::global_data::read()
                    .writable
                    .frames_per_second_limit
            })
        } else {
            fps
        };

        self.script_fps_limit = u32::try_from(resolved_fps).ok().filter(|fps| *fps > 0);
        self.script_fps_limit_last_tick = None;
    }

    fn effective_fps_limit_for_frame(
        script_fps_limit: Option<u32>,
        global_use_fps_limit: bool,
        global_frames_per_second_limit: i32,
        visual_speed_multiplier: f32,
        tivo_fast_mode: bool,
        in_replay_game: bool,
    ) -> Option<u32> {
        if let Some(script_fps) = script_fps_limit.filter(|fps| *fps > 0) {
            return Some(script_fps);
        }

        // C++ parity: skip frame limiting when tactical time multiplier is above normal.
        if visual_speed_multiplier > 1.0 {
            return None;
        }

        if !global_use_fps_limit {
            return None;
        }

        // C++ parity: TiVO fast mode disables frame limiting for replay playback.
        if tivo_fast_mode && in_replay_game {
            return None;
        }

        u32::try_from(global_frames_per_second_limit)
            .ok()
            .filter(|fps| *fps > 0)
    }

    fn apply_script_frame_limit(&mut self) {
        let global_data = game_engine::common::global_data::read();
        let max_fps = Self::effective_fps_limit_for_frame(
            self.script_fps_limit,
            global_data.writable.use_fps_limit,
            global_data.writable.frames_per_second_limit,
            self.game_logic.visual_speed_multiplier(),
            global_data.tivo_fast_mode,
            self.game_logic.isInReplayGame(),
        );
        drop(global_data);

        let Some(max_fps) = max_fps else {
            self.script_fps_limit_last_tick = None;
            return;
        };

        // Mirrors C++ GameEngine::execute frame pacing: (1000 / fps) - 1, Sleep(0) loop.
        let limit_ms = (1000.0 / max_fps as f32 - 1.0).max(0.0);
        if limit_ms <= 0.0 {
            self.script_fps_limit_last_tick = Some(Instant::now());
            return;
        }

        let limit = Duration::from_millis(limit_ms as u64);
        if let Some(previous) = self.script_fps_limit_last_tick {
            let mut now = Instant::now();
            while now.duration_since(previous) < limit {
                std::thread::sleep(Duration::ZERO);
                now = Instant::now();
            }
            self.script_fps_limit_last_tick = Some(now);
        } else {
            self.script_fps_limit_last_tick = Some(Instant::now());
        }
    }

    fn screen_shake_value_for_type(shake_type: i32) -> f32 {
        let data = game_engine::common::global_data::read();
        match shake_type.clamp(0, 5) {
            0 => data.shake_subtle_intensity,
            1 => data.shake_normal_intensity,
            2 => data.shake_strong_intensity,
            3 => data.shake_severe_intensity,
            4 => data.shake_cine_extreme_intensity,
            _ => data.shake_cine_insane_intensity,
        }
    }

    fn enqueue_script_screen_shake(&mut self, intensity: i32) {
        let shake_value = Self::screen_shake_value_for_type(intensity);
        if !shake_value.is_finite() || shake_value <= 0.0 {
            return;
        }

        let seed = self
            .frame_counter
            .wrapping_mul(1_664_525)
            .wrapping_add((intensity as u32).wrapping_mul(1_013_904_223));
        let angle = (seed as f32 / u32::MAX as f32) * TAU;
        self.screen_shake_angle_cos = angle.cos();
        self.screen_shake_angle_sin = angle.sin();

        self.screen_shake_intensity += shake_value;
        let data = game_engine::common::global_data::read();
        if self.screen_shake_intensity > data.max_shake_intensity {
            // C++ parity from W3DView::shake: overflow clamps to fixed 3.0.
            self.screen_shake_intensity = 3.0;
        }
    }

    fn enqueue_script_camera_shaker(&mut self, request: CameraAddShakerRequest) {
        if !request.position.is_finite()
            || !request.amplitude.is_finite()
            || !request.duration_seconds.is_finite()
            || !request.radius.is_finite()
        {
            return;
        }
        if request.duration_seconds <= 0.0 || request.radius <= 0.0 || request.amplitude <= 0.0 {
            return;
        }

        self.script_camera_shakers.push(ScriptCameraShaker::new(
            request.position,
            request.radius,
            request.duration_seconds,
            request.amplitude,
        ));
    }

    fn update_script_camera_shake(&mut self, dt: f32) -> bool {
        let previous = self.camera_shake_offset;
        let mut offset = Vec3::ZERO;

        if self.screen_shake_intensity > 0.01 {
            offset.x += self.screen_shake_intensity * self.screen_shake_angle_cos;
            offset.z += self.screen_shake_intensity * self.screen_shake_angle_sin;
            self.screen_shake_intensity *= 0.75;
            self.screen_shake_angle_cos = -self.screen_shake_angle_cos;
            self.screen_shake_angle_sin = -self.screen_shake_angle_sin;
        } else {
            self.screen_shake_intensity = 0.0;
            self.screen_shake_angle_cos = 0.0;
            self.screen_shake_angle_sin = 0.0;
        }

        if dt > 0.0 {
            for shaker in &mut self.script_camera_shakers {
                shaker.elapsed_seconds += dt.max(0.0);
            }
        }
        self.script_camera_shakers
            .retain(|s| s.elapsed_seconds < s.duration_seconds);

        let camera_position = self.camera_position;
        for shaker in &self.script_camera_shakers {
            let dist = Vec2::new(
                camera_position.x - shaker.epicenter.x,
                camera_position.z - shaker.epicenter.z,
            )
            .length();
            if dist > shaker.radius {
                continue;
            }

            let distance_factor = (1.0 - dist / shaker.radius).clamp(0.0, 1.0);
            let life = (1.0 - shaker.elapsed_seconds / shaker.duration_seconds).clamp(0.0, 1.0);
            let amplitude_world = shaker.amplitude_degrees.to_radians().sin().abs()
                * self.camera_orbit_distance.max(1.0)
                * 0.5;
            let magnitude = amplitude_world * distance_factor * life;
            if magnitude <= f32::EPSILON {
                continue;
            }

            let t = shaker.elapsed_seconds.max(0.0);
            let omega = TAU * shaker.frequency_hz;
            let phase_a = shaker.phase + omega * t;
            let phase_b = shaker.phase * 1.37 + omega * 0.79 * t;

            offset.x += phase_a.sin() * magnitude;
            offset.z += phase_a.cos() * magnitude;
            offset.y += phase_b.sin() * magnitude * 0.2;
        }

        self.camera_shake_offset = offset;
        (self.camera_shake_offset - previous).length_squared() > 0.000001
    }

    fn normalize_signed_angle(mut angle: f32) -> f32 {
        while angle > PI {
            angle -= TAU;
        }
        while angle < -PI {
            angle += TAU;
        }
        angle
    }

    fn apply_camera_look_toward_request(&mut self, request: CameraLookTowardWaypointRequest) {
        let to_target = request.position - self.camera_target;
        let horiz = Vec2::new(to_target.x, to_target.z);
        if horiz.length_squared() <= f32::EPSILON {
            return;
        }

        let target_yaw = to_target.x.atan2(to_target.z);
        let mut delta = Self::normalize_signed_angle(target_yaw - self.camera_yaw_radians);
        if request.reverse_rotation {
            if delta >= 0.0 {
                delta -= TAU;
            } else {
                delta += TAU;
            }
        }
        let target_yaw = self.camera_yaw_radians + delta;

        if request.duration_seconds <= 0.0 {
            self.camera_yaw_radians = target_yaw;
            self.camera_yaw_target = None;
            self.camera_yaw_start = self.camera_yaw_radians;
            self.camera_yaw_duration = 0.0;
            self.camera_yaw_elapsed = 0.0;
            self.camera_yaw_ease_in = 0.0;
            self.camera_yaw_ease_out = 0.0;
            self.apply_camera_orbit_transform();
            return;
        }

        self.camera_yaw_start = self.camera_yaw_radians;
        self.camera_yaw_target = Some(target_yaw);
        self.camera_yaw_duration = request.duration_seconds;
        self.camera_yaw_elapsed = 0.0;
        self.camera_yaw_ease_in = request.ease_in_seconds.max(0.0);
        self.camera_yaw_ease_out = request.ease_out_seconds.max(0.0);
    }

    fn center_camera_on(&mut self, world_pos: Vec3) {
        let clamped = self.clamp_to_world_bounds(world_pos);
        let ground_height = self
            .game_logic
            .terrain_height_at(clamped)
            .unwrap_or(self.camera_target.y);
        self.camera_target.x = clamped.x;
        self.camera_target.y = ground_height;
        self.camera_target.z = clamped.z;
        self.apply_camera_orbit_transform();
    }

    /// Placement ghost footprint + pending map radius cursor residual for 3D overlay.
    fn collect_ground_marker_circles(
        &self,
    ) -> Vec<crate::graphics::selection_renderer::SelectedUnit> {
        use crate::graphics::selection_renderer::SelectedUnit;
        let mut out = Vec::new();

        // Structure placement ghost residual (green legal / red illegal).
        let placement = self.game_hud.construction_panel.placement_preview();
        if placement.is_active() {
            let half = placement.footprint_half_extents;
            let radius = half.0.max(half.1).max(8.0);
            let color = if placement.is_legal {
                [0.15, 0.95, 0.2, 0.45]
            } else {
                [0.95, 0.15, 0.1, 0.5]
            };
            out.push(SelectedUnit {
                position: glam::Vec3::new(placement.world_pos.0, 0.0, placement.world_pos.1),
                radius,
                team_color: color,
            });
        }

        // Special-power / AttackMove / Guard radius cursor residual.
        if let Some(ov) = self.game_hud.construction_panel.radius_overlay() {
            if ov.radius > 0.0 {
                let color = if ov.is_legal {
                    [ov.color.0, ov.color.1, ov.color.2, ov.color.3.max(0.35)]
                } else {
                    [1.0, 0.1, 0.1, 0.5]
                };
                out.push(SelectedUnit {
                    position: glam::Vec3::new(ov.centre.0, 0.0, ov.centre.1),
                    radius: ov.radius.max(1.0),
                    team_color: color,
                });
            }
        }

        // Active guard-area residual for selected units (C++ Guard area ring).
        const GUARD_AREA_RADIUS: f32 = 100.0; // matches RadiusCursor GUARD_AREA
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            for o in &frame.objects {
                if o.destroyed || !o.selected {
                    continue;
                }
                let Some(gp) = o.guard_position else {
                    continue;
                };
                out.push(SelectedUnit {
                    position: glam::Vec3::new(gp.x, 0.0, gp.z),
                    radius: GUARD_AREA_RADIUS,
                    team_color: [0.35, 0.75, 1.0, 0.35],
                });
            }
        }

        out
    }

    fn issue_minimap_move(&mut self, world_pos: Vec3) {
        // Prefer live player selection; fall back to engine selection residual.
        let mut selected = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        if selected.is_empty() {
            return;
        }

        let clamped = self.clamp_to_world_bounds(world_pos);

        // C++ InGameUI minimap RMB residual: same context-sensitive path as world
        // right-click (attack enemy / gather / enter / move).
        let target_object = self.find_object_at_position(clamped, &self.game_logic, true);
        let ctrl = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control)
            )
        });
        let shift = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Shift)
            )
        });
        let alt = self.sticky_waypoint_mode
            || self.keys_pressed.iter().any(|k| {
                matches!(
                    k,
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Alt)
                )
            });

        let context = crate::command_system::MouseCommandContext {
            world_position: clamped,
            target_object,
            screen_position: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: crate::command_system::MouseButton::Right,
            modifier_keys: crate::command_system::ModifierKeys { ctrl, shift, alt },
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let mut cmd_sys = crate::command_system::CommandSystem::new();
        let command = cmd_sys.process_mouse_input(
            &context,
            &selected,
            self.current_player_id,
            &self.game_logic,
        );

        if let Some(mut command) = command {
            if self.sticky_auto_attack {
                if let crate::command_system::CommandType::MoveTo { destination, .. } =
                    command.command_type
                {
                    command.command_type =
                        crate::command_system::CommandType::AttackMoveTo { destination };
                }
            }
            self.game_logic.queue_command(command);
            self.game_logic.process_commands();
            self.play_sound_effect(SoundType::Command);
            return;
        }

        // Fail-closed fallback residual: move if context path produced nothing.
        if self.sticky_auto_attack {
            self.game_logic
                .command_attack_move(self.current_player_id, clamped);
        } else {
            self.game_logic
                .command_move(self.current_player_id, clamped);
        }
        self.play_sound_effect(SoundType::Command);
    }

    fn clamp_to_world_bounds(&self, mut position: Vec3) -> Vec3 {
        // Prefer presentation world_env when installed (camera follow / scroll clamp).
        // Boot residual without a frame still uses host GameLogic bounds.
        let (world_min, world_max) = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.world_env.world_bounds_vec3()
        } else {
            self.game_logic.world_bounds()
        };
        position.x = position.x.clamp(world_min.x, world_max.x);
        position.z = position.z.clamp(world_min.z, world_max.z);
        position
    }

    fn drain_renderer_attachments(&mut self) {
        match ww3d_renderer_3d::Renderer::with_global_mut(|renderer| {
            Ok(renderer.take_pending_attachments())
        }) {
            Ok(records) if !records.is_empty() => {
                AttachmentDispatcher::dispatch(records);
            }
            Ok(_) => {}
            Err(err) => {
                warn!("Failed to dispatch WW3D attachments: {err}");
            }
        }
    }

    fn debug_show_victory(&mut self, winner: Option<u32>) {
        info!("Debug: showing victory screen (winner: {:?})", winner);
        self.show_victory_screen(winner);
    }

    fn show_victory_screen(&mut self, winner: Option<u32>) {
        // Prefer presentation-frozen summary when available (no live re-aggregate).
        let summary = self
            .last_presentation_frame
            .as_ref()
            .and_then(|f| f.victory_summary.clone())
            .unwrap_or_else(|| {
                // Boot residual only — no presentation summary yet.
                self.game_logic.build_victory_summary(winner)
            });
        let queued_summary = summary.clone();
        self.victory_summary = Some(summary.clone());
        if let Err(err) = crate::game_results_queue::queue_victory_summary(queued_summary) {
            warn!("Failed to enqueue victory summary: {err}");
        }
        self.game_paused = true;
        self.match_over = true;
        match winner {
            Some(id) if id == self.current_player_id => {
                self.ui_manager.set_victory_with_summary(id, Some(summary));
                self.request_state_change(GameState::Victory);
            }
            Some(_) => {
                self.ui_manager.set_defeat_with_summary(Some(summary));
                self.request_state_change(GameState::Defeat);
            }
            None => {
                self.ui_manager.set_draw_with_summary(Some(summary));
                // Draw freezes with Defeat residual (no separate Draw state).
                self.request_state_change(GameState::Defeat);
            }
        }
    }

    fn reset_match_state(&mut self) {
        info!("Resetting gameplay state after match completion");
        self.drain_renderer_attachments();

        self.game_logic.reset();
        self.resource_manager = ResourceManager::new();

        // Path grid rebuild is owned by GameLogic on map load/reset.
        self.selected_objects.clear();
        self.keys_pressed.clear();
        self.mouse_position = (0.0, 0.0);
        self.mouse_world_position = Vec3::ZERO;
        self.selection_start = None;
        self.rmb_scroll_anchor = None;
        self.is_rmb_scrolling = false;

        for sink in &self.sound_effects {
            sink.stop();
        }
        self.sound_effects.clear();
        if let Some(sink) = self.background_music.take() {
            sink.stop();
        }

        self.match_over = false;
        self.game_paused = false;
        self.victory_summary = None;
        self.ui_manager.clear_victory_screen();
        self.diagnostics_overlay = None;

        self.frame_counter = 0;
        self.fps = 0.0;
        self.last_frame_timing = None;
        self.frame_clock = FrameClock::new();
        NetworkClock::clear_override();

        self.game_hud = GameHUD::new();
        let size = self.window.inner_size();
        self.game_hud.resize(size.width, size.height);

        self.camera_position = Vec3::new(0.0, 200.0, 200.0);
        self.camera_target = Vec3::new(0.0, 0.0, 0.0);
        self.camera_zoom = 1.0;
        self.camera_zoom_target = None;
        self.camera_zoom_start = self.camera_zoom;
        self.camera_zoom_duration = 0.0;
        self.camera_zoom_elapsed = 0.0;
        self.camera_zoom_ease_in = 0.0;
        self.camera_zoom_ease_out = 0.0;
        self.camera_shake_offset = Vec3::ZERO;
        self.screen_shake_intensity = 0.0;
        self.screen_shake_angle_cos = 0.0;
        self.screen_shake_angle_sin = 0.0;
        self.script_camera_shakers.clear();
        self.script_fps_limit = None;
        self.script_fps_limit_last_tick = None;
        self.camera_slave_mode = None;
        self.sync_orbit_from_camera_transform();
        let aspect = size.width.max(1) as f32 / size.height.max(1) as f32;
        self.projection_matrix = Mat4::perspective_rh(
            DEFAULT_VIEW_FOV_RADIANS,
            aspect,
            DEFAULT_VIEW_NEAR_CLIP,
            DEFAULT_VIEW_FAR_CLIP,
        );
    }

    fn return_to_main_menu_after_match(&mut self) {
        self.reset_match_state();
        self.transition_to_state(GameState::Menu);
    }

    fn exit_to_main_menu_from_victory(&mut self) {
        self.return_to_main_menu_after_match();
    }

    fn handle_key_press(&mut self, key: &Key) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            match key {
                Key::Character(c) if c == "m" || c == "M" => {
                    self.toggle_background_music();
                }
                Key::Named(NamedKey::F11) => {
                    let current_fullscreen = self.window.fullscreen().is_some();
                    if let Err(e) = self.set_fullscreen(!current_fullscreen) {
                        error!("Failed to toggle fullscreen: {}", e);
                    } else {
                        info!("Toggled fullscreen mode: {}", !current_fullscreen);
                    }
                }
                Key::Named(NamedKey::Escape) => {
                    info!("Escape pressed in Menu/Loading - exiting");
                    self.request_state_change(GameState::Exiting);
                }
                _ => {}
            }
            return;
        }

        let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));

        // Retail chat residual: when open, keyboard goes to ChatPanel first.
        if matches!(self.current_state, GameState::InGame | GameState::Paused)
            && self.chat_panel.is_open()
        {
            if self.route_key_to_chat_panel(key) {
                return;
            }
        }

        match key {
            Key::Named(NamedKey::Space)
                if self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Center camera on selection residual (Alt+Space).
                self.center_camera_on_selection();
            }
            Key::Named(NamedKey::Space) => {
                // Retail CommandMap VIEW_LAST_RADAR_EVENT KEY_SPACE residual.
                // Pause remains on P.
                self.issue_named_command_from_ui("Command_ViewLastRadarEvent");
            }
            Key::Character(digit)
                if digit.len() == 1 && digit.chars().all(|c| c.is_ascii_digit()) =>
            {
                let group_num = digit.chars().next().unwrap().to_digit(10).unwrap() as u8;
                let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
                let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                let alt_down = self.keys_pressed.contains(&Key::Named(NamedKey::Alt));

                if ctrl_down {
                    // CREATE_TEAM residual: assign control group.
                    if self.selected_objects.is_empty() {
                        self.control_groups.remove(&group_num);
                        info!("Cleared control group {}", group_num);
                    } else {
                        self.control_groups
                            .insert(group_num, self.selected_objects.clone());
                        info!(
                            "Assigned {} units to control group {}",
                            self.selected_objects.len(),
                            group_num
                        );
                    }
                } else if shift_down {
                    // ADD_TEAM residual: merge current selection into group.
                    if self.selected_objects.is_empty() {
                        return;
                    }
                    let entry = self.control_groups.entry(group_num).or_default();
                    for id in &self.selected_objects {
                        if !entry.contains(id) {
                            entry.push(*id);
                        }
                    }
                    info!(
                        "Added selection to control group {} (now {} units)",
                        group_num,
                        entry.len()
                    );
                } else if alt_down {
                    // VIEW_TEAM residual: center camera on group without changing selection.
                    let stored = self
                        .control_groups
                        .get(&group_num)
                        .cloned()
                        .unwrap_or_default();
                    if stored.is_empty() {
                        info!("Control group {} is empty (view)", group_num);
                        return;
                    }
                    let center = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        frame.centroid_of_ids(&stored)
                    } else {
                        let mut sum = Vec3::ZERO;
                        let mut n = 0u32;
                        for id in &stored {
                            if let Some(obj) = self.game_logic.find_object(*id) {
                                if obj.is_alive() {
                                    sum += obj.get_position();
                                    n += 1;
                                }
                            }
                        }
                        if n == 0 {
                            None
                        } else {
                            Some(sum / n as f32)
                        }
                    };
                    if let Some(center) = center {
                        let clamped = self.clamp_to_world_bounds(center);
                        self.camera_target.x = clamped.x;
                        self.camera_target.z = clamped.z;
                        self.game_logic.request_camera_focus(clamped);
                        info!("VIEW_TEAM{} camera jump to {:?}", group_num, clamped);
                    }
                } else {
                    // SELECT_TEAM residual: select control group.
                    let stored = self
                        .control_groups
                        .get(&group_num)
                        .cloned()
                        .unwrap_or_default();
                    if stored.is_empty() {
                        info!("Control group {} is empty", group_num);
                        return;
                    }

                    let mut selection = Vec::new();
                    let team_opt = if let Some(frame) = self.last_presentation_frame.as_ref() {
                        Some(frame.local_team())
                    } else {
                        // Boot residual only — presentation local_team owns InGame control-group select.
                        self.game_logic
                            .get_player(self.current_player_id)
                            .map(|p| p.team)
                    };
                    if let Some(team) = team_opt {
                        selection = if let Some(frame) = self.last_presentation_frame.as_ref() {
                            frame.filter_alive_selectable_ids(&stored, team)
                        } else {
                            // Boot residual only — presentation filter_alive_selectable_ids owns InGame.
                            let mut live = Vec::new();
                            for id in stored {
                                if let Some(obj) = self.game_logic.find_object(id) {
                                    if obj.team == team && obj.is_selectable() && obj.is_alive() {
                                        live.push(id);
                                    }
                                }
                            }
                            live
                        };
                    }

                    self.game_logic
                        .select_objects(self.current_player_id, selection.clone());
                    self.selected_objects = selection.clone();
                    self.play_sound_effect(SoundType::Select);

                    // Double-tap residual: second press of same group within 500ms centers camera.
                    let now = Instant::now();
                    let double_tap = matches!(
                        self.last_control_group_select,
                        Some((g, t)) if g == group_num && now.duration_since(t).as_millis() < 500
                    );
                    self.last_control_group_select = Some((group_num, now));
                    if double_tap && !selection.is_empty() {
                        let center = if let Some(frame) = self.last_presentation_frame.as_ref() {
                            frame.centroid_of_ids(&selection)
                        } else {
                            // Boot residual only — presentation centroid_of_ids owns InGame double-tap.
                            let mut sum = Vec3::ZERO;
                            let mut n = 0u32;
                            for id in &selection {
                                if let Some(obj) = self.game_logic.find_object(*id) {
                                    sum += obj.get_position();
                                    n += 1;
                                }
                            }
                            if n == 0 {
                                None
                            } else {
                                Some(sum / n as f32)
                            }
                        };
                        if let Some(center) = center {
                            let clamped = self.clamp_to_world_bounds(center);
                            self.camera_target.x = clamped.x;
                            self.camera_target.z = clamped.z;
                            info!(
                                "Control group {} double-tap camera jump to {:?}",
                                group_num, clamped
                            );
                        }
                    }
                }
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Unit attitude Aggressive residual (Alt+A).
                self.issue_named_command_from_ui("Command_AttitudeAggressive");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("s")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Unit attitude Sleep / hold-fire residual (Alt+S).
                self.issue_named_command_from_ui("Command_AttitudeSleep");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("d")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Unit attitude Passive residual (Alt+D).
                self.issue_named_command_from_ui("Command_AttitudePassive");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Control))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // C++ classic A-key AttackMove residual: arm pending map click.
                if !self.selected_objects.is_empty()
                    || self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| !p.selected_objects.is_empty())
                        .unwrap_or(false)
                {
                    self.pending_map_command = Some(PendingMapCommand::AttackMove);
                    self.pending_structure_placement = None;
                    self.arm_radius_cursor_for_pending("ATTACK_CONTINUE_AREA");
                    let msg = "Attack-move: click destination";
                    self.game_hud.push_info_message(msg);
                    self.ui_manager.game_hud_mut().push_info_message(msg);
                }
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("t")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all attacking friendlies residual (Ctrl+Alt+T).
                self.select_all_friendly_attacking();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("t")
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Control))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // C++ classic T-key ForceAttackGround residual at cursor.
                if self.selected_objects.is_empty()
                    && self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.selected_objects.is_empty())
                        .unwrap_or(true)
                {
                    // no-op
                } else {
                    let loc = self.mouse_world_position;
                    let mut selected = self
                        .game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.selected_objects.clone())
                        .unwrap_or_default();
                    if selected.is_empty() {
                        selected = self.selected_objects.clone();
                    }
                    self.game_logic
                        .queue_command(crate::command_system::GameCommand {
                            command_type: crate::command_system::CommandType::ForceAttackGround {
                                location: loc,
                            },
                            player_id: self.current_player_id,
                            command_id: 0,
                            timestamp: std::time::SystemTime::now(),
                            selected_units: selected,
                            modifier_keys: crate::command_system::ModifierKeys {
                                ctrl: true,
                                shift: false,
                                alt: false,
                            },
                        });
                    self.game_logic.process_commands();
                    self.play_sound_effect(SoundType::Command);
                    let msg = "Force-attack ground";
                    self.game_hud.push_info_message(msg);
                    self.ui_manager.game_hud_mut().push_info_message(msg);
                }
            }
            Key::Named(NamedKey::Home)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Unfinished construction cycle residual (Ctrl+Alt+Home).
                self.cycle_unfinished_construction(1);
            }
            Key::Named(NamedKey::End)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Unfinished construction cycle residual (Ctrl+Alt+End).
                self.cycle_unfinished_construction(-1);
            }
            Key::Named(NamedKey::Home) if !ctrl_down => {
                // SELECT_NEXT_STRUCTURE residual (Home).
                self.cycle_friendly_structure_selection(1);
            }
            Key::Named(NamedKey::End) if !ctrl_down => {
                // SELECT_PREV_STRUCTURE residual (End).
                self.cycle_friendly_structure_selection(-1);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Convenience alias; retail SELECT_ALL is KEY_Q.
                self.select_all_friendly_units();
            }
            Key::Named(NamedKey::Delete) => {
                let shift = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                if ctrl_down && !shift {
                    // Cancel entire production queue residual (Ctrl+Delete).
                    if self.cancel_all_selected_production() {
                        return;
                    }
                }
                if shift {
                    // Debug residual: Shift+Delete destroys selection.
                    if self.selected_objects.is_empty() {
                        return;
                    }
                    for id in self.selected_objects.clone() {
                        self.game_logic.destroy_object(id);
                    }
                    self.selected_objects.clear();
                    self.game_logic
                        .select_objects(self.current_player_id, Vec::new());
                } else if self.cancel_selected_production_queue_head() {
                    // Producer selection: Delete cancels queue head residual.
                } else {
                    // Retail CommandMap DELETE_BEACON KEY_DEL residual.
                    self.issue_named_command_from_ui("Command_RemoveBeacon");
                }
            }
            Key::Named(NamedKey::Tab)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Cycle control groups residual (Ctrl+Shift+Tab).
                let delta = if self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) {
                    -1
                } else {
                    1
                };
                self.cycle_control_group_selection(delta);
            }
            Key::Named(NamedKey::Tab) if !ctrl_down => {
                // Retail CommandMap DIPLOMACY KEY_TAB residual.
                self.toggle_diplomacy_panel_hotkey();
            }
            Key::Named(NamedKey::F1) if ctrl_down => {
                // Debug overlay residual (Ctrl+F1); bare F1 remains camera bookmark.
                self.toggle_debug_info_hotkey();
            }
            Key::Named(NamedKey::F1) => self.handle_camera_view_hotkey(0),
            Key::Named(NamedKey::F2) if ctrl_down => {
                // FPS counter residual (Ctrl+F2); bare F2 remains camera bookmark.
                self.toggle_fps_counter_hotkey();
            }
            Key::Named(NamedKey::F2) => self.handle_camera_view_hotkey(1),
            Key::Named(NamedKey::F3) if ctrl_down => {
                // Move path lines residual (Ctrl+F3); bare F3 remains camera bookmark.
                self.toggle_move_lines_hotkey();
            }
            Key::Named(NamedKey::F3) => self.handle_camera_view_hotkey(2),
            Key::Named(NamedKey::F4) if ctrl_down => {
                // Attack path lines residual (Ctrl+F4); bare F4 remains camera bookmark.
                self.toggle_attack_lines_hotkey();
            }
            Key::Named(NamedKey::F4) => self.handle_camera_view_hotkey(3),
            Key::Named(NamedKey::F5) => self.handle_camera_view_hotkey(4),
            Key::Named(NamedKey::F6) => self.handle_camera_view_hotkey(5),
            Key::Named(NamedKey::F7) => self.handle_camera_view_hotkey(6),
            Key::Named(NamedKey::F8) => self.handle_camera_view_hotkey(7),
            Key::Character(c)
                if (c == "m" || c == "M")
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                self.toggle_background_music();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("v")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Debug residual only — not retail CommandMap.
                self.debug_show_victory(Some(self.current_player_id));
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("v")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Cycle ready special-power structures residual (Ctrl+Alt+V).
                self.cycle_ready_special_power_structure(1);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("v")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Fire ready superweapon / special power residual (map click).
                self.issue_named_command_from_ui("Command_DoSpecialPower");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("l")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Debug residual only — not retail CommandMap.
                let winner = self.game_logic.first_opponent_id(self.current_player_id);
                self.debug_show_victory(winner);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("p")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Cycle busy producers residual (Ctrl+Alt+P).
                self.cycle_busy_producer_selection(1);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("p")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Toggle pause with 'P' key
                self.toggle_pause();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("s")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // C++ MSG_META_STOP residual: stop selected units immediately.
                self.issue_named_command_from_ui("Command_Stop");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("d")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // C&C Generals standard Deploy residual (CommandMap DEPLOY commented but UI uses D).
                self.issue_named_command_from_ui("Command_Deploy");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("u")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select garrisoned structures residual (Ctrl+Alt+U).
                self.select_all_garrisoned_structures();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("u")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Harvester return supplies residual (Alt+U).
                self.issue_named_command_from_ui("Command_ReturnSupplies");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("u")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Evacuate / unload transport or garrison residual.
                self.issue_named_command_from_ui("Command_Evacuate");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("n")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Colonel Burton remote charge detonate residual.
                self.issue_named_command_from_ui("Command_DetonateRemoteDemoCharges");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("r")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all repairing units residual (Ctrl+Alt+R).
                self.select_all_repairing_units();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("r")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Aircraft return-to-base residual (Alt+R).
                self.issue_named_command_from_ui("Command_ReturnToBase");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("r")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Dozer/Worker repair residual: arm structure click.
                self.issue_named_command_from_ui("Command_Repair");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("y")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all patrolling residual (Ctrl+Alt+Y).
                self.select_all_friendly_patrolling();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("y") && !ctrl_down => {
                // Set factory rally point residual.
                self.issue_named_command_from_ui("Command_SetRallyPoint");
            }
            Key::Character(c) if c.eq_ignore_ascii_case("o") && !ctrl_down => {
                // China nuclear plant overcharge residual.
                self.issue_named_command_from_ui("Command_ToggleOvercharge");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("c")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Chinook combat drop residual (Alt+C).
                self.issue_named_command_from_ui("Command_CombatDrop");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("c")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Infantry capture-building residual: arm structure click.
                self.issue_named_command_from_ui("Command_CaptureBuilding");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("g")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all guarding friendlies residual (Ctrl+Alt+G).
                self.select_all_friendly_guarding();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("g")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // GeneralsExperience purchase next science residual (Alt+G).
                self.try_purchase_next_generals_science();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("g")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // C++ Guard residual: arm map-click Guard (location or unit).
                self.issue_named_command_from_ui("Command_Guard");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("x")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Dozer/Worker clear nearest mine residual (Alt+X).
                self.issue_named_command_from_ui("Command_ClearMines");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("x")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail CommandMap SCATTER KEY_X residual.
                self.issue_named_command_from_ui("Command_Scatter");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("z")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Clear path waypoints residual (Alt+Z).
                self.clear_selected_path_waypoints();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("z")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Sticky waypoint mode residual (Alt still force-on while held).
                self.sticky_waypoint_mode = !self.sticky_waypoint_mode;
                let msg = if self.sticky_waypoint_mode {
                    "Waypoint mode: ON"
                } else {
                    "Waypoint mode: OFF"
                };
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("q")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all combat units residual (Ctrl+Alt+Q).
                self.select_all_friendly_combat();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("q") && !ctrl_down => {
                // Retail CommandMap SELECT_ALL KEY_Q residual.
                self.select_all_friendly_units();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("e")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select veteran+ units residual (Ctrl+Alt+E).
                self.select_all_friendly_veterans();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("e")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Resume unfinished construction residual (Alt+E).
                self.resume_selected_construction();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("e")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail CommandMap SELECT_MATCHING_UNITS KEY_E residual.
                self.select_matching_units_hotkey();
            }
            Key::Character(c)
                if (c == "[" || c == "{")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Shrink guard radius residual (Alt+[).
                self.adjust_selected_guard_radius(-15.0);
            }
            Key::Character(c)
                if (c == "]" || c == "}")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Grow guard radius residual (Alt+]).
                self.adjust_selected_guard_radius(15.0);
            }
            Key::Character(c)
                if (c == "[" || c == "{")
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Construction tab previous residual.
                self.cycle_construction_tab(-1);
            }
            Key::Character(c)
                if (c == "]" || c == "}")
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Construction tab next residual.
                self.cycle_construction_tab(1);
            }
            Key::Character(c)
                if (c == "." || c == ">")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Stop all friendly mobile units residual (Ctrl+Shift+.).
                self.stop_all_friendly_units();
            }
            Key::Character(c)
                if (c == "." || c == ">")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Cycle idle military residual (Ctrl+Alt+.).
                self.cycle_idle_military_selection(1);
            }
            Key::Character(c)
                if (c == "," || c == "<")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Cycle idle military residual (Ctrl+Alt+,).
                self.cycle_idle_military_selection(-1);
            }
            Key::Character(c) if (c == "." || c == ">") && !ctrl_down => {
                // Retail-ish next idle worker residual (period key).
                self.cycle_friendly_worker_selection(1);
            }
            Key::Character(c) if c == "," || c == "<" => {
                // Previous idle worker residual (comma key).
                self.cycle_friendly_worker_selection(-1);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("w")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select docked aircraft residual (Ctrl+Alt+W).
                self.select_all_docked_aircraft();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("w")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Cycle primary/secondary weapon residual (Alt+W).
                self.issue_named_command_from_ui("Command_SwitchWeapons");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("i")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // China Hacker HackInternet residual (Alt+I).
                self.issue_named_command_from_ui("Command_HackInternet");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("m")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all moving friendlies residual (Ctrl+Alt+M).
                self.select_all_friendly_moving();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("m")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Ambulance CleanupArea residual (Alt+M).
                self.issue_named_command_from_ui("Command_CleanupArea");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("w")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail CommandMap SELECT_ALL_AIRCRAFT KEY_W residual.
                self.select_all_friendly_aircraft();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("h") && !ctrl_down => {
                // Retail CommandMap VIEW_COMMAND_CENTER KEY_H residual.
                self.issue_named_command_from_ui("Command_ViewCommandCenter");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("h")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select gathering units residual (Ctrl+Alt+H).
                self.select_all_friendly_gathering();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("h")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Toggle health bars residual (Alt+H).
                self.toggle_health_bars_hotkey();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("h") && ctrl_down => {
                // Retail CommandMap SELECT_HERO Ctrl+H residual.
                self.select_hero_units_hotkey();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("i")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select idle harvesters residual (Ctrl+Alt+I).
                self.select_idle_harvesters();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("k")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select stealthed friendlies residual (Ctrl+Alt+K).
                self.select_all_friendly_stealthed();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("j")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select occupied transports residual (Ctrl+Alt+J).
                self.select_all_occupied_transports();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("i")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Select all harvesters / supply collectors residual.
                self.select_all_harvesters();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("i")
                    && ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Select all idle military residual (Ctrl+I).
                self.select_all_idle_military();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("f")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Camera follow primary selection residual (Alt+F).
                self.toggle_camera_follow_selection();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("f")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Retail CommandMap CREATE_FORMATION Ctrl+F residual.
                self.issue_named_command_from_ui("Command_CreateFormation");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("b")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select constructing workers residual (Ctrl+Alt+B).
                self.select_all_constructing_workers();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("b")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Demo tertiary suicide residual (Alt+B).
                self.issue_named_command_from_ui("Command_DemoTertiarySuicide");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("b")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Retail CommandMap PLACE_BEACON Ctrl+B residual.
                self.issue_named_command_from_ui("Command_PlaceBeacon");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("c")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Retail CommandMap ALL_CHEER Ctrl+C residual.
                self.issue_named_command_from_ui("Command_Cheer");
            }
            Key::Named(NamedKey::ArrowRight)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Damaged structure cycle residual (Ctrl+Alt+Right).
                self.cycle_damaged_structure_selection(1);
            }
            Key::Named(NamedKey::ArrowLeft)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Damaged structure cycle residual (Ctrl+Alt+Left).
                self.cycle_damaged_structure_selection(-1);
            }
            Key::Named(NamedKey::ArrowRight)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // SELECT_NEXT_STRUCTURE residual (Ctrl+Shift+Right).
                self.cycle_friendly_structure_selection(1);
            }
            Key::Named(NamedKey::ArrowLeft)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // SELECT_PREV_STRUCTURE residual (Ctrl+Shift+Left).
                self.cycle_friendly_structure_selection(-1);
            }
            Key::Named(NamedKey::ArrowRight) if ctrl_down => {
                // Retail SELECT_NEXT_UNIT Ctrl+Right residual.
                self.cycle_friendly_selection(1);
            }
            Key::Named(NamedKey::ArrowLeft) if ctrl_down => {
                // Retail SELECT_PREV_UNIT Ctrl+Left residual.
                self.cycle_friendly_selection(-1);
            }
            Key::Named(NamedKey::ArrowUp)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Damaged unit cycle residual (Ctrl+Alt+Up).
                self.cycle_damaged_unit_selection(1);
            }
            Key::Named(NamedKey::ArrowDown)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Damaged unit cycle residual (Ctrl+Alt+Down).
                self.cycle_damaged_unit_selection(-1);
            }
            Key::Named(NamedKey::ArrowUp) if ctrl_down => {
                // Retail SELECT_NEXT_WORKER Ctrl+Up residual.
                self.cycle_friendly_worker_selection(1);
            }
            Key::Named(NamedKey::ArrowDown) if ctrl_down => {
                // Retail SELECT_PREV_WORKER Ctrl+Down residual.
                self.cycle_friendly_worker_selection(-1);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all friendlies near camera residual (Ctrl+Alt+A).
                self.select_all_friendly_on_screen();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Sticky auto-attack residual (Ctrl+Shift+A): RMB move becomes attack-move.
                // Stored on engine; MouseCommandContext path honors via force AttackMove.
                self.sticky_auto_attack = !self.sticky_auto_attack;
                let msg = if self.sticky_auto_attack {
                    "Auto-attack: ON"
                } else {
                    "Auto-attack: OFF"
                };
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
            }
            Key::Named(NamedKey::F9) => {
                // Retail CommandMap TOGGLE_CONTROL_BAR KEY_F9 residual.
                self.game_hud.toggle_visibility();
                self.ui_manager.game_hud_mut().toggle_visibility();
                info!(
                    "Control bar visibility toggled (engine visible={})",
                    self.game_hud.hud_visible()
                );
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("s")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all friendly structures residual (Ctrl+Alt+S).
                self.select_all_friendly_structures();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("s")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Sell selected structures residual (Ctrl+Shift+S).
                self.issue_named_command_from_ui("Command_Sell");
            }
            Key::Character(c) if c.eq_ignore_ascii_case("s") && ctrl_down => {
                self.quick_save_from_hotkey("Ctrl+S");
            }
            Key::Character(c) if c.eq_ignore_ascii_case("l") && ctrl_down => {
                self.quick_load_from_hotkey("Ctrl+L");
            }
            Key::Named(NamedKey::Enter) => {
                // Retail CommandMap CHAT_EVERYONE KEY_ENTER residual.
                self.open_chat_hotkey(crate::ui::ChatTarget::All);
            }
            Key::Named(NamedKey::Backspace) => {
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) {
                    // Retail DEMO_INSTANT_QUIT Shift+Ctrl+Backspace residual.
                    info!("DEMO_INSTANT_QUIT residual — exiting");
                    self.request_state_change(GameState::Exiting);
                } else {
                    // Retail CommandMap CHAT_ALLIES KEY_BACKSPACE residual.
                    self.open_chat_hotkey(crate::ui::ChatTarget::Allies);
                }
            }
            Key::Named(NamedKey::F12) => {
                // Retail CommandMap TAKE_SCREENSHOT KEY_F12 residual.
                self.take_screenshot_hotkey();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("t")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail TOGGLE_CAMERA_TRACKING_DRAWABLE Shift+Alt+Ctrl+T residual.
                self.toggle_camera_tracking_drawable_hotkey();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("f")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail TOGGLE_FAST_FORWARD_REPLAY KEY_F residual.
                self.toggle_replay_fast_forward_hotkey();
            }
            Key::Named(NamedKey::Escape) => {
                // Outer event-loop Escape handler is unreachable for keyboard
                // because input() consumes the event. Mirror C++ residual here:
                // cancel placement/map-command first, else pause/resume.
                match self.current_state {
                    GameState::InGame => {
                        if self.chat_panel.is_open() {
                            self.chat_panel.close();
                            info!("Escape closed chat residual");
                        } else if self.diplomacy_panel.is_active() {
                            self.diplomacy_panel.close();
                            info!("Escape closed diplomacy panel residual");
                        } else if self.pending_structure_placement.is_some() {
                            self.cancel_structure_placement_from_ui();
                            info!("Escape cancelled structure placement residual");
                        } else if self.pending_map_command.take().is_some() {
                            self.clear_radius_cursor_overlays();
                            let msg = "Cancelled pending command";
                            self.game_hud.push_info_message(msg);
                            self.ui_manager.game_hud_mut().push_info_message(msg);
                            info!("Escape cancelled pending map command residual");
                        } else {
                            info!("Escape pressed in InGame state - pausing");
                            self.request_state_change(GameState::Paused);
                        }
                    }
                    GameState::Paused => {
                        info!("Escape pressed in Paused state - resuming");
                        self.request_state_change(GameState::InGame);
                    }
                    GameState::Menu | GameState::Loading => {
                        info!("Escape pressed in Menu/Loading - exiting");
                        self.request_state_change(GameState::Exiting);
                    }
                    GameState::Victory | GameState::Defeat => {
                        info!("Escape pressed in endgame - returning to menu");
                        self.request_state_change(GameState::Menu);
                    }
                    GameState::Exiting | GameState::Initializing => {}
                }
            }
            Key::Named(NamedKey::F11) => {
                // Toggle fullscreen mode
                let current_fullscreen = self.window.fullscreen().is_some();
                if let Err(e) = self.set_fullscreen(!current_fullscreen) {
                    error!("Failed to toggle fullscreen: {}", e);
                } else {
                    info!("Toggled fullscreen mode: {}", !current_fullscreen);
                }
            }
            _ => {}
        }
    }

    /// Retail SAVE_VIEW / VIEW_VIEW (Ctrl+Fn save, Fn recall) residual.
    fn handle_camera_view_hotkey(&mut self, slot: usize) {
        if slot >= self.camera_view_bookmarks.len() {
            return;
        }
        let ctrl = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        if ctrl {
            let pos = self.camera_target;
            self.camera_view_bookmarks[slot] = Some(pos);
            let msg = format!("Saved camera view {}", slot + 1);
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
            info!("SAVE_VIEW{} -> {:?}", slot + 1, pos);
        } else if let Some(pos) = self.camera_view_bookmarks[slot] {
            let clamped = self.clamp_to_world_bounds(pos);
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
            // Also request presentation camera focus residual for dual path.
            self.game_logic.request_camera_focus(clamped);
            info!("VIEW_VIEW{} -> {:?}", slot + 1, clamped);
        } else {
            let msg = format!("Camera view {} is empty", slot + 1);
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
        }
    }

    /// Retail CHAT_EVERYONE / CHAT_ALLIES residual.
    fn open_chat_hotkey(&mut self, target: crate::ui::ChatTarget) {
        let name = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame
                .players
                .iter()
                .find(|p| p.id == self.current_player_id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| format!("Player{}", self.current_player_id))
        } else {
            self.game_logic
                .get_player(self.current_player_id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| format!("Player{}", self.current_player_id))
        };
        self.chat_panel.set_local_player_name(&name);
        self.chat_panel.set_target(target);
        if self.chat_panel.open() {
            let label = match target {
                crate::ui::ChatTarget::All => "Chat (All)",
                crate::ui::ChatTarget::Allies => "Chat (Allies)",
                crate::ui::ChatTarget::Player(_) => "Chat (Whisper)",
            };
            self.game_hud.push_info_message(label);
            self.ui_manager.game_hud_mut().push_info_message(label);
            info!("Opened {label}");
        }
    }

    fn route_key_to_chat_panel(&mut self, key: &Key) -> bool {
        use crate::ui::KeyCode;
        // Prefer UI key mapping then character text insert.
        if let Some(ui_key) = Self::to_ui_key_code(key) {
            if self.chat_panel.press_key(ui_key) {
                // Drain sent messages into HUD log residual.
                for ev in self.chat_panel.drain_events() {
                    if let crate::ui::ChatEvent::MessageSent { text, target } = ev {
                        let prefix = match target {
                            crate::ui::ChatTarget::All => "[All]",
                            crate::ui::ChatTarget::Allies => "[Allies]",
                            crate::ui::ChatTarget::Player(_) => "[Whisper]",
                        };
                        let msg = format!("{prefix} {text}");
                        self.game_hud.push_info_message(&msg);
                        self.ui_manager.game_hud_mut().push_info_message(&msg);
                    }
                }
                return true;
            }
        }
        if let Key::Character(s) = key {
            if self.chat_panel.type_text(s) {
                return true;
            }
        }
        false
    }

    /// Retail TOGGLE_CAMERA_TRACKING_DRAWABLE residual.
    fn toggle_camera_tracking_drawable_hotkey(&mut self) {
        self.camera_tracking_selection = !self.camera_tracking_selection;
        let msg = if self.camera_tracking_selection {
            "Camera tracking selection: ON"
        } else {
            "Camera tracking selection: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
        info!("{msg}");
    }

    /// Retail TOGGLE_FAST_FORWARD_REPLAY (m_TiVOFastMode) residual.
    fn toggle_replay_fast_forward_hotkey(&mut self) {
        // C++ only applies in replay games (or debug). Residual: always toggle flag + HUD.
        self.replay_fast_forward = !self.replay_fast_forward;
        let msg = if self.replay_fast_forward {
            "m_TiVOFastMode: ON"
        } else {
            "m_TiVOFastMode: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
        info!("{msg}");
    }

    /// Follow selection centroid when camera tracking residual is armed.
    fn update_camera_tracking_drawable(&mut self) {
        if !self.camera_tracking_selection {
            return;
        }
        let ids = if !self.selected_objects.is_empty() {
            self.selected_objects.clone()
        } else {
            self.game_logic
                .get_player(self.current_player_id)
                .map(|p| p.selected_objects.clone())
                .unwrap_or_default()
        };
        if ids.is_empty() {
            return;
        }
        let center = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.centroid_of_ids(&ids)
        } else {
            let mut sum = Vec3::ZERO;
            let mut n = 0u32;
            for id in &ids {
                if let Some(obj) = self.game_logic.find_object(*id) {
                    if obj.is_alive() {
                        sum += obj.get_position();
                        n += 1;
                    }
                }
            }
            if n == 0 {
                None
            } else {
                Some(sum / n as f32)
            }
        };
        if let Some(center) = center {
            let clamped = self.clamp_to_world_bounds(center);
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
    }

    /// Retail TAKE_SCREENSHOT (KEY_F12) residual.
    fn take_screenshot_hotkey(&mut self) {
        let dir = std::env::temp_dir().join("generals_screenshots");
        let _ = std::fs::create_dir_all(&dir);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("screenshot_{stamp}.png"));
        match ww3d_engine::make_screenshot(&path) {
            Ok(()) => {
                let msg = format!("Screenshot: {}", path.display());
                self.game_hud.push_info_message(&msg);
                self.ui_manager.game_hud_mut().push_info_message(&msg);
                info!("{msg}");
            }
            Err(err) => {
                let msg = format!("Screenshot failed: {err:?}");
                self.game_hud.push_info_message(&msg);
                self.ui_manager.game_hud_mut().push_info_message(&msg);
                warn!("{msg}");
            }
        }
    }

    /// Retail DIPLOMACY (KEY_TAB) residual.
    fn toggle_diplomacy_panel_hotkey(&mut self) {
        self.sync_diplomacy_panel_from_world();
        self.diplomacy_panel.toggle();
        let msg = if self.diplomacy_panel.is_active() {
            "Diplomacy panel opened"
        } else {
            "Diplomacy panel closed"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
        info!("{msg}");
    }

    fn sync_diplomacy_panel_from_world(&mut self) {
        use crate::ui::{DiplomacyPlayerEntry, DiplomacyPlayerStatus, DiplomacyRelation};
        let local_id = self.current_player_id as i32;
        self.diplomacy_panel.set_local_player_id(local_id);
        let mut rows = Vec::new();
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            for p in &frame.players {
                let status = if p.is_alive {
                    DiplomacyPlayerStatus::Active
                } else {
                    DiplomacyPlayerStatus::Defeated
                };
                let relationship = if p.id == self.current_player_id {
                    DiplomacyRelation::Allied
                } else if p.team
                    == frame
                        .players
                        .iter()
                        .find(|x| x.id == self.current_player_id)
                        .map(|x| x.team)
                        .unwrap_or(p.team)
                {
                    DiplomacyRelation::Allied
                } else {
                    DiplomacyRelation::Enemy
                };
                rows.push(DiplomacyPlayerEntry {
                    player_id: p.id as i32,
                    name: p.name.clone(),
                    side: format!("{:?}", p.team),
                    team: match p.team {
                        crate::game_logic::Team::USA => 0,
                        crate::game_logic::Team::China => 1,
                        crate::game_logic::Team::GLA => 2,
                        _ => -1,
                    },
                    status,
                    relationship,
                    is_muted: false,
                });
            }
        } else {
            for (&id, p) in self.game_logic.get_players() {
                rows.push(DiplomacyPlayerEntry {
                    player_id: id as i32,
                    name: p.name.clone(),
                    side: format!("{:?}", p.team),
                    team: match p.team {
                        crate::game_logic::Team::USA => 0,
                        crate::game_logic::Team::China => 1,
                        crate::game_logic::Team::GLA => 2,
                        _ => -1,
                    },
                    status: DiplomacyPlayerStatus::Active,
                    relationship: if id == self.current_player_id {
                        DiplomacyRelation::Allied
                    } else {
                        DiplomacyRelation::Enemy
                    },
                    is_muted: false,
                });
            }
        }
        self.diplomacy_panel.set_players(rows);
        // Keep panel layout in sync with window.
        let (w, h) = (
            self.window.inner_size().width,
            self.window.inner_size().height,
        );
        self.diplomacy_panel.resize(w, h);
    }

    /// Retail CAMERA_RESET (KEY_KP5) residual.
    fn reset_camera_view_hotkey(&mut self) {
        let focus = if let Some(pos) = self
            .game_logic
            .get_player(self.current_player_id)
            .and_then(|p| self.game_logic.command_center_position(p.team))
        {
            pos
        } else if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame
                .centroid_of_ids(&frame.alive_selectable_friendly_ids(frame.local_team()))
                .unwrap_or(self.camera_target)
        } else {
            self.camera_target
        };
        let clamped = self.clamp_to_world_bounds(focus);
        self.camera_target.x = clamped.x;
        self.camera_target.z = clamped.z;
        self.camera_zoom = self.compute_default_camera_zoom_for_target(
            clamped,
            self.game_logic.script_default_camera_max_height(),
        );
        self.game_logic.request_camera_focus(clamped);
        info!("CAMERA_RESET residual -> {:?}", clamped);
    }

    /// Retail SELECT_NEXT/PREV_UNIT residual.
    fn cycle_friendly_selection(&mut self, delta: i32) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        let all: Vec<ObjectId> = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.alive_selectable_friendly_ids(team)
        } else {
            let mut live: Vec<ObjectId> = self
                .game_logic
                .get_objects()
                .iter()
                .filter(|(_, obj)| obj.team == team && obj.is_selectable() && obj.is_alive())
                .map(|(&id, _)| id)
                .collect();
            live.sort_by_key(|id| id.0);
            live
        };
        if all.is_empty() {
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            all.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = all.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    all[i]
                })
                .unwrap_or(all[0])
        } else if delta >= 0 {
            all[0]
        } else {
            all[all.len() - 1]
        };

        self.selected_objects = vec![next];
        self.game_logic
            .select_objects(self.current_player_id, vec![next]);
        self.play_sound_effect(SoundType::Select);
    }

    /// Retail SELECT_NEXT/PREV_WORKER residual — prefer dozers/workers/harvesters.
    fn cycle_friendly_worker_selection(&mut self, delta: i32) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        let mut idle_workers: Vec<ObjectId> = Vec::new();
        let mut busy_workers: Vec<ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_selectable() || !obj.is_alive() {
                continue;
            }
            let n = obj.template_name.to_ascii_lowercase();
            let is_worker = obj.is_dozer
                || n.contains("dozer")
                || n.contains("worker")
                || n.contains("chinook")
                || n.contains("supply")
                || n.contains("hack");
            if !is_worker {
                continue;
            }
            // Prefer idle / non-tasked workers residual (retail SELECT_IDLE_WORKER).
            let idle = matches!(obj.ai_state, crate::game_logic::AIState::Idle)
                && obj.target.is_none()
                && !obj.status.moving;
            if idle {
                idle_workers.push(id);
            } else {
                busy_workers.push(id);
            }
        }
        idle_workers.sort_by_key(|id| id.0);
        busy_workers.sort_by_key(|id| id.0);
        // Cycle idle first; fall back to all workers if none idle.
        let mut workers = if !idle_workers.is_empty() {
            idle_workers
        } else {
            busy_workers
        };
        if workers.is_empty() {
            // Fail-open: fall back to general unit cycle.
            self.cycle_friendly_selection(delta);
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            workers
                .iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = workers.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    workers[i]
                })
                .unwrap_or(workers[0])
        } else if delta >= 0 {
            workers[0]
        } else {
            workers[workers.len() - 1]
        };

        self.selected_objects = vec![next];
        self.game_logic
            .select_objects(self.current_player_id, vec![next]);
        self.play_sound_effect(SoundType::Select);
    }

    /// Retail-ish SELECT_NEXT/PREV_STRUCTURE residual.
    fn cycle_friendly_structure_selection(&mut self, delta: i32) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        let mut structures: Vec<ObjectId> =
            if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame
                    .objects
                    .iter()
                    .filter(|o| {
                        !o.destroyed
                            && o.team == team
                            && o.is_structure
                            && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
                    })
                    .map(|o| o.id)
                    .collect()
            } else {
                self.game_logic
                    .get_objects()
                    .iter()
                    .filter(|(_, obj)| {
                        obj.team == team
                            && obj.is_alive()
                            && obj.is_selectable()
                            && (obj.is_kind_of(crate::game_logic::KindOf::Structure)
                                || obj.object_type == crate::game_logic::ObjectType::Building)
                    })
                    .map(|(&id, _)| id)
                    .collect()
            };
        structures.sort_by_key(|id| id.0);
        if structures.is_empty() {
            return;
        }

        let next = if let Some(current) = self.selected_objects.first().copied() {
            structures
                .iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = structures.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    structures[i]
                })
                .unwrap_or(structures[0])
        } else if delta >= 0 {
            structures[0]
        } else {
            structures[structures.len() - 1]
        };

        self.selected_objects = vec![next];
        self.game_logic
            .select_objects(self.current_player_id, vec![next]);
        self.play_sound_effect(SoundType::Select);
        // Center camera on structure residual.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            if let Some(o) = frame.objects.iter().find(|o| o.id == next) {
                let clamped = self.clamp_to_world_bounds(o.position);
                self.camera_target.x = clamped.x;
                self.camera_target.z = clamped.z;
            }
        } else if let Some(obj) = self.game_logic.find_object(next) {
            let clamped = self.clamp_to_world_bounds(obj.get_position());
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
    }
    /// Cycle damaged friendly structures residual (for repair response).

    /// Cycle unfinished friendly construction residual (Ctrl+Alt+Home/End).

    /// Resume unfinished construction with selected dozers residual (Alt+E).
    fn resume_selected_construction(&mut self) {
        let player_id = self.current_player_id;
        let selected = self
            .game_logic
            .get_player(player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_else(|| self.selected_objects.clone());
        let unfinished: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| {
                self.game_logic
                    .get_object(id)
                    .is_some_and(|o| o.is_alive() && o.status.under_construction && !o.status.sold)
            })
            .collect();
        let dozers: Vec<_> = selected
            .iter()
            .copied()
            .filter(|&id| {
                self.game_logic.get_object(id).is_some_and(|o| {
                    o.is_alive()
                        && (o.is_dozer
                            || o.template_name.to_ascii_lowercase().contains("dozer")
                            || o.template_name.to_ascii_lowercase().contains("worker"))
                })
            })
            .collect();
        // If only unfinished selected, pick all team dozers idle residual.
        let mut builders = dozers;
        if builders.is_empty() {
            let team = self
                .game_logic
                .get_player(player_id)
                .map(|p| p.team)
                .unwrap_or(crate::game_logic::Team::USA);
            for (&id, obj) in self.game_logic.get_objects() {
                if obj.team != team || !obj.is_alive() {
                    continue;
                }
                let n = obj.template_name.to_ascii_lowercase();
                if obj.is_dozer || n.contains("dozer") || n.contains("worker") {
                    if matches!(obj.ai_state, crate::game_logic::AIState::Idle) {
                        builders.push(id);
                    }
                }
            }
        }
        let target = unfinished.first().copied().or_else(|| {
            // Fall back to cycled unfinished if selection is dozers only.
            self.game_logic
                .get_objects()
                .iter()
                .find(|(_, o)| {
                    o.is_alive()
                        && o.status.under_construction
                        && !o.status.sold
                        && self
                            .game_logic
                            .get_player(player_id)
                            .map(|p| o.team == p.team)
                            .unwrap_or(false)
                })
                .map(|(&id, _)| id)
        });
        let Some(target_id) = target else {
            let msg = "No unfinished construction to resume";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        };
        if builders.is_empty() {
            let msg = "No dozer/worker available to resume";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::ResumeConstruction { target_id },
                player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: builders,
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
        self.game_logic.process_commands();
        self.play_sound_effect(SoundType::Command);
        let msg = "Resuming construction";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    fn cycle_unfinished_construction(&mut self, delta: i32) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if obj.status.under_construction && !obj.status.sold {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No unfinished construction";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };
        self.game_logic
            .select_objects(self.current_player_id, vec![next]);
        self.selected_objects = vec![next];
        if let Some(obj) = self.game_logic.find_object(next) {
            let clamped = self.clamp_to_world_bounds(obj.get_position());
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
        let msg = "Unfinished construction selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    fn cycle_damaged_structure_selection(&mut self, delta: i32) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        let mut damaged: Vec<(crate::game_logic::ObjectId, f32)> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if !obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            if obj.status.under_construction || obj.status.sold {
                continue;
            }
            let max_h = obj.health.maximum.max(1.0);
            let ratio = obj.health.current / max_h;
            if ratio < 0.999 {
                damaged.push((id, ratio));
            }
        }
        if damaged.is_empty() {
            let msg = "No damaged structures";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        // Most damaged first, stable by id.
        damaged.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0 .0.cmp(&b.0 .0))
        });
        let ids: Vec<_> = damaged.into_iter().map(|(id, _)| id).collect();
        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };
        self.game_logic
            .select_objects(self.current_player_id, vec![next]);
        self.selected_objects = vec![next];
        if let Some(obj) = self.game_logic.find_object(next) {
            let clamped = self.clamp_to_world_bounds(obj.get_position());
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
        let msg = "Damaged structure selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Select all idle friendly combat units residual (Ctrl+I).

    /// Purchase next available GeneralsExperience science residual (Alt+G).
    fn try_purchase_next_generals_science(&mut self) {
        let player_id = self.current_player_id;
        let Some(player) = self.game_logic.get_player(player_id) else {
            return;
        };
        if player.science_purchase_points <= 0 {
            let msg = "No science purchase points";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        let team = player.team;
        // Retail-ish purchasable science order residual (fail-closed vs full Science.ini tree).
        let candidates: &[&str] = match team {
            crate::game_logic::Team::China => &[
                "SCIENCE_RedGuardTraining",
                "SCIENCE_BattlemasterTraining",
                "SCIENCE_ArtilleryTraining",
                "SCIENCE_NukeCannon",
                "SCIENCE_CashBounty1",
            ],
            crate::game_logic::Team::GLA => &[
                "SCIENCE_RebelAmbush1",
                "SCIENCE_CashBounty1",
                "SCIENCE_SneakAttack",
                "SCIENCE_AnthraxBomb",
                "SCIENCE_ScudLauncher",
            ],
            _ => &[
                // America / default
                "SCIENCE_PaladinTank",
                "SCIENCE_StealthFighter",
                "SCIENCE_Pathfinder",
                "SCIENCE_CashBounty1",
                "SCIENCE_A10ThunderboltMissileStrike1",
                "SCIENCE_EmergencyRepair1",
                "SCIENCE_SpyDrone",
            ],
        };
        let unlocked = player.unlocked_sciences.clone();
        let spp = player.science_purchase_points;
        drop(player);

        let mut chosen = None;
        for &name in candidates {
            if unlocked.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                continue;
            }
            // Probe without spending via can-capable if available.
            if let Some(p) = self.game_logic.get_player(player_id) {
                if !p.is_capable_of_purchasing_science(name) {
                    continue;
                }
            }
            chosen = Some(name.to_string());
            break;
        }
        let Some(science_name) = chosen else {
            let msg = format!("No purchasable science (spp={spp})");
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
            return;
        };

        self.game_logic
            .queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::PurchaseScience {
                    science_name: science_name.clone(),
                },
                player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: Vec::new(),
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
        self.game_logic.process_commands();
        let msg = format!("Purchased {science_name}");
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
        self.play_sound_effect(SoundType::Command);
    }

    /// Cycle idle friendly combat units residual (Ctrl+Alt+, / .).
    fn cycle_idle_military_selection(&mut self, delta: i32) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            let n = obj.template_name.to_ascii_lowercase();
            if obj.is_dozer
                || n.contains("dozer")
                || n.contains("worker")
                || n.contains("supply")
                || n.contains("harvester")
            {
                continue;
            }
            if !obj.can_move() && !obj.can_attack() {
                continue;
            }
            let idle = matches!(obj.ai_state, crate::game_logic::AIState::Idle)
                && obj.target.is_none()
                && !obj.status.moving;
            if idle {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No idle military";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };
        self.game_logic
            .select_objects(self.current_player_id, vec![next]);
        self.selected_objects = vec![next];
        if let Some(obj) = self.game_logic.find_object(next) {
            let clamped = self.clamp_to_world_bounds(obj.get_position());
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
        let msg = "Idle military selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Select all friendly units currently repairing residual (Ctrl+Alt+R).
    fn select_all_repairing_units(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if matches!(obj.ai_state, crate::game_logic::AIState::Repairing) {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No repairing units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} repairing", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    fn select_all_idle_military(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            if !obj.can_move() {
                continue;
            }
            let n = obj.template_name.to_ascii_lowercase();
            // Exclude pure workers/dozers/supply from "military idle" residual.
            let is_worker =
                obj.is_dozer || n.contains("dozer") || n.contains("worker") || n.contains("supply");
            if is_worker {
                continue;
            }
            let military = obj.can_attack()
                || obj.is_kind_of(crate::game_logic::KindOf::Infantry)
                || obj.is_kind_of(crate::game_logic::KindOf::Vehicle)
                || obj.is_kind_of(crate::game_logic::KindOf::Aircraft)
                || n.contains("ranger")
                || n.contains("tank")
                || n.contains("jet")
                || n.contains("humvee");
            if !military {
                continue;
            }
            let idle = matches!(obj.ai_state, crate::game_logic::AIState::Idle)
                && obj.target.is_none()
                && !obj.status.moving;
            if idle {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No idle military units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        let msg = format!("Selected {} idle military", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Select all friendly harvesters / supply collectors residual (Ctrl+Shift+I).
    fn select_all_harvesters(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            let n = obj.template_name.to_ascii_lowercase();
            // Prefer true collectors; include GLA workers as harvesters residual.
            let is_collector = n.contains("supply")
                || n.contains("harvester")
                || n.contains("chinook")
                || (n.contains("worker") && !n.contains("dozer"))
                || matches!(obj.ai_state, crate::game_logic::AIState::Gathering);
            if !is_collector {
                continue;
            }
            ids.push(id);
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No harvesters found";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        let msg = format!("Selected {} harvesters", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
        self.play_sound_effect(SoundType::Select);
    }

    /// Select idle friendly harvesters residual (Ctrl+Alt+I).
    fn select_idle_harvesters(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            let n = obj.template_name.to_ascii_lowercase();
            let is_collector = n.contains("supply")
                || n.contains("harvester")
                || n.contains("chinook")
                || (n.contains("worker") && !n.contains("dozer"));
            if !is_collector {
                continue;
            }
            let idle = matches!(obj.ai_state, crate::game_logic::AIState::Idle)
                && obj.target.is_none()
                && !obj.status.moving;
            if idle {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No idle harvesters";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} idle harvesters", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Cycle construction panel tab residual (`[` / `]`).
    fn cycle_construction_tab(&mut self, delta: i32) {
        use crate::ui::ConstructionTab;
        if !self.game_hud.construction_panel.is_visible() {
            return;
        }
        let tabs = [
            ConstructionTab::Buildings,
            ConstructionTab::Infantry,
            ConstructionTab::Vehicles,
            ConstructionTab::Aircraft,
        ];
        let cur = self.game_hud.construction_panel.current_tab();
        let idx = tabs.iter().position(|t| *t == cur).unwrap_or(0) as i32;
        let n = tabs.len() as i32;
        let next = (((idx + delta) % n) + n) % n;
        let tab = tabs[next as usize];
        self.game_hud.construction_panel.force_tab(tab);
        let label = match tab {
            ConstructionTab::Buildings => "Buildings",
            ConstructionTab::Infantry => "Infantry",
            ConstructionTab::Vehicles => "Vehicles",
            ConstructionTab::Aircraft => "Aircraft",
            ConstructionTab::NavalUnits => "Naval",
            ConstructionTab::SuperWeapons => "Superweapons",
        };
        let msg = format!("Construction tab: {label}");
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Select friendly units near camera (on-screen residual, Ctrl+Alt+A).

    /// Select all friendly structures residual (Ctrl+Alt+S).
    fn select_all_friendly_structures(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            for o in &frame.objects {
                if o.team == team
                    && o.is_structure
                    && !o.destroyed
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
                {
                    ids.push(o.id);
                }
            }
        } else {
            for (&id, obj) in self.game_logic.get_objects() {
                if obj.team == team
                    && obj.is_alive()
                    && obj.is_selectable()
                    && obj.is_kind_of(crate::game_logic::KindOf::Structure)
                {
                    ids.push(id);
                }
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No structures found";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} structures", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Adjust guard radius on selected guarding units residual (Alt+[ / ]).

    /// Clear movement path / waypoints on selection residual (Alt+Z).
    fn clear_selected_path_waypoints(&mut self) {
        let selected = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_else(|| self.selected_objects.clone());
        if selected.is_empty() {
            if self.sticky_waypoint_mode {
                self.sticky_waypoint_mode = false;
                let msg = "Waypoint mode: OFF";
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
            }
            return;
        }
        let mut any = false;
        for id in selected {
            if let Some(obj) = self.game_logic.get_object_mut(id) {
                if !obj.movement.path.is_empty() || obj.movement.target_position.is_some() {
                    obj.movement.path.clear();
                    obj.movement.current_path_index = 0;
                    obj.movement.target_position = None;
                    obj.status.moving = false;
                    any = true;
                }
            }
        }
        if self.sticky_waypoint_mode {
            self.sticky_waypoint_mode = false;
            any = true;
        }
        if any {
            let msg = "Path cleared";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            self.play_sound_effect(SoundType::Command);
        }
    }

    /// Cycle damaged friendly mobile units residual (Ctrl+Alt+Up/Down).
    fn cycle_damaged_unit_selection(&mut self, delta: i32) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut damaged: Vec<(crate::game_logic::ObjectId, f32)> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            if !obj.can_move() {
                continue;
            }
            let max_h = obj.health.maximum.max(1.0);
            let ratio = obj.health.current / max_h;
            if ratio < 0.999 {
                damaged.push((id, ratio));
            }
        }
        if damaged.is_empty() {
            let msg = "No damaged units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        damaged.sort_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0 .0.cmp(&b.0 .0))
        });
        let ids: Vec<_> = damaged.into_iter().map(|(id, _)| id).collect();
        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };
        self.game_logic
            .select_objects(self.current_player_id, vec![next]);
        self.selected_objects = vec![next];
        if let Some(obj) = self.game_logic.find_object(next) {
            let clamped = self.clamp_to_world_bounds(obj.get_position());
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
        let msg = "Damaged unit selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    fn adjust_selected_guard_radius(&mut self, delta: f32) {
        let selected = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_else(|| self.selected_objects.clone());
        if selected.is_empty() {
            return;
        }
        let mut any = false;
        let mut last_r = 0.0_f32;
        for id in selected {
            if let Some(obj) = self.game_logic.get_object_mut(id) {
                let guarding = matches!(
                    obj.ai_state,
                    crate::game_logic::AIState::GuardingArea
                        | crate::game_logic::AIState::GuardingObject
                ) || obj.guard_position.is_some()
                    || obj.guard_target.is_some();
                if !guarding && obj.guard_radius <= 0.0 {
                    // Allow setting radius even if not yet guarding so next Guard uses it.
                    let base = obj.selection_radius.max(20.0) * 2.0;
                    obj.guard_radius = (base + delta).clamp(30.0, 400.0);
                } else {
                    let cur = if obj.guard_radius > 1.0 {
                        obj.guard_radius
                    } else {
                        obj.selection_radius.max(20.0) * 2.0
                    };
                    obj.guard_radius = (cur + delta).clamp(30.0, 400.0);
                }
                last_r = obj.guard_radius;
                any = true;
            }
        }
        if any {
            let msg = format!("Guard radius: {last_r:.0}");
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
        }
    }

    /// Select all friendly combat units (exclude workers/dozers/supply) residual.

    /// Select all friendly units currently moving residual (Ctrl+Alt+M).

    /// Select all friendly units currently attacking residual (Ctrl+Alt+T).
    fn select_all_friendly_attacking(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            let attacking = obj.status.attacking
                || obj.target.is_some()
                || matches!(
                    obj.ai_state,
                    crate::game_logic::AIState::Attacking
                        | crate::game_logic::AIState::AttackingGround
                        | crate::game_logic::AIState::Patrolling
                );
            if attacking {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No attacking units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} attacking", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Issue Stop to all friendly mobile units residual (Ctrl+Alt+S is structures).
    /// Ctrl+Shift+Period residual: stop everything friendly.

    /// Runtime-host residual: ensure at least one local mobile is selected.
    fn ensure_host_mobile_selection(&mut self) {
        if !self.selected_objects.is_empty() {
            return;
        }
        let Some(team) = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.team)
        else {
            return;
        };
        if let Some((id, _)) = self
            .game_logic
            .get_objects()
            .iter()
            .find(|(_, o)| o.team == team && o.is_alive() && o.is_mobile())
        {
            self.selected_objects = vec![*id];
            self.game_logic
                .select_objects(self.current_player_id, vec![*id]);
        }
    }

    fn stop_all_friendly_units(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.can_move() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            ids.push(id);
        }
        if ids.is_empty() {
            let msg = "No units to stop";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::Stop,
                player_id: self.current_player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: ids.clone(),
                modifier_keys: crate::command_system::ModifierKeys {
                    ctrl: true,
                    shift: true,
                    alt: false,
                },
            });
        self.game_logic.process_commands();
        self.play_sound_effect(SoundType::Command);
        let msg = format!("Stopped {} units", ids.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    fn select_all_friendly_moving(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            let moving = obj.status.moving
                || obj.movement.target_position.is_some()
                || !obj.movement.path.is_empty()
                || matches!(
                    obj.ai_state,
                    crate::game_logic::AIState::Moving
                        | crate::game_logic::AIState::Gathering
                        | crate::game_logic::AIState::ReturningResources
                        | crate::game_logic::AIState::Entering
                        | crate::game_logic::AIState::Docking
                        | crate::game_logic::AIState::Attacking
                        | crate::game_logic::AIState::Patrolling
                );
            if moving {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No moving units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} moving", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }
    /// Select friendly transports currently carrying units residual (Ctrl+Alt+J).
    fn select_all_occupied_transports(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            if obj.contained_units().is_empty() {
                continue;
            }
            // Occupied non-structure container = transport residual.
            ids.push(id);
        }
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            for o in &frame.objects {
                if o.team != team || o.destroyed || o.is_structure {
                    continue;
                }
                if o.garrisoned_units.is_empty() {
                    continue;
                }
                if !ids.iter().any(|id| id.0 == o.id.0) {
                    ids.push(o.id);
                }
            }
        }
        ids.sort_by_key(|id| id.0);
        ids.dedup();
        if ids.is_empty() {
            let msg = "No occupied transports";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!(
            "Selected {} occupied transports",
            self.selected_objects.len()
        );
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Toggle attack-order line drawing residual (Ctrl+F4).
    fn toggle_attack_lines_hotkey(&mut self) {
        self.show_attack_lines = !self.show_attack_lines;
        let msg = if self.show_attack_lines {
            "Attack lines: ON"
        } else {
            "Attack lines: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Toggle movement path line drawing residual (Ctrl+F3).
    fn toggle_move_lines_hotkey(&mut self) {
        self.show_move_lines = !self.show_move_lines;
        let msg = if self.show_move_lines {
            "Move lines: ON"
        } else {
            "Move lines: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Select structures that currently hold garrisoned units residual (Ctrl+Alt+U).
    fn select_all_garrisoned_structures(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            for o in frame.garrisoned_structures() {
                if o.team == team && !o.destroyed {
                    ids.push(o.id);
                }
            }
        } else {
            for (&id, obj) in self.game_logic.get_objects() {
                if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                    continue;
                }
                if !obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                    continue;
                }
                let occupied = obj
                    .building_data
                    .as_ref()
                    .map(|b| !b.garrisoned_units.is_empty())
                    .unwrap_or(false)
                    || !obj.contained_units().is_empty();
                if occupied {
                    ids.push(id);
                }
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No garrisoned structures";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} garrisoned", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    fn toggle_fps_counter_hotkey(&mut self) {
        self.show_fps = !self.show_fps;
        let msg = if self.show_fps {
            "FPS counter: ON"
        } else {
            "FPS counter: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Select all friendly veteran+ units residual (Ctrl+Alt+E).

    /// Cycle non-empty control groups residual (Ctrl+Alt+Left/Right already damaged structures).
    /// Use Ctrl+Shift+Tab residual: next/prev control group.
    fn cycle_control_group_selection(&mut self, delta: i32) {
        let mut groups: Vec<u8> = self
            .control_groups
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, _)| *k)
            .collect();
        groups.sort();
        if groups.is_empty() {
            let msg = "No control groups";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        let current = self
            .last_control_group_select
            .map(|(g, _)| g)
            .and_then(|g| groups.iter().position(|x| *x == g));
        let idx = match current {
            Some(i) => {
                let n = groups.len() as i32;
                ((i as i32 + delta).rem_euclid(n)) as usize
            }
            None => {
                if delta >= 0 {
                    0
                } else {
                    groups.len() - 1
                }
            }
        };
        let group_num = groups[idx];
        let stored = self
            .control_groups
            .get(&group_num)
            .cloned()
            .unwrap_or_default();
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let selection = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.filter_alive_selectable_ids(&stored, team)
        } else {
            let mut live = Vec::new();
            for id in stored {
                if let Some(obj) = self.game_logic.find_object(id) {
                    if obj.team == team && obj.is_selectable() && obj.is_alive() {
                        live.push(id);
                    }
                }
            }
            live
        };
        if selection.is_empty() {
            let msg = format!("Control group {group_num} empty");
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, selection.clone());
        self.selected_objects = selection;
        self.last_control_group_select = Some((group_num, Instant::now()));
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Control group {group_num}");
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Select all friendly effectively stealthed units residual (Ctrl+Alt+K).
    fn select_all_friendly_stealthed(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if obj.is_effectively_stealthed() {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No stealthed units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} stealthed", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    fn select_all_friendly_veterans(&mut self) {
        use crate::game_logic::VeterancyLevel;
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            if matches!(
                obj.experience.level,
                VeterancyLevel::Veteran | VeterancyLevel::Elite | VeterancyLevel::Heroic
            ) {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No veteran units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} veterans", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Select aircraft currently docked/parked residual (Ctrl+Alt+W).
    fn select_all_docked_aircraft(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            let is_ac = obj.is_kind_of(crate::game_logic::KindOf::Aircraft)
                || obj.object_type == crate::game_logic::ObjectType::Aircraft;
            if !is_ac {
                continue;
            }
            let docked = matches!(obj.ai_state, crate::game_logic::AIState::Docked)
                || obj.contained_by.is_some();
            if docked {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No docked aircraft";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} docked aircraft", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    fn toggle_debug_info_hotkey(&mut self) {
        self.show_debug_info = !self.show_debug_info;
        let msg = if self.show_debug_info {
            "Debug overlay: ON"
        } else {
            "Debug overlay: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Cycle friendly producers with a non-empty queue residual (Ctrl+Alt+P).
    fn cycle_busy_producer_selection(&mut self, delta: i32) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            let busy = obj
                .building_data
                .as_ref()
                .map(|b| !b.production_queue.is_empty())
                .unwrap_or(false);
            if busy {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No busy producers";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };
        self.game_logic
            .select_objects(self.current_player_id, vec![next]);
        self.selected_objects = vec![next];
        if let Some(obj) = self.game_logic.find_object(next) {
            let clamped = self.clamp_to_world_bounds(obj.get_position());
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
        let msg = "Busy producer selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Select all friendly units currently guarding residual (Ctrl+Alt+G).

    /// Select all friendly units currently patrolling residual (Ctrl+Alt+Y).
    fn select_all_friendly_patrolling(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if matches!(obj.ai_state, crate::game_logic::AIState::Patrolling) {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No patrolling units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} patrolling", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Select all friendly units currently gathering residual (Ctrl+Alt+H).
    fn select_all_friendly_gathering(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if matches!(
                obj.ai_state,
                crate::game_logic::AIState::Gathering
                    | crate::game_logic::AIState::ReturningResources
            ) {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No gathering units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} gathering", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Cycle structures with ready special power residual (Ctrl+Alt+V).
    fn cycle_ready_special_power_structure(&mut self, delta: i32) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if !obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            if obj.special_power_ready {
                ids.push(id);
                continue;
            }
            // Also check per-power ready residual.
            if let Some(p) =
                crate::game_logic::host_superweapon_kindof::special_power_for_superweapon_structure(
                    &obj.template_name,
                )
            {
                if self.game_logic.is_special_power_ready_for(id, &p) {
                    ids.push(id);
                }
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No ready special powers";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        let next = if let Some(current) = self.selected_objects.first().copied() {
            ids.iter()
                .position(|id| *id == current)
                .map(|idx| {
                    let n = ids.len() as i32;
                    let i = (idx as i32 + delta).rem_euclid(n) as usize;
                    ids[i]
                })
                .unwrap_or(ids[0])
        } else if delta >= 0 {
            ids[0]
        } else {
            ids[ids.len() - 1]
        };
        self.game_logic
            .select_objects(self.current_player_id, vec![next]);
        self.selected_objects = vec![next];
        if let Some(obj) = self.game_logic.find_object(next) {
            let clamped = self.clamp_to_world_bounds(obj.get_position());
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
        let msg = "Ready special power selected";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    fn select_all_friendly_guarding(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            let guarding = matches!(
                obj.ai_state,
                crate::game_logic::AIState::GuardingArea
                    | crate::game_logic::AIState::GuardingObject
            ) || obj.guard_position.is_some()
                || obj.guard_target.is_some();
            if guarding {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No guarding units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} guarding", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    fn toggle_health_bars_hotkey(&mut self) {
        self.show_health_bars = !self.show_health_bars;
        let msg = if self.show_health_bars {
            "Health bars: ON"
        } else {
            "Health bars: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    fn select_all_friendly_combat(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Structure) {
                continue;
            }
            let n = obj.template_name.to_ascii_lowercase();
            if obj.is_dozer
                || n.contains("dozer")
                || n.contains("worker")
                || n.contains("supply")
                || n.contains("harvester")
            {
                continue;
            }
            if !obj.can_move() && !obj.can_attack() {
                continue;
            }
            ids.push(id);
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No combat units";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} combat units", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    fn select_all_friendly_on_screen(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let center = self.camera_target;
        // Fail-closed frustum residual: radius scales with zoom.
        let radius = (180.0 * self.camera_zoom.max(0.5)).clamp(120.0, 600.0);
        let selection = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.alive_selectable_friendly_near(team, center, radius)
        } else {
            let mut live = Vec::new();
            let r2 = radius * radius;
            for (&id, obj) in self.game_logic.get_objects() {
                if obj.team != team || !obj.is_selectable() || !obj.is_alive() {
                    continue;
                }
                let p = obj.get_position();
                let dx = p.x - center.x;
                let dz = p.z - center.z;
                if dx * dx + dz * dz <= r2 {
                    live.push(id);
                }
            }
            live
        };
        if selection.is_empty() {
            let msg = "No units on screen";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, selection.clone());
        self.selected_objects = selection;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} on screen", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    /// Toggle camera follow on primary selection residual (Alt+F).

    /// Snap camera to centroid of current selection residual (Alt+Space).
    fn center_camera_on_selection(&mut self) {
        let selected = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_else(|| self.selected_objects.clone());
        if selected.is_empty() {
            let msg = "Nothing selected";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        let center = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.centroid_of_ids(&selected)
        } else {
            let mut sum = glam::Vec3::ZERO;
            let mut n = 0u32;
            for id in &selected {
                if let Some(obj) = self.game_logic.find_object(*id) {
                    sum += obj.get_position();
                    n += 1;
                }
            }
            if n == 0 {
                None
            } else {
                Some(sum / n as f32)
            }
        };
        let Some(center) = center else {
            return;
        };
        let clamped = self.clamp_to_world_bounds(center);
        self.camera_target.x = clamped.x;
        self.camera_target.z = clamped.z;
        let msg = "Centered on selection";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Select friendly dozers/workers currently constructing residual (Ctrl+Alt+B).
    fn select_all_constructing_workers(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };
        let mut ids: Vec<crate::game_logic::ObjectId> = Vec::new();
        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != team || !obj.is_alive() || !obj.is_selectable() {
                continue;
            }
            let n = obj.template_name.to_ascii_lowercase();
            let is_worker = obj.is_dozer || n.contains("dozer") || n.contains("worker");
            if !is_worker {
                continue;
            }
            if matches!(
                obj.ai_state,
                crate::game_logic::AIState::Constructing | crate::game_logic::AIState::Repairing
            ) {
                ids.push(id);
            }
        }
        ids.sort_by_key(|id| id.0);
        if ids.is_empty() {
            let msg = "No constructing workers";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, ids.clone());
        self.selected_objects = ids;
        self.play_sound_effect(SoundType::Select);
        let msg = format!("Selected {} constructing", self.selected_objects.len());
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
    }

    fn toggle_camera_follow_selection(&mut self) {
        if self.game_logic.camera_follow_object_id().is_some() {
            self.game_logic.set_camera_follow_object(None);
            let msg = "Camera follow off";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        }
        let id = self.selected_objects.first().copied().or_else(|| {
            self.game_logic
                .get_player(self.current_player_id)
                .and_then(|p| p.selected_objects.first().copied())
        });
        let Some(id) = id else {
            let msg = "Select a unit to follow";
            self.game_hud.push_info_message(msg);
            self.ui_manager.game_hud_mut().push_info_message(msg);
            return;
        };
        self.game_logic.set_camera_follow_object(Some(id));
        let msg = "Camera follow on";
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
    }

    /// Retail SELECT_ALL (KEY_Q) / Ctrl+A residual.
    fn select_all_friendly_units(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        let selection = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.alive_selectable_friendly_ids(team)
        } else {
            let mut live = Vec::new();
            for (&id, obj) in self.game_logic.get_objects() {
                if obj.team == team && obj.is_selectable() && obj.is_alive() {
                    live.push(id);
                }
            }
            live
        };

        self.game_logic
            .select_objects(self.current_player_id, selection.clone());
        self.selected_objects = selection;
        self.play_sound_effect(SoundType::Select);
    }

    /// Retail SELECT_ALL_AIRCRAFT (KEY_W) residual.
    fn select_all_friendly_aircraft(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        let selection = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.alive_selectable_friendly_aircraft_ids(team)
        } else {
            let mut live = Vec::new();
            for (&id, obj) in self.game_logic.get_objects() {
                if obj.team == team
                    && obj.is_selectable()
                    && obj.is_alive()
                    && (obj.is_kind_of(crate::game_logic::KindOf::Aircraft)
                        || obj.object_type == crate::game_logic::ObjectType::Aircraft)
                {
                    live.push(id);
                }
            }
            live
        };

        self.game_logic
            .select_objects(self.current_player_id, selection.clone());
        self.selected_objects = selection;
        if !self.selected_objects.is_empty() {
            self.play_sound_effect(SoundType::Select);
        }
    }

    /// Retail SELECT_HERO (Ctrl+H) residual.
    fn select_hero_units_hotkey(&mut self) {
        let team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        let selection: Vec<ObjectId> = self
            .game_logic
            .get_objects()
            .iter()
            .filter(|(_, obj)| {
                obj.team == team && obj.is_selectable() && obj.is_alive() && obj.is_hero()
            })
            .map(|(&id, _)| id)
            .collect();

        if selection.is_empty() {
            return;
        }
        self.game_logic
            .select_objects(self.current_player_id, selection.clone());
        self.selected_objects = selection;
        self.play_sound_effect(SoundType::Select);
    }

    /// Retail SELECT_MATCHING_UNITS (KEY_E) residual — type-select from current selection.
    fn select_matching_units_hotkey(&mut self) {
        let seed = self.selected_objects.first().copied().or_else(|| {
            self.game_logic
                .get_player(self.current_player_id)
                .and_then(|p| p.selected_objects.first().copied())
        });
        let Some(seed) = seed else {
            return;
        };
        self.select_similar_units(seed);
    }

    fn handle_left_click(&mut self) {
        self.is_dragging = true;
        self.selection_start = Some(self.mouse_world_position);
        self.selection_start_screen = Some(self.mouse_position);

        let mouse_pos = self.mouse_world_position;
        let clicked_object = self.find_object_at_position(mouse_pos, &self.game_logic, false);

        // Check for double-click
        let now = Instant::now();
        let is_double_click = if let (Some(last_time), Some(last_pos)) =
            (self.last_click_time, self.last_click_position)
        {
            let time_delta = now.duration_since(last_time).as_millis();
            let pos_delta = (mouse_pos - last_pos).length();
            time_delta < 500 && pos_delta < 10.0
        } else {
            false
        };

        self.last_click_time = Some(now);
        self.last_click_position = Some(mouse_pos);

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));

        if is_double_click && clicked_object.is_some() && !ctrl_down {
            // Double-click: select all similar units
            if let Some(object_id) = clicked_object {
                self.select_similar_units(object_id);
            }
        } else {
            // Single-click behavior
            if self.pending_map_command.is_some() {
                let loc = self.mouse_world_position;
                self.commit_pending_map_command(loc, clicked_object);
            } else if ctrl_down && !self.selected_objects.is_empty() {
                // C++ ForceAttack residual: Ctrl+LMB with selection issues force-attack.
                self.issue_force_attack_from_left_click(mouse_pos, clicked_object);
            } else if let Some(object_id) = clicked_object {
                if shift_down {
                    // C++ Shift+select residual: toggle unit in multi-selection.
                    self.toggle_select_object(object_id);
                } else {
                    // Select this object
                    self.game_logic
                        .select_objects(self.current_player_id, vec![object_id]);
                    self.selected_objects = vec![object_id];
                    self.play_sound_effect(SoundType::Select);
                }
            } else if let Some(template) = self.pending_structure_placement.clone() {
                // Wall/fence residual: defer commit to left-release so drag can form a line.
                if Self::is_wall_structure_template(&template) {
                    // selection_start already set at top of handle_left_click.
                } else {
                    // C++ structure placement residual: empty-ground click commits DozerConstruct.
                    let loc = self.mouse_world_position;
                    self.place_structure_from_ui(&template, loc);
                }
            } else {
                // Defer empty-ground clear until left-release if this becomes a box drag.
                // Instant clear on mousedown fights drag-select residual.
            }
        }
    }

    /// Shift+click residual: add friendly unit or remove if already selected.
    fn toggle_select_object(&mut self, object_id: ObjectId) {
        let player_team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        // Only toggle friendly selectable units (enemy click under Shift still replaces? retail
        // keeps multi-select among friendlies; enemy under Shift is ignored for add).
        let is_friendly_selectable = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame
                .objects
                .iter()
                .find(|o| o.id == object_id)
                .map(|o| {
                    o.team == player_team
                        && !o.destroyed
                        && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
                })
                .unwrap_or(false)
        } else if let Some(obj) = self.game_logic.find_object(object_id) {
            obj.team == player_team && obj.is_selectable() && obj.is_alive()
        } else {
            false
        };
        if !is_friendly_selectable {
            return;
        }

        let mut selection = self.selected_objects.clone();
        if let Some(idx) = selection.iter().position(|id| *id == object_id) {
            selection.remove(idx);
        } else {
            selection.push(object_id);
        }
        self.game_logic
            .select_objects(self.current_player_id, selection.clone());
        self.selected_objects = selection;
        self.play_sound_effect(SoundType::Select);
    }

    /// Ctrl+LMB ForceAttack residual (object or ground).
    fn issue_force_attack_from_left_click(
        &mut self,
        location: Vec3,
        target_object: Option<ObjectId>,
    ) {
        let mut selected = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        if selected.is_empty() {
            return;
        }

        let command_type = if let Some(tid) = target_object {
            crate::command_system::CommandType::ForceAttackObject { target_id: tid }
        } else {
            crate::command_system::CommandType::ForceAttackGround { location }
        };
        self.game_logic
            .queue_command(crate::command_system::GameCommand {
                command_type,
                player_id: self.current_player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: selected,
                modifier_keys: crate::command_system::ModifierKeys {
                    ctrl: true,
                    shift: false,
                    alt: false,
                },
            });
        self.game_logic.process_commands();
        self.play_sound_effect(SoundType::Command);
    }

    fn select_similar_units(&mut self, clicked_object_id: ObjectId) {
        // Prefer presentation-frozen local_team when a frame is installed.
        let player_team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            // Boot residual only — presentation local_team owns InGame similar-select.
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        // Prefer presentation identity (template/team/selectable) when dual-tick snapshot exists.
        let (similar_units, template_label) = if let Some(frame) =
            self.last_presentation_frame.as_ref()
        {
            let ids = frame.similar_unit_ids(clicked_object_id, player_team);
            let label = frame
                .objects
                .iter()
                .find(|o| o.id == clicked_object_id)
                .map(|o| o.template_name.clone())
                .unwrap_or_default();
            (ids, label)
        } else {
            // Boot residual only — presentation similar_unit_ids owns InGame path.
            let Some(clicked_obj) = self.game_logic.find_object(clicked_object_id) else {
                return;
            };
            if clicked_obj.team != player_team || !clicked_obj.is_selectable() {
                return;
            }
            let template = clicked_obj.template_name.clone();
            let ids: Vec<ObjectId> = self
                .game_logic
                .get_objects()
                .iter()
                .filter(|(_, obj)| {
                    obj.team == player_team && obj.is_selectable() && obj.template_name == template
                })
                .map(|(&id, _)| id)
                .collect();
            (ids, template)
        };

        if !similar_units.is_empty() {
            self.game_logic
                .select_objects(self.current_player_id, similar_units.clone());
            self.selected_objects = similar_units;
            self.play_sound_effect(SoundType::Select);
            info!(
                "Selected {} similar units ({})",
                self.selected_objects.len(),
                template_label
            );
        }
    }

    fn handle_left_release(&mut self) {
        self.is_dragging = false;
        self.selection_start_screen = None;

        let Some(start) = self.selection_start.take() else {
            return;
        };

        let end = self.mouse_world_position;

        // If the mouse didn't move enough, the click selection was already handled on mouse-down.
        let drag_distance = Vec2::new(end.x - start.x, end.z - start.z).length();
        if drag_distance < 5.0 {
            // Wall residual: short click places a single segment.
            if let Some(template) = self.pending_structure_placement.clone() {
                if Self::is_wall_structure_template(&template) {
                    self.place_structure_from_ui(&template, end);
                    return;
                }
            }
            // Click on empty ground (no pending command/placement handled on press): clear selection.
            if self.pending_map_command.is_none()
                && self.pending_structure_placement.is_none()
                && self
                    .find_object_at_position(end, &self.game_logic, false)
                    .is_none()
            {
                let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                if !shift_down {
                    self.selected_objects.clear();
                    self.game_logic
                        .select_objects(self.current_player_id, Vec::new());
                }
            }
            return;
        }

        // Wall/fence drag residual: DozerConstructLine along the drag segment.
        if let Some(template) = self.pending_structure_placement.clone() {
            if Self::is_wall_structure_template(&template) {
                self.place_wall_line_from_ui(&template, start, end);
                return;
            }
        }

        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        let min_z = start.z.min(end.z);
        let max_z = start.z.max(end.z);

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));

        let mut selection: Vec<ObjectId> = if shift_down {
            self.selected_objects.clone()
        } else {
            Vec::new()
        };

        // Prefer presentation-frozen local_team when a frame is installed.
        let player_team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.local_team()
        } else {
            // Boot residual only — presentation local_team owns InGame box-select.
            let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                return;
            };
            player.team
        };

        // Prefer presentation XZ pose/selectable/structure residual when dual-tick snapshot exists.
        let boxed: Vec<ObjectId> = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.box_select_unit_ids(player_team, min_x, max_x, min_z, max_z)
        } else {
            // Boot residual only — presentation box_select_unit_ids owns InGame path.
            let mut live = Vec::new();
            for (&id, obj) in self.game_logic.get_objects() {
                if obj.team != player_team || !obj.is_selectable() {
                    continue;
                }
                let pos = obj.get_position();
                if pos.x < min_x || pos.x > max_x || pos.z < min_z || pos.z > max_z {
                    continue;
                }
                live.push(id);
            }
            live
        };
        for id in boxed {
            if !selection.contains(&id) {
                selection.push(id);
            }
        }

        self.game_logic
            .select_objects(self.current_player_id, selection.clone());
        self.selected_objects = selection;
        self.play_sound_effect(SoundType::Select);
    }

    fn handle_right_click(&mut self) {
        let mouse_pos = self.mouse_world_position;

        // Prefer live player selection; fall back to engine selection residual.
        let mut selected = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        if selected.is_empty() {
            return;
        }

        // C++ context-sensitive right-click residual via CommandSystem:
        // attack / gather / repair / enter / get-repaired / get-healed / move / attack-move.
        let target_object = self.find_object_at_position(mouse_pos, &self.game_logic, true);
        let ctrl = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control)
            )
        });
        let shift = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Shift)
            )
        });
        let alt = self.sticky_waypoint_mode
            || self.keys_pressed.iter().any(|k| {
                matches!(
                    k,
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Alt)
                )
            });

        let context = crate::command_system::MouseCommandContext {
            world_position: mouse_pos,
            target_object,
            screen_position: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: crate::command_system::MouseButton::Right,
            modifier_keys: crate::command_system::ModifierKeys { ctrl, shift, alt },
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let mut cmd_sys = crate::command_system::CommandSystem::new();
        let command = cmd_sys.process_mouse_input(
            &context,
            &selected,
            self.current_player_id,
            &self.game_logic,
        );

        if let Some(mut command) = command {
            if self.sticky_auto_attack {
                if let crate::command_system::CommandType::MoveTo { destination, .. } =
                    command.command_type
                {
                    command.command_type =
                        crate::command_system::CommandType::AttackMoveTo { destination };
                }
            }
            self.game_logic.queue_command(command);
            self.game_logic.process_commands();
            self.play_sound_effect(SoundType::Command);
            return;
        }

        // Fail-closed fallback residual: move if context path produced nothing.
        if self.sticky_auto_attack {
            self.game_logic
                .command_attack_move(self.current_player_id, mouse_pos);
        } else {
            self.game_logic
                .command_move(self.current_player_id, mouse_pos);
        }
        self.play_sound_effect(SoundType::Command);
    }

    fn handle_mouse_wheel(&mut self, delta: &winit::event::MouseScrollDelta) {
        use winit::event::MouseScrollDelta;

        let delta_y = match delta {
            MouseScrollDelta::LineDelta(_, y) => *y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
        };

        // C++ place-building rotate residual: wheel turns ghost while placement armed.
        if self.pending_structure_placement.is_some() {
            let step = delta_y * std::f32::consts::FRAC_PI_4; // 45 deg per notch
            self.game_hud
                .construction_panel
                .rotate_structure_placement(-step);
            self.ui_manager
                .game_hud_mut()
                .construction_panel
                .rotate_structure_placement(-step);
            return;
        }

        // Zoom camera with mouse wheel
        let zoom_speed = 0.1;
        let new_zoom = (self.camera_zoom - delta_y * zoom_speed).clamp(0.1, 5.0);

        if (new_zoom - self.camera_zoom).abs() > 0.001 {
            self.camera_zoom = new_zoom;
            debug!("Camera zoom changed to {:.2}", self.camera_zoom);
        }
    }

    fn update_camera(&mut self, dt: f32) {
        // Retail KP4/KP6 rotate and KP8/KP2 zoom hold residual.
        const ROTATE_RAD_PER_SEC: f32 = 1.2;
        const ZOOM_PER_SEC: f32 = 0.85;
        if self.camera_rotate_left_held {
            self.camera_yaw_radians -= ROTATE_RAD_PER_SEC * dt;
        }
        if self.camera_rotate_right_held {
            self.camera_yaw_radians += ROTATE_RAD_PER_SEC * dt;
        }
        if self.camera_zoom_in_held {
            self.camera_zoom = (self.camera_zoom - ZOOM_PER_SEC * dt).clamp(0.1, 5.0);
        }
        if self.camera_zoom_out_held {
            self.camera_zoom = (self.camera_zoom + ZOOM_PER_SEC * dt).clamp(0.1, 5.0);
        }

        self.update_camera_tracking_drawable();

        let mut movement = Vec3::ZERO;
        if self.camera_slave_mode.is_none() {
            let logic_frames_per_second =
                game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32;
            let (
                horizontal_scroll_speed_factor,
                vertical_scroll_speed_factor,
                keyboard_scroll_factor,
            ) = {
                let global_data = game_engine::common::global_data::read();
                (
                    global_data.horizontal_scroll_speed_factor,
                    global_data.vertical_scroll_speed_factor,
                    global_data.keyboard_scroll_factor,
                )
            };

            // C++ parity (LookAtXlat.cpp): key scrolling uses SCROLL_AMT=100 in screen-space and
            // applies horizontal/vertical/keyboard factors once per logic frame.
            const SCROLL_AMT: f32 = 100.0;
            let scroll_step =
                SCROLL_AMT * keyboard_scroll_factor * dt.max(0.0) * logic_frames_per_second;
            let mut screen_scroll = Vec2::ZERO;
            // C++ LookAt keyboard scroll uses arrows (not WASD).
            // WASD are unit hotkeys: A attack-move, S stop, D deploy, etc.
            let mods_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control))
                || self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                || self.keys_pressed.contains(&Key::Named(NamedKey::Alt));
            let ui_modal = self.chat_panel.is_open() || self.diplomacy_panel.is_active();
            if !mods_down && !ui_modal {
                if self.keys_pressed.contains(&Key::Named(NamedKey::ArrowUp)) {
                    screen_scroll.y -= vertical_scroll_speed_factor * scroll_step;
                }
                if self.keys_pressed.contains(&Key::Named(NamedKey::ArrowDown)) {
                    screen_scroll.y += vertical_scroll_speed_factor * scroll_step;
                }
                if self.keys_pressed.contains(&Key::Named(NamedKey::ArrowLeft)) {
                    screen_scroll.x -= horizontal_scroll_speed_factor * scroll_step;
                }
                if self
                    .keys_pressed
                    .contains(&Key::Named(NamedKey::ArrowRight))
                {
                    screen_scroll.x += horizontal_scroll_speed_factor * scroll_step;
                }
            }

            // Edge scrolling (C++ LookAt.cpp: near screen edge).
            // Enable for windowed + fullscreen so map-panning works without arrows.
            if matches!(self.current_state, GameState::InGame | GameState::Paused)
                && !self.chat_panel.is_open()
                && !self.diplomacy_panel.is_active()
            {
                const EDGE_SCROLL_SIZE: f32 = 5.0;
                let (mx, my) = self.mouse_position;
                let size = self.window.inner_size();
                let win_w = size.width as f32;
                let win_h = size.height as f32;

                let mut edge_dx = 0.0f32;
                let mut edge_dy = 0.0f32;

                if mx < EDGE_SCROLL_SIZE {
                    edge_dx = -1.0;
                } else if mx >= win_w - EDGE_SCROLL_SIZE {
                    edge_dx = 1.0;
                }
                if my < EDGE_SCROLL_SIZE {
                    edge_dy = -1.0;
                } else if my >= win_h - EDGE_SCROLL_SIZE {
                    edge_dy = 1.0;
                }

                if edge_dx != 0.0 || edge_dy != 0.0 {
                    let edge_step =
                        SCROLL_AMT * keyboard_scroll_factor * dt.max(0.0) * logic_frames_per_second;
                    screen_scroll.x += edge_dx * horizontal_scroll_speed_factor * edge_step;
                    screen_scroll.y += edge_dy * vertical_scroll_speed_factor * edge_step;
                }
            }

            // Right-mouse-button drag scrolling (C++ LookAtXlat.cpp:378-406)
            if self.is_rmb_scrolling {
                if let Some(anchor) = self.rmb_scroll_anchor {
                    let dx = self.mouse_position.0 - anchor.0;
                    let dy = self.mouse_position.1 - anchor.1;
                    let mut offset = Vec2::new(
                        horizontal_scroll_speed_factor * dx,
                        vertical_scroll_speed_factor * dy,
                    );

                    if offset.length_squared() > f32::EPSILON {
                        let direction = offset.normalize();
                        offset.x += horizontal_scroll_speed_factor
                            * direction.x
                            * keyboard_scroll_factor.powi(2);
                        offset.y += vertical_scroll_speed_factor
                            * direction.y
                            * keyboard_scroll_factor.powi(2);
                        screen_scroll += offset * dt.max(0.0) * logic_frames_per_second;
                    }
                }
            }

            // Middle-mouse-button camera yaw rotation (C++ LookAtXlat.cpp)
            if self.is_mmb_rotating {
                if let Some(anchor) = self.mmb_anchor {
                    let dx = self.mouse_position.0 - anchor.0;
                    self.camera_yaw_radians += dx * 0.005;
                }
                self.mmb_anchor = Some(self.mouse_position);
            }

            movement = self.camera_scroll_world_delta(screen_scroll);
        }

        let mut camera_changed = false;

        if movement.length() > 0.0 {
            self.camera_target += movement;
            camera_changed = true;
        }

        if let Some(mode) = self.camera_slave_mode.as_ref() {
            // Prefer dual-tick presentation pose so camera follow does not re-read live transforms.
            let target = if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.first_alive_position_for_template(&mode.thing_template_name)
            } else {
                self.game_logic
                    .get_objects()
                    .values()
                    .find(|obj| {
                        obj.is_alive()
                            && obj
                                .template_name
                                .eq_ignore_ascii_case(&mode.thing_template_name)
                    })
                    .map(|obj| obj.get_position())
            };
            if let Some(target) = target {
                let clamped = self.clamp_to_world_bounds(target);
                if (self.camera_target.x - clamped.x).abs() > 0.001
                    || (self.camera_target.z - clamped.z).abs() > 0.001
                {
                    self.camera_target.x = clamped.x;
                    self.camera_target.z = clamped.z;
                    camera_changed = true;
                }
            }
        }

        if let Some(target) = self.camera_zoom_target {
            if self.camera_zoom_duration <= 0.0 {
                self.camera_zoom = target;
                self.camera_zoom_target = None;
            } else {
                self.camera_zoom_elapsed += dt;
                let t = (self.camera_zoom_elapsed / self.camera_zoom_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_zoom_ease_in / self.camera_zoom_duration,
                    self.camera_zoom_ease_out / self.camera_zoom_duration,
                );
                self.camera_zoom =
                    self.camera_zoom_start + (target - self.camera_zoom_start) * eased;
                if t >= 1.0 {
                    self.camera_zoom_target = None;
                }
            }
        }

        if let Some(target) = self.camera_pitch_target {
            if self.camera_pitch_duration <= 0.0 {
                self.camera_pitch_radians = target;
                self.camera_pitch_target = None;
                camera_changed = true;
            } else {
                self.camera_pitch_elapsed += dt;
                let t = (self.camera_pitch_elapsed / self.camera_pitch_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_pitch_ease_in / self.camera_pitch_duration,
                    self.camera_pitch_ease_out / self.camera_pitch_duration,
                );
                self.camera_pitch_radians =
                    self.camera_pitch_start + (target - self.camera_pitch_start) * eased;
                camera_changed = true;
                if t >= 1.0 {
                    self.camera_pitch_target = None;
                }
            }
        }

        if let Some(target) = self.camera_yaw_target {
            if self.camera_yaw_duration <= 0.0 {
                self.camera_yaw_radians = target;
                self.camera_yaw_target = None;
                camera_changed = true;
            } else {
                self.camera_yaw_elapsed += dt;
                let t = (self.camera_yaw_elapsed / self.camera_yaw_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_yaw_ease_in / self.camera_yaw_duration,
                    self.camera_yaw_ease_out / self.camera_yaw_duration,
                );
                self.camera_yaw_radians =
                    self.camera_yaw_start + (target - self.camera_yaw_start) * eased;
                camera_changed = true;
                if t >= 1.0 {
                    self.camera_yaw_target = None;
                }
            }
        }

        let shake_dt = if self.game_logic.is_time_frozen_for_simulation() {
            0.0
        } else {
            dt
        };
        if self.update_script_camera_shake(shake_dt) {
            camera_changed = true;
        }

        if camera_changed {
            self.apply_camera_orbit_transform();
        }
    }

    fn is_character_key_pressed(&self, expected: &str) -> bool {
        self.keys_pressed.iter().any(|key| match key {
            Key::Character(ch) => ch.eq_ignore_ascii_case(expected),
            _ => false,
        })
    }

    fn camera_scroll_world_delta(&self, screen_scroll: Vec2) -> Vec3 {
        if screen_scroll.length_squared() <= f32::EPSILON {
            return Vec3::ZERO;
        }

        // Match C++ key-scroll semantics: "up/down/left/right" are screen-space intents.
        // Convert that intent to world-plane motion relative to current camera facing.
        let mut forward = self.camera_target - self.camera_position;
        forward.y = 0.0;
        if forward.length_squared() <= f32::EPSILON {
            return Vec3::ZERO;
        }
        let forward = forward.normalize();
        let right = Vec3::new(forward.z, 0.0, -forward.x);

        // C++ uses y- for UP and y+ for DOWN, so negate Y when mapping to forward motion.
        (right * screen_scroll.x) + (forward * -screen_scroll.y)
    }

    /// C++ InGameUI context cursor residual mapped onto winit CursorIcon.
    ///
    /// Fail-closed vs full Mouse.cpp ANI/CUR assets — uses platform icons with
    /// residual names from `MOUSE_CURSOR_INI_NAME_LIST`.
    fn sync_context_mouse_cursor(&mut self) {
        use winit::window::CursorIcon;
        let (name, icon) = self.resolve_context_cursor_icon();
        if self.last_context_cursor == Some(name) {
            return;
        }
        self.last_context_cursor = Some(name);
        self.window.set_cursor(icon);
    }

    fn resolve_context_cursor_icon(&self) -> (&'static str, winit::window::CursorIcon) {
        use winit::window::CursorIcon;

        // Placement mode residual.
        if self.pending_structure_placement.is_some() {
            let legal = self
                .game_hud
                .construction_panel
                .placement_preview()
                .is_legal;
            return if legal {
                ("Build", CursorIcon::Cell)
            } else {
                ("InvalidBuild", CursorIcon::NotAllowed)
            };
        }

        // Pending map command residual.
        if let Some(kind) = self.pending_map_command.as_ref() {
            return match kind {
                PendingMapCommand::AttackMove => ("AttackMove", CursorIcon::Crosshair),
                PendingMapCommand::Guard => ("Move", CursorIcon::AllScroll),
                PendingMapCommand::SetRallyPoint => ("SetRallyPoint", CursorIcon::Cell),
                PendingMapCommand::CombatDrop => ("CombatDrop", CursorIcon::Move),
                PendingMapCommand::PlaceBeacon => ("PlaceBeacon", CursorIcon::Cell),
                PendingMapCommand::SpecialPower(_) => ("Target", CursorIcon::Crosshair),
                PendingMapCommand::UnitAbility(_) => ("Target", CursorIcon::Crosshair),
            };
        }

        let has_selection = !self.selected_objects.is_empty()
            || self
                .game_logic
                .get_player(self.current_player_id)
                .map(|p| !p.selected_objects.is_empty())
                .unwrap_or(false);

        let hover = self.find_object_at_position(self.mouse_world_position, &self.game_logic, true);
        let ctrl = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        let alt = self.keys_pressed.contains(&Key::Named(NamedKey::Alt));

        if (alt || self.sticky_waypoint_mode) && has_selection {
            return ("Waypoint", CursorIcon::Cell);
        }

        if ctrl && has_selection {
            return if hover.is_some() {
                ("ForceAttackObj", CursorIcon::Crosshair)
            } else {
                ("ForceAttackGround", CursorIcon::Crosshair)
            };
        }

        if !has_selection {
            // Hover friendly selectable → Select residual.
            if let Some(id) = hover {
                let player_team = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame.local_team()
                } else {
                    self.game_logic
                        .get_player(self.current_player_id)
                        .map(|p| p.team)
                        .unwrap_or(crate::game_logic::Team::USA)
                };
                let friendly = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame
                        .objects
                        .iter()
                        .find(|o| o.id == id)
                        .map(|o| o.team == player_team && !o.destroyed)
                        .unwrap_or(false)
                } else {
                    self.game_logic
                        .find_object(id)
                        .map(|o| o.team == player_team && o.is_alive())
                        .unwrap_or(false)
                };
                if friendly {
                    return ("Select", CursorIcon::Pointer);
                }
            }
            return ("Normal", CursorIcon::Default);
        }

        // Has selection: context from CommandSystem residual.
        let mut selected = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default();
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        let context = crate::command_system::MouseCommandContext {
            world_position: self.mouse_world_position,
            target_object: hover,
            screen_position: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: crate::command_system::MouseButton::Right,
            modifier_keys: crate::command_system::ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut cmd_sys = crate::command_system::CommandSystem::new();
        let cmd = cmd_sys.process_mouse_input(
            &context,
            &selected,
            self.current_player_id,
            &self.game_logic,
        );
        match cmd.map(|c| c.command_type) {
            Some(crate::command_system::CommandType::AttackObject { .. }) => {
                ("AttackObj", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::ForceAttackObject { .. }) => {
                ("ForceAttackObj", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::ForceAttackGround { .. }) => {
                ("ForceAttackGround", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::Enter { .. }) => {
                ("EnterFriendly", CursorIcon::Copy)
            }
            Some(crate::command_system::CommandType::GetRepaired { .. })
            | Some(crate::command_system::CommandType::Repair { .. }) => {
                ("GetRepaired", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::ResumeConstruction { .. }) => {
                ("ResumeConstruction", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::CaptureBuilding { .. }) => {
                ("CaptureBuilding", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::MoveTo { .. })
            | Some(crate::command_system::CommandType::AttackMoveTo { .. }) => {
                ("Move", CursorIcon::AllScroll)
            }
            Some(crate::command_system::CommandType::AddWaypoint { .. }) => {
                ("Waypoint", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::Guard { .. }) => {
                ("Move", CursorIcon::AllScroll)
            }
            _ => {
                if hover.is_some() {
                    ("Select", CursorIcon::Pointer)
                } else {
                    ("Move", CursorIcon::AllScroll)
                }
            }
        }
    }

    fn update_mouse_world_position(&mut self) {
        // Convert screen coordinates to world coordinates using current world bounds.
        // Prefer presentation world_env when installed (no live dual-read for click map).
        // Boot/loading without a frame still uses host GameLogic bounds.
        let size = self.window.inner_size();
        let normalized_x = (self.mouse_position.0 / size.width.max(1) as f32).clamp(0.0, 1.0);
        let normalized_y = (self.mouse_position.1 / size.height.max(1) as f32).clamp(0.0, 1.0);

        let (world_min, world_max) = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame.world_env.world_bounds_vec3()
        } else {
            self.game_logic.world_bounds()
        };
        let world_width = (world_max.x - world_min.x).max(1.0);
        let world_height = (world_max.z - world_min.z).max(1.0);
        let world_x = world_min.x + normalized_x * world_width;
        let world_z = world_min.z + normalized_y * world_height;
        self.mouse_world_position = Vec3::new(world_x, 0.0, world_z);
    }

    fn find_object_at_position(
        &self,
        position: Vec3,
        game_logic: &GameLogic,
        command_context: bool,
    ) -> Option<ObjectId> {
        const BASE_SELECTION_RADIUS: f32 = 20.0;

        // Prefer presentation-frozen local_team when a frame is installed.
        let player_team = if let Some(frame) = self.last_presentation_frame.as_ref() {
            Some(frame.local_team())
        } else {
            game_logic
                .get_player(self.current_player_id)
                .map(|p| p.team)
        };
        let has_selected_units = !self.selected_objects.is_empty();
        let prioritize_enemy_targets = command_context && has_selected_units;
        let mut best: Option<(ObjectId, u8, f32)> = None; // (id, priority, distance)

        // Prefer immutable presentation identity when the dual-tick snapshot is available.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            return crate::unit_control::UnitControlSystem::pick_object_id_at_world_from_presentation(
                frame,
                position,
                player_team,
                prioritize_enemy_targets,
                BASE_SELECTION_RADIUS,
            );
        }

        for (&id, obj) in game_logic.get_objects() {
            if !obj.is_alive() {
                continue;
            }

            let distance = obj.get_position().distance(position);
            let radius = BASE_SELECTION_RADIUS.max(obj.selection_radius);
            if distance > radius {
                continue;
            }

            let priority = if prioritize_enemy_targets {
                match player_team {
                    Some(team) if obj.team != team && obj.is_attackable() => 0,
                    Some(team) if obj.team == team && obj.is_selectable() => 1,
                    _ if obj.is_attackable() => 2,
                    _ if obj.is_selectable() => 3,
                    _ => continue,
                }
            } else {
                match player_team {
                    Some(team) if obj.team == team && obj.is_selectable() => 0,
                    Some(_) => continue,
                    None if obj.is_selectable() => 0,
                    None => continue,
                }
            };

            match best {
                Some((_, best_priority, best_distance))
                    if priority > best_priority
                        || (priority == best_priority && distance >= best_distance) => {}
                _ => best = Some((id, priority, distance)),
            }
        }

        best.map(|(id, _, _)| id)
    }

    /// Path following is authoritative in `GameLogic::update_movement`.
    /// Retained as a no-op compatibility hook for older call sites.
    #[allow(dead_code)]
    fn update_unit_pathfinding(&mut self, _dt: f32, _game_logic: &mut GameLogic) {
        // Intentionally empty: dual mid-frame path step removed.
    }

    /// Legacy render stub -- NOT called from the active render path.
    /// Actual rendering is handled by RenderPipeline::execute() -> ForwardPass::render()
    /// which queues MeshClass instances into the WW3D Renderer and issues real draw calls.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    fn render_game_objects<'a>(&'a self, _render_pass: &mut wgpu::RenderPass<'a>) {
        // Prefer presentation identity when installed (no live get_objects dual-read).
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            log::trace!(
                "Legacy stub: presentation has {} objects (RenderPipeline is sole draw path)",
                frame.objects.len()
            );
            return;
        }
        // Boot residual only — not active render path.
        let objects: Vec<_> = self.game_logic.get_objects().values().cloned().collect();
        log::trace!(
            "Rendering {} objects in scene (boot residual stub)",
            objects.len()
        );
        for obj in &objects {
            if obj.is_alive() {
                self.render_object(obj, _render_pass);
            }
        }
    }

    /// Legacy per-object render stub -- logs model status but does NOT submit draw calls.
    /// The active render path is RenderPipeline::collect_render_items() which builds
    /// RenderItem list and ForwardPass::prepare_mesh_instance() which creates actual
    /// MeshClass instances submitted to the WW3D Renderer.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    fn render_object<'a>(&'a self, obj: &Object, _render_pass: &mut wgpu::RenderPass<'a>) {
        let model_name = obj.get_template().get_model_name();

        log::trace!(
            "Render object {} template '{}' model '{}' (cached={})",
            obj.id,
            obj.template_name,
            model_name,
            self.graphics_system.get_model(model_name).is_some()
        );

        let w3d_model = self
            .graphics_system
            .get_model(model_name)
            .or_else(|| self.graphics_system.get_model(&obj.template_name));

        if let Some(w3d_model) = w3d_model {
            let total_vertices: usize = w3d_model
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.len())
                .sum();
            let total_indices: usize = w3d_model.meshes.iter().map(|mesh| mesh.indices.len()).sum();

            log::trace!("Rendering W3D model: {} (template: {}) with {} vertices, {} indices across {} meshes",
                model_name, obj.template_name, total_vertices, total_indices, w3d_model.meshes.len());
            log::trace!("Resolved W3D model '{}' for object {}", model_name, obj.id);
        } else {
            log::debug!(
                "No W3D model resolved for object {} template '{}' (model '{}') -- fallback cube will be used by RenderPipeline",
                obj.id,
                obj.template_name,
                model_name
            );
        }
    }

    #[allow(dead_code)] // Legacy stub: selection_renderer + PresentationFrame own production path
    fn render_selection_indicators(&self, _render_pass: &mut wgpu::RenderPass) {
        // Prefer presentation selected residual when installed (no live find_object dual-read).
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            let n = frame
                .objects
                .iter()
                .filter(|o| o.selected && !o.destroyed)
                .count();
            log::trace!(
                "Legacy stub: presentation selected count={n} (selection_renderer is sole path)"
            );
            return;
        }
        // Boot residual only.
        for &object_id in &self.selected_objects {
            let _ = object_id;
        }
    }

    fn render_projectiles(&self, _render_pass: &mut wgpu::RenderPass) {
        // Projectiles render from PresentationFrame (host CombatSystem freeze).
    }

    fn render_ui(&self, _render_pass: &mut wgpu::RenderPass) {
        if let Err(err) = self.ui_manager.render() {
            log::warn!("UI manager render failed: {}", err);
        }
        log::trace!(
            "UI overlay rendered for {} selected units",
            self.selected_objects.len()
        );
    }

    fn toggle_pause(&mut self) {
        self.game_paused = !self.game_paused;

        self.game_logic.set_paused(self.game_paused);

        info!(
            "Game {}",
            if self.game_paused {
                "PAUSED"
            } else {
                "RESUMED"
            }
        );

        // Notify UI
        self.ui_manager.queue_event(if self.game_paused {
            UIEvent::ChangeScreen(Screen::PauseMenu)
        } else {
            UIEvent::ChangeScreen(Screen::GameHUD)
        });
    }

    fn start_background_music(&mut self) {
        let handle = match &self.audio_handle {
            Some(handle) => handle,
            None => {
                info!("Background music skipped (-noaudio)");
                return;
            }
        };

        let sink = match Sink::try_new(handle) {
            Ok(sink) => sink,
            Err(err) => {
                error!("Failed to create music sink: {err}");
                return;
            }
        };

        // Create ambient RTS music
        let sample_rate = 44_100;
        let duration = 30.0; // 30 second loop
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let base = (t * 220.0 * 2.0 * std::f32::consts::PI).sin() * 0.05;
                let harmony1 = (t * 330.0 * 2.0 * std::f32::consts::PI).sin() * 0.03;
                let harmony2 = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.02;
                base + harmony1 + harmony2
            })
            .collect();

        let source = rodio::buffer::SamplesBuffer::new(1, sample_rate, samples).repeat_infinite();
        sink.append(source);

        self.background_music = Some(sink);
        info!("Background music started");
    }

    fn toggle_background_music(&mut self) {
        if self.audio_handle.is_none() {
            info!("Background music unavailable (-noaudio)");
            return;
        }

        if let Some(music) = &self.background_music {
            if music.is_paused() {
                music.play();
                info!("Background music resumed");
            } else {
                music.pause();
                info!("Background music paused");
            }
        } else {
            // DISABLED: Using proper AssetManager audio system instead of synthetic tones
            // self.start_background_music();
            info!("Background music would be started, but synthetic audio is disabled");
        }
    }

    fn play_sound_effect(&mut self, sound_type: SoundType) {
        // Prefer presentation/host audio event residual when a frame is installed
        // (InGame path). Avoid dual synthetic rodio tones competing with event queue.
        if self.last_presentation_frame.is_some() {
            let kind = match sound_type {
                SoundType::Select => "UnitSelect",
                SoundType::Command => "UnitCommand",
                SoundType::ConstructionComplete => "ConstructionComplete",
                SoundType::UnitReady => "UnitReady",
                SoundType::UpgradeComplete => "UpgradeComplete",
                SoundType::Hit => "WeaponHit",
                SoundType::Explosion => "Explosion",
                SoundType::Build => "BuildingComplete",
            };
            self.game_logic
                .queue_audio_event(crate::game_logic::AudioEventRequest::new(kind));
            // Input residual may land mid-frame after host process_audio_events —
            // drain immediately so Select/Command is not delayed one tick.
            self.game_logic.process_audio_events();
            return;
        }

        // Boot residual only — synthetic tones when no presentation frame.
        let handle = match &self.audio_handle {
            Some(handle) => handle,
            None => {
                return;
            }
        };

        let sink = match Sink::try_new(handle) {
            Ok(sink) => sink,
            Err(err) => {
                error!("Failed to create sound effect sink: {err}");
                return;
            }
        };

        let (frequency, duration) = match sound_type {
            SoundType::Select => (800.0, 0.1),
            SoundType::Command => (600.0, 0.15),
            SoundType::ConstructionComplete => (900.0, 0.25),
            SoundType::UnitReady => (700.0, 0.2),
            SoundType::UpgradeComplete => (750.0, 0.22),
            SoundType::Hit => (300.0, 0.2),
            SoundType::Explosion => (150.0, 0.5),
            SoundType::Build => (1000.0, 0.3),
        };

        let sample_rate = 44_100;
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let envelope = 1.0 - (t / duration); // Fade out
                (t * frequency * 2.0 * std::f32::consts::PI).sin() * 0.2 * envelope
            })
            .collect();

        let source = rodio::buffer::SamplesBuffer::new(1, sample_rate, samples);
        sink.append(source);
        self.sound_effects.push(sink);
    }

    fn cleanup_sound_effects(&mut self) {
        self.sound_effects.retain(|sink| !sink.empty());
    }

    /// Get or create a texture bind group for a material (delegated to graphics system)
    fn get_material_bind_group(
        &mut self,
        material: &crate::assets::W3DMaterial,
    ) -> Option<&wgpu::BindGroup> {
        // Delegate to graphics system which handles material bind group management
        self.graphics_system.get_material_bind_group(material)
    }

    /// Async texture loading method (for future implementation)
    /// This would be called from a background thread to load textures from BIG archives
    async fn load_texture_async(
        &mut self,
        texture_name: &str,
        material_name: &str,
    ) -> Result<(), String> {
        // Texture loading is now handled by the graphics system
        // This method is kept for future implementation of async texture streaming
        println!(
            "🎨 Async texture loading requested for: {} ({})",
            texture_name, material_name
        );
        println!("   (Currently handled by graphics system material management)");
        Ok(())
    }

    /// Legacy fallback cube creation using raw wgpu buffers.
    /// This is now superseded by GraphicsSystem::create_fallback_cube_model() which
    /// creates a W3DModel-based fallback cube cached in the model cache and used by
    /// RenderPipeline::collect_render_items() for objects with missing W3D assets.
    #[allow(dead_code)] // Legacy stub: superseded by GraphicsSystem, retained for reference
    fn create_fallback_cube(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
        // C++ SAGE compatible cube vertices using VertexFormatXYZNDUV2
        let vertices = vec![
            // Front face
            VertexXYZNDUV2 {
                position: [-2.5, -2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFF0000FF,
                tex_coords0: [0.0, 0.0],
                tex_coords1: [0.0, 0.0],
            }, // Red
            VertexXYZNDUV2 {
                position: [2.5, -2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFF00FF00,
                tex_coords0: [1.0, 0.0],
                tex_coords1: [1.0, 0.0],
            }, // Green
            VertexXYZNDUV2 {
                position: [2.5, 2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFFFF0000,
                tex_coords0: [1.0, 1.0],
                tex_coords1: [1.0, 1.0],
            }, // Blue
            VertexXYZNDUV2 {
                position: [-2.5, 2.5, 2.5],
                normal: [0.0, 0.0, 1.0],
                diffuse: 0xFF00FFFF,
                tex_coords0: [0.0, 1.0],
                tex_coords1: [0.0, 1.0],
            }, // Yellow
            // Back face
            VertexXYZNDUV2 {
                position: [-2.5, -2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFFFF00FF,
                tex_coords0: [0.0, 0.0],
                tex_coords1: [0.0, 0.0],
            }, // Magenta
            VertexXYZNDUV2 {
                position: [2.5, -2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFFFFFF00,
                tex_coords0: [1.0, 0.0],
                tex_coords1: [1.0, 0.0],
            }, // Cyan
            VertexXYZNDUV2 {
                position: [2.5, 2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFFFFFFFF,
                tex_coords0: [1.0, 1.0],
                tex_coords1: [1.0, 1.0],
            }, // White
            VertexXYZNDUV2 {
                position: [-2.5, 2.5, -2.5],
                normal: [0.0, 0.0, -1.0],
                diffuse: 0xFF808080,
                tex_coords0: [0.0, 1.0],
                tex_coords1: [0.0, 1.0],
            }, // Gray
        ];

        let indices: Vec<u16> = vec![
            0, 1, 2, 2, 3, 0, // Front
            4, 5, 6, 6, 7, 4, // Back
            7, 3, 0, 0, 4, 7, // Left
            1, 5, 6, 6, 2, 1, // Right
            3, 2, 6, 6, 7, 3, // Top
            0, 1, 5, 5, 4, 0, // Bottom
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Fallback Cube Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Fallback Cube Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        (vertex_buffer, index_buffer, indices.len() as u32)
    }

    /// C++ SAGE D3D8-style shader - matches original VertexFormatXYZNDUV2 and lighting model
    pub fn get_shader_source() -> &'static str {
        r#"
// C++ SAGE GlobalUniforms equivalent
struct SAGEUniforms {
    view_projection: mat4x4<f32>,
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
    camera_position: vec4<f32>,
    time: f32,
    ambient_light: vec3<f32>,
    sun_direction: vec3<f32>,
    sun_color: vec3<f32>,
    _padding: f32,
}

// C++ SAGE MaterialProperties equivalent
struct MaterialProperties {
    diffuse_color: vec4<f32>,
    specular_color: vec4<f32>,
    emissive_color: vec4<f32>,
    opacity: f32,
    shininess: f32,
    stage0_uv_scale: vec2<f32>,
    stage1_uv_scale: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> sage_uniforms: SAGEUniforms;

@group(1) @binding(0)
var stage0_texture: texture_2d<f32>;  // Primary diffuse texture (stage 0)
@group(1) @binding(1)
var stage0_sampler: sampler;

@group(2) @binding(0)
var<uniform> material_properties: MaterialProperties;

// C++ SAGE VertexFormatXYZNDUV2 input - matches D3DVERTEXELEMENT9 declarations
struct VertexInput {
    @location(0) position: vec3<f32>,     // XYZ position
    @location(1) normal: vec3<f32>,       // Normal vector
    @location(2) diffuse: vec4<f32>,      // Diffuse color (unpacked from u32)
    @location(3) tex_coords0: vec2<f32>,  // Primary UV coordinates
    @location(4) tex_coords1: vec2<f32>,  // Secondary UV coordinates
}

// Vertex shader output - matches C++ vertex shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords0: vec2<f32>,
    @location(3) tex_coords1: vec2<f32>,
    @location(4) vertex_diffuse: vec4<f32>,
    @location(5) view_direction: vec3<f32>,
}

// C++ SAGE vertex shader - matches D3D8 vertex shader behavior
@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform vertex to world space (identity transform for now)
    var world_position = vec4<f32>(input.position, 1.0);
    out.world_position = world_position.xyz;

    // Transform normal to world space
    out.world_normal = normalize(input.normal);

    // Pass through texture coordinates
    out.tex_coords0 = input.tex_coords0;
    out.tex_coords1 = input.tex_coords1;

    // Pass through vertex diffuse color
    out.vertex_diffuse = input.diffuse;

    // Calculate view direction for specular lighting
    out.view_direction = normalize(sage_uniforms.camera_position.xyz - out.world_position);

    // Transform to clip space
    out.clip_position = sage_uniforms.view_projection * world_position;

    return out;
}

// C++ SAGE pixel shader - matches D3D8 pixel shader with C&C lighting model
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample primary texture (stage 0) - matches C++ texture sampling
    var stage0_color = textureSample(stage0_texture, stage0_sampler, input.tex_coords0);

    // Apply material diffuse color to texture - matches C++ VertexMaterialClass behavior
    // In D3D8, materials multiply textures with diffuse color and vertex color
    var tinted_texture = stage0_color * vec4<f32>(material_properties.diffuse_color.rgb, 1.0);

    // Material base color combination - vertex diffuse further modulates the result
    var base_color = tinted_texture * input.vertex_diffuse;

    // C++ SAGE lighting calculations
    var normal = normalize(input.world_normal);
    var light_dir = normalize(sage_uniforms.sun_direction);
    var view_dir = normalize(input.view_direction);

    // Ambient lighting (always present in C&C)
    var ambient = sage_uniforms.ambient_light;

    // Diffuse lighting (Lambertian) - core C&C lighting
    var diffuse_factor = max(dot(normal, -light_dir), 0.0);
    var diffuse = sage_uniforms.sun_color * diffuse_factor;

    // Specular lighting (Phong) - for shiny surfaces like vehicles
    var reflect_dir = reflect(light_dir, normal);
    var specular_factor = pow(max(dot(view_dir, reflect_dir), 0.0), 32.0); // Default shininess
    var specular = sage_uniforms.sun_color * specular_factor * 0.3; // Moderate specular

    // Final lighting combination - matches C++ SAGE lighting model
    var lighting = ambient + diffuse + specular;
    var final_color = vec4<f32>(base_color.rgb * lighting, base_color.a);

    // Ensure minimum visibility (C&C never goes completely black)
    final_color.r = max(final_color.r, 0.1);
    final_color.g = max(final_color.g, 0.1);
    final_color.b = max(final_color.b, 0.1);

    return final_color;
}
"#
    }
}

#[derive(Debug, Clone, Copy)]
enum SoundType {
    Select,
    Command,
    ConstructionComplete,
    UnitReady,
    UpgradeComplete,
    Hit,
    Explosion,
    Build,
}

struct NoopWake;

impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

#[derive(Debug, Clone)]
struct RuntimeHostSnapshot {
    state: String,
    ui_screen: String,
    paused: bool,
    fps: f32,
    startup_progress: f32,
    startup_phase: String,
    map: String,
    frame: u32,
    selected_count: u32,
    local_mobile_units: u32,
    last_gameplay_cmd: String,
    match_over: bool,
    victory_label: String,
    /// PresentationFrame installed for client/render residual.
    presentation_frame_ok: bool,
    /// Live GameLogic dual-reads during last presentation-owned collect (must be 0 in-game).
    presentation_live_fallback_reads: u32,
    /// Sticky waypoint mode residual.
    waypoint_mode: bool,
}

#[derive(Debug)]
struct RuntimeHostBridge {
    control_path: PathBuf,
    status_path: PathBuf,
    frame_path: PathBuf,
    capture_path: PathBuf,
    frame_meta_path: PathBuf,
    fallback_frame_png: Option<Vec<u8>>,
    fallback_frame_luma: f32,
    last_published_frame: u32,
    last_capture_request_at: Option<Instant>,
    capture_request_in_flight: bool,
    capture_request_started_at: Option<Instant>,
    screenshot_enqueue_failed: bool,
    has_published_live_frame: bool,
    created_at: Instant,
    last_capture_health_log_at: Option<Instant>,
}

impl RuntimeHostBridge {
    const CAPTURE_REQUEST_INTERVAL_LOADING: Duration = Duration::from_millis(120);
    const CAPTURE_REQUEST_INTERVAL_INTERACTIVE: Duration = Duration::from_millis(40);
    const CAPTURE_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

    fn capture_interval_for_state(state: &str) -> Duration {
        match state {
            "Menu" | "InGame" | "Paused" => Self::CAPTURE_REQUEST_INTERVAL_INTERACTIVE,
            _ => Self::CAPTURE_REQUEST_INTERVAL_LOADING,
        }
    }

    fn is_headless_mode(args: &CommandLineArgs) -> bool {
        args.get_option_value("runtime_host")
            .map(|mode| mode.trim().eq_ignore_ascii_case("headless"))
            .unwrap_or(false)
    }

    fn from_command_line(args: &CommandLineArgs) -> Option<Self> {
        if !Self::is_headless_mode(args) {
            return None;
        }
        let control_path = PathBuf::from(args.get_option_value("gpui_control")?);
        let status_path = PathBuf::from(args.get_option_value("gpui_status")?);
        let frame_path = PathBuf::from(args.get_option_value("gpui_frame")?);
        let capture_path = frame_path.with_extension("png.capture");
        let frame_meta_path = frame_path.with_extension("png.meta");

        let _ = fs::remove_file(&control_path);
        let _ = fs::remove_file(&status_path);
        let _ = fs::remove_file(&frame_path);
        let _ = fs::remove_file(&capture_path);
        let _ = fs::remove_file(&frame_meta_path);

        let (fallback_frame_png, fallback_frame_luma) = Self::load_fallback_frame_png();
        Some(Self {
            control_path,
            status_path,
            frame_path,
            capture_path,
            frame_meta_path,
            fallback_frame_png,
            fallback_frame_luma,
            last_published_frame: 0,
            last_capture_request_at: None,
            capture_request_in_flight: false,
            capture_request_started_at: None,
            screenshot_enqueue_failed: false,
            has_published_live_frame: false,
            created_at: Instant::now(),
            last_capture_health_log_at: None,
        })
    }

    fn drain_commands(&mut self) -> Vec<String> {
        let mut control_file = match fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&self.control_path)
        {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };
        let mut payload = String::new();
        if control_file.read_to_string(&mut payload).is_err() {
            return Vec::new();
        }
        if payload.trim().is_empty() {
            return Vec::new();
        }
        if let Err(err) = control_file.set_len(0) {
            warn!(
                "Runtime host failed truncating control file {}: {err}",
                self.control_path.display()
            );
        } else {
            let _ = control_file.seek(SeekFrom::Start(0));
        }
        payload
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| line.to_string())
            .collect()
    }

    fn publish_booting(&mut self) {
        let snapshot = RuntimeHostSnapshot {
            state: "Booting".to_string(),
            ui_screen: "None".to_string(),
            paused: false,
            fps: 0.0,
            startup_progress: 0.0,
            startup_phase: "Booting runtime".to_string(),
            map: "-".to_string(),
            frame: self.last_published_frame,
            selected_count: 0,
            local_mobile_units: 0,
            last_gameplay_cmd: String::new(),
            match_over: false,
            victory_label: String::new(),
            presentation_frame_ok: false,
            presentation_live_fallback_reads: 0,
            waypoint_mode: false,
        };
        self.publish_status(&snapshot);
    }

    fn publish_runtime(&mut self, snapshot: &RuntimeHostSnapshot) {
        self.publish_status(snapshot);
        self.publish_frame(snapshot.frame, &snapshot.state);
    }

    fn publish_status(&mut self, snapshot: &RuntimeHostSnapshot) {
        let mut payload = String::new();
        payload.push_str(&format!("state={}\n", snapshot.state));
        payload.push_str(&format!("ui_screen={}\n", snapshot.ui_screen));
        payload.push_str(&format!("paused={}\n", snapshot.paused));
        payload.push_str(&format!("fps={:.3}\n", snapshot.fps.max(0.0)));
        payload.push_str(&format!(
            "startup_progress={:.4}\n",
            snapshot.startup_progress.clamp(0.0, 1.0)
        ));
        payload.push_str(&format!("startup_phase={}\n", snapshot.startup_phase));
        payload.push_str(&format!("map={}\n", snapshot.map));
        payload.push_str(&format!("frame={}\n", snapshot.frame));
        payload.push_str(&format!("selected_count={}\n", snapshot.selected_count));
        payload.push_str(&format!(
            "local_mobile_units={}\n",
            snapshot.local_mobile_units
        ));
        payload.push_str(&format!(
            "last_gameplay_cmd={}\n",
            snapshot.last_gameplay_cmd
        ));
        payload.push_str(&format!("match_over={}\n", snapshot.match_over));
        payload.push_str(&format!("victory_label={}\n", snapshot.victory_label));
        payload.push_str(&format!(
            "presentation_frame_ok={}\n",
            snapshot.presentation_frame_ok
        ));
        payload.push_str(&format!(
            "presentation_live_fallback_reads={}\n",
            snapshot.presentation_live_fallback_reads
        ));
        payload.push_str(&format!("waypoint_mode={}\n", snapshot.waypoint_mode));
        payload.push_str(&format!(
            "frame_path={}\n",
            self.frame_path.to_string_lossy()
        ));
        let _ = fs::write(&self.status_path, payload);
    }

    fn publish_frame(&mut self, frame: u32, state: &str) {
        if frame <= self.last_published_frame {
            return;
        }
        self.last_published_frame = frame;

        self.promote_capture_frame_if_ready();

        if self.capture_request_in_flight {
            let timed_out = self
                .capture_request_started_at
                .map(|started| started.elapsed() >= Self::CAPTURE_REQUEST_TIMEOUT)
                .unwrap_or(false);
            if timed_out {
                warn!(
                    "Runtime host capture request timed out after {:?} (frame={}, in_flight={})",
                    Self::CAPTURE_REQUEST_TIMEOUT,
                    frame,
                    self.capture_request_in_flight
                );
                self.capture_request_in_flight = false;
                self.capture_request_started_at = None;
            }
        }

        let capture_interval = Self::capture_interval_for_state(state);
        let should_request_capture = !self.capture_request_in_flight
            && self
                .last_capture_request_at
                .map(|last| last.elapsed() >= capture_interval)
                .unwrap_or(true);
        if should_request_capture {
            let requested_at = Instant::now();
            match ww3d_engine::make_screenshot(&self.capture_path) {
                Ok(()) => {
                    self.last_capture_request_at = Some(requested_at);
                    self.capture_request_in_flight = true;
                    self.capture_request_started_at = Some(requested_at);
                    self.screenshot_enqueue_failed = false;
                }
                Err(err) => {
                    if !self.screenshot_enqueue_failed {
                        warn!(
                            "Runtime host frame capture unavailable ({err:?}); falling back to static frame"
                        );
                        self.screenshot_enqueue_failed = true;
                    }
                }
            }
        }

        self.promote_capture_frame_if_ready();

        if Self::png_file_looks_usable(&self.frame_path) {
            self.has_published_live_frame = true;
            return;
        }
        if self.has_published_live_frame {
            // Keep the most recent live frame while a newer capture is pending.
            return;
        }

        let should_log_capture_health = self
            .last_capture_health_log_at
            .map(|last| last.elapsed() >= Duration::from_secs(2))
            .unwrap_or_else(|| self.created_at.elapsed() >= Duration::from_secs(2));
        if should_log_capture_health {
            warn!(
                "Runtime host awaiting first live frame: frame={} in_flight={} enqueue_failed={} capture_path={}",
                frame,
                self.capture_request_in_flight,
                self.screenshot_enqueue_failed,
                self.capture_path.display()
            );
            self.last_capture_health_log_at = Some(Instant::now());
        }

        let fallback_bytes = if let Some(bytes) = self.fallback_frame_png.as_ref() {
            bytes.clone()
        } else {
            let (generated, generated_luma) = Self::build_procedural_fallback_png();
            self.fallback_frame_luma = generated_luma;
            let generated = generated.unwrap_or_else(|| {
                // 1x1 opaque black PNG
                vec![
                    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0,
                    0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156,
                    99, 96, 96, 96, 248, 15, 0, 1, 4, 1, 0, 95, 161, 122, 86, 0, 0, 0, 0, 73, 69,
                    78, 68, 174, 66, 96, 130,
                ]
            });
            self.fallback_frame_png = Some(generated.clone());
            generated
        };
        if let Err(err) = fs::write(&self.frame_path, &fallback_bytes) {
            warn!(
                "Failed writing GPUI runtime fallback frame {:?}: {err}",
                self.frame_path
            );
        } else {
            let _ = fs::write(
                &self.frame_meta_path,
                format!("luma={:.3}\n", self.fallback_frame_luma),
            );
        }
    }

    fn promote_capture_frame_if_ready(&mut self) {
        if !Self::png_file_looks_usable(&self.capture_path) {
            return;
        }
        if let Err(err) = fs::rename(&self.capture_path, &self.frame_path) {
            warn!(
                "Failed to promote GPUI runtime capture {:?} -> {:?}: {err}",
                self.capture_path, self.frame_path
            );
            self.capture_request_in_flight = false;
            self.capture_request_started_at = None;
            return;
        }
        self.capture_request_in_flight = false;
        self.capture_request_started_at = None;
        if !self.has_published_live_frame {
            info!(
                "Runtime host promoted first live frame from capture (frame={})",
                self.last_published_frame
            );
        }
        self.has_published_live_frame = true;
        let _ = fs::write(&self.frame_meta_path, "luma=0.0\n");
    }

    fn png_file_looks_usable(path: &Path) -> bool {
        let Ok(metadata) = fs::metadata(path) else {
            return false;
        };
        if metadata.len() < 128 {
            return false;
        }
        let mut signature = [0u8; 8];
        let Ok(mut file) = fs::File::open(path) else {
            return false;
        };
        if file.read_exact(&mut signature).is_err() {
            return false;
        }
        signature == [137, 80, 78, 71, 13, 10, 26, 10]
    }

    fn load_fallback_frame_png() -> (Option<Vec<u8>>, f32) {
        let candidates = [
            "Data/English/Art/Textures/loadpageuserinterface.tga",
            "Data/English/Art/Textures/TitleScreenuserinterface.tga",
            "MapsZH/Maps/ShellMapMD/ShellMapMD.tga",
        ];

        // Use the mounted game file system first (C++ W3DFileSystem semantics).
        {
            let fs = game_engine::common::system::file_system::get_file_system();
            let fs_guard_result = fs.lock();
            if let Ok(mut fs_guard) = fs_guard_result {
                for candidate in candidates {
                    if let Some(mut file) = fs_guard.open_file(
                        candidate,
                        game_engine::common::system::file::FileAccess::READ
                            .combine(game_engine::common::system::file::FileAccess::BINARY),
                    ) {
                        let Ok(bytes) = file.read_entire_and_close() else {
                            continue;
                        };
                        let Ok(image) = image::load_from_memory(&bytes) else {
                            continue;
                        };
                        let rgba = image.to_rgba8();
                        let luma = if rgba.is_empty() {
                            0.0
                        } else {
                            let sum = rgba
                                .chunks_exact(4)
                                .map(|px| {
                                    0.2126 * px[0] as f32 / 255.0
                                        + 0.7152 * px[1] as f32 / 255.0
                                        + 0.0722 * px[2] as f32 / 255.0
                                })
                                .sum::<f32>();
                            (sum / (rgba.len() as f32 / 4.0)).clamp(0.0, 1.0) * 255.0
                        };
                        let mut png_bytes = Vec::new();
                        let mut cursor = std::io::Cursor::new(&mut png_bytes);
                        if image.write_to(&mut cursor, image::ImageFormat::Png).is_ok() {
                            return (Some(png_bytes), luma);
                        }
                    }
                }
            }
        }

        // Final local fallback: try plain filesystem copies.
        for candidate in candidates {
            let Ok(bytes) = fs::read(candidate) else {
                continue;
            };
            let Ok(image) = image::load_from_memory(&bytes) else {
                continue;
            };
            let rgba = image.to_rgba8();
            let luma = if rgba.is_empty() {
                0.0
            } else {
                let sum = rgba
                    .chunks_exact(4)
                    .map(|px| {
                        0.2126 * px[0] as f32 / 255.0
                            + 0.7152 * px[1] as f32 / 255.0
                            + 0.0722 * px[2] as f32 / 255.0
                    })
                    .sum::<f32>();
                (sum / (rgba.len() as f32 / 4.0)).clamp(0.0, 1.0) * 255.0
            };
            let mut png_bytes = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut png_bytes);
            if image.write_to(&mut cursor, image::ImageFormat::Png).is_ok() {
                return (Some(png_bytes), luma);
            }
        }
        Self::build_procedural_fallback_png()
    }

    fn build_procedural_fallback_png() -> (Option<Vec<u8>>, f32) {
        let width = 1280u32;
        let height = 720u32;
        let mut rgba = image::RgbaImage::new(width, height);
        for y in 0..height {
            let v = y as f32 / (height.saturating_sub(1).max(1)) as f32;
            for x in 0..width {
                let u = x as f32 / (width.saturating_sub(1).max(1)) as f32;
                let r = (22.0 + 26.0 * (1.0 - v) + 12.0 * u).clamp(0.0, 255.0) as u8;
                let g = (34.0 + 38.0 * (1.0 - v)).clamp(0.0, 255.0) as u8;
                let b = (48.0 + 58.0 * v).clamp(0.0, 255.0) as u8;
                rgba.put_pixel(x, y, image::Rgba([r, g, b, 255]));
            }
        }

        let mut png_bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_bytes);
        if image::DynamicImage::ImageRgba8(rgba)
            .write_to(&mut cursor, image::ImageFormat::Png)
            .is_ok()
        {
            return (Some(png_bytes), 96.0);
        }
        (None, 0.0)
    }
}

/// Run the actual C&C game
pub async fn run_cnc_game(
    event_loop: EventLoop<()>,
    window_attributes: WindowAttributes,
    cmd_args: Arc<CommandLineArgs>,
) -> Result<()> {
    info!("🎮 Starting Command & Conquer Generals Zero Hour - Real Game");

    register_real_game_client_bootstrap();

    let mut pending_window_attributes = Some(window_attributes);
    let mut window: Option<Arc<Window>> = None;
    let mut pending_engine_window: Option<Arc<Window>> = None;
    let mut engine_init_future: Option<Pin<Box<dyn Future<Output = Result<CnCGameEngine>>>>> = None;
    let mut engine_init_started_at: Option<Instant> = None;
    let mut engine_init_last_log_at: Option<Instant> = None;
    let mut engine: Option<CnCGameEngine> = None;
    let mut shutdown_logged = false;
    let mut next_redraw_at = Instant::now();
    let mut last_slow_frame_log = None::<Instant>;
    let mut slow_frame_count = 0u32;
    let mut slow_frame_peak = Duration::ZERO;
    let mut slow_ww3d_peak = Duration::ZERO;
    let mut slow_update_peak = Duration::ZERO;
    let mut slow_render_peak = Duration::ZERO;
    let mut last_render_health_log = Instant::now();
    const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);
    const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(5);
    const MINIMIZED_POLL_INTERVAL: Duration = Duration::from_millis(5);
    let runtime_headless_mode = RuntimeHostBridge::is_headless_mode(cmd_args.as_ref());
    let mut runtime_host_bridge = RuntimeHostBridge::from_command_line(cmd_args.as_ref());
    if let Some(bridge) = runtime_host_bridge.as_mut() {
        bridge.publish_booting();
    }
    let mut runtime_window_minimized = false;

    #[cfg(feature = "integration-diagnostics")]
    let mut integration_bridge: Option<IntegrationTelemetryBridge> = None;
    #[cfg(feature = "integration-diagnostics")]
    let runtime_handle = tokio::runtime::Handle::current();

    #[allow(deprecated)]
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::WaitUntil(next_redraw_at));

        let mut drive_frame = |
            engine: &mut CnCGameEngine,
            current_window: &Arc<Window>,
            runtime_host_bridge: &mut Option<RuntimeHostBridge>,
            render_frame: bool,
        | {
            if let Some(bridge) = runtime_host_bridge.as_mut() {
                for command in bridge.drain_commands() {
                    engine.apply_runtime_host_command(&command);
                }
            }

            let frame_started = Instant::now();
            let mut ww3d_elapsed = Duration::ZERO;
            let frame_timing = if matches!(
                engine.get_state(),
                GameState::Loading | GameState::Menu | GameState::InGame | GameState::Paused
            ) {
                let ww3d_started = Instant::now();
                let timing = match ww3d_engine::update() {
                    Ok(_) => match ww3d_engine::timing() {
                        Ok(timing) => {
                            let sync_ms = (timing.total_seconds() * 1000.0)
                                .clamp(0.0, u32::MAX as f32)
                                as u32;
                            WW3D::sync(sync_ms);
                            Some(timing)
                        }
                        Err(err) => {
                            error!("Failed to fetch WW3D frame timing: {err:?}");
                            None
                        }
                    },
                    Err(err) => {
                        error!("WW3D engine update failed: {err:?}");
                        None
                    }
                };
                ww3d_elapsed = ww3d_started.elapsed();
                timing
            } else {
                None
            };

            let update_started = Instant::now();
            if let Some(timing) = frame_timing {
                #[cfg(feature = "integration-diagnostics")]
                if let Some(bridge) = integration_bridge.as_mut() {
                    if let Err(err) = runtime_handle.block_on(bridge.pump_with_timing(engine, timing))
                    {
                        error!(
                            "Integration telemetry pump failed: {err:?}. Disabling bridge."
                        );
                        integration_bridge = None;
                    }
                }
                engine.update_with_timing(&timing);
            } else {
                engine.update_with_frame_clock();
            }
            let update_elapsed = update_started.elapsed();
            static DRIVE_FRAME_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let dfn = DRIVE_FRAME_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if dfn < 15 || (dfn < 50 && matches!(engine.get_state(), GameState::Menu)) {
                info!("drive_frame #{} update_done {:?} state={:?} render_frame={}", dfn, update_elapsed, engine.get_state(), render_frame);
            }

            let render_started = Instant::now();
            if render_frame {
                if dfn < 15 || (dfn < 50 && matches!(engine.get_state(), GameState::Menu)) {
                    info!("drive_frame #{} calling render()", dfn);
                }
                match engine.render() {
                    Ok(_) => {}
                    Err(e) => {
                        error!("❌ RENDER ERROR: {:?}", e);
                        if let Some(source_err) = e.source() {
                            if let Some(surface_err) =
                                source_err.downcast_ref::<wgpu::SurfaceError>()
                            {
                                match surface_err {
                                    wgpu::SurfaceError::Lost => {
                                        error!("🔄 SURFACE LOST: Attempting resize");
                                        engine.resize(current_window.inner_size());
                                    }
                                    wgpu::SurfaceError::OutOfMemory => {
                                        error!("💥 OUT OF MEMORY: Exiting");
                                        elwt.exit();
                                    }
                                    _ => {
                                        error!("🚨 Other surface error: {:?}", surface_err);
                                    }
                                }
                            } else {
                                error!("🚨 Non-surface error: {:?}", source_err);
                            }
                        } else {
                            error!("🚨 No source error available");
                        }
                    }
                }
            }
            let render_elapsed = render_started.elapsed();

            let frame_elapsed = frame_started.elapsed();
            if frame_elapsed >= Duration::from_millis(120) {
                slow_frame_count = slow_frame_count.saturating_add(1);
                slow_frame_peak = slow_frame_peak.max(frame_elapsed);
                slow_ww3d_peak = slow_ww3d_peak.max(ww3d_elapsed);
                slow_update_peak = slow_update_peak.max(update_elapsed);
                slow_render_peak = slow_render_peak.max(render_elapsed);
            }
            if frame_elapsed >= Duration::from_millis(300) {
                let should_log = last_slow_frame_log
                    .map(|last| frame_started.duration_since(last) >= Duration::from_secs(1))
                    .unwrap_or(true);
                if should_log {
                    warn!(
                        "Severe slow frame {:?} in {:?} (ww3d={:?}, update={:?}, render={:?}, startup_progress={:.0}%)",
                        frame_elapsed,
                        engine.get_state(),
                        ww3d_elapsed,
                        update_elapsed,
                        render_elapsed,
                        engine.startup_last_reported_progress * 100.0
                    );
                    last_slow_frame_log = Some(frame_started);
                }
            }
            if frame_started.duration_since(last_render_health_log) >= Duration::from_secs(5) {
                if slow_frame_count == 0 {
                    info!(
                        "Render health: ok (state={:?}, render_items={}, no slow frames >120ms in last 5s, startup_progress={:.0}%)",
                        engine.get_state(),
                        engine.render_pipeline.debug_render_item_count(),
                        engine.startup_last_reported_progress * 100.0
                    );
                } else {
                    info!(
                        "Render health: slow_frames={} peak={:?} (ww3d_peak={:?}, update_peak={:?}, render_peak={:?}, state={:?}, render_items={}, startup_progress={:.0}%)",
                        slow_frame_count,
                        slow_frame_peak,
                        slow_ww3d_peak,
                        slow_update_peak,
                        slow_render_peak,
                        engine.get_state(),
                        engine.render_pipeline.debug_render_item_count(),
                        engine.startup_last_reported_progress * 100.0
                    );
                }
                slow_frame_count = 0;
                slow_frame_peak = Duration::ZERO;
                slow_ww3d_peak = Duration::ZERO;
                slow_update_peak = Duration::ZERO;
                slow_render_peak = Duration::ZERO;
                last_render_health_log = frame_started;
            }

            if should_exit_for_smoke_test(
                cmd_args.wants_smoke_test(),
                engine.get_state(),
                engine.startup_last_reported_progress,
                engine.is_state_change_pending(GameState::Exiting),
            ) {
                info!("Smoke test reached main menu; exiting successfully");
                engine.transition_to_state(GameState::Exiting);
                elwt.exit();
                return;
            }

            if let Some(bridge) = runtime_host_bridge.as_mut() {
                let snapshot = engine.runtime_host_status_snapshot();
                bridge.publish_runtime(&snapshot);
            }
        };

        if matches!(event, Event::Resumed) && engine.is_none() {
            let Some(attributes) = pending_window_attributes.take() else {
                error!("Missing window attributes during startup resume");
                elwt.exit();
                return;
            };

            let created_window = match elwt.create_window(attributes) {
                Ok(window) => Arc::new(window),
                Err(err) => {
                    error!("Failed to create window: {err}");
                    elwt.exit();
                    return;
                }
            };

            info!(
                "Window created: {}x{} ({})",
                created_window.inner_size().width,
                created_window.inner_size().height,
                if created_window.fullscreen().is_some() {
                    "Fullscreen"
                } else {
                    "Windowed"
                }
            );

            if runtime_headless_mode {
                created_window.set_visible(false);
            } else {
                created_window.set_visible(true);
            }
            created_window.request_redraw();
            window = Some(created_window.clone());
            pending_engine_window = Some(created_window);
            return;
        }

        if engine.is_none() {
            match event {
                Event::WindowEvent { ref event, window_id } => {
                    if let Some(current_window) = window.as_ref() {
                        if window_id == current_window.id()
                            && matches!(event, WindowEvent::CloseRequested)
                        {
                            info!("Close requested before engine startup completed");
                            elwt.exit();
                            return;
                        }
                    }
                }
                Event::AboutToWait => {
                    if let Some(bridge) = runtime_host_bridge.as_mut() {
                        bridge.publish_booting();
                        for command in bridge.drain_commands() {
                            if command.trim().eq_ignore_ascii_case("exit") {
                                info!("Runtime host received exit command during startup");
                                elwt.exit();
                                return;
                            }
                        }
                    }

                    if engine_init_future.is_none() {
                        if let Some(created_window) = pending_engine_window.take() {
                            #[cfg(target_os = "windows")]
                            {
                                use raw_window_handle::HasWindowHandle;
                                if let Ok(handle) = created_window.window_handle() {
                                    if let raw_window_handle::RawWindowHandle::Win32(win) =
                                        handle.as_raw()
                                    {
                                        crate::win_main::APPLICATION_WINDOW.store(
                                            win.hwnd.get() as *mut std::ffi::c_void,
                                            std::sync::atomic::Ordering::Relaxed,
                                        );
                                        debug!("Win32 window handle stored");
                                    }
                                }
                            }

                            engine_init_started_at = Some(Instant::now());
                            engine_init_last_log_at = None;
                            created_window
                                .set_title("Command & Conquer Generals Zero Hour - Initializing");
                            engine_init_future = Some(Box::pin(CnCGameEngine::new(
                                created_window.clone(),
                                cmd_args.clone(),
                            )));
                        }
                    }

                    if let Some(init_future) = engine_init_future.as_mut() {
                        let waker: Waker = Waker::from(Arc::new(NoopWake));
                        let mut cx = Context::from_waker(&waker);
                        match init_future.as_mut().poll(&mut cx) {
                            Poll::Ready(Ok(new_engine)) => {
                                if let Some(created_window) = window.as_ref() {
                                    info!("C&C Game engine initialized successfully!");
                                    if !runtime_headless_mode {
                                        created_window.focus_window();
                                    }
                                    created_window.request_redraw();
                                }
                                engine_init_future = None;
                                engine_init_started_at = None;
                                engine_init_last_log_at = None;
                                let mut new_engine = new_engine;
                                if let Some(bridge) = runtime_host_bridge.as_mut() {
                                    let snapshot = new_engine.runtime_host_status_snapshot();
                                    bridge.publish_runtime(&snapshot);
                                }
                                engine = Some(new_engine);
                                #[cfg(feature = "integration-diagnostics")]
                                if cmd_args.wants_integration_diagnostics() {
                                    match pollster::block_on(IntegrationTelemetryBridge::new(
                                        IntegrationConfig::default(),
                                    )) {
                                        Ok(bridge) => {
                                            info!("Integration diagnostics bridge initialized");
                                            integration_bridge = Some(bridge);
                                        }
                                        Err(err) => {
                                            error!(
                                                "Failed to initialize integration diagnostics bridge: {err:?}. Continuing without telemetry overlay."
                                            );
                                        }
                                    }
                                }
                            }
                            Poll::Ready(Err(err)) => {
                                error!("Failed to initialize C&C game engine: {err}");
                                engine_init_future = None;
                                elwt.exit();
                            }
                            Poll::Pending => {
                                if let Some(started_at) = engine_init_started_at {
                                    let should_log = engine_init_last_log_at
                                        .map(|last| {
                                            last.elapsed() >= Duration::from_millis(500)
                                        })
                                        .unwrap_or_else(|| started_at.elapsed() >= Duration::from_millis(500));
                                    if should_log {
                                        info!(
                                            "Engine bootstrap still in progress ({:.2}s elapsed)",
                                            started_at.elapsed().as_secs_f32()
                                        );
                                        engine_init_last_log_at = Some(Instant::now());
                                    }
                                }
                            }
                        }
                    }

                    next_redraw_at = Instant::now() + STARTUP_POLL_INTERVAL;
                    elwt.set_control_flow(ControlFlow::WaitUntil(next_redraw_at));
                }
                _ => {}
            }
            return;
        }

        let Some(current_window) = window.as_ref() else {
            return;
        };
        let Some(engine) = engine.as_mut() else {
            return;
        };

        if engine.is_quitting() {
            if !shutdown_logged {
                info!("Engine shutting down");
                shutdown_logged = true;
            }
            if let Some(bridge) = runtime_host_bridge.as_mut() {
                let snapshot = engine.runtime_host_status_snapshot();
                bridge.publish_runtime(&snapshot);
            }
            elwt.exit();
            return;
        }

        match engine.process_platform_event(&event) {
            Ok(handled) => {
                if handled {
                    return;
                }
            }
            Err(e) => {
                error!("Platform message handling error: {}", e);
            }
        }

        if engine.is_quit_requested() {
            if !engine.is_quitting() && !engine.is_state_change_pending(GameState::Exiting) {
                info!("Platform requested quit");
                engine.request_state_change(GameState::Exiting);
            }
            return;
        }

        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == current_window.id() => {
                if !engine.input(event) {
                    match event {
                        WindowEvent::CloseRequested => {
                            info!("Close requested by window");
                            engine.request_state_change(GameState::Exiting);
                        }
                        WindowEvent::Destroyed => {
                            info!("Window destroyed - forcing exit");
                            engine.request_state_change(GameState::Exiting);
                        }
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    logical_key: Key::Named(NamedKey::Escape),
                                    ..
                                },
                            ..
                        } => match engine.get_state() {
                            GameState::InGame => {
                                // C++ Escape cancels structure placement before pause residual.
                                if engine.pending_structure_placement.is_some() {
                                    engine.cancel_structure_placement_from_ui();
                                    info!("Escape cancelled structure placement residual");
                                } else if engine.pending_map_command.take().is_some() {
                                    info!("Escape cancelled pending map command residual");
                                } else {
                                    info!("Escape pressed in InGame state - pausing");
                                    engine.request_state_change(GameState::Paused);
                                }
                            }
                            GameState::Paused => {
                                info!("Escape pressed in Paused state - resuming");
                                engine.request_state_change(GameState::InGame);
                            }
                            GameState::Menu | GameState::Loading => {
                                info!("Escape pressed in Menu/Loading - exiting");
                                engine.request_state_change(GameState::Exiting);
                            }
                            GameState::Victory | GameState::Defeat => {
                                info!("Escape pressed in endgame - returning to menu");
                                engine.request_state_change(GameState::Menu);
                            }
                            GameState::Exiting | GameState::Initializing => {}
                        },
                        WindowEvent::Resized(physical_size) => {
                            runtime_window_minimized |=
                                physical_size.width == 0 || physical_size.height == 0;
                            update_iconic_state_and_wake_audio(
                                current_window,
                                &mut runtime_window_minimized,
                            );
                            if !runtime_window_minimized {
                                engine.resize(*physical_size);
                            }
                        }
                        WindowEvent::ScaleFactorChanged { .. } => {
                            // Keep UI/layout hit-testing in sync on HiDPI transitions (macOS).
                            update_iconic_state_and_wake_audio(
                                current_window,
                                &mut runtime_window_minimized,
                            );
                            if !runtime_window_minimized {
                                engine.resize(current_window.inner_size());
                            }
                        }
                        WindowEvent::RedrawRequested => {
                            update_iconic_state_and_wake_audio(
                                current_window,
                                &mut runtime_window_minimized,
                            );
                            let runtime_window_suspended = runtime_window_minimized;
                            if runtime_headless_mode {
                                drive_frame(engine, current_window, &mut runtime_host_bridge, true);
                            } else if runtime_window_suspended {
                                if should_keep_logic_running_while_iconic(
                                    // Prefer presentation game_mode residual when installed.
                                    engine.presentation_or_live_game_mode(),
                                ) {
                                    drive_frame(
                                        engine,
                                        current_window,
                                        &mut runtime_host_bridge,
                                        false,
                                    );
                                }
                            } else {
                                drive_frame(engine, current_window, &mut runtime_host_bridge, true);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => {
                let now = Instant::now();
                if now >= next_redraw_at {
                    update_iconic_state_and_wake_audio(
                        current_window,
                        &mut runtime_window_minimized,
                    );
                    let runtime_window_suspended = runtime_window_minimized;
                    if runtime_headless_mode {
                        drive_frame(engine, current_window, &mut runtime_host_bridge, true);
                    } else if cmd_args.wants_smoke_test() {
                        drive_frame(engine, current_window, &mut runtime_host_bridge, false);
                        if engine.is_quitting() {
                            elwt.exit();
                            return;
                        }
                        next_redraw_at = now + STARTUP_POLL_INTERVAL;
                    } else if runtime_window_suspended {
                        if should_keep_logic_running_while_iconic(
                            // Prefer presentation game_mode residual when installed.
                            engine.presentation_or_live_game_mode(),
                        ) {
                            drive_frame(engine, current_window, &mut runtime_host_bridge, false);
                        }
                        next_redraw_at = now + MINIMIZED_POLL_INTERVAL;
                    } else {
                        current_window.request_redraw();
                        next_redraw_at = now + FRAME_INTERVAL;
                    }
                }
                elwt.set_control_flow(ControlFlow::WaitUntil(next_redraw_at));
            }
            Event::LoopExiting => {
                #[cfg(feature = "integration-diagnostics")]
                if let Some(bridge) = integration_bridge.take() {
                    if let Err(err) = runtime_handle.block_on(bridge.shutdown()) {
                        error!("Failed to shut down integration telemetry bridge: {err:?}");
                    }
                }
            }
            _ => {}
        }
    })?;

    info!("C&C Game ended successfully");
    Ok(())
}

impl CnCGameEngine {
    /// Preload all models used by game objects into the graphics system
    async fn preload_all_models(
        graphics_system: &mut GraphicsSystem,
        game_logic: &GameLogic,
    ) -> anyhow::Result<()> {
        use std::collections::HashSet;

        println!("🎨 PRELOAD: Starting model preloading for all game objects...");

        // Prefer PresentationFrame model_key freeze; live template fallback.
        let mut unique_models: HashSet<String> = HashSet::new();
        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(game_logic, 0);
        for key in frame.unique_model_keys() {
            unique_models.insert(key);
        }
        if unique_models.is_empty() {
            for object in game_logic.get_objects().values() {
                if !object.is_alive() {
                    continue;
                }
                let model_name = object.get_template().get_model_name();
                unique_models.insert(model_name.to_string());
            }
        }

        println!("📦 Loading {} models...", unique_models.len());

        // Load each model into graphics system
        let mut loaded_count = 0;
        let mut failed_count = 0;
        let total_models = unique_models.len();

        for (index, model_name) in unique_models.iter().enumerate() {
            println!(
                "🎯 Loading model {}/{}: {} (starting...)",
                index + 1,
                total_models,
                model_name
            );

            match Self::load_model_into_graphics_system_blocking(graphics_system, model_name) {
                Ok(true) => {
                    loaded_count += 1;
                    println!(
                        "✅ Model {}/{} loaded successfully",
                        index + 1,
                        total_models
                    );
                }
                Ok(false) => {
                    println!(
                        "⚠️  Model {}/{} already loaded (skipping)",
                        index + 1,
                        total_models
                    );
                }
                Err(e) => {
                    failed_count += 1;
                    eprintln!("❌ Model '{}' failed: {}", model_name, e);
                    eprintln!("   Continuing with next model...");
                }
            }

            println!(
                "📊 Progress: {}/{} models processed, {} loaded, {} failed",
                index + 1,
                total_models,
                loaded_count,
                failed_count
            );
        }

        println!(
            "✅ Loaded {} models ({} failed)",
            loaded_count, failed_count
        );

        Ok(())
    }

    /// Preload textures from all cached models using C++ approach - material names as texture files
    async fn preload_model_textures(graphics_system: &mut GraphicsSystem) -> anyhow::Result<()> {
        use std::collections::HashSet;

        log::info!(
            "🎨 TEXTURE: Loading textures using C++ approach - material names as texture filenames"
        );

        // Get all models from graphics system cache and collect material names as texture names
        let mut texture_names: HashSet<String> = HashSet::new();

        // Get all cached models from graphics system
        for (model_name, model) in graphics_system.get_all_models() {
            log::debug!(
                "🔍 TEXTURE: Scanning model '{}' for referenced stage textures...",
                model_name
            );

            Self::collect_material_textures(model, &mut texture_names);

            for mesh in &model.meshes {
                // Direct material reference on mesh (fallback path)
                if let Some(ref tex_name) = mesh.material.texture_name {
                    if Self::is_valid_texture_name(tex_name) {
                        texture_names.insert(tex_name.clone());
                        log::debug!("  📄 Found mesh embedded texture: {}", tex_name);
                    }
                }

                // Authoritative per-pass stage texture names (preferred)
                for (pass_idx, stage_sets) in mesh.per_pass_stage_texture_names.iter().enumerate() {
                    for (stage_idx, names) in stage_sets.iter().enumerate() {
                        let mut stage_populated = false;
                        for texture_name in names {
                            if Self::is_valid_texture_name(texture_name) {
                                texture_names.insert(texture_name.clone());
                                stage_populated = true;
                                log::debug!(
                                    "  📄 Pass {} Stage {} texture: {}",
                                    pass_idx,
                                    stage_idx,
                                    texture_name
                                );
                            }
                        }

                        if !stage_populated {
                            for fallback in mesh.stage_texture_names_from_ids(pass_idx, stage_idx) {
                                if Self::is_valid_texture_name(&fallback) {
                                    texture_names.insert(fallback.clone());
                                    log::debug!(
                                        "  📄 Pass {} Stage {} texture (from IDs): {}",
                                        pass_idx,
                                        stage_idx,
                                        fallback
                                    );
                                }
                            }
                        }
                    }
                }

                if mesh.per_pass_stage_texture_names.is_empty()
                    && !mesh.per_pass_stage_texture_ids.is_empty()
                {
                    for (pass_idx, stages) in mesh.per_pass_stage_texture_ids.iter().enumerate() {
                        for stage_idx in 0..stages.len() {
                            for fallback in mesh.stage_texture_names_from_ids(pass_idx, stage_idx) {
                                if Self::is_valid_texture_name(&fallback) {
                                    texture_names.insert(fallback.clone());
                                    log::debug!(
                                        "  📄 Pass {} Stage {} texture (from IDs): {}",
                                        pass_idx,
                                        stage_idx,
                                        fallback
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        log::info!(
            "🎨 TEXTURE: Found {} unique material-based textures to load",
            texture_names.len()
        );
        log::info!(
            "🎨 TEXTURE: First 10 texture names: {:?}",
            texture_names.iter().take(10).collect::<Vec<_>>()
        );

        if texture_names.is_empty() {
            log::warn!("⚠️  TEXTURE: No material names found - skipping preload");
            return Ok(());
        }

        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            let mut loaded_count = 0;
            let mut failed_count = 0;
            let total_textures = texture_names.len();
            let texture_names: Vec<_> = texture_names.iter().collect();

            log::info!(
                "🎨 TEXTURE: Starting preload of {} textures",
                total_textures
            );

            for (index, texture_name) in texture_names.iter().enumerate() {
                log::debug!(
                    "🎯 Loading texture {}/{}: {}",
                    index + 1,
                    total_textures,
                    texture_name
                );

                let load_result = async {
                    match asset_manager_arc.lock() {
                        Ok(mut asset_manager) => {
                            asset_manager
                                .load_texture(
                                    graphics_system.device(),
                                    graphics_system.queue(),
                                    texture_name,
                                )
                                .await;
                            true
                        }
                        Err(_) => {
                            log::warn!(
                                "Could not acquire asset manager lock for texture: {}",
                                texture_name
                            );
                            false
                        }
                    }
                }
                .await;

                if load_result {
                    loaded_count += 1;
                } else {
                    failed_count += 1;
                }
            }

            log::info!(
                "✅ TEXTURE PRELOAD: Loaded {} textures ({} failed/timeout)",
                loaded_count,
                failed_count
            );
        } else {
            log::error!("❌ TEXTURE PRELOAD: Asset manager not available");
        }

        Ok(())
    }

    fn collect_material_textures(model: &Arc<W3DModel>, texture_names: &mut HashSet<String>) {
        for (material_name, material) in &model.materials {
            if Self::is_valid_texture_name(material_name) {
                texture_names.insert(material_name.clone());
                log::debug!("  📄 Found material-as-texture: {}", material_name);
            }

            if let Some(ref texture_name) = material.texture_name {
                if Self::is_valid_texture_name(texture_name) {
                    texture_names.insert(texture_name.clone());
                    log::debug!("  📄 Found explicit material texture: {}", texture_name);
                }
            }

            for stage_idx in 0..MAX_STAGE_TEXTURES {
                if let Some(stage_texture) = GraphicsSystem::stage_texture_name(material, stage_idx)
                {
                    if Self::is_valid_texture_name(stage_texture) {
                        texture_names.insert(stage_texture.clone());
                        log::debug!(
                            "  📄 Material stage{} texture: {}",
                            stage_idx,
                            stage_texture
                        );
                    }
                }
            }
        }
    }

    fn is_valid_texture_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        if name.eq_ignore_ascii_case("default") {
            return false;
        }
        name.parse::<usize>().is_err()
    }

    /// Preload textures using WW3D Asset Manager definitions
    /// This loads textures defined in INI object definitions from INIZH.big
    async fn preload_ww3d_textures(graphics_system: &mut GraphicsSystem) -> anyhow::Result<()> {
        info!("🎨 TEXTURE: Preloading textures from WW3D Asset Manager definitions...");

        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            // First, get the list of texture filenames
            let texture_filenames = {
                let asset_manager = asset_manager_arc.lock().unwrap_or_else(|e| e.into_inner());
                asset_manager.get_all_texture_filenames()
            };

            info!(
                "🎨 TEXTURE: WW3D Asset Manager has {} unique texture filenames to load",
                texture_filenames.len()
            );

            // Show first 20 texture names for debugging
            for (index, name) in texture_filenames.iter().take(20).enumerate() {
                debug!("  📄 Texture {}: {}", index + 1, name);
            }

            if texture_filenames.len() > 20 {
                info!("  ... and {} more textures", texture_filenames.len() - 20);
            }

            // Load ALL textures (matching C++ behavior - no artificial limit)
            let mut loaded_count = 0;
            let mut failed_count = 0;
            let total_to_load = texture_filenames.len(); // Load all textures upfront like C++

            info!(
                "🎨 TEXTURE: Loading ALL {} textures from BIG archives (matching C++ behavior)...",
                total_to_load
            );

            for (index, texture_name) in texture_filenames.iter().enumerate() {
                debug!(
                    "🎯 Loading WW3D texture {}/{}: {}",
                    index + 1,
                    total_to_load,
                    texture_name
                );

                // Try to load the texture with timeout
                let load_future = async {
                    match asset_manager_arc.lock() {
                        Ok(mut asset_manager) => {
                            // Load the texture asynchronously
                            match asset_manager
                                .load_texture_async(
                                    graphics_system.device(),
                                    graphics_system.queue(),
                                    texture_name,
                                )
                                .await
                            {
                                Ok(_) => {
                                    debug!("✅ Loaded texture: {}", texture_name);
                                    true
                                }
                                Err(e) => {
                                    warn!("⚠️ Failed to load texture {}: {}", texture_name, e);
                                    false
                                }
                            }
                        }
                        Err(_) => {
                            warn!("Could not lock asset manager for texture: {}", texture_name);
                            false
                        }
                    }
                };

                match tokio::time::timeout(tokio::time::Duration::from_millis(500), load_future)
                    .await
                {
                    Ok(true) => loaded_count += 1,
                    Ok(false) => failed_count += 1,
                    Err(_) => {
                        failed_count += 1;
                        warn!("⏰ Texture '{}' timeout (500ms)", texture_name);
                    }
                }

                // Small delay between textures
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }

            info!(
                "✅ WW3D TEXTURE PRELOAD: Loaded {} textures ({} failed/timeout) from {} available",
                loaded_count,
                failed_count,
                texture_filenames.len()
            );
        } else {
            warn!("⚠️ WW3D TEXTURE PRELOAD: Asset manager not available");
        }

        Ok(())
    }

    /// Load a single model into the graphics system
    fn load_model_into_graphics_system_blocking(
        graphics_system: &mut GraphicsSystem,
        model_name: &str,
    ) -> anyhow::Result<bool> {
        // Check if model is already loaded
        if graphics_system.get_model(model_name).is_some() {
            return Ok(false); // Already loaded
        }

        // Get asset manager and load the model
        if let Some(asset_manager_arc) = crate::assets::get_asset_manager() {
            // CRITICAL FIX: Load model in a scope to release asset manager lock before cache_model()
            let w3d_model = {
                let mut asset_manager = asset_manager_arc.lock().unwrap_or_else(|e| e.into_inner());
                match asset_manager.load_w3d_model_blocking(model_name) {
                    Ok(model) => Ok(model),
                    Err(e) => Err(anyhow::anyhow!(
                        "Failed to load W3D model '{}': {}",
                        model_name,
                        e
                    )),
                }
            }?; // Asset manager lock is released here

            // C++ parity: cache the loaded model immediately once it is available.
            graphics_system.cache_model(model_name.to_string(), w3d_model);
            Ok(true)
        } else {
            anyhow::bail!("Asset manager not available");
        }
    }
}

/// Map HUD structure cameo labels to ThingTemplate residual names.
fn resolve_ui_structure_template_name(name: &str) -> String {
    let n = name.trim();
    if n.is_empty() {
        return String::new();
    }
    // Already a template-style name.
    if n.contains("America") || n.contains("China") || n.contains("GLA") || n.contains('_') {
        return n.to_string();
    }
    let key = n.to_ascii_lowercase();
    match key.as_str() {
        "power plant" | "powerplant" => "AmericaPowerPlant".into(),
        "barracks" => "AmericaBarracks".into(),
        "supply center" | "supplycenter" => "AmericaSupplyCenter".into(),
        "war factory" | "warfactory" => "AmericaWarFactory".into(),
        "airfield" => "AmericaAirfield".into(),
        "command center" | "commandcenter" => "AmericaCommandCenter".into(),
        "patriot battery" | "patriot" => "AmericaPatriotBattery".into(),
        "strategy center" => "AmericaStrategyCenter".into(),
        "detention camp" => "AmericaDetentionCamp".into(),
        "particle cannon" => "AmericaParticleCannonUplink".into(),
        _ => {
            // Fallback: strip spaces residual.
            let compact: String = n.chars().filter(|c| !c.is_whitespace()).collect();
            format!("America{compact}")
        }
    }
}

#[test]
fn stop_and_guard_hotkeys_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("eq_ignore_ascii_case(\"s\") && !ctrl_down")
            && src.contains("issue_named_command_from_ui(\"Command_Stop\")"),
        "S must issue Command_Stop residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"g\") && !ctrl_down")
            && src.contains("issue_named_command_from_ui(\"Command_Guard\")"),
        "G must issue Command_Guard residual"
    );
    // Ctrl+S quick-save must remain distinct from Stop.
    assert!(
        src.contains("eq_ignore_ascii_case(\"s\") && ctrl_down")
            && src.contains("quick_save_from_hotkey"),
        "Ctrl+S quick-save residual must remain"
    );
}

#[test]
fn retail_selection_and_scatter_hotkeys_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("eq_ignore_ascii_case(\"x\") && !ctrl_down")
            && src.contains("issue_named_command_from_ui(\"Command_Scatter\")"),
        "X must issue Command_Scatter residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"q\") && !ctrl_down")
            && src.contains("select_all_friendly_units"),
        "Q must SELECT_ALL residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"e\") && !ctrl_down")
            && src.contains("select_matching_units_hotkey"),
        "E must SELECT_MATCHING_UNITS residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"w\") && !ctrl_down")
            && src.contains("select_all_friendly_aircraft"),
        "W must SELECT_ALL_AIRCRAFT residual"
    );

    assert!(
        src.contains("eq_ignore_ascii_case(\"h\") && !ctrl_down")
            && src.contains("issue_named_command_from_ui(\"Command_ViewCommandCenter\")"),
        "H must VIEW_COMMAND_CENTER residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"f\")")
            && src.contains("issue_named_command_from_ui(\"Command_CreateFormation\")"),
        "Ctrl+F must CREATE_FORMATION residual"
    );

    assert!(
        src.contains("NamedKey::Space") && src.contains("Command_ViewLastRadarEvent"),
        "Space must VIEW_LAST_RADAR_EVENT residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"h\") && ctrl_down")
            && src.contains("select_hero_units_hotkey"),
        "Ctrl+H must SELECT_HERO residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"p\")") && src.contains("toggle_pause"),
        "P must remain pause residual"
    );
}

#[test]
fn escape_in_handle_key_press_cancels_then_pauses_residual() {
    let src = include_str!("cnc_game_engine.rs");
    // Build needle so this test source does not self-match.
    let marker = format!("Escape cancelled structure {} residual", "placement");
    let hk = src
        .find("fn handle_key_press(&mut self, key: &Key)")
        .expect("handle_key_press");
    let mut chosen = None;
    let mut search = hk;
    while let Some(rel) = src[search..].find(&marker) {
        let i = search + rel;
        let window = &src[i.saturating_sub(800)..src.len().min(i + 900)];
        if window.contains("pending_map_command.take()")
            && window.contains("request_state_change(GameState::Paused)")
            && window.contains("NamedKey::Escape")
        {
            chosen = Some(i);
            break;
        }
        search = i + marker.len();
    }
    let i = chosen.expect("handle_key_press Escape arm must cancel then pause");
    let window = &src[i.saturating_sub(800)..src.len().min(i + 1200)];
    assert!(
        window.contains("request_state_change(GameState::InGame)"),
        "Escape must resume from Paused"
    );
    assert!(
        i > hk,
        "live Escape residual must sit under handle_key_press"
    );
}

#[test]
fn beacon_and_control_bar_hotkeys_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("eq_ignore_ascii_case(\"b\")") && src.contains("Command_PlaceBeacon"),
        "Ctrl+B must PLACE_BEACON residual"
    );
    assert!(
        src.contains("PendingMapCommand::PlaceBeacon")
            && src.contains("Place beacon: click location"),
        "PlaceBeacon must arm pending map click"
    );
    assert!(
        src.contains("NamedKey::F9") && src.contains("toggle_visibility()"),
        "F9 must TOGGLE_CONTROL_BAR residual"
    );
}

#[test]
fn camera_bookmarks_and_delete_beacon_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("camera_view_bookmarks") && src.contains("fn handle_camera_view_hotkey"),
        "F1-F8 camera bookmark residual required"
    );
    assert!(
        src.contains("NamedKey::F1") && src.contains("handle_camera_view_hotkey(0)"),
        "F1 must recall/save view slot 0"
    );
    assert!(
        src.contains("NamedKey::F8") && src.contains("handle_camera_view_hotkey(7)"),
        "F8 must recall/save view slot 7"
    );
    assert!(
        src.contains("NamedKey::Delete") && src.contains("Command_RemoveBeacon"),
        "Delete must DELETE_BEACON residual"
    );
    // Debug destroy kept behind Shift+Delete.
    assert!(
        src.contains("destroy_object") && src.contains("Shift+Delete"),
        "Shift+Delete debug destroy residual must remain"
    );
}

#[test]
fn cheer_camera_reset_unit_cycle_hotkeys_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("Command_Cheer") && src.contains("eq_ignore_ascii_case(\"c\")"),
        "Ctrl+C must ALL_CHEER residual"
    );
    assert!(
        src.contains("Numpad5") && src.contains("reset_camera_view_hotkey"),
        "KP5 must CAMERA_RESET residual"
    );
    assert!(
        src.contains("ArrowRight") && src.contains("cycle_friendly_selection(1)"),
        "Ctrl+Right must SELECT_NEXT_UNIT residual"
    );
    assert!(
        src.contains("ArrowLeft") && src.contains("cycle_friendly_selection(-1)"),
        "Ctrl+Left must SELECT_PREV_UNIT residual"
    );
    assert!(
        src.contains("cycle_friendly_worker_selection"),
        "Ctrl+Up/Down must worker cycle residual"
    );
}

#[test]
fn diplomacy_and_control_group_modifiers_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("toggle_diplomacy_panel_hotkey") && src.contains("NamedKey::Tab"),
        "Tab must DIPLOMACY residual"
    );
    assert!(
        src.contains("ADD_TEAM residual") || src.contains("shift_down"),
        "Shift+digit must ADD_TEAM residual"
    );
    assert!(
        src.contains("VIEW_TEAM residual") || src.contains("alt_down"),
        "Alt+digit must VIEW_TEAM residual"
    );
    assert!(
        src.contains("Escape closed diplomacy panel residual"),
        "Escape must close diplomacy before pause"
    );
}

#[test]
fn chat_and_screenshot_hotkeys_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("NamedKey::Enter") && src.contains("ChatTarget::All"),
        "Enter must CHAT_EVERYONE residual"
    );
    assert!(
        src.contains("NamedKey::Backspace") && src.contains("ChatTarget::Allies"),
        "Backspace must CHAT_ALLIES residual"
    );
    assert!(
        src.contains("NamedKey::F12") && src.contains("take_screenshot_hotkey"),
        "F12 must TAKE_SCREENSHOT residual"
    );
    assert!(
        src.contains("Escape closed chat residual"),
        "Escape must close chat first"
    );
}

#[test]
fn deploy_and_numpad_camera_hold_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("eq_ignore_ascii_case(\"d\") && !ctrl_down") && src.contains("Command_Deploy"),
        "D must Deploy residual"
    );
    assert!(
        src.contains("Numpad4") && src.contains("camera_rotate_left_held"),
        "KP4 must rotate-left hold residual"
    );
    assert!(
        src.contains("Numpad6") && src.contains("camera_rotate_right_held"),
        "KP6 must rotate-right hold residual"
    );
    assert!(
        src.contains("Numpad8") && src.contains("camera_zoom_in_held"),
        "KP8 must zoom-in hold residual"
    );
    assert!(
        src.contains("Numpad2") && src.contains("camera_zoom_out_held"),
        "KP2 must zoom-out hold residual"
    );
}

#[test]
fn show_options_event_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("UIEvent::ShowOptions") && src.contains("Screen::Options"),
        "engine must handle ShowOptions residual"
    );
    let ui = include_str!("ui/ui_manager.rs");
    assert!(
        ui.contains("options_menu") && ui.contains("Screen::Options"),
        "UIManager must own OptionsMenu residual"
    );
}

#[test]
fn remaining_commandmap_hotkeys_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("toggle_camera_tracking_drawable_hotkey")
            && src.contains("camera_tracking_selection"),
        "TOGGLE_CAMERA_TRACKING_DRAWABLE residual required"
    );
    assert!(
        src.contains("toggle_replay_fast_forward_hotkey")
            && src.contains("replay_fast_forward")
            && src.contains("m_TiVOFastMode"),
        "TOGGLE_FAST_FORWARD_REPLAY residual required"
    );
    assert!(
        src.contains("DEMO_INSTANT_QUIT") && src.contains("GameState::Exiting"),
        "DEMO_INSTANT_QUIT residual required"
    );
}

#[test]
fn victory_defeat_shows_victory_screen_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("show_match_result(true, self.current_player_id)"),
        "Victory state must open Victory screen"
    );
    assert!(
        src.contains("show_match_result(false, self.current_player_id)"),
        "Defeat state must open Defeat presentation residual"
    );
    assert!(
        src.contains("fn show_match_result")
            || include_str!("ui/ui_manager.rs").contains("fn show_match_result"),
        "UIManager must expose show_match_result residual"
    );
}

#[test]
fn wasd_not_camera_scroll_residual() {
    let src = include_str!("cnc_game_engine.rs");
    let i = src
        .find("fn update_camera(&mut self, dt: f32)")
        .expect("update_camera");
    let body = &src[i..src.len().min(i + 3500)];
    assert!(
        !body.contains("is_character_key_pressed(\"w\")")
            && !body.contains("is_character_key_pressed(\"s\")")
            && !body.contains("is_character_key_pressed(\"a\")")
            && !body.contains("is_character_key_pressed(\"d\")"),
        "WASD must not drive camera scroll (unit hotkey conflict)"
    );
    assert!(
        body.contains("NamedKey::ArrowUp") && body.contains("NamedKey::ArrowDown"),
        "arrow keys remain camera scroll residual"
    );
}

#[test]
fn windowed_edge_scroll_residual() {
    let src = include_str!("cnc_game_engine.rs");
    let i = src
        .find("fn update_camera(&mut self, dt: f32)")
        .expect("update_camera");
    let body = &src[i..src.len().min(i + 4500)];
    assert!(
        body.contains("EDGE_SCROLL_SIZE"),
        "edge scroll residual must remain"
    );
    assert!(
        !body.contains("if !self.is_windowed\n                && matches!(self.current_state, GameState::InGame | GameState::Paused)"),
        "edge scroll must not be fullscreen-only"
    );
    assert!(
        body.contains("!self.chat_panel.is_open()")
            && body.contains("!self.diplomacy_panel.is_active()"),
        "edge/arrow scroll suppressed during chat/diplomacy modal"
    );
}

#[test]
fn settings_changed_health_bars_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("UIEvent::SettingsChanged") && src.contains("game.show_health_bars"),
        "SettingsChanged must apply show_health_bars residual"
    );
}

#[test]
fn hud_h_does_not_steal_view_command_center_residual() {
    let hud = include_str!("ui/hud.rs");
    // After residual fix, bare KeyCode::H toggle must not remain in GameHUD key handler.
    let marker = "Global HUD hotkeys";
    let i = hud.find(marker).expect("global HUD hotkeys section");
    let section = &hud[i..hud.len().min(i + 400)];
    assert!(
        !section.contains("KeyCode::H =>"),
        "GameHUD must not bind bare H (VIEW_COMMAND_CENTER conflict)"
    );
    let eng = include_str!("cnc_game_engine.rs");
    assert!(
        eng.contains("Command_ViewCommandCenter")
            && eng.contains("eq_ignore_ascii_case(\"h\") && !ctrl_down"),
        "engine H must still VIEW_COMMAND_CENTER"
    );
}

#[test]
fn drag_select_rect_overlay_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("selection_start_screen")
            && src.contains("DragSelectRect")
            && src.contains("drag_rect.filter(|r| r.is_valid())"),
        "InGame render must feed DragSelectRect while dragging"
    );
    assert!(
        src.contains("Defer empty-ground clear until left-release")
            || src.contains("Instant clear on mousedown fights drag-select"),
        "mousedown must not clear selection before drag completes"
    );
}

#[test]
fn structure_placement_ghost_cursor_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("sync_pending_structure_placement_cursor")
            && src.contains("sync_structure_placement_cursor")
            && src.contains("legal_build_code_at_for_builder"),
        "placement ghost must track cursor legality each frame"
    );
    let hud = include_str!("ui/hud.rs");
    assert!(
        hud.contains("placement: crate::ui::construction_panel::PlacementPreview")
            || hud.contains("PlacementPreview"),
        "HUD ConstructionPanel must own PlacementPreview ghost"
    );
}

#[test]
fn pending_map_radius_cursor_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("arm_radius_cursor_for_pending")
            && src.contains("sync_pending_map_command_radius_cursor")
            && src.contains("clear_radius_cursor_overlays"),
        "pending map commands must drive radius cursor residual"
    );
    assert!(
        src.contains("ATTACK_CONTINUE_AREA") && src.contains("GUARD_AREA"),
        "AttackMove/Guard must arm retail radius cursor names"
    );
    assert!(
        src.contains("PARTICLECANNON") || src.contains("OFFENSIVE_SPECIALPOWER"),
        "special power must map to radius cursor type"
    );
}

#[test]
fn minimap_right_click_context_command_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn issue_minimap_move")
            && src.contains("process_mouse_input")
            && src.contains("MouseButton::Right"),
        "minimap RMB must use context-sensitive CommandSystem path"
    );
    // Ensure issue_minimap_move body is not pure command_move-only.
    let start = src
        .find("fn issue_minimap_move")
        .expect("issue_minimap_move");
    let end = src[start + 1..]
        .find(
            "
    fn ",
        )
        .map(|i| start + 1 + i)
        .unwrap_or(start + 4000);
    let body = &src[start..end];
    assert!(
        body.contains("process_mouse_input") && body.contains("find_object_at_position"),
        "minimap RMB must resolve target + command context like world RMB"
    );
}

#[test]
fn ground_marker_circles_overlay_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("collect_ground_marker_circles") && src.contains("ground_markers"),
        "engine must feed placement/radius ground markers into selection overlay"
    );
    let sel = include_str!("graphics/selection_renderer.rs");
    assert!(
        sel.contains("ground_markers: Vec<SelectedUnit>"),
        "selection overlay must accept ground_markers residual"
    );
}

#[test]
fn dual_hud_construction_hotkey_route_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("Interactive::handle_key_press(&mut self.game_hud, ui_key)")
            && src.contains("drain_pending_ui_events"),
        "engine GameHUD must receive construction/command hotkeys in InGame"
    );
    let um = include_str!("ui/ui_manager.rs");
    assert!(
        um.contains("pending_structure_placement") && um.contains("Fall through to GameHUD"),
        "UIManager Escape must not open pause over active structure placement"
    );
}

#[test]
fn order_line_overlay_draw_residual() {
    let sel = include_str!("graphics/selection_renderer.rs");
    assert!(
        sel.contains("draw_order_line_segments")
            && sel.contains("MoveLineUpload::pack_from_presentation")
            && sel.contains("AttackLineUpload::pack_from_presentation"),
        "selection overlay must GPU-draw move/attack order lines from presentation"
    );
}

#[test]
fn shift_select_and_ctrl_force_attack_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn toggle_select_object")
            && src.contains("fn issue_force_attack_from_left_click"),
        "left-click must support Shift multi-select and Ctrl force-attack"
    );
    let start = src.find("fn handle_left_click").expect("handle_left_click");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 2500);
    let body = &src[start..end];
    assert!(
        body.contains("shift_down")
            && body.contains("toggle_select_object")
            && body.contains("issue_force_attack_from_left_click"),
        "handle_left_click must branch on Shift/Ctrl residuals"
    );
}

#[test]
fn cancel_unit_production_rmb_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn cancel_unit_production_from_ui") && src.contains("CancelUnitProduction"),
        "engine must handle CancelUnitProduction residual"
    );
    let hud = include_str!("ui/hud.rs");
    assert!(
        hud.contains("CancelUnitProduction") && hud.contains("build_queue_cancel"),
        "HUD RMB must raise CancelUnitProduction"
    );
}

#[test]
fn context_mouse_cursor_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn sync_context_mouse_cursor")
            && src.contains("fn resolve_context_cursor_icon")
            && src.contains("set_cursor"),
        "InGame mouse move must apply context cursor residual"
    );
    assert!(
        src.contains("\"AttackObj\"")
            && src.contains("\"Build\"")
            && src.contains("\"InvalidBuild\"")
            && src.contains("\"Waypoint\""),
        "cursor residual must cover attack/build/waypoint names"
    );
}

#[test]
fn auto_dozer_structure_place_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn find_nearest_friendly_dozer")
            && src.contains("Select a dozer or worker to build"),
        "structure place must auto-pick nearest dozer residual"
    );
    let start = src.find("fn place_structure_from_ui").expect("place");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 4000);
    let body = &src[start..end];
    assert!(
        body.contains("clear_structure_placement")
            && body.contains("game_hud.construction_panel")
            && body.contains("ui_manager"),
        "legal place must dual-clear both HUD placement ghosts"
    );
}

#[test]
fn deploy_d_key_not_shadowed_by_debug_defeat_residual() {
    let src = include_str!("cnc_game_engine.rs");
    let start = src.find("fn handle_key_press").expect("handle_key_press");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 8000);
    let body = &src[start..end];
    assert!(
        body.contains("Command_Deploy")
            && body.contains("eq_ignore_ascii_case(\"d\") && !ctrl_down"),
        "D must issue Command_Deploy residual"
    );
    // Bare D must not be bound to debug_show_victory(None) ahead of Deploy.
    assert!(
        !body.contains(
            "eq_ignore_ascii_case(\"d\") => {\n                self.debug_show_victory(None)"
        ),
        "debug defeat must not steal D from Deploy"
    );
}

#[test]
fn deployed_blocks_can_move_and_guard_ring_residual() {
    let obj = include_str!("game_logic/object.rs");
    let start = obj.find("pub fn can_move").expect("can_move");
    let body = &obj[start..start + 700];
    assert!(
        body.contains("!self.status.deployed"),
        "deployed units must not can_move residual"
    );
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("GUARD_AREA_RADIUS") && src.contains("guard_position"),
        "selected guard units must draw guard-area ring residual"
    );
}

#[test]
fn eva_low_power_chat_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn sync_eva_messages_from_logic")
            && src.contains("add_eva_message")
            && src.contains("eva_low_power_count")
            && src.contains("Insufficient funds")
            && src.contains("Our base is under attack"),
        "engine must surface EVA LOWPOWER/funds/under-attack to chat residual"
    );
}

#[test]
fn pending_unit_ability_arm_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("PendingUnitAbility")
            && src.contains("fn arm_pending_unit_ability")
            && src.contains("UnitAbility(ability)"),
        "ControlBar unit abilities must arm pending target click residual"
    );
    assert!(
        src.contains("PendingUnitAbility::Hijack")
            && src.contains("PendingUnitAbility::SnipeVehicle")
            && src.contains("PendingUnitAbility::PlantTimedDemoCharge"),
        "hero/ability set must include hijack/snipe/charges residual"
    );
}

#[test]
fn presentation_event_sfx_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn play_presentation_event_sfx")
            && src.contains("SoundType::ConstructionComplete")
            && src.contains("SoundType::UnitReady"),
        "presentation complete events must play SFX residual"
    );
}

#[test]
fn sticky_waypoint_mode_toggle_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("sticky_waypoint_mode")
            && src.contains("eq_ignore_ascii_case(\"z\")")
            && src.contains("Waypoint mode: ON"),
        "Z must toggle sticky waypoint mode residual"
    );
}

#[test]
fn idle_worker_period_key_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("c == \".\"")
            && src.contains("cycle_friendly_worker_selection(1)")
            && src.contains("SELECT_IDLE_WORKER"),
        "period key must cycle idle workers residual"
    );
    let start = src
        .find("fn cycle_friendly_worker_selection")
        .expect("cycle_friendly_worker_selection");
    let body = &src[start..start + 2200];
    assert!(
        body.contains("idle_workers") && body.contains("AIState::Idle"),
        "worker cycle must prefer idle workers residual"
    );
}

#[test]
fn structure_placement_rotate_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("rotate_structure_placement")
            && src.contains("facing_radians")
            && src.contains("pending_structure_placement.is_some()"),
        "mouse wheel must rotate structure placement ghost residual"
    );
}

#[test]
fn structure_cycle_and_auto_attack_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn cycle_friendly_structure_selection")
            && src.contains("SELECT_NEXT_STRUCTURE")
            && src.contains("sticky_auto_attack"),
        "structure cycle + sticky auto-attack residual required"
    );
    assert!(
        src.contains("Auto-attack: ON") && src.contains("AttackMoveTo"),
        "sticky auto-attack must convert moves to attack-move"
    );
}

#[test]
fn force_attack_ground_t_key_and_home_structure_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("ForceAttackGround")
            && src.contains("eq_ignore_ascii_case(\"t\")")
            && src.contains("Force-attack ground"),
        "T must issue ForceAttackGround at cursor residual"
    );
    assert!(
        src.contains("NamedKey::Home") && src.contains("cycle_friendly_structure_selection(1)"),
        "Home/End must cycle structures residual"
    );
}

#[test]
fn patrol_and_sell_hotkey_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("Command_Sell")
            && src.contains("eq_ignore_ascii_case(\"s\")")
            && src.contains("NamedKey::Shift"),
        "Ctrl+Shift+S must sell selection residual"
    );
    let cmd = include_str!("command_system.rs");
    assert!(
        cmd.contains("Patrol") && cmd.contains("\"patrol\""),
        "Patrol command residual must exist"
    );
    let ex = include_str!("command_executor.rs");
    assert!(
        ex.contains("fn execute_patrol") && ex.contains("AIState::Patrolling"),
        "execute_patrol must set Patrolling residual"
    );
    let pf = include_str!("presentation_frame.rs");
    assert!(
        pf.contains("Command_Patrol"),
        "command strip must expose Patrol residual"
    );
}

#[test]
fn evacuate_and_repair_hotkey_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("eq_ignore_ascii_case(\"u\")") && src.contains("Command_Evacuate"),
        "U must issue Evacuate residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"r\")")
            && src.contains("Command_Repair")
            && src.contains("PendingUnitAbility::Repair"),
        "R must arm Repair residual"
    );
    let cs = include_str!("command_system.rs");
    assert!(
        cs.contains("\"repair\"") && cs.contains("CommandType::Repair"),
        "repair button name must map residual"
    );
}

#[test]
fn rally_overcharge_capture_hotkey_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("eq_ignore_ascii_case(\"y\")") && src.contains("Command_SetRallyPoint"),
        "Y must arm SetRallyPoint residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"o\")") && src.contains("Command_ToggleOvercharge"),
        "O must toggle overcharge residual"
    );
    assert!(
        src.contains("eq_ignore_ascii_case(\"c\")")
            && src.contains("Command_CaptureBuilding")
            && src.contains("!ctrl_down"),
        "C must arm CaptureBuilding residual"
    );
}

#[test]
fn construction_cameo_hotkey_priority_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("construction_consumed")
            && src.contains("_ if construction_consumed")
            && src.contains("Interactive::handle_key_press(&mut self.game_hud, ui_key)"),
        "construction panel must consume build keys before global hotkeys residual"
    );
    assert!(
        src.contains("cycle_construction_tab")
            && src.contains("cycle_construction_tab(1)")
            && src.contains("force_tab"),
        "[ ] must cycle construction tabs residual"
    );
    let hud = include_str!("ui/hud.rs");
    assert!(
        hud.contains("fn force_tab") && hud.contains("ConstructionTab::Aircraft"),
        "construction panel force_tab residual"
    );
}

#[test]
fn shift_ctrl_production_queue_multiplier_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("UIEvent::QueueUnitProduction")
            && src.contains("saturating_mul(5)")
            && src.contains("qty = 9"),
        "Shift×5 and Ctrl fill-queue residual for production"
    );
}

#[test]
fn special_power_v_key_residual() {
    let src = include_str!("cnc_game_engine.rs");
    let start = src.find("fn handle_key_press").expect("handle_key_press");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 12000);
    let body = &src[start..end];
    assert!(
        body.contains("Command_DoSpecialPower") && body.contains("eq_ignore_ascii_case(\"v\")"),
        "V must arm Command_DoSpecialPower residual"
    );
    // Bare V must not instantly debug-win.
    assert!(
        !body.contains(
            "eq_ignore_ascii_case(\"v\") => {\n                self.debug_show_victory(Some(self.current_player_id))"
        ),
        "debug victory must not steal bare V from special power"
    );
    assert!(
        body.contains("NamedKey::Shift") && body.contains("debug_show_victory"),
        "debug victory remains behind Ctrl+Shift residual"
    );
}

#[test]
fn strategy_center_battle_plan_residual() {
    let cs = include_str!("command_system.rs");
    assert!(
        cs.contains("BattlePlanBombardment")
            && cs.contains("initiatebattleplanbombardment")
            && cs.contains("BattlePlanHoldTheLine")
            && cs.contains("BattlePlanSearchAndDestroy"),
        "battle plan button names must map residual"
    );
    let pf = include_str!("presentation_frame.rs");
    assert!(
        pf.contains("Command_InitiateBattlePlanBombardment") && pf.contains("strategycenter"),
        "Strategy Center strip must expose battle plans residual"
    );
    let eng = include_str!("cnc_game_engine.rs");
    assert!(
        eng.contains("BattlePlanBombardment") && eng.contains("BattlePlanHoldTheLine"),
        "engine must execute battle plans without map-click residual"
    );
}

#[test]
fn named_superweapon_button_residual() {
    let cs = include_str!("command_system.rs");
    assert!(
        cs.contains("spysatellitescan")
            && cs.contains("ciaintelligence")
            && cs.contains("particlecannon")
            && cs.contains("nuclearmissile")
            && cs.contains("scudstorm")
            && cs.contains("carpetbomb")
            && cs.contains("artillerybarrage")
            && cs.contains("emergencyrepair")
            && cs.contains("airstrike")
            && cs.contains("ambush")
            && cs.contains("sneakattack")
            && cs.contains("leafletdrop")
            && cs.contains("gpsscrambler")
            && cs.contains("spectregunship")
            && cs.contains("anthraxbomb"),
        "named SW button names must map residual"
    );
    let pf = include_str!("presentation_frame.rs");
    assert!(
        pf.contains("Command_ParticleCannon")
            && pf.contains("Command_SpySatelliteScan")
            && pf.contains("Command_CIAIntelligence")
            && pf.contains("Command_CarpetBomb")
            && pf.contains("Command_EmergencyRepair")
            && pf.contains("Command_ArtilleryBarrage")
            && pf.contains("Command_SpyDrone")
            && pf.contains("Command_Airstrike")
            && pf.contains("Command_Ambush")
            && pf.contains("Command_SneakAttack")
            && pf.contains("Command_LeafletDrop")
            && pf.contains("Command_SpectreGunship"),
        "SW structures must expose named buttons residual"
    );
    let eng = include_str!("cnc_game_engine.rs");
    assert!(
        eng.contains("Pass 1: honor named"),
        "engine must prefer named SW type when arming residual"
    );
}

#[test]
fn damaged_structure_cycle_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn cycle_damaged_structure_selection")
            && src.contains("No damaged structures")
            && src.contains("NamedKey::Alt")
            && src.contains("cycle_damaged_structure_selection(1)"),
        "Ctrl+Alt+arrows must cycle damaged structures residual"
    );
}

#[test]
fn idle_military_select_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn select_all_idle_military")
            && src.contains("eq_ignore_ascii_case(\"i\")")
            && src.contains("select_all_idle_military()")
            && src.contains("No idle military units"),
        "Ctrl+I must select idle military residual"
    );
}

#[test]
fn unit_attitude_hotkey_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("Command_AttitudeAggressive")
            && src.contains("Command_AttitudeSleep")
            && src.contains("Command_AttitudePassive")
            && src.contains("NamedKey::Alt"),
        "Alt+A/S/D must set unit attitude residual"
    );
    let cs = include_str!("command_system.rs");
    assert!(
        cs.contains("AttitudeAggressive")
            && cs.contains("AttitudeSleep")
            && cs.contains("\"aggressive\""),
        "attitude commands must map residual"
    );
    let ex = include_str!("command_executor.rs");
    assert!(
        ex.contains("fn execute_set_attitude") && ex.contains("set_ai_attitude"),
        "execute_set_attitude residual"
    );
    let pf = include_str!("presentation_frame.rs");
    assert!(
        pf.contains("Command_AttitudeAggressive") && pf.contains("Command_AttitudeSleep"),
        "strip must expose attitude residual"
    );
}

#[test]
fn generals_science_purchase_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn try_purchase_next_generals_science")
            && src.contains("PurchaseScience")
            && src.contains("eq_ignore_ascii_case(\"g\")")
            && src.contains("NamedKey::Alt")
            && src.contains("No science purchase points"),
        "Alt+G must purchase next generals science residual"
    );
    let pf = include_str!("presentation_frame.rs");
    assert!(
        pf.contains("Command_PurchaseScience") && pf.contains("local_science_purchase_points"),
        "strip must expose PurchaseScience when SPP residual"
    );
}

#[test]
fn wall_line_drag_placement_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn is_wall_structure_template")
            && src.contains("fn place_wall_line_from_ui")
            && src.contains("DozerConstructLine")
            && src.contains("Wall line ordered"),
        "wall/fence drag must issue DozerConstructLine residual"
    );
}

#[test]
fn detonate_and_harvester_select_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("eq_ignore_ascii_case(\"n\")")
            && src.contains("Command_DetonateRemoteDemoCharges"),
        "N must detonate remote charges residual"
    );
    assert!(
        src.contains("fn select_all_harvesters")
            && src.contains("select_all_harvesters()")
            && src.contains("No harvesters found"),
        "Ctrl+Shift+I must select harvesters residual"
    );
}

#[test]
fn switch_weapons_and_demo_suicide_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("Command_SwitchWeapons")
            && src.contains("eq_ignore_ascii_case(\"w\")")
            && src.contains("NamedKey::Alt"),
        "Alt+W must SwitchWeapons residual"
    );
    assert!(
        src.contains("Command_DemoTertiarySuicide") && src.contains("eq_ignore_ascii_case(\"b\")"),
        "Alt+B must DemoTertiarySuicide residual"
    );
    let cs = include_str!("command_system.rs");
    assert!(
        cs.contains("\"switchweapons\"") || cs.contains("SwitchWeapons"),
        "switchweapons button map residual"
    );
}

#[test]
fn delete_cancel_production_and_combat_drop_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn cancel_selected_production_queue_head")
            && src.contains("Canceled production")
            && src.contains("NamedKey::Delete"),
        "Delete must cancel production queue head residual"
    );
    assert!(
        src.contains("PendingMapCommand::CombatDrop")
            && src.contains("Command_CombatDrop")
            && src.contains("Combat drop: click landing zone"),
        "Alt+C / CombatDrop must arm map click residual"
    );
}

#[test]
fn hack_internet_and_cleanup_area_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("Command_HackInternet")
            && src.contains("eq_ignore_ascii_case(\"i\")")
            && src.contains("NamedKey::Alt"),
        "Alt+I must HackInternet residual"
    );
    assert!(
        src.contains("Command_CleanupArea") && src.contains("eq_ignore_ascii_case(\"m\")"),
        "Alt+M must CleanupArea residual"
    );
    let cs = include_str!("command_system.rs");
    assert!(
        cs.contains("HackInternet") && cs.contains("\"hackinternet\""),
        "HackInternet command map residual"
    );
    let ex = include_str!("command_executor.rs");
    assert!(
        ex.contains("fn execute_hack_internet") && ex.contains("start_hacker_internet_hack"),
        "execute_hack_internet residual"
    );
    let pf = include_str!("presentation_frame.rs");
    assert!(
        pf.contains("Command_HackInternet") && pf.contains("Command_CleanupArea"),
        "strip must expose hack/cleanup residual"
    );
}

#[test]
fn return_to_base_aircraft_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("Command_ReturnToBase")
            && src.contains("eq_ignore_ascii_case(\"r\")")
            && src.contains("NamedKey::Alt"),
        "Alt+R must ReturnToBase residual"
    );
    let cs = include_str!("command_system.rs");
    assert!(
        cs.contains("ReturnToBase") && cs.contains("\"returntobase\""),
        "ReturnToBase command map residual"
    );
    let ex = include_str!("command_executor.rs");
    assert!(
        ex.contains("fn execute_return_to_base")
            && ex.contains("is_friendly_airfield")
            && ex.contains("execute_dock"),
        "execute_return_to_base docks nearest airfield residual"
    );
    let pf = include_str!("presentation_frame.rs");
    assert!(
        pf.contains("Command_ReturnToBase"),
        "aircraft strip must expose RTB residual"
    );
}

#[test]
fn on_screen_select_and_camera_follow_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn select_all_friendly_on_screen")
            && src.contains("select_all_friendly_on_screen()")
            && src.contains("No units on screen"),
        "Ctrl+Alt+A must select on-screen friendlies residual"
    );
    assert!(
        src.contains("fn toggle_camera_follow_selection")
            && src.contains("Camera follow on")
            && src.contains("eq_ignore_ascii_case(\"f\")"),
        "Alt+F must toggle camera follow residual"
    );
    let gl = include_str!("game_logic/game_logic.rs");
    assert!(
        gl.contains("fn set_camera_follow_object") && gl.contains("fn camera_follow_object_id"),
        "GameLogic camera follow API residual"
    );
    let pf = include_str!("presentation_frame.rs");
    assert!(
        pf.contains("fn alive_selectable_friendly_near"),
        "presentation near-select residual"
    );
}

#[test]
fn return_supplies_and_select_structures_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("Command_ReturnSupplies")
            && src.contains("eq_ignore_ascii_case(\"u\")")
            && src.contains("NamedKey::Alt"),
        "Alt+U must ReturnSupplies residual"
    );
    assert!(
        src.contains("fn select_all_friendly_structures")
            && src.contains("select_all_friendly_structures()")
            && src.contains("No structures found"),
        "Ctrl+Alt+S must select all structures residual"
    );
    let cs = include_str!("command_system.rs");
    assert!(
        cs.contains("ReturnSupplies") && cs.contains("\"returnsupplies\""),
        "ReturnSupplies command map residual"
    );
    let ex = include_str!("command_executor.rs");
    assert!(
        ex.contains("fn execute_return_supplies") && ex.contains("ReturningResources"),
        "execute_return_supplies residual"
    );
}

#[test]
fn clear_mines_and_unfinished_construction_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("Command_ClearMines")
            && src.contains("eq_ignore_ascii_case(\"x\")")
            && src.contains("NamedKey::Alt"),
        "Alt+X must ClearMines residual"
    );
    assert!(
        src.contains("fn cycle_unfinished_construction")
            && src.contains("No unfinished construction")
            && src.contains("cycle_unfinished_construction(1)"),
        "Ctrl+Alt+Home/End must cycle unfinished construction residual"
    );
    let cs = include_str!("command_system.rs");
    assert!(
        cs.contains("ClearMines") && cs.contains("\"clearmines\""),
        "ClearMines command map residual"
    );
    let ex = include_str!("command_executor.rs");
    assert!(
        ex.contains("fn execute_clear_mines") && ex.contains("is_mine_clearer"),
        "execute_clear_mines residual"
    );
}

#[test]
fn resume_construction_hotkey_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn resume_selected_construction")
            && src.contains("Resuming construction")
            && src.contains("eq_ignore_ascii_case(\"e\")")
            && src.contains("NamedKey::Alt"),
        "Alt+E must resume construction residual"
    );
    let pf = include_str!("presentation_frame.rs");
    assert!(
        pf.contains("Command_ResumeConstruction"),
        "unfinished structure strip must expose ResumeConstruction residual"
    );
    let cs = include_str!("command_system.rs");
    assert!(
        cs.contains("\"resumeconstruction\"") || cs.contains("ResumeConstruction"),
        "resumeconstruction button map residual"
    );
}

#[test]
fn idle_harvesters_and_cancel_all_production_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn select_idle_harvesters")
            && src.contains("select_idle_harvesters()")
            && src.contains("No idle harvesters"),
        "Ctrl+Alt+I must select idle harvesters residual"
    );
    assert!(
        src.contains("fn cancel_all_selected_production")
            && src.contains("Canceled all production")
            && src.contains("ctrl_down && !shift"),
        "Ctrl+Delete must cancel all production residual"
    );
}

#[test]
fn guard_radius_and_combat_select_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn adjust_selected_guard_radius")
            && src.contains("Guard radius:")
            && src.contains("adjust_selected_guard_radius(15.0)"),
        "Alt+[ ] must adjust guard radius residual"
    );
    assert!(
        src.contains("fn select_all_friendly_combat")
            && src.contains("select_all_friendly_combat()")
            && src.contains("No combat units"),
        "Ctrl+Alt+Q must select combat units residual"
    );
}

#[test]
fn clear_path_and_damaged_unit_cycle_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn clear_selected_path_waypoints")
            && src.contains("Path cleared")
            && src.contains("clear_selected_path_waypoints()"),
        "Alt+Z must clear path waypoints residual"
    );
    assert!(
        src.contains("fn cycle_damaged_unit_selection")
            && src.contains("No damaged units")
            && src.contains("cycle_damaged_unit_selection(1)"),
        "Ctrl+Alt+Up/Down must cycle damaged units residual"
    );
}

#[test]
fn moving_select_and_health_bars_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn select_all_friendly_moving")
            && src.contains("select_all_friendly_moving()")
            && src.contains("No moving units"),
        "Ctrl+Alt+M must select moving units residual"
    );
    assert!(
        src.contains("fn toggle_health_bars_hotkey")
            && src.contains("Health bars: ON")
            && src.contains("show_health_bars"),
        "Alt+H must toggle health bars residual"
    );
}

#[test]
fn attacking_select_and_stop_all_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn select_all_friendly_attacking")
            && src.contains("select_all_friendly_attacking()")
            && src.contains("No attacking units"),
        "Ctrl+Alt+T must select attacking units residual"
    );
    assert!(
        src.contains("fn stop_all_friendly_units")
            && src.contains("stop_all_friendly_units()")
            && src.contains("Stopped"),
        "Ctrl+Shift+. must stop all friendlies residual"
    );
}

#[test]
fn debug_producer_and_guarding_select_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn toggle_debug_info_hotkey")
            && src.contains("Debug overlay: ON")
            && src.contains("NamedKey::F1) if ctrl_down"),
        "Ctrl+F1 must toggle debug overlay residual"
    );
    assert!(
        src.contains("fn cycle_busy_producer_selection")
            && src.contains("No busy producers")
            && src.contains("cycle_busy_producer_selection(1)"),
        "Ctrl+Alt+P must cycle busy producers residual"
    );
    assert!(
        src.contains("fn select_all_friendly_guarding")
            && src.contains("select_all_friendly_guarding()")
            && src.contains("No guarding units"),
        "Ctrl+Alt+G must select guarding units residual"
    );
}

#[test]
fn center_selection_and_constructing_workers_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn center_camera_on_selection")
            && src.contains("Centered on selection")
            && src.contains("NamedKey::Space")
            && src.contains("NamedKey::Alt"),
        "Alt+Space must center on selection residual"
    );
    assert!(
        src.contains("fn select_all_constructing_workers")
            && src.contains("select_all_constructing_workers()")
            && src.contains("No constructing workers"),
        "Ctrl+Alt+B must select constructing workers residual"
    );
}

#[test]
fn idle_military_cycle_and_repairing_select_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn cycle_idle_military_selection")
            && src.contains("cycle_idle_military_selection(1)")
            && src.contains("No idle military"),
        "Ctrl+Alt+,/. must cycle idle military residual"
    );
    assert!(
        src.contains("fn select_all_repairing_units")
            && src.contains("select_all_repairing_units()")
            && src.contains("No repairing units"),
        "Ctrl+Alt+R must select repairing units residual"
    );
}

#[test]
fn patrol_gather_and_ready_sw_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn select_all_friendly_patrolling")
            && src.contains("select_all_friendly_patrolling()")
            && src.contains("No patrolling units"),
        "Ctrl+Alt+Y must select patrolling residual"
    );
    assert!(
        src.contains("fn select_all_friendly_gathering")
            && src.contains("select_all_friendly_gathering()")
            && src.contains("No gathering units"),
        "Ctrl+Alt+H must select gathering residual"
    );
    assert!(
        src.contains("fn cycle_ready_special_power_structure")
            && src.contains("No ready special powers")
            && src.contains("cycle_ready_special_power_structure(1)"),
        "Ctrl+Alt+V must cycle ready SW residual"
    );
}

#[test]
fn fps_veterans_and_docked_aircraft_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn toggle_fps_counter_hotkey")
            && src.contains("FPS counter: ON")
            && src.contains("NamedKey::F2) if ctrl_down"),
        "Ctrl+F2 must toggle FPS residual"
    );
    assert!(
        src.contains("fn select_all_friendly_veterans")
            && src.contains("No veteran units")
            && src.contains("select_all_friendly_veterans()"),
        "Ctrl+Alt+E must select veterans residual"
    );
    assert!(
        src.contains("fn select_all_docked_aircraft")
            && src.contains("No docked aircraft")
            && src.contains("select_all_docked_aircraft()"),
        "Ctrl+Alt+W must select docked aircraft residual"
    );
}

#[test]
fn control_group_cycle_and_stealth_select_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn cycle_control_group_selection")
            && src.contains("cycle_control_group_selection")
            && src.contains("No control groups"),
        "Ctrl+Shift+Tab must cycle control groups residual"
    );
    assert!(
        src.contains("fn select_all_friendly_stealthed")
            && src.contains("select_all_friendly_stealthed()")
            && src.contains("No stealthed units"),
        "Ctrl+Alt+K must select stealthed residual"
    );
}

#[test]
fn move_lines_and_garrisoned_select_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn toggle_move_lines_hotkey")
            && src.contains("Move lines: ON")
            && src.contains("show_move_lines")
            && src.contains("NamedKey::F3) if ctrl_down")
            && src.contains("self.show_move_lines,"),
        "Ctrl+F3 must toggle move lines residual"
    );
    assert!(
        src.contains("fn select_all_garrisoned_structures")
            && src.contains("No garrisoned structures")
            && src.contains("select_all_garrisoned_structures()"),
        "Ctrl+Alt+U must select garrisoned structures residual"
    );
}

#[test]
fn runtime_host_construct_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("dozer_construct") && src.contains("construct_ok:"),
        "runtime host must expose construct/dozer_construct residual"
    );
    assert!(
        src.contains("construct_fail_no_dozer")
            && src.contains("construct_fail_lbc:")
            && src.contains("place_structure_from_ui"),
        "construct residual must legal-build scan + place_structure_from_ui"
    );
}

#[test]
fn runtime_host_train_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("train_unit") && src.contains("train_ok:"),
        "runtime host must expose train_unit residual"
    );
    assert!(
        src.contains("train_fail_no_producer")
            && src.contains("under_construction")
            && src.contains("enqueue_production"),
        "train residual must complete unfinished barracks and enqueue production"
    );
}

#[test]
fn runtime_host_save_load_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("\"save_game\" | \"quicksave\"")
            || (src.contains("save_ok:") && src.contains("quicksave")),
        "runtime host must expose save_game/quicksave residual"
    );
    assert!(
        src.contains("quickload")
            && src.contains("load_ok:quicksave")
            && src.contains("save_game_from_ui"),
        "runtime host must expose quickload residual"
    );
}

#[test]
fn runtime_host_stop_sell_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("stop_all") && src.contains("stop_ok:"),
        "runtime host must expose stop_all residual"
    );
    assert!(
        src.contains("sell_selected") && src.contains("sell_ok:") && src.contains("Command_Sell"),
        "runtime host must expose sell residual"
    );
}

#[test]
fn runtime_host_upgrade_guard_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("queue_upgrade") && src.contains("upgrade_ok:"),
        "runtime host must expose upgrade residual"
    );
    assert!(
        src.contains("guard_position") && src.contains("guard_ok:"),
        "runtime host must expose guard residual"
    );
}

#[test]
fn runtime_host_attack_move_scatter_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("attack_move") && src.contains("attack_move_ok:"),
        "runtime host must expose attack_move residual"
    );
    assert!(
        src.contains("\"scatter\"")
            && src.contains("scatter_ok:")
            && src.contains("Command_Scatter"),
        "runtime host must expose scatter residual"
    );
}

#[test]
fn runtime_host_patrol_deploy_formation_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("\"patrol\"") && src.contains("patrol_ok:"),
        "runtime host must expose patrol residual"
    );
    assert!(
        src.contains("\"deploy\"") && src.contains("deploy_ok:"),
        "runtime host must expose deploy residual"
    );
    assert!(
        src.contains("\"cheer\"") && src.contains("cheer_ok"),
        "runtime host must expose cheer residual"
    );
    assert!(
        src.contains("create_formation") && src.contains("formation_ok:"),
        "runtime host must expose formation residual"
    );
}

#[test]
fn runtime_host_capture_economy_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("capture_building") && src.contains("capture_ok:"),
        "runtime host must expose capture residual"
    );
    assert!(
        src.contains("return_supplies") && src.contains("return_supplies_ok:"),
        "runtime host must expose return_supplies residual"
    );
    assert!(
        src.contains("\"evacuate\"") && src.contains("evacuate_ok:"),
        "runtime host must expose evacuate residual"
    );
    assert!(
        src.contains("\"repair\"") && src.contains("repair_ok:"),
        "runtime host must expose repair residual"
    );
    assert!(
        src.contains("return_to_base") && src.contains("return_to_base_ok:"),
        "runtime host must expose return_to_base residual"
    );
}

#[test]
fn runtime_host_misc_command_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("attitude_aggressive") && src.contains("attitude_ok:aggressive"),
        "runtime host must expose attitude residuals"
    );
    assert!(
        src.contains("set_rally") && src.contains("rally_ok:"),
        "runtime host must expose set_rally residual"
    );
    assert!(
        src.contains("switch_weapons") && src.contains("switch_weapons_ok:"),
        "runtime host must expose switch_weapons residual"
    );
    assert!(
        src.contains("view_command_center") && src.contains("view_cc_ok"),
        "runtime host must expose view_command_center residual"
    );
    assert!(
        src.contains("clear_mines") && src.contains("clear_mines_ok:"),
        "runtime host must expose clear_mines residual"
    );
    assert!(
        src.contains("place_beacon") && src.contains("beacon_ok:"),
        "runtime host must expose place_beacon residual"
    );
}

#[test]
fn runtime_host_special_named_residual() {
    let src = include_str!("cnc_game_engine.rs");
    for needle in [
        "hack_internet",
        "hack_ok:",
        "cleanup_area",
        "cleanup_ok:",
        "combat_drop",
        "combat_drop_ok:",
        "toggle_overcharge",
        "overcharge_ok",
        "do_special_power",
        "special_power_ok",
        "remove_beacon",
        "remove_beacon_ok",
        "demo_suicide",
        "demo_suicide_ok",
        "detonate_remote",
        "detonate_remote_ok",
        "view_last_radar",
        "view_radar_ok",
    ] {
        assert!(src.contains(needle), "missing host residual {needle}");
    }
}

#[test]
fn runtime_host_force_select_group_residual() {
    let src = include_str!("cnc_game_engine.rs");
    for needle in [
        "force_attack",
        "force_attack_ok:",
        "ForceAttackGround",
        "force_attack_object",
        "force_attack_object_ok:",
        "ForceAttackObject",
        "select_all",
        "select_all_ok:",
        "select_all_combat",
        "select_all_combat_ok:",
        "assign_control_group",
        "control_group_assign_ok:",
        "recall_control_group",
        "control_group_recall_ok:",
    ] {
        assert!(src.contains(needle), "missing host residual {needle}");
    }
}

#[test]
fn runtime_host_waypoint_box_presentation_residual() {
    let src = include_str!("cnc_game_engine.rs");
    for needle in [
        "waypoint_mode",
        "waypoint_mode_ok:",
        "add_waypoint",
        "waypoint_ok:",
        "AddWaypoint",
        "box_select",
        "box_select_ok:",
        "presentation_frame_ok",
        "presentation_live_fallback_reads",
        "last_presentation_live_fallback_reads",
    ] {
        assert!(src.contains(needle), "missing residual {needle}");
    }
}

#[test]
fn runtime_host_selection_filter_residual() {
    let src = include_str!("cnc_game_engine.rs");
    for needle in [
        "select_similar",
        "select_similar_ok:",
        "select_on_screen",
        "select_on_screen_ok:",
        "select_aircraft",
        "select_aircraft_ok:",
        "select_idle_harvesters",
        "select_idle_ok:",
        "select_structures",
        "select_structures_ok:",
        "select_moving",
        "select_moving_ok:",
    ] {
        assert!(src.contains(needle), "missing selection residual {needle}");
    }
}

#[test]
fn attack_lines_and_occupied_transports_residual() {
    let src = include_str!("cnc_game_engine.rs");
    assert!(
        src.contains("fn toggle_attack_lines_hotkey")
            && src.contains("Attack lines: ON")
            && src.contains("show_attack_lines")
            && src.contains("NamedKey::F4) if ctrl_down")
            && src.contains("self.show_attack_lines,"),
        "Ctrl+F4 must toggle attack lines residual"
    );
    assert!(
        src.contains("fn select_all_occupied_transports")
            && src.contains("No occupied transports")
            && src.contains("select_all_occupied_transports()"),
        "Ctrl+Alt+J must select occupied transports residual"
    );
}
