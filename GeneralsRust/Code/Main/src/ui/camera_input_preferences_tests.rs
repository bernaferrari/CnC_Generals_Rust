use super::*;
use game_engine::common::{
    ini::ini_game_data::CameraInputRules, user_preferences::UserPreferences,
};
fn app(path: &std::path::Path) -> UIManager {
    let mut preferences = UserPreferences::new();
    let _ = preferences.load(path.to_str().unwrap());
    let mut app = UIManager::new(801, 603);
    app.admit_options_preferences(preferences);
    app
}
#[test]
fn two_application_preferences_use_receiving_map_default_and_preserve_actual_store_across_reset() {
    let directory = tempfile::tempdir().unwrap();
    let path_a = directory.path().join("A.ini");
    let path_b = directory.path().join("B.ini");
    std::fs::write(&path_a, "MusicVolume = 71\n").unwrap();
    std::fs::write(&path_b, "ScrollFactor = 25suffix\n").unwrap();
    let (mut a, mut b) = (app(&path_a), app(&path_b));
    let (mut world, mut peer) = (
        crate::game_logic::GameLogic::new(),
        crate::game_logic::GameLogic::new(),
    );
    let application = CameraInputRules {
        keyboard_default_scroll_factor: 0.375,
        ..Default::default()
    };
    world.admit_camera_application_input(application);
    peer.admit_camera_application_input(CameraInputRules::default());
    world
        .apply_camera_input_overrides(&[
            "GameData\n KeyboardDefaultScrollSpeedFactor = 0.675\nEnd\n".into(),
        ])
        .unwrap();
    assert_eq!(
        a.camera_keyboard_scroll_factor(world.camera_input_rules()),
        0.675
    );
    assert_eq!(
        b.camera_keyboard_scroll_factor(peer.camera_input_rules()),
        0.25
    );
    let app_before = format!("{:?}", a.options_preferences_mut());
    let peer_before = format!("{:?}", b.options_preferences_mut());
    let rules_before = world.camera_input_rules();
    assert!(
        world
            .apply_camera_input_overrides(&[
                "GameData\n KeyboardDefaultScrollSpeedFactor = not-a-real\nEnd\n".into()
            ])
            .is_err()
    );
    assert_eq!(world.camera_input_rules(), rules_before);
    assert_eq!(format!("{:?}", a.options_preferences_mut()), app_before);
    world.reset();
    assert_eq!(
        a.camera_keyboard_scroll_factor(world.camera_input_rules()),
        0.375
    );
    assert_eq!(format!("{:?}", a.options_preferences_mut()), app_before);
    assert_eq!(format!("{:?}", b.options_preferences_mut()), peer_before);
    a.options_preferences_mut().set_int("ScrollFactor", 80);
    assert_eq!(
        a.camera_keyboard_scroll_factor(world.camera_input_rules()),
        0.8
    );
    assert_eq!(
        b.camera_keyboard_scroll_factor(peer.camera_input_rules()),
        0.25
    );
}
#[test]
fn source_native_input_reads_actual_application_and_definition_without_saved_preference_fields() {
    let source = include_str!("../cnc_game_engine/mouse/camera.rs");
    assert!(source.contains("self.game_logic.camera_input_rules()"));
    assert!(source.contains("self.ui_manager.camera_keyboard_scroll_factor(input_rules)"));
    assert!(!source.contains("global_data::read()"));
    let boot = include_str!("../cnc_game_engine/boot.rs");
    assert!(
        boot.find("camera_options.load(").unwrap() < boot.find("spawn_startup_map_load(").unwrap()
    );
    let native = include_str!("../cnc_game_engine/camera_continuation.rs");
    assert!(!native.contains("CameraInputRules"));
    assert!(!native.contains("UserPreferences"));
}
