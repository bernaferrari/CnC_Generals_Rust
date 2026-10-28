use super::*;
use crate::common::ini::ini_game_data::TerrainTrackRules;

fn isolated(name: &str, body: impl FnOnce()) {
    match crate::test_process::run_bounded(module_path!(), name) {
        crate::test_process::TestProcess::ParentVerified => {}
        crate::test_process::TestProcess::Child => body(),
    }
}

#[test]
fn static_lod_parser_tracks_current_and_ideal_low_detail() {
    isolated(
        "static_lod_parser_tracks_current_and_ideal_low_detail",
        || {
            reset_static_lod_state_for_tests();
            set_static_lod_from_string("Medium");
            set_ideal_static_lod_from_string("Medium");
            assert!(!prefers_low_res_movies());

            set_static_lod_from_string("Low");
            assert!(prefers_low_res_movies());

            set_static_lod_from_string("High");
            set_ideal_static_lod_from_string("Low");
            assert!(prefers_low_res_movies());
        },
    );
}

#[test]
fn session_static_lod_reports_only_real_applications_and_reapplies_custom() {
    isolated(
        "session_static_lod_reports_only_real_applications_and_reapplies_custom",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            let global = get_global_data().expect("global data initialized");
            {
                let mut manager = get_game_lod_manager_mut();
                manager.set_current_static_lod_level(StaticGameLODLevel::High);
                let high =
                    &mut manager.static_game_lod_info[StaticGameLODLevel::High.to_index().unwrap()];
                high.max_tank_track_edges = 29;
                high.max_tank_track_opaque_edges = 8;
                high.max_tank_track_fade_delay = 456;
            }
            {
                let mut data = global.write();
                data.max_tank_track_edges = 23;
                data.max_tank_track_opaque_edges = 7;
                data.max_tank_track_fade_delay = 321;
            }

            let Some(SessionStaticLodResult::Apply {
                rules: already_applied,
                ..
            }) = set_static_lod_from_string_for_session("High")
            else {
                panic!("session B still needs the current app preset snapshot");
            };
            assert_eq!(already_applied.max_tank_track_edges, 29);
            assert_eq!(already_applied.max_tank_track_opaque_edges, 8);
            assert_eq!(already_applied.max_tank_track_fade_delay, 456);
            let Some(SessionStaticLodResult::Apply { rules: first, .. }) =
                set_static_lod_from_string_for_session("Custom")
            else {
                panic!("Custom is applied even when it is already selected");
            };
            assert_eq!(first, TerrainTrackRules::default());
            global.write().max_tank_track_edges = 11;
            let Some(SessionStaticLodResult::Apply {
                rules: repeated, ..
            }) = set_static_lod_from_string_for_session("Custom")
            else {
                panic!("repeated Custom reapplies updated current settings");
            };
            assert_eq!(repeated, TerrainTrackRules::default());
        },
    );
}

#[test]
fn disabled_static_lod_updates_only_the_session_marker_before_reenable() {
    isolated(
        "disabled_static_lod_updates_only_the_session_marker_before_reenable",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            let global = get_global_data().expect("global data initialized");
            get_game_lod_manager_mut().set_current_static_lod_level(StaticGameLODLevel::High);
            global.write().enable_static_lod = false;

            assert_eq!(
                set_static_lod_from_string_for_session("High"),
                Some(SessionStaticLodResult::MarkerOnly {
                    level: StaticGameLODLevel::Custom
                })
            );
            assert_eq!(
                get_game_lod_manager().get_static_lod_level(),
                StaticGameLODLevel::Custom
            );

            global.write().enable_static_lod = true;
            let Some(SessionStaticLodResult::Apply { level, .. }) =
                set_static_lod_from_string_for_session("High")
            else {
                panic!("High must apply after disabled LOD reset the current level to Custom");
            };
            assert_eq!(level, StaticGameLODLevel::High);
        },
    );
}

