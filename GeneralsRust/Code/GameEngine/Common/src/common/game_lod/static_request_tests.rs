use super::*;
fn isolated(name: &str, body: impl FnOnce()) {
    match crate::test_process::run_bounded(module_path!(), name) {
        crate::test_process::TestProcess::ParentVerified => {}
        crate::test_process::TestProcess::Child => body(),
    }
}
#[test]
fn disabled_static_selects_custom_without_applying_effects() {
    isolated(
        "disabled_static_selects_custom_without_applying_effects",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            set_ideal_static_lod_from_string("High");
            set_static_lod_from_string("High");
            assert_eq!(
                get_static_lod(),
                "High",
                "admit requested level before control"
            );
            let data = get_global_data().unwrap();
            {
                let mut settings = data.write();
                settings.enable_static_lod = false;
                settings.max_particle_count = 333;
            }
            register_rebuild_tank_tracks(|| panic!("disabled static LOD must not apply callbacks"));
            set_static_lod_from_string("Low");
            assert_eq!(get_static_lod(), "Custom");
            assert_eq!(data.read().max_particle_count, 333);
        },
    );
}
#[test]
fn invalid_and_unknown_requests_preserve_actual_applied_level() {
    isolated(
        "invalid_and_unknown_requests_preserve_actual_applied_level",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            set_ideal_static_lod_from_string("High");
            set_static_lod_from_string("High");
            assert_eq!(
                get_static_lod(),
                "High",
                "admit requested level before control"
            );
            register_rebuild_tank_tracks(|| {
                panic!("invalid/Unknown request must not apply callbacks")
            });
            set_static_lod_from_string("invalid-fixture-level");
            assert_eq!(get_static_lod(), "High");
            set_static_lod_from_string("Unknown");
            assert_eq!(get_static_lod(), "High");
        },
    );
}
