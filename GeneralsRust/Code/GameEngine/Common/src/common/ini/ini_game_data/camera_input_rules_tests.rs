use super::*;
#[test]
fn input_definition_defaults_and_private_overlays_match_source() {
    let app = CameraInputRules::default();
    assert_eq!(
        (
            app.horizontal_scroll_speed_factor,
            app.vertical_scroll_speed_factor,
            app.keyboard_default_scroll_factor,
            app.keyboard_camera_rotate_speed
        ),
        (1.0, 1.0, 0.5, 0.1)
    );
    let map=app.overlay_ini_text("GameData\n HorizontalScrollSpeedFactor = 2.5\n VerticalScrollSpeedFactor = -0.75\n KeyboardDefaultScrollSpeedFactor = 0.675\n KeyboardCameraRotateSpeed = 0.125\nEnd\n").unwrap();
    assert_eq!(
        (
            map.horizontal_scroll_speed_factor,
            map.vertical_scroll_speed_factor,
            map.keyboard_default_scroll_factor,
            map.keyboard_camera_rotate_speed
        ),
        (2.5, -0.75, 0.675, 0.125)
    );
    let solo = map
        .overlay_ini_text("GameData\n KeyboardDefaultScrollSpeedFactor = 0.375\nEnd\n")
        .unwrap();
    assert_eq!(solo.horizontal_scroll_speed_factor, 2.5);
    assert_eq!(solo.keyboard_default_scroll_factor, 0.375);
    assert_eq!(app.keyboard_camera_rotate_speed, 0.1);
}
#[test]
fn selected_scroll_uses_the_one_existing_parser_and_absent_uses_receiving_default() {
    let rules = CameraInputRules {
        keyboard_default_scroll_factor: 1.25,
        ..Default::default()
    };
    let mut preferences = UserPreferences::new();
    assert_eq!(rules.keyboard_scroll_factor(&preferences), 1.25);
    preferences.set_int("ScrollFactor", 75);
    assert_eq!(rules.keyboard_scroll_factor(&preferences), 0.75);
    preferences.set_int("ScrollFactor", -5);
    assert_eq!(rules.keyboard_scroll_factor(&preferences), 0.0);
    preferences.set_int("ScrollFactor", 101);
    assert_eq!(rules.keyboard_scroll_factor(&preferences), 1.0);
    preferences.set_string("ScrollFactor", "25suffix".into());
    assert_eq!(rules.keyboard_scroll_factor(&preferences), 0.25);
    preferences.set_string("ScrollFactor", "  +12.5".into());
    assert_eq!(rules.keyboard_scroll_factor(&preferences), 0.12);
    preferences.set_string("ScrollFactor", "-25tail".into());
    assert_eq!(rules.keyboard_scroll_factor(&preferences), 0.0);
    preferences.set_string("ScrollFactor", "not-an-int".into());
    assert_eq!(rules.keyboard_scroll_factor(&preferences), 0.0);
}