#[test]
fn custom_static_lod_snapshots_current_global_settings_before_apply() {
    isolated(
        "custom_static_lod_snapshots_current_global_settings_before_apply",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            let global_data = get_global_data().expect("global data initialized");

            {
                let mut global = global_data.write();
                global.max_particle_count = 4321;
                global.use_shadow_volumes = false;
                global.use_shadow_decals = true;
                global.use_cloud_map = false;
                global.use_light_map = true;
                global.show_soft_water_edge = false;
                global.max_tank_track_edges = 17;
                global.max_tank_track_opaque_edges = 9;
                global.max_tank_track_fade_delay = 12345;
                global.use_draw_module_lod = true;
                global.use_heat_effects = false;
                global.texture_reduction_factor = 2;
                global.use_fps_limit = true;
                global.enable_dynamic_lod = false;
                global.use_trees = false;
            }

            {
                let mut manager = get_game_lod_manager_mut();
                let custom_index = StaticGameLODLevel::Custom.to_index().unwrap();
                let custom = &mut manager.static_game_lod_info[custom_index];
                custom.max_particle_count = 111;
                custom.use_shadow_volumes = true;
                custom.use_cloud_map = true;
                custom.texture_reduction = 0;
                custom.enable_dynamic_lod = true;
                custom.use_trees = true;
            }

            set_static_lod_from_string("Custom");

            {
                let global = global_data.read();
                assert_eq!(global.max_particle_count, 4321);
                assert!(!global.use_shadow_volumes);
                assert!(global.use_shadow_decals);
                assert!(!global.use_cloud_map);
                assert!(global.use_light_map);
                assert!(!global.show_soft_water_edge);
                assert_eq!(global.max_tank_track_edges, 17);
                assert_eq!(global.max_tank_track_opaque_edges, 9);
                assert_eq!(global.max_tank_track_fade_delay, 12345);
                assert!(global.use_draw_module_lod);
                assert!(!global.use_heat_effects);
                assert_eq!(global.texture_reduction_factor, 2);
                assert!(global.use_fps_limit);
                assert!(!global.enable_dynamic_lod);
                assert!(!global.use_trees);
            }

            let manager = get_game_lod_manager();
            let custom =
                &manager.static_game_lod_info[StaticGameLODLevel::Custom.to_index().unwrap()];
            assert_eq!(custom.max_particle_count, 4321);
            assert!(!custom.use_shadow_volumes);
            assert!(!custom.use_buildup_scaffolds);
            assert!(!custom.use_tree_sway);
            assert_eq!(custom.texture_reduction, 2);
            assert!(!custom.enable_dynamic_lod);
            assert!(!custom.use_trees);
        },
    );
}

#[test]
fn non_custom_static_lod_uses_cpp_memory_recommended_texture_reduction() {
    isolated(
        "non_custom_static_lod_uses_cpp_memory_recommended_texture_reduction",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            let global_data = get_global_data().expect("global data initialized");

            {
                let mut manager = get_game_lod_manager_mut();
                let low_index = StaticGameLODLevel::Low.to_index().unwrap();
                let high_index = StaticGameLODLevel::High.to_index().unwrap();
                manager.static_game_lod_info[low_index].texture_reduction = 3;
                manager.static_game_lod_info[high_index].texture_reduction = 0;
                manager.static_game_lod_info[high_index].use_trees = true;
            }

            {
                let mut global = global_data.write();
                global.texture_reduction_factor = 0;
                global.use_trees = true;
                global.shell_map_on = true;
            }

            set_ideal_static_lod_from_string("Unknown");
            set_mem_passed_override_for_tests(Some(false));
            set_static_lod_from_string("High");

            {
                let global = global_data.read();
                assert_eq!(global.texture_reduction_factor, 3);
                assert!(!global.use_trees);
                assert!(!global.shell_map_on);
            }

            set_mem_passed_override_for_tests(None);
        },
    );
}

