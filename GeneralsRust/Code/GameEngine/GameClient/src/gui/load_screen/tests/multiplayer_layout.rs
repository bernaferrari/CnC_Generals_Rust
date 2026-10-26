// LoadScreen.cpp multiplayer layout fixture cases; included in the original logical test module.

#[test]
fn multiplayer_init_compacts_visible_context_slots_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "multiplayer_init_compacts_visible_context_slots_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            module_path!(),
            "multiplayer_init_compacts_visible_context_slots_like_cpp",
        ) {
            return;
        }
        // Own the selected classic GameText catalog in this exact process.
        // Language fallback registrations cannot override a loaded retail CSF.
        let directory = tempfile::tempdir().unwrap();
        let strings = directory.path().join("MultiplayerTeams.str");
        std::fs::write(&strings, "Team:3\n\"Team Three\"\nEND\n").unwrap();
        GameText::init_map_string_file(strings.to_str().unwrap())
            .expect("admit actual selected team map-string catalog");
        assert_eq!(
            GameText::fetch_with_exists("Team:3"),
            ("Team Three".to_owned(), true),
        );
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
        named_test_window(
            &mut wm,
            "MultiplayerLoadScreen.wnd:WinMapPreview",
            &mut runtime.effects,
        );
        wm.find_window_by_name("MultiplayerLoadScreen.wnd:WinMapPreview")
            .expect("preview")
            .borrow_mut()
            .set_status(WindowStatus::IMAGE);

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
            slots: vec![
                load_screen_slot_with_text_color(
                    "Alice",
                    "USA",
                    0,
                    Some(2),
                    Some(0xFF11_2233),
                    false,
                    true,
                ),
                load_screen_slot("Empty", "GLA", 1, false, false),
                load_screen_slot_with_color("Bob", "China", 2, Some(4), false, true),
            ],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context, runtime);

        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextPlayer0"),
            "Alice"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextPlayer1"),
            "Bob"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextTeam1"),
            "Team Three"
        );
        assert_eq!(
            window_text_color(&wm, "MultiplayerLoadScreen.wnd:StaticTextPlayer0"),
            0xFF11_2233
        );
        assert_eq!(
            window_text_color(&wm, "MultiplayerLoadScreen.wnd:StaticTextSide0"),
            0xFF11_2233
        );
        assert_eq!(
            window_text_color(&wm, "MultiplayerLoadScreen.wnd:StaticTextTeam0"),
            0xFF11_2233
        );
        assert_eq!(
            window_image_name(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait", 0),
            Some("SAFactionLogoLg_US".to_string())
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures"),
            "USA"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralName"),
            "USA"
        );
        assert_eq!(
            window_image_name(&wm, "MultiplayerLoadScreen.wnd:ProgressLoad1", 6),
            Some("LoadingBar_ProgressCenter4".to_string())
        );
        assert!(
            !window_status(&wm, "MultiplayerLoadScreen.wnd:WinMapPreview")
                .contains(WindowStatus::IMAGE)
        );
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ProgressLoad1"
        ));
        assert!(window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ProgressLoad2"
        ));
        assert!(window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:StaticTextPlayer2"
        ));
    });
}

#[test]
fn multiplayer_row_team_text_is_localized_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "multiplayer_row_team_text_is_localized_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            module_path!(),
            "multiplayer_row_team_text_is_localized_like_cpp",
        ) {
            return;
        }
        // Own the selected classic GameText catalog in this exact process.
        // Language fallback registrations cannot override a loaded retail CSF.
        let directory = tempfile::tempdir().unwrap();
        let strings = directory.path().join("MultiplayerTeams.str");
        std::fs::write(&strings, "Team:2\n\"Team Two\"\nEND\n").unwrap();
        GameText::init_map_string_file(strings.to_str().unwrap())
            .expect("admit actual selected team map-string catalog");
        assert_eq!(
            GameText::fetch_with_exists("Team:2"),
            ("Team Two".to_owned(), true),
        );

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
        named_test_window(
            &mut wm,
            "MultiplayerLoadScreen.wnd:WinMapPreview",
            &mut runtime.effects,
        );

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
            slots: vec![load_screen_slot("Local", "USA", 1, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context, runtime);

        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:StaticTextTeam0"),
            "Team Two"
        );
    });
}

#[test]
fn multiplayer_progress_bar_image_falls_back_when_color_image_missing_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "multiplayer_progress_bar_image_falls_back_when_color_image_missing_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let collection = get_mapped_image_collection();
        {
            let mut collection = collection.write();
            collection.clear();
            collection.add_image(crate::display::image::Image::with_name(
                "LoadingBar_Progress",
            ));
        }

        let colored = load_screen_slot_with_color("Player", "USA", 3, Some(6), false, true);
        assert_eq!(
            multiplayer_progress_bar_image(&colored),
            Some("LoadingBar_Progress".to_string())
        );

        collection.write().clear();
    });
}

