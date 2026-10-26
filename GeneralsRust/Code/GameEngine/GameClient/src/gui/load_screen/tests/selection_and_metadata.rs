// LoadScreen.cpp selection and metadata fixture cases; included in the original logical test module.

#[test]
fn selection_matches_cpp_game_logic_modes() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let base = LoadScreenRequest {
            mode: LoadScreenGameMode::None,
            loading_save_game: false,
            has_current_campaign: false,
            current_campaign_is_challenge: false,
        };

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Shell,
                ..base
            }),
            Some(LoadScreenKind::ShellGame)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Replay,
                ..base
            }),
            Some(LoadScreenKind::ShellGame)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Skirmish,
                ..base
            }),
            Some(LoadScreenKind::Multiplayer)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Lan,
                ..base
            }),
            Some(LoadScreenKind::Multiplayer)
        );
        assert_eq!(
            select_load_screen(LoadScreenRequest {
                mode: LoadScreenGameMode::Internet,
                ..base
            }),
            Some(LoadScreenKind::GameSpy)
        );
        assert_eq!(select_load_screen(base), None);
    });
}

#[test]
fn single_player_selection_matches_campaign_and_save_rules() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let normal_campaign = LoadScreenRequest {
            mode: LoadScreenGameMode::SinglePlayer,
            loading_save_game: false,
            has_current_campaign: true,
            current_campaign_is_challenge: false,
        };
        assert_eq!(
            select_load_screen(normal_campaign),
            Some(LoadScreenKind::SinglePlayer)
        );

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                current_campaign_is_challenge: true,
                ..normal_campaign
            }),
            Some(LoadScreenKind::Challenge)
        );

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                loading_save_game: true,
                ..normal_campaign
            }),
            Some(LoadScreenKind::ShellGame)
        );

        assert_eq!(
            select_load_screen(LoadScreenRequest {
                has_current_campaign: false,
                ..normal_campaign
            }),
            Some(LoadScreenKind::ShellGame)
        );
    });
}

#[test]
fn descriptors_match_cpp_layout_names() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let single = descriptor_for_kind(LoadScreenKind::SinglePlayer);
        assert_eq!(single.layout, "Menus/SinglePlayerLoadScreen.wnd");
        assert_eq!(
            single.primary_progress,
            "SinglePlayerLoadScreen.wnd:ProgressLoad"
        );
        assert!(single.uses_progress_fudge);

        let multiplayer = descriptor_for_kind(LoadScreenKind::Multiplayer);
        assert_eq!(multiplayer.layout, "Menus/MultiplayerLoadScreen.wnd");
        assert_eq!(
            multiplayer.primary_progress,
            "MultiplayerLoadScreen.wnd:ProgressLoad0"
        );
        assert_eq!(multiplayer.slot_count, MAX_LOAD_SCREEN_SLOTS);

        let map_transfer = descriptor_for_kind(LoadScreenKind::MapTransfer);
        assert_eq!(map_transfer.layout, "Menus/MapTransferScreen.wnd");
        assert_eq!(
            map_transfer.primary_progress,
            "MapTransferScreen.wnd:ProgressLoad0"
        );
        assert_eq!(map_transfer.slot_count, MAX_LOAD_SCREEN_SLOTS);
        assert!(!map_transfer.uses_progress_fudge);
    });
}

#[test]
fn load_screen_init_context_default_preserves_single_local_slot() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let context = LoadScreenInitContext {
            local_player_name: "Fallback".to_string(),
            local_side_name: "GLA".to_string(),
            local_template_name: "FactionGLA".to_string(),
            local_general_name: "GLA".to_string(),
            local_general_features: "GLA".to_string(),
            local_general_portrait: None,
            local_load_screen_music: String::new(),
            local_team_number: 4,
            shell_game_did_mem_pass: true,
            map_name: None,
            start_positions: Vec::new(),
            slots: Vec::new(),
        };

        let slots = multiplayer_slot_contexts(&context);

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].player_name, "Fallback");
        assert_eq!(slots[0].side_name, "GLA");
        assert_eq!(multiplayer_team_text(&slots[0]), "Team:5");
    });
}

#[test]
fn multiplayer_team_text_matches_cpp_team_plus_one() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let ai_slot = load_screen_slot("AI", "GLA", -1, true, true);

        assert_eq!(multiplayer_team_text(&ai_slot), "Team:0");
        assert!(!load_screen_has_team_windows("GameSpyLoadScreen.wnd"));
        assert!(load_screen_has_team_windows("MultiplayerLoadScreen.wnd"));
    });
}

