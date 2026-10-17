use super::*;
use game_engine::common::{
    ini::ini_game_data::CameraInputRules, user_preferences::UserPreferences,
};
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_options_apply_uses_own_path_then_cancel_does_not_commit_draft_scroll() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_options_apply_uses_own_path_then_cancel_does_not_commit_draft_scroll",
        || {
            let directory = tempfile::tempdir().unwrap();
            // The real Client Apply also probes/persists recommended LOD through
            // its legacy UserDataDir adapter. Contain that independent write in
            // this isolated child without changing asset discovery or A/B owners.
            #[cfg(feature = "game_client")]
            let legacy_user_data = directory.path().join("legacy-lod-user-data");
            #[cfg(feature = "game_client")]
            game_engine::common::ini::ini_game_data::ensure_global_data()
                .write()
                .set_path_user_data(legacy_user_data.to_str().unwrap().to_string());
            let a_path = directory.path().join("A.ini");
            let b_path = directory.path().join("B.ini");
            std::fs::write(&a_path, "MusicVolume = 71\n").unwrap();
            std::fs::write(&b_path, "ScrollFactor = 25\n").unwrap();
            let (mut a, mut b) = (OptionsMenu::new(), OptionsMenu::new());
            for (menu, path) in [(&mut a, &a_path), (&mut b, &b_path)] {
                let mut prefs = UserPreferences::new();
                let _ = prefs.load(path.to_str().unwrap());
                menu.admit_preferences(prefs);
                menu.initialize().unwrap();
            }
            let rules = CameraInputRules {
                keyboard_default_scroll_factor: 0.675,
                ..Default::default()
            };
            a.admit_camera_input_defaults(rules);
            assert!(
                matches!(a.control_value("controls.scroll_speed"),Some(OptionValue::Float(value)) if *value==0.67)
            );
            assert_eq!(rules.keyboard_scroll_factor(a.preferences()), 0.675);
            for control in a.iter_controls_mut() {
                if control.key == "controls.scroll_speed" {
                    control.value = OptionValue::Float(0.8);
                }
            }
            game_engine::common::global_data::write().keyboard_scroll_factor = 0.99;
            a.apply_settings();
            assert_eq!(a.preferences().get_int("ScrollFactor"), Some(80));
            assert_eq!(rules.keyboard_scroll_factor(a.preferences()), 0.8);
            assert_eq!(b.preferences().get_int("ScrollFactor"), Some(25));
            assert!(
                std::fs::read_to_string(&a_path)
                    .unwrap()
                    .contains("ScrollFactor = 80")
            );
            assert_eq!(
                std::fs::read_to_string(&b_path).unwrap(),
                "ScrollFactor = 25\n"
            );
            for control in a.iter_controls_mut() {
                if control.key == "controls.scroll_speed" {
                    control.value = OptionValue::Float(0.12);
                }
            }
            a.revert_settings();
            assert_eq!(a.preferences().get_int("ScrollFactor"), Some(80));
            assert!(
                matches!(a.control_value("controls.scroll_speed"),Some(OptionValue::Float(value)) if *value==0.8)
            );
            #[cfg(feature = "game_client")]
            {
                let legacy_options =
                    std::fs::read_to_string(legacy_user_data.join("Options.ini")).unwrap();
                assert!(
                    legacy_options
                        .lines()
                        .any(|line| line.starts_with("IdealStaticGameLOD = ")),
                    "actual LOD recommendation write must stay in the child temp directory"
                );
                assert!(
                    legacy_options
                        .lines()
                        .any(|line| line.starts_with("StaticGameLOD = "))
                );
            }
        },
    );
}

#[test]
fn malformed_present_scroll_view_shares_actual_preference_parser_instead_of_map_fallback() {
    let mut menu = OptionsMenu::new();
    menu.setup_tabs();
    menu.setup_action_buttons();
    menu.setup_control_options();
    menu.default_values = menu.snapshot_current_values();
    let rules = CameraInputRules {
        keyboard_default_scroll_factor: 0.675,
        ..Default::default()
    };
    menu.admit_camera_input_defaults(rules);
    assert!(
        matches!(menu.control_value("controls.scroll_speed"),Some(OptionValue::Float(value)) if *value==0.67)
    );
    menu.preferences_mut()
        .set_string("ScrollFactor", "not-an-int".into());
    menu.refresh_options_controls();
    assert_eq!(rules.keyboard_scroll_factor(menu.preferences()), 0.0);
    assert!(
        matches!(menu.control_value("controls.scroll_speed"),Some(OptionValue::Float(value)) if *value==0.0)
    );
    menu.preferences_mut()
        .set_string("ScrollFactor", "25x".into());
    menu.load_current_settings();
    assert_eq!(rules.keyboard_scroll_factor(menu.preferences()), 0.0);
    assert!(
        matches!(menu.control_value("controls.scroll_speed"),Some(OptionValue::Float(value)) if *value==0.0)
    );
}

#[test]
fn absent_scroll_default_keeps_the_single_source_slider_truncation() {
    let mut menu = OptionsMenu::new();
    menu.setup_tabs();
    menu.setup_action_buttons();
    menu.setup_control_options();
    menu.default_values = menu.snapshot_current_values();
    let rules = CameraInputRules {
        keyboard_default_scroll_factor: 0.535,
        ..Default::default()
    };
    menu.admit_camera_input_defaults(rules);
    // OptionsMenu.cpp1744 truncates the raw factor once: 0.535 * 100 -> 53.
    // Its editable projection is 53/100, not a second truncation to 52/100.
    assert!(menu.preferences().get_string("ScrollFactor").is_none());
    assert_eq!(rules.keyboard_scroll_factor(menu.preferences()), 0.535);
    for refresh in [false, true] {
        if refresh {
            menu.refresh_options_controls();
        } else {
            menu.load_current_settings();
        }
        assert!(
            matches!(menu.control_value("controls.scroll_speed"),Some(OptionValue::Float(value)) if *value==0.53)
        );
        assert!(matches!(menu.camera_scroll_view(),OptionValue::Float(value) if value==0.53));
        assert!(menu.preferences().get_string("ScrollFactor").is_none());
    }
}