#[test]
fn multiplayer_init_uses_context_general_portrait_features_and_name_like_cpp() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

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
        named_test_window(
            &mut wm,
            "MultiplayerLoadScreen.wnd:WinMapPreview",
            &mut runtime.effects,
        );

        let context = LoadScreenInitContext {
            local_player_name: "Local".to_string(),
            local_side_name: "USA".to_string(),
            local_template_name: "FactionAmericaSuperWeaponGeneral".to_string(),
            local_general_name: "General Alexander".to_string(),
            local_general_features: "Super Weapon General".to_string(),
            local_general_portrait: Some("SAGeneralPortrait".to_string()),
            local_load_screen_music: "Load_USA".to_string(),
            local_team_number: 0,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: vec![load_screen_slot("Local", "USA", 0, false, true)],
        };

        initialize_multiplayer_windows(&mut wm, "MultiplayerLoadScreen.wnd", &context, runtime);

        assert_eq!(
            window_image_name(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralPortrait", 0),
            Some("SAGeneralPortrait".to_string())
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralFeatures"),
            "Super Weapon General"
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:LocalGeneralName"),
            "General Alexander"
        );
    });
}

#[test]
fn gamespy_init_keeps_player_row_for_ai_but_hides_ai_stats() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let mut wm = WindowManager::new();
        create_multiplayer_slot_windows(&mut wm, "GameSpyLoadScreen.wnd", 3, &mut runtime.effects);
        create_gamespy_slot_windows(&mut wm, 3, &mut runtime.effects);
        named_test_window(
            &mut wm,
            "GameSpyLoadScreen.wnd:LocalGeneralPortrait",
            &mut runtime.effects,
        );
        named_test_window(
            &mut wm,
            "GameSpyLoadScreen.wnd:LocalGeneralFeatures",
            &mut runtime.effects,
        );
        named_test_window(
            &mut wm,
            "GameSpyLoadScreen.wnd:LocalGeneralName",
            &mut runtime.effects,
        );

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
            slots: vec![
                load_screen_slot("Human", "USA", 0, false, true),
                load_screen_slot("AI", "GLA", -1, true, true),
            ],
        };

        initialize_gamespy_windows(&mut wm, &context, runtime);

        assert!(!window_hidden(&wm, "GameSpyLoadScreen.wnd:WinPlayer0"));
        assert!(!window_hidden(
            &wm,
            "GameSpyLoadScreen.wnd:StaticTextWinLoss0"
        ));
        assert!(!window_hidden(&wm, "GameSpyLoadScreen.wnd:WinPlayer1"));
        assert!(window_hidden(
            &wm,
            "GameSpyLoadScreen.wnd:StaticTextWinLoss1"
        ));
        assert!(
            wm.find_window_by_name("GameSpyLoadScreen.wnd:StaticTextTeam1")
                .is_none()
        );
        assert_eq!(
            window_image_name(&wm, "GameSpyLoadScreen.wnd:LocalGeneralPortrait", 0),
            Some("SAFactionLogo144_US".to_string())
        );
        assert!(window_hidden(&wm, "GameSpyLoadScreen.wnd:WinPlayer2"));
    });
}

#[test]
fn map_transfer_init_compacts_human_slots_and_hides_done_progress_like_cpp() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        reset_map_transfer_load_screen_state(runtime);
        let mut wm = WindowManager::new();
        create_map_transfer_slot_windows(&mut wm, 4, &mut runtime.effects);

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
            slots: vec![
                load_screen_slot_with_map(0, "Host", Some(0xFF11_2233), true, false, true),
                load_screen_slot_with_map(1, "AI", Some(0xFF44_5566), true, true, true),
                load_screen_slot_with_map(2, "NeedsMap", Some(0xFF77_8899), false, false, true),
                load_screen_slot_with_map(3, "HasMap", Some(0xFFAA_BBCC), true, false, true),
            ],
        };

        initialize_map_transfer_windows(&mut wm, &context, runtime);

        assert_eq!(
            window_text(&wm, "MapTransferScreen.wnd:StaticTextPlayer0"),
            "Host"
        );
        assert_eq!(
            window_text(&wm, "MapTransferScreen.wnd:StaticTextPlayer1"),
            "NeedsMap"
        );
        assert_eq!(
            window_text(&wm, "MapTransferScreen.wnd:StaticTextPlayer2"),
            "HasMap"
        );
        assert!(window_hidden(&wm, "MapTransferScreen.wnd:ProgressLoad0"));
        assert!(!window_hidden(&wm, "MapTransferScreen.wnd:ProgressLoad1"));
        assert!(window_hidden(&wm, "MapTransferScreen.wnd:ProgressLoad2"));
        assert!(window_hidden(&wm, "MapTransferScreen.wnd:ProgressLoad3"));
        assert_eq!(
            window_text_color(&wm, "MapTransferScreen.wnd:StaticTextProgress1"),
            0xFF77_8899
        );
        assert_eq!(
            progress_fill_color(&wm, "MapTransferScreen.wnd:ProgressLoad1"),
            crate::gui::gadgets::Color::rgba(0x77, 0x88, 0x99, 0xFF)
        );

        let state = with_map_transfer_load_screen_state(|state| state.clone(), runtime);
        assert_eq!(state.player_lookup[0], 0);
        assert_eq!(state.player_lookup[1], -1);
        assert_eq!(state.player_lookup[2], 1);
        assert_eq!(state.player_lookup[3], 2);
    });
}

