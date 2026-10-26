// LoadScreen.cpp progress and input fixture cases; included in the original logical test module.

#[test]
fn map_transfer_progress_timeout_and_filename_cache_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "map_transfer_progress_timeout_and_filename_cache_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let owner = operation_windows.clone();
        (|| {
            #[cfg(not(target_arch = "wasm32"))]
            if let crate::test_process::TestProcess::ParentVerified =
                crate::test_process::run_bounded(
                    module_path!(),
                    "map_transfer_progress_timeout_and_filename_cache_like_cpp",
                )
            {
                return;
            }
            // This exact process owns the classic GameText catalog. A Language
            // fallback registration cannot override an already loaded retail CSF.
            // Admit the selected strings through the actual map-string parser,
            // without changing the production CSF-first lookup order.
            let directory = tempfile::tempdir().unwrap();
            let strings = directory.path().join("MapTransferFixture.str");
            std::fs::write(
                &strings,
                concat!(
                    "MapTransfer:Unpacking\n\"Unpacking map\"\nEND\n",
                    "MapTransfer:Timeout\n\"%d:%d remaining\"\nEND\n",
                    "MapTransfer:CurrentFile\n\"Current: %s\"\nEND\n",
                ),
            )
            .unwrap();
            GameText::init_map_string_file(strings.to_str().unwrap())
                .expect("admit actual selected map-string catalog");
            for (label, expected) in [
                ("MapTransfer:Unpacking", "Unpacking map"),
                ("MapTransfer:Timeout", "%d:%d remaining"),
                ("MapTransfer:CurrentFile", "Current: %s"),
            ] {
                assert_eq!(
                    GameText::fetch_with_exists(label),
                    (expected.to_owned(), true)
                );
            }
            reset_map_transfer_load_screen_state(runtime);

            let mut wm = WindowManager::new();
            create_map_transfer_slot_windows(&mut wm, 2, &mut runtime.effects);
            let context = LoadScreenInitContext {
                local_player_name: "Local".to_string(),
                local_side_name: "USA".to_string(),
                local_template_name: "FactionAmerica".to_string(),
                local_general_name: "USA".to_string(),
                local_general_features: "USA".to_string(),
                local_general_portrait: None,
                local_load_screen_music: String::new(),
                local_team_number: 0,
                shell_game_did_mem_pass: true,
                map_name: None,
                start_positions: Vec::new(),
                slots: vec![load_screen_slot_with_map(
                    2, "NeedsMap", None, false, false, true,
                )],
            };
            initialize_map_transfer_windows(&mut wm, &context, runtime);
            runtime.windows.with_manager(|receiving_wm| {
                *receiving_wm = wm;
            });

            assert!(!process_load_screen_progress(
                LoadScreenKind::MapTransfer,
                2,
                47.0,
                runtime
            ));
            assert!(process_map_transfer_progress(
                2,
                47,
                "MapTransfer:Unpacking",
                runtime
            ));
            assert_eq!(
                progress_value("MapTransferScreen.wnd:ProgressLoad0", runtime),
                Some(0.47)
            );
            runtime.windows.with_manager(|wm| {
                assert_eq!(
                    window_text(wm, "MapTransferScreen.wnd:StaticTextProgress0"),
                    "Unpacking map"
                );
            });
            assert!(!process_map_transfer_progress(
                2,
                47,
                "MapTransfer:Unpacking",
                runtime
            ));
            assert!(!process_map_transfer_progress(
                7,
                30,
                "MapTransfer:Unpacking",
                runtime
            ));

            assert!(process_map_transfer_timeout(125, runtime));
            runtime.windows.with_manager(|wm| {
                assert_eq!(
                    window_text(wm, "MapTransferScreen.wnd:StaticTextTimeout"),
                    "2:5 remaining"
                );
            });
            assert!(!process_map_transfer_timeout(125, runtime));

            set_map_transfer_current_filename("Maps\\Official\\Tournament.map", runtime);
            runtime.windows.with_manager(|wm| {
                assert_eq!(
                    window_text(wm, "MapTransferScreen.wnd:StaticTextCurrentFile"),
                    "Current: Tournament.map"
                );
            });
        })();
    });
}