#[test]
fn repeated_non_custom_static_lod_does_not_reapply_cpp_current_level() {
    isolated(
        "repeated_non_custom_static_lod_does_not_reapply_cpp_current_level",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            let global_data = get_global_data().expect("global data initialized");

            {
                let mut manager = get_game_lod_manager_mut();
                let high_index = StaticGameLODLevel::High.to_index().unwrap();
                manager.static_game_lod_info[high_index].max_particle_count = 222;
            }

            {
                let mut global = global_data.write();
                global.max_particle_count = 111;
            }

            set_mem_passed_override_for_tests(Some(true));
            set_cpu_freq_mhz_override_for_tests(Some(1000));
            set_static_lod_from_string("High");
            assert_eq!(global_data.read().max_particle_count, 222);

            global_data.write().max_particle_count = 333;
            set_static_lod_from_string("High");
            assert_eq!(global_data.read().max_particle_count, 333);

            global_data.write().max_particle_count = 444;
            set_static_lod_from_string("Custom");
            assert_eq!(global_data.read().max_particle_count, 444);

            set_mem_passed_override_for_tests(None);
            set_cpu_freq_mhz_override_for_tests(None);
        },
    );
}

#[test]
fn really_low_mhz_disables_shell_map_even_when_memory_passes() {
    isolated(
        "really_low_mhz_disables_shell_map_even_when_memory_passes",
        || {
            reset_static_lod_state_for_tests();
            crate::common::ini::ini_game_data::init_global_data();
            let global_data = get_global_data().expect("global data initialized");

            {
                let mut manager = get_game_lod_manager_mut();
                manager.set_really_low_mhz(400);
            }

            {
                let mut global = global_data.write();
                global.shell_map_on = true;
            }

            set_mem_passed_override_for_tests(Some(true));
            set_cpu_freq_mhz_override_for_tests(Some(399));
            set_static_lod_from_string("High");

            assert!(!global_data.read().shell_map_on);

            set_mem_passed_override_for_tests(None);
            set_cpu_freq_mhz_override_for_tests(None);
        },
    );
}

#[test]
fn did_mem_pass_uses_override_for_cpp_load_screen_gate() {
    isolated(
        "did_mem_pass_uses_override_for_cpp_load_screen_gate",
        || {
            set_mem_passed_override_for_tests(Some(false));
            assert!(!did_mem_pass());

            set_mem_passed_override_for_tests(Some(true));
            assert!(did_mem_pass());

            set_mem_passed_override_for_tests(None);
        },
    );
}

#[test]
fn find_static_lod_level_walks_presets_high_to_low() {
    isolated("find_static_lod_level_walks_presets_high_to_low", || {
        reset_static_lod_state_for_tests();
        {
            let mut manager = get_game_lod_manager_mut();
            manager.init();
            let high = manager.new_lod_preset(2).expect("high preset");
            high.cpu_type = CpuType::P4;
            high.mhz = 1500;
            high.video_type = ChipsetType::GeForce4;
            high.memory = 256;
            let low = manager.new_lod_preset(0).expect("low preset");
            low.cpu_type = CpuType::P3;
            low.mhz = 800;
            low.video_type = ChipsetType::TNT2;
            low.memory = 128;
        }

        set_hardware_overrides_for_tests(
            Some(CpuType::P4),
            Some(2000),
            Some(ChipsetType::Radeon9700),
            Some(1024),
        );
        assert_eq!(find_static_lod_level(), "High");

        reset_static_lod_state_for_tests();
        set_hardware_overrides_for_tests(
            Some(CpuType::P3),
            Some(850),
            Some(ChipsetType::TNT2),
            Some(128),
        );
        assert_eq!(find_static_lod_level(), "Low");
        set_hardware_overrides_for_tests(None, None, None, None);
        get_game_lod_manager_mut().init();
    });
}

#[test]
fn get_slow_death_scale_reads_live_dynamic_lod() {
    isolated("get_slow_death_scale_reads_live_dynamic_lod", || {
        reset_static_lod_state_for_tests();
        set_dynamic_lod_from_string("High");
        {
            let mut manager = get_game_lod_manager_mut();
            let low = DynamicGameLODLevel::Low.to_index().unwrap();
            manager.dynamic_game_lod_info[low].slow_death_scale = 0.25;
        }
        set_dynamic_lod_from_string("Low");
        assert!((get_slow_death_scale() - 0.25).abs() < f32::EPSILON);
        set_dynamic_lod_from_string("High");
    });
}
