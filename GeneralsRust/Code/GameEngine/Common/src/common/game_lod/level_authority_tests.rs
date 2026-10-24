use super::*;
fn isolated(name: &str, body: impl FnOnce()) {
    match crate::test_process::run_bounded(module_path!(), name) {
        crate::test_process::TestProcess::ParentVerified => {}
        crate::test_process::TestProcess::Child => body(),
    }
}
#[test]
fn original_constructor_current_level_is_unknown() {
    isolated("original_constructor_current_level_is_unknown", || {
        assert_eq!(get_static_lod(), "Unknown");
    });
}
#[test]
fn actual_static_setter_is_observed_outside_the_calling_thread() {
    isolated(
        "actual_static_setter_is_observed_outside_the_calling_thread",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            set_ideal_static_lod_from_string("Medium");
            set_static_lod_from_string("Medium");
            assert_eq!(
                get_static_lod(),
                "Medium",
                "admit requested level before worker"
            );
            std::thread::spawn(|| set_static_lod_from_string("Low"))
                .join()
                .unwrap();
            assert_eq!(get_static_lod(), "Low");
        },
    );
}
#[test]
fn actual_ideal_setter_is_observed_outside_the_calling_thread() {
    isolated(
        "actual_ideal_setter_is_observed_outside_the_calling_thread",
        || {
            reset_static_lod_state_for_tests();
            set_ideal_static_lod_from_string("High");
            std::thread::spawn(|| set_ideal_static_lod_from_string("Low"))
                .join()
                .unwrap();
            assert_eq!(get_ideal_static_lod(), "Low");
        },
    );
}
#[test]
fn existing_dynamic_manager_write_is_public_query_authority() {
    isolated(
        "existing_dynamic_manager_write_is_public_query_authority",
        || {
            reset_static_lod_state_for_tests();
            get_game_lod_manager_mut().set_dynamic_lod_level(DynamicGameLODLevel::Medium);
            assert_eq!(get_dynamic_lod(), "Medium");
        },
    );
}
#[test]
fn same_static_level_on_same_manager_does_not_reapply_on_another_thread() {
    isolated(
        "same_static_level_on_same_manager_does_not_reapply_on_another_thread",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            let data = get_global_data().unwrap();
            {
                let mut manager = get_game_lod_manager_mut();
                manager.static_game_lod_info[StaticGameLODLevel::High.to_index().unwrap()]
                    .max_particle_count = 222;
            }
            set_static_lod_from_string("High");
            assert_eq!(data.read().max_particle_count, 222);
            data.write().max_particle_count = 333;
            std::thread::spawn(|| set_static_lod_from_string("High"))
                .join()
                .unwrap();
            assert_eq!(data.read().max_particle_count, 333);
        },
    );
}
#[test]
fn actual_apply_callback_observes_previous_level_until_setter_commits() {
    isolated(
        "actual_apply_callback_observes_previous_level_until_setter_commits",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            set_ideal_static_lod_from_string("High");
            register_rebuild_tank_tracks(|| assert_eq!(get_static_lod(), "Unknown"));
            set_static_lod_from_string("High");
            assert_eq!(get_static_lod(), "High");
        },
    );
}