#[test]
fn multiplayer_process_progress_uses_cpp_player_lookup_mapping() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let owner = operation_windows.clone();
        (|| {
            reset_multiplayer_load_screen_state(runtime);
            let mut wm = WindowManager::new();
            create_multiplayer_slot_windows(
                &mut wm,
                "MultiplayerLoadScreen.wnd",
                3,
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralPortrait",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralFeatures",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralName",
                &mut runtime.effects,
            );

            let context = LoadScreenInitContext {
                local_player_name: "Alice".to_string(),
                local_side_name: "USA".to_string(),
                local_template_name: "FactionAmerica".to_string(),
                local_general_name: "USA".to_string(),
                local_general_features: "USA".to_string(),
                local_general_portrait: None,
                local_load_screen_music: String::new(),
                local_team_number: 0,
                shell_game_did_mem_pass: true,
                map_name: None,
                start_positions: Vec::new(),
                slots: vec![
                    load_screen_slot("Alice", "USA", 0, false, true),
                    load_screen_slot("Empty", "GLA", 1, false, false),
                    load_screen_slot("Bob", "China", 2, false, true),
                ],
            };

            initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context, runtime);
            runtime.windows.with_manager(|receiving_wm| {
                *receiving_wm = wm;
            });

            assert!(process_load_screen_progress(
                LoadScreenKind::Multiplayer,
                2,
                62.0,
                runtime
            ));
            assert_eq!(
                progress_value("MultiplayerLoadScreen.wnd:ProgressLoad1", runtime),
                Some(0.62)
            );
            assert_eq!(
                progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0", runtime),
                Some(0.0)
            );
            assert!(!process_load_screen_progress(
                LoadScreenKind::Multiplayer,
                1,
                30.0,
                runtime
            ));
        })();
    });
}

#[test]
fn multiplayer_update_without_lookup_does_not_fallback_to_slot_zero() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let owner = operation_windows.clone();
        (|| {
            reset_multiplayer_load_screen_state(runtime);
            let mut wm = WindowManager::new();
            create_multiplayer_slot_windows(
                &mut wm,
                "MultiplayerLoadScreen.wnd",
                2,
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralPortrait",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralFeatures",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralName",
                &mut runtime.effects,
            );

            let context = LoadScreenInitContext {
                local_player_name: "Missing".to_string(),
                local_side_name: "USA".to_string(),
                local_template_name: "FactionAmerica".to_string(),
                local_general_name: "USA".to_string(),
                local_general_features: "USA".to_string(),
                local_general_portrait: None,
                local_load_screen_music: String::new(),
                local_team_number: 7,
                shell_game_did_mem_pass: true,
                map_name: None,
                start_positions: Vec::new(),
                slots: vec![load_screen_slot("Alice", "USA", 0, false, true)],
            };

            initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context, runtime);
            runtime.windows.with_manager(|receiving_wm| {
                *receiving_wm = wm;
            });

            update_load_screen_with_pump(
                LoadScreenKind::Multiplayer,
                41.0,
                Some(&mut *presentation_pump),
                runtime,
            )
            .unwrap();

            assert_eq!(
                progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0", runtime),
                Some(0.0)
            );
        })();
    });
}