#[test]
fn game_info_context_preserves_original_slot_ids_and_apparent_colors() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0001);
        game_info.set_map("Maps/Test/Test.map".to_string());

        let mut alice = GameSlot::new();
        alice.set_state(SlotState::Player, "Alice".to_string(), 0x7F00_0001);
        alice.set_player_template(-1);
        alice.set_team_number(0);
        alice.set_color(2);
        alice.set_start_pos(1);

        let mut empty = GameSlot::new();
        empty.set_state(SlotState::Open, String::new(), 0);

        let mut bob = GameSlot::new();
        bob.set_state(SlotState::BrutalAI, String::new(), 0);
        bob.set_player_template(-1);
        bob.set_team_number(-1);
        bob.set_color(5);
        bob.set_start_pos(0);

        game_info.set_slot(0, alice);
        game_info.set_slot(1, empty);
        game_info.set_slot(2, bob);

        let context = load_screen_init_context_from_game_info(
            &game_info,
            runtime.effects.challenge_generals(),
        );

        assert_eq!(context.map_name.as_deref(), Some("Maps/Test/Test.map"));
        assert_eq!(context.start_positions[0], Some(1));
        assert_eq!(context.start_positions[1], None);
        assert_eq!(context.start_positions[2], Some(0));
        assert_eq!(context.local_player_name, "Alice");
        assert_eq!(context.local_team_number, 0);
        assert_eq!(context.slots.len(), 2);
        assert_eq!(context.slots[0].player_id, 0);
        assert_eq!(context.slots[0].apparent_color, Some(2));
        assert_eq!(context.slots[1].player_id, 2);
        assert_eq!(context.slots[1].team_number, -1);
        assert_eq!(context.slots[1].apparent_color, Some(5));
        assert!(context.slots[1].is_ai);
    });
}

#[test]
fn game_info_context_hides_map_when_local_pregame_slot_lacks_map_like_cpp() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0002);
        game_info.set_map("Maps/MissingLocal/MissingLocal.map".to_string());

        let mut host = GameSlot::new();
        host.set_state(SlotState::Player, "Host".to_string(), 0x7F00_0001);
        host.set_player_template(-1);
        host.set_start_pos(0);
        game_info.set_slot(0, host);

        let mut local = GameSlot::new();
        local.set_state(SlotState::Player, "Local".to_string(), 0x7F00_0002);
        local.set_player_template(-1);
        local.set_start_pos(1);
        local.set_map_availability(false);
        game_info.set_slot(1, local);

        let context = load_screen_init_context_from_game_info(
            &game_info,
            runtime.effects.challenge_generals(),
        );

        assert_eq!(game_info.get_local_slot_num(), 1);
        assert_eq!(context.local_player_name, "Local");
        assert_eq!(context.map_name, None);
        assert_eq!(context.start_positions[0], Some(0));
        assert_eq!(context.start_positions[1], Some(1));

        game_info.set_game_in_progress(true);
        let context = load_screen_init_context_from_game_info(
            &game_info,
            runtime.effects.challenge_generals(),
        );
        assert_eq!(
            context.map_name.as_deref(),
            Some("Maps/MissingLocal/MissingLocal.map")
        );
    });
}

#[test]
fn game_info_context_hides_map_when_local_slot_is_missing_like_cpp() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0009);
        game_info.set_map("Maps/NoLocal/NoLocal.map".to_string());

        let mut host = GameSlot::new();
        host.set_state(SlotState::Player, "Host".to_string(), 0x7F00_0001);
        host.set_player_template(-1);
        host.set_start_pos(0);
        game_info.set_slot(0, host);

        let context = load_screen_init_context_from_game_info(
            &game_info,
            runtime.effects.challenge_generals(),
        );

        assert_eq!(game_info.get_local_slot_num(), -1);
        assert_eq!(context.local_player_name, "Host");
        assert_eq!(context.map_name, None);
    });
}