#[test]
fn retail_timeout_formats_decimal_precision_like_cpp() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        for (seconds, expected) in [
            (0, "Abort in 0:00"),
            (5, "Abort in 0:05"),
            (59, "Abort in 0:59"),
            (60, "Abort in 1:00"),
            (125, "Abort in 2:05"),
            (-1, "Abort in 0:-01"),
            (-65, "Abort in -1:-05"),
            (i32::MAX, "Abort in 35791394:07"),
            (i32::MIN, "Abort in -35791394:-08"),
        ] {
            assert_eq!(
                format_map_transfer_timeout_for_template("Abort in %d:%2.2d", seconds),
                expected,
                "seconds={seconds}",
            );
        }
        assert_eq!(
            format_map_transfer_timeout_for_template("%d:%d remaining", 125),
            "2:5 remaining",
            "retain the original unpadded regression's selected format",
        );
        assert_eq!(
            format_map_transfer_timeout_for_template("残り %d:%2.2d (100%%)", 125),
            "残り 2:05 (100%)",
        );
        assert_eq!(
            format_map_transfer_timeout_for_template("%%d %d %%2.2d %2.2d", 125),
            "%d 2 %2.2d 05",
            "escaped percent does not consume a countdown argument",
        );
    });
}

#[test]
fn multiplayer_init_resets_all_progress_bars_like_cpp() {
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

            for slot in 0..3 {
                let name = format!("MultiplayerLoadScreen.wnd:ProgressLoad{slot}");
                wm.find_window_by_name(&name)
                    .expect("progress")
                    .borrow_mut()
                    .progress_bar_mut()
                    .expect("progress widget")
                    .set_value(0.87);
            }

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

            assert_eq!(
                progress_value("MultiplayerLoadScreen.wnd:ProgressLoad0", runtime),
                Some(0.0)
            );
            assert_eq!(
                progress_value("MultiplayerLoadScreen.wnd:ProgressLoad1", runtime),
                Some(0.0)
            );
            assert_eq!(
                progress_value("MultiplayerLoadScreen.wnd:ProgressLoad2", runtime),
                Some(0.0)
            );
        })();
    });
}

#[test]
fn multiplayer_start_position_buttons_match_map_waypoints_and_apparent_slots() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "multiplayer_start_position_buttons_match_map_waypoints_and_apparent_slots"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        Language::clear_localized_strings();
        let mut wm = WindowManager::new();
        named_test_window(
            &mut wm,
            "MultiplayerLoadScreen.wnd:WinMapPreview",
            &mut runtime.effects,
        );
        wm.find_window_by_name("MultiplayerLoadScreen.wnd:WinMapPreview")
            .expect("preview")
            .borrow_mut()
            .set_size(100, 100)
            .expect("preview size");
        create_multiplayer_start_position_windows(
            &mut wm,
            "MultiplayerLoadScreen.wnd",
            &mut runtime.effects,
        );

        let mut metadata = MapMetaData::new();
        metadata.is_multiplayer = true;
        metadata.num_players = 3;
        metadata.extent =
            Region3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(100.0, 100.0, 0.0));
        metadata.set_waypoint("Player_1_Start".to_string(), Coord3D::new(25.0, 75.0, 0.0));
        metadata.set_waypoint("Player_2_Start".to_string(), Coord3D::new(75.0, 25.0, 0.0));
        metadata.set_waypoint("Player_3_Start".to_string(), Coord3D::new(25.0, 75.0, 0.0));

        update_multiplayer_start_position_buttons(
            &mut wm,
            "MultiplayerLoadScreen.wnd",
            Some(&metadata),
            &[Some(1), None, Some(0)],
        );

        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"
        ));
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"
        ));
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition2"
        ));
        assert!(window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition3"
        ));
        assert_eq!(
            window_position(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"),
            (20, 20)
        );
        assert_eq!(
            window_position(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"),
            (70, 70)
        );
        assert_eq!(
            window_position(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition2"),
            (20, 31)
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"),
            GameText::fetch("NUMBER:3")
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"),
            GameText::fetch("NUMBER:1")
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition2"),
            ""
        );
        Language::clear_localized_strings();
    });
}