#[test]
fn multiplayer_update_reports_progress_before_local_ui_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "multiplayer_update_reports_progress_before_local_ui_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let owner = operation_windows.clone();
        (|| {
            reset_multiplayer_load_screen_state(runtime);
            clear_multiplayer_load_progress_hook();
            let calls = Arc::new(Mutex::new(Vec::new()));
            let hook_calls = Arc::clone(&calls);
            register_multiplayer_load_progress_hook(move |player_id, percent| {
                hook_calls.lock().unwrap().push((player_id, percent));
            });

            let mut wm = WindowManager::new();
            create_multiplayer_slot_windows(
                &mut wm,
                "MultiplayerLoadScreen.wnd",
                2,
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralPortrait",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralFeatures",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralName",
                &mut runtime.effects,
            );

            let context = LoadScreenInitContext {
                local_player_name: "Bob".to_string(),
                local_side_name: "China".to_string(),
                local_template_name: "FactionChina".to_string(),
                local_general_name: "China".to_string(),
                local_general_features: "China".to_string(),
                local_general_portrait: None,
                local_load_screen_music: String::new(),
                local_team_number: 1,
                shell_game_did_mem_pass: true,
                map_name: None,
                start_positions: Vec::new(),
                slots: vec![
                    load_screen_slot("Alice", "USA", 0, false, true),
                    load_screen_slot("Bob", "China", 1, false, true),
                ],
            };

            initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context, runtime);
            runtime.windows.with_manager(|receiving_wm| {
                *receiving_wm = wm;
            });

            update_load_screen_with_pump(
                LoadScreenKind::Multiplayer,
                41.0,
                Some(&mut *presentation_pump),
                runtime,
            )
            .unwrap();

            assert_eq!(*calls.lock().unwrap(), vec![(1, 41)]);
            assert_eq!(
                progress_value("MultiplayerLoadScreen.wnd:ProgressLoad1", runtime),
                Some(0.41)
            );
            clear_multiplayer_load_progress_hook();
        })();
    });
}

#[test]
fn multiplayer_update_above_complete_only_pumps_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "multiplayer_update_above_complete_only_pumps_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let owner = operation_windows.clone();
        (|| {
            reset_multiplayer_load_screen_state(runtime);
            clear_multiplayer_load_progress_hook();
            let calls = Arc::new(Mutex::new(Vec::new()));
            let hook_calls = Arc::clone(&calls);
            register_multiplayer_load_progress_hook(move |player_id, percent| {
                hook_calls.lock().unwrap().push((player_id, percent));
            });

            let mut wm = WindowManager::new();
            create_multiplayer_slot_windows(
                &mut wm,
                "MultiplayerLoadScreen.wnd",
                1,
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralPortrait",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralFeatures",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "MultiplayerLoadScreen.wnd:LocalGeneralName",
                &mut runtime.effects,
            );

            let context = LoadScreenInitContext {
                local_player_name: "Alice".to_string(),
                local_side_name: "USA".to_string(),
                local_template_name: "FactionAmerica".to_string(),
                local_general_name: "USA".to_string(),
                local_general_features: "USA".to_string(),
                local_general_portrait: None,
                local_load_screen_music: String::new(),
                local_team_number: 0,
                shell_game_did_mem_pass: true,
                map_name: None,
                start_positions: Vec::new(),
                slots: vec![load_screen_slot("Alice", "USA", 0, false, true)],
            };

            initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context, runtime);
            runtime.windows.with_manager(|receiving_wm| {
                *receiving_wm = wm;
            });

            update_load_screen_with_pump(
                LoadScreenKind::Multiplayer,
                101.0,
                Some(&mut *presentation_pump),
                runtime,
            )
            .unwrap();

            assert!(calls.lock().unwrap().is_empty());
            assert_eq!(
                progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0", runtime),
                Some(0.0)
            );
            clear_multiplayer_load_progress_hook();
        })();
    });
}

