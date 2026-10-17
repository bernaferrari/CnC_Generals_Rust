use super::*;
use game_engine::common::ini::ini_game_data::CameraMovementRules;
#[test]
fn receiving_camera_rule_uses_actual_ordered_cli_decoder_after_ini_without_global_copy() {
    let ini = CameraMovementRules::default()
        .overlay_ini_text("GameData\n DisableCameraMovements = No\n End\n")
        .unwrap();
    let raw = vec!["generals".into(), "-nomovecamera".into()];
    assert!(
        CnCGameEngine::admit_camera_movement_startup_switches(ini, &raw, true)
            .disable_camera_movement
    );
    assert!(
        !CnCGameEngine::admit_camera_movement_startup_switches(ini, &raw, false)
            .disable_camera_movement
    );
    let already = CameraMovementRules {
        disable_camera_movement: true,
        ..Default::default()
    };
    assert!(
        CnCGameEngine::admit_camera_movement_startup_switches(already, &[], false)
            .disable_camera_movement
    );
    let source = include_str!("boot.rs");
    assert!(source.contains("Self::admit_camera_movement_startup_switches("));
    assert!(source.contains("game_logic.admit_camera_application_movement(camera_movement_base)"));
    let worker = include_str!("shell.rs");
    assert!(worker.contains("game_logic.admit_camera_application_movement(camera_movement_base)"));
}

#[test]
fn receiving_tivo_startup_uses_existing_debug_parser_and_native_routes_use_existing_owner() {
    let texts = vec!["[Debug]\nTiVOFastMode = true\n".into()];
    assert!(
        crate::config::GlobalData::camera_startup_options_from_texts(&texts)
            .unwrap()
            .1
    );
    assert!(
        !crate::config::GlobalData::camera_startup_options_from_texts(&[])
            .unwrap()
            .1
    );
    let texts = vec![
        "[Debug]\nTiVOFastMode = true\n".into(),
        "[Debug]\nTiVOFastMode = false\n".into(),
    ];
    assert!(
        !crate::config::GlobalData::camera_startup_options_from_texts(&texts)
            .unwrap()
            .1
    );
    // These are source guards, not window/Engine execution evidence.
    let display = include_str!("native_display.rs");
    assert!(display.contains("let tivo = self.replay_fast_forward;"));
    assert!(!display.contains(".tivo_fast_mode"));
    let hotkey = include_str!("hotkeys.rs");
    assert!(hotkey.contains("self.replay_fast_forward = !self.replay_fast_forward;"));
    assert!(!hotkey.contains("global.tivo_fast_mode ="));
    assert!(
        include_str!("run_loop.rs")
            .contains("self.replay_fast_forward && self.presentation_or_boot_in_replay_game()")
    );
    assert!(include_str!("start_game.rs").contains("            self.replay_fast_forward,"));
    let continuation = include_str!("camera_continuation.rs");
    assert!(continuation.contains("self.replay_fast_forward = state.replay_fast_forward;"));
    assert!(include_str!("boot.rs").contains("            replay_fast_forward,"));
    assert!(!include_str!("boot.rs").contains("replay_fast_forward: false"));
}

#[test]
fn receiving_tivo_new_game_reset_precedes_saved_native_admission_and_follows_preflight() {
    let live = include_str!("start_game.rs");
    let begin = live
        .split("fn complete_parked_match_start(")
        .nth(1)
        .unwrap()
        .split("\n    pub(super) fn ")
        .next()
        .unwrap();
    let reset = begin
        .find("self.reset_replay_pacing_for_new_game();")
        .unwrap();
    assert!(begin.find("self.host_game_engine_reset();").unwrap() < reset);
    assert!(reset < begin.find("if mode == GameMode::Skirmish").unwrap());
    assert!(
        reset
            < begin
                .find("self.host_start_new_game_with_player_template(")
                .unwrap()
    );
    assert!(
        reset
            < begin
                .find("self.host_start_new_game_with_faction(")
                .unwrap()
    );
    let startup = include_str!("shell.rs")
        .split("fn host_finalize_startup_map_load(")
        .nth(1)
        .unwrap()
        .split("\n    pub(super) fn ")
        .next()
        .unwrap();
    assert!(
        startup
            .find("self.reset_replay_pacing_for_new_game();")
            .unwrap()
            < startup
                .find("self.host_replace_game_logic(result.game_logic)")
                .unwrap()
    );
    let restore = include_str!("host_authority.rs")
        .split("fn host_replace_staged_restore_world(")
        .nth(1)
        .unwrap()
        .split("\n    pub(super) fn ")
        .next()
        .unwrap();
    let clear = restore
        .find("self.reset_replay_pacing_for_new_game();")
        .unwrap();
    assert!(restore.find("runtime_world.preflight_native()?").unwrap() < clear);
    assert!(
        clear
            < restore
                .find("self.admit_camera_continuation(camera)")
                .unwrap()
    );
    assert_eq!(
        restore
            .matches("self.reset_replay_pacing_for_new_game();")
            .count(),
        1
    );
    assert!(!include_str!("camera_continuation.rs").contains("reset_replay_pacing_for_new_game"));
    // These call-order guards accompany the actual Engine reset/restore probe;
    // they are not evidence that complete start/load branches have executed.
}