#[test]
fn multiplayer_map_preview_keeps_start_positions_when_preview_image_missing_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "multiplayer_map_preview_keeps_start_positions_when_preview_image_missing_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        Language::clear_localized_strings();
        game_engine::common::ini::ini_map_cache::init_global_map_cache();

        let map_name = "Maps/TestNoPreview/TestNoPreview.map";
        let mut metadata = MapMetaData::new();
        metadata.is_multiplayer = true;
        metadata.num_players = 2;
        metadata.extent =
            Region3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(100.0, 100.0, 0.0));
        metadata.set_waypoint("Player_1_Start".to_string(), Coord3D::new(20.0, 80.0, 0.0));
        metadata.set_waypoint("Player_2_Start".to_string(), Coord3D::new(80.0, 20.0, 0.0));
        {
            let mut cache =
                game_engine::common::ini::ini_map_cache::get_map_cache_mut().expect("map cache");
            cache.insert(map_name.to_string(), metadata);
        }

        let mut wm = WindowManager::new();
        named_test_window(
            &mut wm,
            "MultiplayerLoadScreen.wnd:WinMapPreview",
            &mut runtime.effects,
        );
        wm.find_window_by_name("MultiplayerLoadScreen.wnd:WinMapPreview")
            .expect("preview")
            .borrow_mut()
            .set_size(100, 100)
            .expect("preview size");
        create_multiplayer_start_position_windows(
            &mut wm,
            "MultiplayerLoadScreen.wnd",
            &mut runtime.effects,
        );

        initialize_multiplayer_map_preview(
            &mut wm,
            "MultiplayerLoadScreen.wnd",
            Some(map_name),
            &[Some(1), Some(0)],
        );

        assert!(
            !window_status(&wm, "MultiplayerLoadScreen.wnd:WinMapPreview")
                .contains(WindowStatus::IMAGE)
        );
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"
        ));
        assert!(!window_hidden(
            &wm,
            "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"
        ));
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition0"),
            GameText::fetch("NUMBER:2")
        );
        assert_eq!(
            window_text(&wm, "MultiplayerLoadScreen.wnd:ButtonMapStartPosition1"),
            GameText::fetch("NUMBER:1")
        );

        game_engine::common::ini::ini_map_cache::get_map_cache_mut()
            .expect("map cache")
            .remove(map_name);
        Language::clear_localized_strings();
    });
}

#[test]
fn multiplayer_start_position_buttons_hide_without_multiplayer_metadata() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let mut wm = WindowManager::new();
        named_test_window(
            &mut wm,
            "GameSpyLoadScreen.wnd:WinMapPreview",
            &mut runtime.effects,
        );
        create_multiplayer_start_position_windows(
            &mut wm,
            "GameSpyLoadScreen.wnd",
            &mut runtime.effects,
        );

        let mut metadata = MapMetaData::new();
        metadata.is_multiplayer = false;
        metadata.num_players = 2;

        update_multiplayer_start_position_buttons(
            &mut wm,
            "GameSpyLoadScreen.wnd",
            Some(&metadata),
            &[Some(0)],
        );

        for slot in 0..MAX_LOAD_SCREEN_SLOTS {
            assert!(window_hidden(
                &wm,
                &format!("GameSpyLoadScreen.wnd:ButtonMapStartPosition{slot}")
            ));
        }
    });
}

#[test]
fn multiplayer_local_general_faction_logos_match_cpp_fallbacks() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        assert_eq!(
            multiplayer_local_general_faction_logo("USA", "MultiplayerLoadScreen.wnd"),
            Some("SAFactionLogoLg_US")
        );
        assert_eq!(
            multiplayer_local_general_faction_logo("FactionGLA", "MultiplayerLoadScreen.wnd"),
            Some("SUFactionLogoLg_GLA")
        );
        assert_eq!(
            multiplayer_local_general_faction_logo("China", "GameSpyLoadScreen.wnd"),
            Some("SNFactionLogo144_China")
        );
        assert_eq!(
            multiplayer_local_general_faction_logo("Random", "GameSpyLoadScreen.wnd"),
            None
        );
    });
}