#[test]
fn gamespy_update_reports_progress_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "gamespy_update_reports_progress_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let owner = operation_windows.clone();
        (|| {
            reset_multiplayer_load_screen_state(runtime);
            clear_multiplayer_load_progress_hook();
            let calls = Arc::new(Mutex::new(Vec::new()));
            let hook_calls = Arc::clone(&calls);
            register_multiplayer_load_progress_hook(move |player_id, percent| {
                hook_calls.lock().unwrap().push((player_id, percent));
            });

            let mut wm = WindowManager::new();
            create_multiplayer_slot_windows(
                &mut wm,
                "GameSpyLoadScreen.wnd",
                1,
                &mut runtime.effects,
            );
            create_gamespy_slot_windows(&mut wm, 1, &mut runtime.effects);

            let context = LoadScreenInitContext {
                local_player_name: "Alice".to_string(),
                local_side_name: "USA".to_string(),
                local_template_name: "FactionAmerica".to_string(),
                local_general_name: "USA".to_string(),
                local_general_features: "USA".to_string(),
                local_general_portrait: None,
                local_load_screen_music: String::new(),
                local_team_number: 0,
                shell_game_did_mem_pass: true,
                map_name: None,
                start_positions: Vec::new(),
                slots: vec![load_screen_slot("Alice", "USA", 0, false, true)],
            };

            initialize_gamespy_windows(&mut wm, &context, runtime);
            runtime.windows.with_manager(|receiving_wm| {
                *receiving_wm = wm;
            });

            update_load_screen_with_pump(
                LoadScreenKind::GameSpy,
                77.0,
                Some(&mut *presentation_pump),
                runtime,
            )
            .unwrap();

            assert_eq!(*calls.lock().unwrap(), vec![(0, 77)]);
            assert_eq!(
                progress_value("GameSpyLoadScreen.wnd:ProgressLoad0", runtime),
                Some(0.77)
            );
            clear_multiplayer_load_progress_hook();
        })();
    });
}

#[test]
fn set_progress_window_uses_cpp_progress_message_range_rules() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let mut wm = WindowManager::new();
        named_progress_test_window(
            &mut wm,
            "SinglePlayerLoadScreen.wnd:ProgressLoad",
            &mut runtime.effects,
        );

        set_progress_window(
            &mut wm,
            "SinglePlayerLoadScreen.wnd:ProgressLoad",
            37.9,
            &mut runtime.effects,
        );
        let progress = wm
            .find_window_by_name("SinglePlayerLoadScreen.wnd:ProgressLoad")
            .expect("progress");
        assert_eq!(
            progress
                .borrow_mut()
                .progress_bar_mut()
                .unwrap()
                .percentage(),
            37.0
        );

        set_progress_window(
            &mut wm,
            "SinglePlayerLoadScreen.wnd:ProgressLoad",
            138.4,
            &mut runtime.effects,
        );
        assert_eq!(
            progress
                .borrow_mut()
                .progress_bar_mut()
                .unwrap()
                .percentage(),
            37.0
        );

        set_progress_window(
            &mut wm,
            "SinglePlayerLoadScreen.wnd:ProgressLoad",
            -4.2,
            &mut runtime.effects,
        );
        assert_eq!(
            progress
                .borrow_mut()
                .progress_bar_mut()
                .unwrap()
                .percentage(),
            37.0
        );
    });
}

#[test]
fn update_load_screen_clears_cursor_tooltip_like_cpp() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        with_fixture_mouse(runtime, |mouse| {
            mouse.set_cursor_tooltip("Stale tooltip".to_string(), Some(0), None, None);
            assert_eq!(mouse.cursor_tooltip_state().tooltip_text, "Stale tooltip");
            assert!(!mouse.cursor_tooltip_state().is_tooltip_empty);
        });

        update_load_screen_with_pump(
            LoadScreenKind::SinglePlayer,
            50.0,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();

        with_fixture_mouse(runtime, |mouse| {
            assert_eq!(mouse.cursor_tooltip_state().tooltip_text, "");
            assert!(mouse.cursor_tooltip_state().is_tooltip_empty);
        });
    });
}