#[test]
fn game_info_context_uses_game_lod_mem_pass_for_shell_intro_gate() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        game_engine::common::game_lod::set_mem_passed_override_for_tests(Some(false));

        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0001);

        let mut local = GameSlot::new();
        local.set_state(SlotState::Player, "Local".to_string(), 0x7F00_0001);
        local.set_player_template(-1);
        game_info.set_slot(0, local);

        let context = load_screen_init_context_from_game_info(
            &game_info,
            runtime.effects.challenge_generals(),
        );
        assert!(!context.shell_game_did_mem_pass);

        game_engine::common::game_lod::set_mem_passed_override_for_tests(Some(true));
        let context = load_screen_init_context_from_game_info(
            &game_info,
            runtime.effects.challenge_generals(),
        );
        assert!(context.shell_game_did_mem_pass);

        game_engine::common::game_lod::set_mem_passed_override_for_tests(None);
    });
}

#[test]
fn game_info_context_uses_local_template_for_general_presentation_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "game_info_context_uses_local_template_for_general_presentation_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        Language::clear_localized_strings();
        Language::register_localized_string("GUI:AirFeatures", "Air Force General");
        Language::register_localized_string("CHALLENGE:AirName", "General Granger");

        {
            let mut store =
                game_engine::common::rts::player_template::get_player_template_store_mut();
            store.clear();
            let mut template = PlayerTemplate::new("FactionAirForceGeneral".to_string());
            template.display_name = "Air Force".to_string();
            template.features = "GUI:AirFeatures".to_string();
            template.load_screen_music = "Load_USA".to_string();
            store.add_template(template);
        }

        let mut generals = ChallengeGenerals::new();
        {
            let positions = generals.challenge_generals_mut();
            positions[0] = GeneralPersona::new();
            positions[0].set_player_template_name("FactionAirForceGeneral".to_string());
            positions[0].set_bio_name("CHALLENGE:AirName".to_string());
            positions[0].set_bio_portrait_large(Some("AirGeneralPortrait".to_string()));
        }

        let mut game_info = GameInfo::new();
        game_info.set_in_game();
        game_info.set_local_ip(0x7F00_0001);
        let mut local = GameSlot::new();
        local.set_state(SlotState::Player, "Local".to_string(), 0x7F00_0001);
        local.set_player_template(0);
        local.set_team_number(0);
        local.set_color(2);
        local.set_start_pos(0);
        game_info.set_slot(0, local);

        let context = load_screen_init_context_from_game_info(&game_info, &generals);

        assert_eq!(context.local_player_name, "Local");
        assert_eq!(context.local_general_name, "General Granger");
        assert_eq!(context.local_general_features, "Air Force General");
        assert_eq!(
            context.local_general_portrait.as_deref(),
            Some("AirGeneralPortrait")
        );
        assert_eq!(context.local_load_screen_music, "Load_USA");

        game_engine::common::rts::player_template::get_player_template_store_mut().clear();
        Language::clear_localized_strings();
    });
}

#[test]
fn multiplayer_progress_bar_images_match_cpp_apparent_color_names() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "multiplayer_progress_bar_images_match_cpp_apparent_color_names"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        get_mapped_image_collection().write().clear();

        let colored = load_screen_slot_with_color("Player", "USA", 3, Some(6), false, true);
        assert_eq!(
            multiplayer_progress_bar_image(&colored),
            Some("LoadingBar_ProgressCenter6".to_string())
        );

        let fallback = load_screen_slot("Player", "USA", 3, false, true);
        assert_eq!(multiplayer_progress_bar_image(&fallback), None);

        let invalid = load_screen_slot_with_color("Player", "USA", 3, Some(-1), false, true);
        assert_eq!(multiplayer_progress_bar_image(&invalid), None);

        get_mapped_image_collection().write().clear();
    });
}

#[test]
fn progress_fudge_matches_single_player_cpp_formula() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let single = descriptor_for_kind(LoadScreenKind::SinglePlayer);
        assert!((transformed_progress_percent(single, 0.0) - (30.0 / 1.3)).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(single, 100.0) - 100.0).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(single, 150.0) - (180.0 / 1.3)).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(single, -50.0) - (-20.0 / 1.3)).abs() < f32::EPSILON);

        let shell = descriptor_for_kind(LoadScreenKind::ShellGame);
        assert!((transformed_progress_percent(shell, 42.0) - 42.0).abs() < f32::EPSILON);
        assert!((transformed_progress_percent(shell, 150.0) - 150.0).abs() < f32::EPSILON);

        let map_transfer = descriptor_for_kind(LoadScreenKind::MapTransfer);
        assert!((transformed_progress_percent(map_transfer, 42.0) - 42.0).abs() < f32::EPSILON);
    });
}
