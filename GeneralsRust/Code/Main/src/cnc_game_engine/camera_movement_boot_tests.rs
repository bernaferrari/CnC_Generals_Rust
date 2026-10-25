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
}
