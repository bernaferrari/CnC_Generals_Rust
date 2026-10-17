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
