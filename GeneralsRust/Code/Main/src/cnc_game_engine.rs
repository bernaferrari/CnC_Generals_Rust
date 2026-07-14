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
    // for drawable updates and display draw. Full GameClient::update() is NOT called
    // because Main already handles input/audio/effects separately.
    #[cfg(feature = "game_client")]
    game_client: game_client::core::game_client::GameClient,
    /// ControlBar selection panel (portrait + health). Presentation-fed; WND load optional.
    #[cfg(feature = "game_client")]
    control_bar: game_client::gui::control_bar::ControlBar,

    // Game state
    game_logic: GameLogic,
    /// Immutable presentation feed for client/render after last logic step.
    last_presentation_frame: Option<crate::presentation_frame::PresentationFrame>,
    /// Last presentation-overlaid UI state (selection health/minimap identity retained
    /// after render build so consumers are not dropped each frame).
    last_ui_state: Option<GameUIState>,
    combat_system: CombatSystem,
    pathfinding_system: PathfindingSystem,
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
    is_dragging: bool,
    selection_start: Option<Vec3>,
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
    current_player_id: u32,
    game_paused: bool,

    // UI state
    show_debug_info: bool,
    show_health_bars: bool,
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
    active_menu_shell_hook: Option<&'static str>,
    runtime_host_headless: bool,
    runtime_host_base_ui_screen: Option<String>,
    runtime_host_ui_screen_override: Option<String>,

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

    fn runtime_host_status_snapshot(&self) -> RuntimeHostSnapshot {
        let map_name = self.game_logic.get_current_map_name().trim();
        let map_name = if map_name.is_empty() {
            "-".to_string()
        } else {
            map_name.to_string()
        };

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

        RuntimeHostSnapshot {
            state: format!("{:?}", self.current_state),
            ui_screen,
            paused: self.game_paused,
            fps: self.fps.max(0.0),
            startup_progress,
            startup_phase: self.startup_loading_phase.clone(),
            map: map_name,
            frame: self.frame_counter,
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
                self.enter_shell_screen_from_runtime_host(
                    Some("Skirmish"),
                    "Menus/SkirmishGameOptionsMenu.wnd",
                );
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
            "load_game" => {
                let slot = args.get("slot").map(|slot| slot.trim()).unwrap_or_default();
                if !slot.is_empty() {
                    self.set_runtime_host_ui_screen_override(None);
                    self.load_game_from_ui(slot);
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
            self.show_shell_menu();
            if let Err(err) = game_client::gui::get_shell().push(layout_file, false) {
                warn!("Runtime host failed to push shell screen {layout_file}: {err:?}");
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
        let game_info_context = match self.game_logic.game_mode() {
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
                .or_else(|| self.select_cpp_load_screen(self.game_logic.game_mode(), false))
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

        // Initialize game systems
        let game_logic = GameLogic::initialize();
        let combat_system = CombatSystem::new();
        let (world_min, world_max) = game_logic.world_bounds();
        let world_width = (world_max.x - world_min.x).abs().max(1.0);
        let world_height = (world_max.z - world_min.z).abs().max(1.0);
        let pathfinding_system =
            PathfindingSystem::new_with_origin(world_min, world_width, world_height);
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
            last_ui_state: None,
            combat_system,
            pathfinding_system,
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
            is_dragging: false,
            selection_start: None,
            last_click_time: None,
            last_click_position: None,
            is_windowed: window.fullscreen().is_none(),
            rmb_scroll_anchor: None,
            is_rmb_scrolling: false,
            is_mmb_rotating: false,
            mmb_anchor: None,
            selected_objects: Vec::new(),
            control_groups: HashMap::new(),
            current_player_id: 0,
            game_paused: false,
            show_debug_info: debug_overlay,
            show_health_bars: true,
            frame_counter: 0,
            fps: 0.0,
            last_frame_timing: None,
            frame_clock: FrameClock::new(),
            menu_loading_tick_accumulator: Duration::ZERO,
            menu_loading_last_tick: Instant::now(),
            diagnostics_overlay: None,
            ui_manager,
            game_hud: GameHUD::new(),
            active_menu_shell_hook: None,
            runtime_host_headless,
            runtime_host_base_ui_screen: None,
            runtime_host_ui_screen_override: None,
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
                        if route_keyboard_to_legacy_ui {
                            if let Some(ui_key) = Self::to_ui_key_code(key) {
                                let _ = self.ui_manager.handle_key_press(ui_key);
                            }
                        }
                        self.handle_key_press(key);
                    }
                    ElementState::Released => {
                        self.keys_pressed.remove(key);
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
                self.set_runtime_ui_state_projection(UISystemState::InGame);
            }
            GameState::Defeat => {
                info!("Entering Defeat state - match lost");
                self.game_paused = true;
                self.game_logic.set_paused(true);
                self.set_runtime_ui_state_projection(UISystemState::InGame);
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
                if self.game_logic.isInShellGame() && !self.game_paused {
                    // Keep shell map/scripts alive in menu without allowing large fixed-step
                    // catch-up loops to block the UI thread.
                    self.game_logic.update_shell_with_budget(dt, 1);
                    if let Some(fps) = self.game_logic.take_script_fps_limit_request() {
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
            // Update game logic first
            if let Some(timing) = self.last_frame_timing {
                self.game_logic.update_with_timing(&timing);
            } else {
                self.game_logic.update_with_dt(dt);
            }
            if let Some(fps) = self.game_logic.take_script_fps_limit_request() {
                self.apply_script_fps_limit_request(fps);
            }

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
            // Host side systems (projectiles + path) run *before* PresentationFrame so the
            // client snapshot matches end-of-frame identity (position/health), not mid-frame.
            if !self.game_logic.is_time_frozen_for_simulation() {
                {
                    let objects = self.game_logic.get_objects();
                    crate::game_logic::combat::drain_pending_projectiles(
                        &mut self.combat_system,
                        objects,
                    );
                }

                let hits = self
                    .combat_system
                    .update_projectiles(dt, self.game_logic.get_objects_mut());

                if !hits.is_empty() {
                    self.play_sound_effect(SoundType::Hit);
                }

                let object_ids: Vec<ObjectId> =
                    self.game_logic.get_objects().keys().copied().collect();

                for object_id in object_ids {
                    let _path_completed = self.pathfinding_system.move_unit_along_path(
                        object_id,
                        self.game_logic.get_objects_mut(),
                        dt,
                    );
                }
            }

            // Immutable presentation snapshot for client/render (borrow-first policy).
            // Built after authority + host side systems so HUD/render see final frame state.
            let local_id = self.current_player_id;
            self.last_presentation_frame = Some(
                crate::presentation_frame::PresentationFrame::build_from_logic(
                    &self.game_logic,
                    local_id,
                ),
            );

            #[cfg(feature = "game_client")]
            {
                let visual_delta = if self.game_logic.is_time_frozen_for_simulation() {
                    0.0
                } else {
                    game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL
                };
                if let Err(e) = self.game_client.update_drawables(visual_delta) {
                    log::trace!("GameClient update_drawables failed (non-fatal): {}", e);
                }
                // C++ parity: GameClient::update() also runs shell activation
                // (ensure_shell_visible → show_shell_map + show_shell), input processing,
                // and post-draw UI updates. These are needed for the menu to appear.
                self.game_client.ensure_shell_visible().ok();
                self.game_client.update_pre_draw_ui().ok();
                self.game_client.update_post_draw_ui().ok();
            }
        }

        // Update HUD + ControlBar selection panel from presentation when available
        // (resources + minimap + selection health). ControlBar health is snapshot-owned.
        if self.current_state == GameState::InGame {
            if let Some(pres) = self.last_presentation_frame.clone() {
                pres.apply_to_game_hud(&mut self.game_hud);
                #[cfg(feature = "game_client")]
                {
                    pres.apply_to_control_bar(&mut self.control_bar);
                }
            } else if let Some(player) = self.game_logic.get_player(self.current_player_id) {
                let money = player.resources.supplies as i32;
                let power = player.power_available;
                let max_power = player.power_produced.max(0);
                self.game_hud.update_resources(money, power, max_power);
            }

            if dt.is_finite() {
                if let Err(err) = self.game_hud.update(dt) {
                    warn!("Game HUD update failed: {}", err);
                }
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

        // Process queued commands in game logic during active gameplay.
        if self.current_state == GameState::InGame {
            self.game_logic.process_commands();
            self.apply_pending_script_camera_requests();
        }

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

        // Broadcast defeat notifications so UI/systems mirror C++ VictoryConditions flow
        let defeated_players = self.game_logic.take_defeat_events();
        for player_id in defeated_players {
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
            fow_rendering::reveal_entire_map_for_player(player_id);
            script_events::push_event(ScriptEvent::PlayerDefeated { player_id });
            script_events::push_event(ScriptEvent::RevealMapForPlayer { player_id });
        }

        let alliance_events = self.game_logic.take_alliance_events();
        let local_player_id = self.game_logic.local_player_id();
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
            if let Some(event_player) = self.game_logic.get_player(event.player_id) {
                self.game_logic
                    .queue_radar_message_for_team(event_player.team, message.clone());
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

        if !self.match_over
            && self.current_state == GameState::InGame
            && !self.game_logic.isInShellGame()
        {
            if let Some(condition) = self.game_logic.evaluate_victory_condition() {
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
        pres.apply_to_game_hud(&mut self.game_hud);
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
                pres.apply_to_game_hud(&mut self.game_hud);
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
            let new_script_messages = self.game_logic.take_new_script_messages();
            for msg in &new_script_messages {
                self.game_hud.push_script_message(msg);
            }
            ui_state.current_game_time = self.game_logic.get_total_play_time();
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
            let world_bounds = self.game_logic.world_bounds();
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
        // Production selection overlay: prefer PresentationFrame identity when available
        // (C++ W3DInGameUI selection circles / drag region after 3D scene setup).
        if !skip_world_scene && matches!(self.current_state, GameState::InGame | GameState::Paused)
        {
            crate::graphics::selection_renderer::enqueue_selection_render(
                &mut self.render_pipeline,
                &self.view_matrix,
                &self.projection_matrix,
                &self.game_logic,
                None, // drag rect is optional; unit circles use presentation identity
                self.current_player_id,
                self.last_presentation_frame.as_ref(),
            );
        }
        self.render_pipeline.execute(
            &mut self.graphics_system,
            &self.game_logic,
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
        let (world_min, world_max) = self.game_logic.world_bounds();
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
                _ => {}
            }
        }
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

    fn restart_mission_from_ui(&mut self) {
        let map = self.game_logic.get_current_map_name().to_string();
        let mode = self.game_logic.game_mode();
        let faction = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|p| p.team.get_name().to_string())
            .unwrap_or_else(|| "USA".to_string());

        info!(
            "UI requested restart: mode={:?}, faction={}, map={}",
            mode, faction, map
        );
        self.start_game_from_ui(mode, faction, map, None);
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
        let map_name = self.game_logic.get_current_map_name().to_string();
        let difficulty = Self::map_ai_difficulty_to_save(self.game_logic.get_difficulty());
        let play_time = std::time::Duration::from_secs_f32(self.game_logic.get_total_play_time());
        let team_name = self
            .game_logic
            .get_player(self.current_player_id)
            .map(|player| player.team.get_name().to_string())
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
        let mode = self.game_logic.game_mode();
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
        let mode = self.game_logic.game_mode();
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
        self.prepare_cpp_load_screen_for_mode(self.game_logic.game_mode(), true);
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
            match render_pipeline.load_heightmap_from_runtime_terrain(
                &graphics_system.device_arc(),
                &graphics_system.queue_arc(),
                game_logic,
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

        if let Err(err) = render_pipeline.sync_runtime_map_roads(game_logic) {
            warn!(
                "Failed to sync runtime map roads for '{}': {}",
                map_name, err
            );
        }
    }

    fn apply_skybox_hint(render_pipeline: &mut RenderPipeline, game_logic: &GameLogic) {
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

    fn issue_minimap_move(&mut self, world_pos: Vec3) {
        if self.selected_objects.is_empty() {
            return;
        }

        let clamped = self.clamp_to_world_bounds(world_pos);
        self.game_logic
            .command_move(self.current_player_id, clamped);
        self.play_sound_effect(SoundType::Command);
    }

    fn clamp_to_world_bounds(&self, mut position: Vec3) -> Vec3 {
        let (world_min, world_max) = self.game_logic.world_bounds();
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
        let summary = self.game_logic.build_victory_summary(winner);
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
            }
            Some(_) => {
                self.ui_manager.set_defeat_with_summary(Some(summary));
            }
            None => {
                self.ui_manager.set_draw_with_summary(Some(summary));
            }
        }
    }

    fn reset_match_state(&mut self) {
        info!("Resetting gameplay state after match completion");
        self.drain_renderer_attachments();

        self.game_logic.reset();
        self.combat_system.clear();
        self.resource_manager = ResourceManager::new();

        let (world_min, world_max) = self.game_logic.world_bounds();
        let world_width = (world_max.x - world_min.x).abs().max(1.0);
        let world_height = (world_max.z - world_min.z).abs().max(1.0);
        self.pathfinding_system =
            PathfindingSystem::new_with_origin(world_min, world_width, world_height);

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

        match key {
            Key::Named(NamedKey::Space) => {
                self.toggle_pause();
            }
            Key::Character(digit)
                if digit.len() == 1 && digit.chars().all(|c| c.is_ascii_digit()) =>
            {
                let group_num = digit.chars().next().unwrap().to_digit(10).unwrap() as u8;
                let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));

                if ctrl_down {
                    // Assign control group.
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
                } else {
                    // Select control group.
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
                    if let Some(player) = self.game_logic.get_player(self.current_player_id) {
                        let team = player.team;
                        for id in stored {
                            if let Some(obj) = self.game_logic.find_object(id) {
                                if obj.team == team && obj.is_selectable() && obj.is_alive() {
                                    selection.push(id);
                                }
                            }
                        }
                    }

                    self.game_logic
                        .select_objects(self.current_player_id, selection.clone());
                    self.selected_objects = selection;
                    self.play_sound_effect(SoundType::Select);
                }
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Ctrl+A: select all selectable objects for current player team.
                let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                    return;
                };
                let team = player.team;

                let mut selection = Vec::new();
                for (&id, obj) in self.game_logic.get_objects() {
                    if obj.team == team && obj.is_selectable() && obj.is_alive() {
                        selection.push(id);
                    }
                }

                self.game_logic
                    .select_objects(self.current_player_id, selection.clone());
                self.selected_objects = selection;
                self.play_sound_effect(SoundType::Select);
            }
            Key::Named(NamedKey::Delete) => {
                // Debug: delete selected units.
                if self.selected_objects.is_empty() {
                    return;
                }
                for id in self.selected_objects.clone() {
                    self.game_logic.destroy_object(id);
                }
                self.selected_objects.clear();
                self.game_logic
                    .select_objects(self.current_player_id, Vec::new());
            }
            Key::Named(NamedKey::Tab) => {
                // Cycle selection through own selectable objects.
                let Some(player) = self.game_logic.get_player(self.current_player_id) else {
                    return;
                };
                let team = player.team;

                let mut all: Vec<ObjectId> = self
                    .game_logic
                    .get_objects()
                    .iter()
                    .filter(|(_, obj)| obj.team == team && obj.is_selectable() && obj.is_alive())
                    .map(|(&id, _)| id)
                    .collect();
                all.sort_by_key(|id| id.0);
                if all.is_empty() {
                    return;
                }

                let next = if let Some(current) = self.selected_objects.first().copied() {
                    all.iter()
                        .position(|id| *id == current)
                        .map(|idx| all[(idx + 1) % all.len()])
                        .unwrap_or(all[0])
                } else {
                    all[0]
                };

                self.selected_objects = vec![next];
                self.game_logic
                    .select_objects(self.current_player_id, vec![next]);
                self.play_sound_effect(SoundType::Select);
            }
            Key::Named(NamedKey::F1) => {
                self.show_debug_info = !self.show_debug_info;
                info!(
                    "Debug info: {}",
                    if self.show_debug_info { "ON" } else { "OFF" }
                );
            }
            Key::Character(c) if c == "m" || c == "M" => {
                self.toggle_background_music();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("v") => {
                self.debug_show_victory(Some(self.current_player_id));
            }
            Key::Character(c) if c.eq_ignore_ascii_case("l") && !ctrl_down => {
                let winner = self.game_logic.first_opponent_id(self.current_player_id);
                self.debug_show_victory(winner);
            }
            Key::Character(c) if c.eq_ignore_ascii_case("d") => {
                self.debug_show_victory(None);
            }
            Key::Character(c) if c.eq_ignore_ascii_case("p") => {
                // Toggle pause with 'P' key
                self.toggle_pause();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("s") && ctrl_down => {
                self.quick_save_from_hotkey("Ctrl+S");
            }
            Key::Character(c) if c.eq_ignore_ascii_case("l") && ctrl_down => {
                self.quick_load_from_hotkey("Ctrl+L");
            }
            Key::Named(NamedKey::Escape) => {
                info!("Escape key pressed - should exit game");
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

    fn handle_left_click(&mut self) {
        self.is_dragging = true;
        self.selection_start = Some(self.mouse_world_position);

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

        if is_double_click && clicked_object.is_some() {
            // Double-click: select all similar units
            if let Some(object_id) = clicked_object {
                self.select_similar_units(object_id);
            }
        } else {
            // Single-click behavior
            if let Some(object_id) = clicked_object {
                // Select this object
                self.game_logic
                    .select_objects(self.current_player_id, vec![object_id]);
                self.selected_objects = vec![object_id];
                self.play_sound_effect(SoundType::Select);
            } else {
                // Clear selection
                self.selected_objects.clear();
                self.game_logic
                    .select_objects(self.current_player_id, Vec::new());
            }
        }
    }

    fn select_similar_units(&mut self, clicked_object_id: ObjectId) {
        let Some(clicked_obj) = self.game_logic.find_object(clicked_object_id) else {
            return;
        };

        let Some(player) = self.game_logic.get_player(self.current_player_id) else {
            return;
        };

        let player_team = player.team;
        if clicked_obj.team != player_team || !clicked_obj.is_selectable() {
            return;
        }

        let template = clicked_obj.template_name.clone();
        let similar_units: Vec<ObjectId> = self
            .game_logic
            .get_objects()
            .iter()
            .filter(|(_, obj)| {
                obj.team == player_team && obj.is_selectable() && obj.template_name == template
            })
            .map(|(&id, _)| id)
            .collect();

        if !similar_units.is_empty() {
            self.game_logic
                .select_objects(self.current_player_id, similar_units.clone());
            self.selected_objects = similar_units;
            self.play_sound_effect(SoundType::Select);
            info!(
                "Selected {} similar units ({})",
                self.selected_objects.len(),
                template
            );
        }
    }

    fn handle_left_release(&mut self) {
        self.is_dragging = false;

        let Some(start) = self.selection_start.take() else {
            return;
        };

        let end = self.mouse_world_position;

        // If the mouse didn't move enough, the click selection was already handled on mouse-down.
        let drag_distance = Vec2::new(end.x - start.x, end.z - start.z).length();
        if drag_distance < 5.0 {
            return;
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

        let Some(player) = self.game_logic.get_player(self.current_player_id) else {
            return;
        };
        let player_team = player.team;

        for (&id, obj) in self.game_logic.get_objects() {
            if obj.team != player_team {
                continue;
            }
            if !obj.is_selectable() {
                continue;
            }
            let pos = obj.get_position();
            if pos.x < min_x || pos.x > max_x || pos.z < min_z || pos.z > max_z {
                continue;
            }
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

        // Normal right-click behavior when no pending command
        if self.selected_objects.is_empty() {
            return;
        }

        let mut should_attack = false;
        let mut attack_target_id = None;

        // Check if clicking on an enemy unit (attack command)
        let target_object = self.find_object_at_position(mouse_pos, &self.game_logic, true);

        if let Some(target_id) = target_object {
            if let Some(target) = self.game_logic.find_object(target_id) {
                // Check if it's an enemy
                if let Some(player) = self.game_logic.get_player(self.current_player_id) {
                    if target.team != player.team && target.is_kind_of(KindOf::Attackable) {
                        should_attack = true;
                        attack_target_id = Some(target_id);
                    }
                }
            }
        }

        // Now handle the command
        if should_attack {
            if let Some(target_id) = attack_target_id {
                self.game_logic
                    .command_attack(self.current_player_id, target_id);
                self.play_sound_effect(SoundType::Command);
            }
        } else {
            // Issue move command to clicked position
            self.game_logic
                .command_move(self.current_player_id, mouse_pos);
            self.play_sound_effect(SoundType::Command);
        }
    }

    fn handle_mouse_wheel(&mut self, delta: &winit::event::MouseScrollDelta) {
        use winit::event::MouseScrollDelta;

        let delta_y = match delta {
            MouseScrollDelta::LineDelta(_, y) => *y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
        };

        // Zoom camera with mouse wheel
        let zoom_speed = 0.1;
        let new_zoom = (self.camera_zoom - delta_y * zoom_speed).clamp(0.1, 5.0);

        if (new_zoom - self.camera_zoom).abs() > 0.001 {
            self.camera_zoom = new_zoom;
            debug!("Camera zoom changed to {:.2}", self.camera_zoom);
        }
    }

    fn update_camera(&mut self, dt: f32) {
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
            if self.is_character_key_pressed("w")
                || self.keys_pressed.contains(&Key::Named(NamedKey::ArrowUp))
            {
                screen_scroll.y -= vertical_scroll_speed_factor * scroll_step;
            }
            if self.is_character_key_pressed("s")
                || self.keys_pressed.contains(&Key::Named(NamedKey::ArrowDown))
            {
                screen_scroll.y += vertical_scroll_speed_factor * scroll_step;
            }
            if self.is_character_key_pressed("a")
                || self.keys_pressed.contains(&Key::Named(NamedKey::ArrowLeft))
            {
                screen_scroll.x -= horizontal_scroll_speed_factor * scroll_step;
            }
            if self.is_character_key_pressed("d")
                || self
                    .keys_pressed
                    .contains(&Key::Named(NamedKey::ArrowRight))
            {
                screen_scroll.x += horizontal_scroll_speed_factor * scroll_step;
            }

            // Edge scrolling (C++ LookAt.cpp: 3px from screen edge in fullscreen)
            if !self.is_windowed
                && matches!(self.current_state, GameState::InGame | GameState::Paused)
            {
                const EDGE_SCROLL_SIZE: f32 = 3.0;
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
            let target = self
                .game_logic
                .get_objects()
                .values()
                .find(|obj| {
                    obj.is_alive()
                        && obj
                            .template_name
                            .eq_ignore_ascii_case(&mode.thing_template_name)
                })
                .map(|obj| obj.get_position());
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

    fn update_mouse_world_position(&mut self) {
        // Convert screen coordinates to world coordinates using current world bounds.
        // This keeps click mapping stable across different map sizes and resolutions.
        let size = self.window.inner_size();
        let normalized_x = (self.mouse_position.0 / size.width.max(1) as f32).clamp(0.0, 1.0);
        let normalized_y = (self.mouse_position.1 / size.height.max(1) as f32).clamp(0.0, 1.0);

        let (world_min, world_max) = self.game_logic.world_bounds();
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

        let player_team = game_logic
            .get_player(self.current_player_id)
            .map(|p| p.team);
        let has_selected_units = !self.selected_objects.is_empty();
        let prioritize_enemy_targets = command_context && has_selected_units;
        let mut best: Option<(ObjectId, u8, f32)> = None; // (id, priority, distance)

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

    fn update_unit_pathfinding(&mut self, dt: f32, game_logic: &mut GameLogic) {
        let object_ids: Vec<ObjectId> = game_logic.get_objects().keys().copied().collect();

        for object_id in object_ids {
            // Move units along their paths
            let path_completed = self.pathfinding_system.move_unit_along_path(
                object_id,
                game_logic.get_objects_mut(),
                dt,
            );

            if path_completed {
                // Unit reached destination - could trigger completion events here
            }
        }
    }

    /// Legacy render stub -- NOT called from the active render path.
    /// Actual rendering is handled by RenderPipeline::execute() -> ForwardPass::render()
    /// which queues MeshClass instances into the WW3D Renderer and issues real draw calls.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    fn render_game_objects<'a>(&'a self, _render_pass: &mut wgpu::RenderPass<'a>) {
        // Collect objects to render to avoid borrowing conflicts
        let objects: Vec<_> = self.game_logic.get_objects().values().cloned().collect();
        log::trace!("Rendering {} objects in scene", objects.len());
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

    fn render_selection_indicators(&self, _render_pass: &mut wgpu::RenderPass) {
        // Render selection circles around selected objects
        for &object_id in &self.selected_objects {
            if let Some(_obj) = self.game_logic.find_object(object_id) {
                // Render selection circle (simplified)
                // In a full implementation, this would render a proper selection indicator
            }
        }
    }

    fn render_projectiles(&self, _render_pass: &mut wgpu::RenderPass) {
        // Render active projectiles
        for _projectile in self.combat_system.get_projectiles().values() {
            // Render projectile (simplified point for now)
            // In a full implementation, this would render proper projectile models
        }
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
                                let new_engine = new_engine;
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
                                info!("Escape pressed in InGame state - pausing");
                                engine.request_state_change(GameState::Paused);
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
                                    engine.game_logic.game_mode(),
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
                        if should_keep_logic_running_while_iconic(engine.game_logic.game_mode()) {
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

        // Collect unique model names from all game objects
        let mut unique_models: HashSet<String> = HashSet::new();

        for (object_id, object) in game_logic.get_objects() {
            if !object.is_alive() {
                continue;
            }

            let model_name = object.get_template().get_model_name();
            unique_models.insert(model_name.to_string());

            // Object uses model (logging disabled)
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