#[test]
fn map_transfer_update_pumps_liteupdate_before_clearing_tooltip_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "map_transfer_update_pumps_liteupdate_before_clearing_tooltip_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = Arc::clone(&calls);
        clear_map_transfer_liteupdate_hook();
        register_map_transfer_liteupdate_hook(move |receiving_mouse| {
            hook_calls.fetch_add(1, Ordering::SeqCst);
            {
                let mouse = receiving_mouse.expect("fixture callback has actual Mouse");
                mouse.set_cursor_tooltip(
                    "Network pump touched tooltip".to_string(),
                    None,
                    None,
                    None,
                )
            }
        });

        update_load_screen_with_pump(
            LoadScreenKind::MapTransfer,
            0.0,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        with_fixture_mouse(runtime, |mouse| {
            assert_eq!(mouse.cursor_tooltip_state().tooltip_text, "");
            assert!(mouse.cursor_tooltip_state().is_tooltip_empty);
        });
        clear_map_transfer_liteupdate_hook();
    });
}

#[test]
fn non_map_transfer_updates_do_not_pump_liteupdate() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "non_map_transfer_updates_do_not_pump_liteupdate"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = Arc::clone(&calls);
        clear_map_transfer_liteupdate_hook();
        register_map_transfer_liteupdate_hook(move |receiving_mouse| {
            hook_calls.fetch_add(1, Ordering::SeqCst);
        });

        update_load_screen_with_pump(
            LoadScreenKind::SinglePlayer,
            0.0,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        clear_map_transfer_liteupdate_hook();
    });
}

#[test]
fn non_multiplayer_updates_do_not_report_load_progress() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "non_multiplayer_updates_do_not_report_load_progress"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        clear_multiplayer_load_progress_hook();
        let calls = Arc::new(AtomicUsize::new(0));
        let hook_calls = Arc::clone(&calls);
        register_multiplayer_load_progress_hook(move |_, _| {
            hook_calls.fetch_add(1, Ordering::SeqCst);
        });

        update_load_screen_with_pump(
            LoadScreenKind::SinglePlayer,
            50.0,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();
        update_load_screen_with_pump(
            LoadScreenKind::Challenge,
            50.0,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();
        update_load_screen_with_pump(
            LoadScreenKind::MapTransfer,
            50.0,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        clear_multiplayer_load_progress_hook();
    });
}

#[test]
fn update_load_screen_finishes_every_branch_after_local_work_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "update_load_screen_finishes_every_branch_after_local_work_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        clear_map_transfer_liteupdate_hook();
        clear_multiplayer_load_progress_hook();
        clear_load_screen_finish_update_hook();
        clear_load_screen_presentation_pump();

        let finish_calls = Arc::new(AtomicUsize::new(0));
        let hook_finish_calls = Arc::clone(&finish_calls);
        register_load_screen_finish_update_hook(move |receiving_mouse| {
            hook_finish_calls.fetch_add(1, Ordering::SeqCst);
            {
                let mouse = receiving_mouse.expect("fixture callback has actual Mouse");
                assert_eq!(mouse.cursor_tooltip_state().tooltip_text, "");
                assert!(mouse.cursor_tooltip_state().is_tooltip_empty);
            }
        });

        let presentation_calls = Arc::new(AtomicUsize::new(0));
        let hook_presentation_calls = Arc::clone(&presentation_calls);
        register_load_screen_presentation_pump(move || {
            hook_presentation_calls.fetch_add(1, Ordering::SeqCst);
        });

        register_map_transfer_liteupdate_hook(move |receiving_mouse| {
            let mouse = receiving_mouse.expect("fixture callback has actual Mouse");
            mouse.set_cursor_tooltip("Liteupdate touched tooltip".to_string(), None, None, None)
        });

        update_load_screen_with_pump(
            LoadScreenKind::MapTransfer,
            0.0,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();
        update_load_screen_with_pump(
            LoadScreenKind::SinglePlayer,
            25.0,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();
        update_load_screen_with_pump(
            LoadScreenKind::Challenge,
            50.0,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();
        update_load_screen_with_pump(
            LoadScreenKind::Multiplayer,
            75.0,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();

        assert_eq!(finish_calls.load(Ordering::SeqCst), 4);
        assert_eq!(presentation_calls.load(Ordering::SeqCst), 4);
        clear_map_transfer_liteupdate_hook();
        clear_load_screen_finish_update_hook();
        clear_load_screen_presentation_pump();
    });
}
