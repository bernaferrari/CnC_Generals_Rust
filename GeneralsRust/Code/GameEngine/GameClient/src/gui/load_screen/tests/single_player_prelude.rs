// LoadScreen.cpp single player prelude fixture cases; included in the original logical test module.

#[test]
fn single_player_audio_prelude_plays_only_ambient_like_zh_md() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        // C++ LoadScreen.cpp:532-533 PULLED FROM THE MISSION DISK — BriefingVoice
        // is never force-played after the prelude. Only LoadScreenAmbient (:590).

        reset_single_player_load_screen_audio_state(runtime);
        with_single_player_load_screen_state(
            |state| *state = SinglePlayerLoadScreenState::default(),
            runtime,
        );
        {
            let manager = runtime.effects.campaign_manager.as_deref_mut().unwrap();
            let campaign = manager.new_campaign("AudioPrelude".to_string());
            let mission = campaign.new_mission("Mission1".to_string());
            mission.briefing_voice =
                game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_sound_file(
                    "BriefingVoiceEvent".to_string(),
                );
            manager.set_campaign_and_mission("AudioPrelude", "Mission1");
        }

        finish_single_player_load_screen_audio_prelude(runtime);

        let (briefing_played, briefing_handle, ambient_handle) =
            with_single_player_load_screen_state(
                |state| {
                    (
                        state.briefing_voice_played,
                        state.briefing_voice_handle,
                        state.ambient_loop_handle.clone(),
                    )
                },
                runtime,
            );
        assert!(!briefing_played);
        assert_eq!(briefing_handle, 0);
        assert_eq!(
            ambient_handle,
            accepted_handle(&audio_trace, "LoadScreenAmbient")
        );

        reset_single_player_load_screen_audio_state(runtime);
        let (briefing_played, briefing_handle, ambient_handle) =
            with_single_player_load_screen_state(
                |state| {
                    (
                        state.briefing_voice_played,
                        state.briefing_voice_handle,
                        state.ambient_loop_handle.clone(),
                    )
                },
                runtime,
            );
        assert!(!briefing_played);
        assert_eq!(briefing_handle, 0);
        assert_eq!(ambient_handle, None);
    });
}

#[test]
fn single_player_movie_open_failure_returns_before_audio_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "single_player_movie_open_failure_returns_before_audio_like_cpp"
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
            reset_single_player_load_screen_audio_state(runtime);
            clear_single_player_movie_play_hook();
            clear_single_player_movie_playing_hook();
            let movie_requests = Arc::new(Mutex::new(Vec::new()));
            let hook_requests = Arc::clone(&movie_requests);
            register_single_player_movie_play_hook(move |movie_name| {
                hook_requests.lock().unwrap().push(movie_name.to_string());
                false
            });

            {
                let manager = runtime.effects.campaign_manager.as_deref_mut().unwrap();
                let campaign = manager.new_campaign("USA".to_string());
                let mission = campaign.new_mission("Mission1".to_string());
                mission.movie_label = "MissingMovie.bik".to_string();
                mission.briefing_voice =
                    game_engine::common::ini::ini_misc_audio::AudioEventRTS::from_sound_file(
                        "BriefingVoiceEvent".to_string(),
                    );
                manager.set_campaign_and_mission("USA", "Mission1");
            }

            let mut wm = WindowManager::new();
            named_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
                &mut runtime.effects,
            );
            named_progress_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:ProgressLoad",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:Percent",
                &mut runtime.effects,
            );

            initialize_single_player_windows(&mut wm, true, runtime);

            assert_eq!(
                *movie_requests.lock().unwrap(),
                vec!["MissingMovie.bik".to_string()]
            );
            let (prelude_active, briefing_played, briefing_handle, ambient_handle) =
                with_single_player_load_screen_state(
                    |state| {
                        (
                            state.movie_prelude_active,
                            state.briefing_voice_played,
                            state.briefing_voice_handle,
                            state.ambient_loop_handle.clone(),
                        )
                    },
                    runtime,
                );
            assert!(!prelude_active);
            assert!(!briefing_played);
            assert_eq!(briefing_handle, 0);
            assert_eq!(ambient_handle, None);
            assert!(
                wm.find_window_by_name("SinglePlayerLoadScreen.wnd:Percent")
                    .expect("percent")
                    .borrow()
                    .is_hidden()
            );
            assert_eq!(
                window_enabled_image_name(
                    &wm,
                    "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
                    0
                ),
                None
            );
            assert_eq!(
                window_enabled_image_name(&wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", 6),
                None
            );

            runtime
                .windows
                .with_manager(|receiving_wm| *receiving_wm = wm);
            assert_eq!(
                run_load_screen_prelude_with_pump(
                    LoadScreenKind::SinglePlayer,
                    1,
                    Some(&mut *presentation_pump),
                    runtime
                )
                .unwrap(),
                LoadScreenPreludeOutcome::Failed,
                "a failed movie open must not turn into a synthetic prelude or audio path"
            );

            clear_single_player_movie_play_hook();
            clear_single_player_movie_playing_hook();
            reset_single_player_load_screen_audio_state(runtime);
        })();
    });
}

#[test]
fn single_player_movie_prelude_defers_ambient_until_movie_finishes_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "single_player_movie_prelude_defers_ambient_until_movie_finishes_like_cpp"
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
            reset_single_player_load_screen_audio_state(runtime);
            clear_single_player_movie_play_hook();
            clear_single_player_movie_playing_hook();
            let movie_requests = Arc::new(Mutex::new(Vec::new()));
            let hook_requests = Arc::clone(&movie_requests);
            register_single_player_movie_play_hook(move |movie_name| {
                hook_requests.lock().unwrap().push(movie_name.to_string());
                true
            });
            let movie_playing = Arc::new(AtomicBool::new(true));
            let hook_movie_playing = Arc::clone(&movie_playing);
            register_single_player_movie_playing_hook(move |movie_name| {
                assert_eq!(movie_name, "EA_LOGO.BIK");
                hook_movie_playing.load(Ordering::SeqCst)
            });

            {
                let manager = runtime.effects.campaign_manager.as_deref_mut().unwrap();
                let campaign = manager.new_campaign("China".to_string());
                let mission = campaign.new_mission("Mission1".to_string());
                mission.movie_label = "EA_LOGO.BIK".to_string();
                manager.set_campaign_and_mission("China", "Mission1");
            }

            let mut wm = WindowManager::new();
            named_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
                &mut runtime.effects,
            );
            named_progress_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:ProgressLoad",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:Percent",
                &mut runtime.effects,
            );

            initialize_single_player_windows(&mut wm, true, runtime);

            assert_eq!(
                *movie_requests.lock().unwrap(),
                vec!["EA_LOGO.BIK".to_string()]
            );
            let (prelude_active, ambient_handle) = with_single_player_load_screen_state(
                |state| {
                    (
                        state.movie_prelude_active,
                        state.ambient_loop_handle.clone(),
                    )
                },
                runtime,
            );
            assert!(prelude_active);
            assert_eq!(ambient_handle, None);
            assert!(
                wm.find_window_by_name("SinglePlayerLoadScreen.wnd:Percent")
                    .expect("percent")
                    .borrow()
                    .is_hidden()
            );
            assert_eq!(
                window_enabled_image_name(
                    &wm,
                    "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
                    0
                ),
                Some("MissionLoad_China".to_string())
            );
            assert_eq!(
                window_enabled_image_name(&wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", 6),
                Some("LoadingBar_ProgressCenter1".to_string())
            );

            runtime.windows.with_manager(|receiving_wm| {
                *receiving_wm = wm;
            });

            update_load_screen_with_pump(
                LoadScreenKind::SinglePlayer,
                25.0,
                Some(&mut *presentation_pump),
                runtime,
            )
            .unwrap();
            let (prelude_active, ambient_handle) = with_single_player_load_screen_state(
                |state| {
                    (
                        state.movie_prelude_active,
                        state.ambient_loop_handle.clone(),
                    )
                },
                runtime,
            );
            assert!(prelude_active);
            assert_eq!(ambient_handle, None);
            assert_eq!(
                progress_value("SinglePlayerLoadScreen.wnd:ProgressLoad", runtime),
                Some(0.42)
            );
            runtime.windows.with_manager(|wm| {
                assert_eq!(window_text(wm, "SinglePlayerLoadScreen.wnd:Percent"), "42%");
                assert!(
                    wm.find_window_by_name("SinglePlayerLoadScreen.wnd:Percent")
                        .expect("percent")
                        .borrow()
                        .is_hidden()
                );
            });

            movie_playing.store(false, Ordering::SeqCst);
            update_load_screen_with_pump(
                LoadScreenKind::SinglePlayer,
                50.0,
                Some(&mut *presentation_pump),
                runtime,
            )
            .unwrap();
            let (prelude_active, ambient_handle) = with_single_player_load_screen_state(
                |state| {
                    (
                        state.movie_prelude_active,
                        state.ambient_loop_handle.clone(),
                    )
                },
                runtime,
            );
            assert!(!prelude_active);
            assert_eq!(
                ambient_handle,
                accepted_handle(&audio_trace, "LoadScreenAmbient")
            );
            assert_eq!(
                progress_value("SinglePlayerLoadScreen.wnd:ProgressLoad", runtime),
                Some(0.61)
            );
            runtime.windows.with_manager(|wm| {
                assert_eq!(window_text(wm, "SinglePlayerLoadScreen.wnd:Percent"), "61%");
                assert!(
                    wm.find_window_by_name("SinglePlayerLoadScreen.wnd:Percent")
                        .expect("percent")
                        .borrow()
                        .is_hidden()
                );
            });

            reset_single_player_load_screen_audio_state(runtime);
            clear_single_player_movie_play_hook();
            clear_single_player_movie_playing_hook();
        })();
    });
}

#[test]
fn single_player_prelude_driver_pumps_until_movie_completion_before_map_work() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "single_player_prelude_driver_pumps_until_movie_completion_before_map_work"
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
            reset_single_player_load_screen_audio_state(runtime);
            clear_single_player_movie_play_hook();
            clear_single_player_movie_playing_hook();
            clear_load_screen_presentation_pump();

            register_single_player_movie_play_hook(|movie_name| movie_name == "EA_LOGO.BIK");
            let checks = Arc::new(AtomicUsize::new(0));
            let hook_checks = Arc::clone(&checks);
            register_single_player_movie_playing_hook(move |movie_name| {
                assert_eq!(movie_name, "EA_LOGO.BIK");
                hook_checks.fetch_add(1, Ordering::SeqCst) < 2
            });

            {
                let manager = runtime.effects.campaign_manager.as_deref_mut().unwrap();
                let campaign = manager.new_campaign("USA".to_string());
                let mission = campaign.new_mission("MissionPrelude".to_string());
                mission.movie_label = "EA_LOGO.BIK".to_string();
                manager.set_campaign_and_mission("USA", "MissionPrelude");
            }

            let mut wm = WindowManager::new();
            named_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
                &mut runtime.effects,
            );
            named_progress_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:ProgressLoad",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:Percent",
                &mut runtime.effects,
            );
            initialize_single_player_windows(&mut wm, true, runtime);
            runtime
                .windows
                .with_manager(|receiving_wm| *receiving_wm = wm);

            let presentation_pumps = Rc::new(Cell::new(0));
            let hook_pumps = Rc::clone(&presentation_pumps);
            register_load_screen_presentation_pump(move || {
                hook_pumps.set(hook_pumps.get() + 1);
            });

            assert_eq!(
                run_load_screen_prelude_with_pump(
                    LoadScreenKind::SinglePlayer,
                    8,
                    Some(&mut *presentation_pump),
                    runtime
                )
                .unwrap(),
                LoadScreenPreludeOutcome::Complete
            );
            assert_eq!(checks.load(Ordering::SeqCst), 3);
            assert_eq!(presentation_pumps.get(), 3);
            let (prelude_state, briefing_played, ambient_handle) =
                with_single_player_load_screen_state(
                    |state| {
                        (
                            state.prelude_state,
                            state.briefing_voice_played,
                            state.ambient_loop_handle.clone(),
                        )
                    },
                    runtime,
                );
            assert_eq!(prelude_state, LoadScreenPreludeState::Complete);
            assert!(
                !briefing_played,
                "ZH MD LoadScreen.cpp:532-533 never plays BriefingVoice after prelude"
            );
            assert_eq!(
                ambient_handle,
                accepted_handle(&audio_trace, "LoadScreenAmbient")
            );

            clear_load_screen_presentation_pump();
            clear_single_player_movie_play_hook();
            clear_single_player_movie_playing_hook();
            reset_single_player_load_screen_audio_state(runtime);
        })();
    });
}

#[test]
fn single_player_prelude_driver_forces_terminal_skip_when_movie_never_finishes() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "single_player_prelude_driver_forces_terminal_skip_when_movie_never_finishes"
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
            reset_single_player_load_screen_audio_state(runtime);
            clear_single_player_movie_play_hook();
            clear_single_player_movie_playing_hook();
            clear_load_screen_presentation_pump();

            register_single_player_movie_play_hook(|movie_name| movie_name == "StuckMovie.bik");
            register_single_player_movie_playing_hook(|movie_name| movie_name == "StuckMovie.bik");
            {
                let manager = runtime.effects.campaign_manager.as_deref_mut().unwrap();
                let campaign = manager.new_campaign("GLA".to_string());
                let mission = campaign.new_mission("MissionStuck".to_string());
                mission.movie_label = "StuckMovie.bik".to_string();
                manager.set_campaign_and_mission("GLA", "MissionStuck");
            }

            let mut wm = WindowManager::new();
            named_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
                &mut runtime.effects,
            );
            named_progress_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:ProgressLoad",
                &mut runtime.effects,
            );
            named_test_window(
                &mut wm,
                "SinglePlayerLoadScreen.wnd:Percent",
                &mut runtime.effects,
            );
            initialize_single_player_windows(&mut wm, true, runtime);
            runtime
                .windows
                .with_manager(|receiving_wm| *receiving_wm = wm);

            let presentation_pumps = Rc::new(Cell::new(0));
            let hook_pumps = Rc::clone(&presentation_pumps);
            register_load_screen_presentation_pump(move || {
                hook_pumps.set(hook_pumps.get() + 1);
            });

            assert_eq!(
                run_load_screen_prelude_with_pump(
                    LoadScreenKind::SinglePlayer,
                    2,
                    Some(&mut *presentation_pump),
                    runtime
                )
                .unwrap(),
                LoadScreenPreludeOutcome::Skipped
            );
            assert_eq!(presentation_pumps.get(), 2);
            let (prelude_state, ambient_handle) = with_single_player_load_screen_state(
                |state| (state.prelude_state, state.ambient_loop_handle.clone()),
                runtime,
            );
            assert_eq!(prelude_state, LoadScreenPreludeState::Skipped);
            assert_eq!(
                ambient_handle,
                accepted_handle(&audio_trace, "LoadScreenAmbient")
            );

            clear_load_screen_presentation_pump();
            clear_single_player_movie_play_hook();
            clear_single_player_movie_playing_hook();
            reset_single_player_load_screen_audio_state(runtime);
        })();
    });
}

#[test]
fn single_player_campaign_images_match_cpp_side_mapping() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        assert_eq!(
            single_player_campaign_images("USA"),
            Some(("MissionLoad_USA", "LoadingBar_ProgressCenter2"))
        );
        assert_eq!(
            single_player_campaign_images("gla"),
            Some(("MissionLoad_GLA", "LoadingBar_ProgressCenter3"))
        );
        assert_eq!(
            single_player_campaign_images("China"),
            Some(("MissionLoad_China", "LoadingBar_ProgressCenter1"))
        );
        assert_eq!(single_player_campaign_images("Challenge"), None);
    });
}

#[test]
fn single_player_mission_text_fetches_cpp_labels() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "single_player_mission_text_fetches_cpp_labels"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        Language::clear_localized_strings();
        Language::register_localized_string("MISSION:Objective0", "Capture the base");
        Language::register_localized_string("MISSION:Objective2", "Hold position");
        Language::register_localized_string("UNIT:Ranger", "Ranger");
        Language::register_localized_string("UNIT:Humvee", "Humvee");
        Language::register_localized_string("MISSION:Location", "Northern sector");

        let mut mission = Mission::new();
        mission.mission_objectives_label[0] = "MISSION:Objective0".to_string();
        mission.mission_objectives_label[2] = "MISSION:Objective2".to_string();
        mission.unit_names[0] = "UNIT:Ranger".to_string();
        mission.unit_names[1] = "UNIT:Humvee".to_string();
        mission.location_name_label = "MISSION:Location".to_string();

        let text = single_player_mission_text(&mission);

        assert_eq!(text.objective_lines[0], "Capture the base");
        assert_eq!(text.objective_lines[1], "");
        assert_eq!(text.objective_lines[2], "Hold position");
        assert_eq!(text.unit_descriptions[0], "Ranger");
        assert_eq!(text.unit_descriptions[1], "Humvee");
        assert_eq!(text.unit_descriptions[2], "");
        assert_eq!(text.location, "Northern sector");

        with_single_player_load_screen_state(
            |state| {
                state.mission_text = text.clone();
                state.current_objective_line = 0;
                state.current_objective_width_offset = 0;
                state.current_objective_line_character = 0;
                state.finished_objective_text = false;
            },
            runtime,
        );
        let cached = with_single_player_load_screen_state(|state| state.clone(), runtime);
        assert_eq!(cached.mission_text.objective_lines[0], "Capture the base");
        assert_eq!(cached.current_objective_line, 0);
        assert_eq!(cached.current_objective_width_offset, 0);
        assert_eq!(cached.current_objective_line_character, 0);
        assert!(!cached.finished_objective_text);

        Language::clear_localized_strings();
    });
}

#[test]
fn nonblocking_campaign_presents_last_frame_then_background_before_audio() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "nonblocking_campaign_presents_last_frame_then_background_before_audio"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        reset_single_player_load_screen_audio_state(runtime);
        clear_load_screen_presentation_pump();
        with_single_player_load_screen_state(
            |state| {
                state.prelude_state = LoadScreenPreludeState::Movie;
                state.movie_prelude_active = true;
                state.movie_label = "TraceCampaign".into();
            },
            runtime,
        );
        let (_, trace) = with_window_video_manager(runtime.videos(), |manager| {
            super::super::window_video_manager::load_movie_tests::insert(
                manager,
                usize::MAX,
                "TraceCampaign",
            )
        });
        begin_nonblocking_load_screen_prelude(LoadScreenKind::SinglePlayer, runtime);
        let observed = Rc::new(RefCell::new(Vec::new()));
        let hook_observed = observed.clone();
        let hook_trace = trace.clone();
        let hook_audio_trace = Rc::clone(&audio_trace);
        register_load_screen_presentation_pump(move || {
            let closed = hook_trace
                .lock()
                .unwrap()
                .iter()
                .any(|(op, _)| *op == "close");
            let ambient = hook_audio_trace
                .borrow()
                .iter()
                .any(|(name, _)| name == "LoadScreenAmbient");
            hook_observed.borrow_mut().push((closed, ambient));
        });
        assert_eq!(
            runtime.state.single_player.prelude_state,
            LoadScreenPreludeState::Movie
        );
        assert!(matches!(
            advance_load_screen_prelude_with_pump(
                LoadScreenKind::SinglePlayer,
                true,
                Some(&mut *presentation_pump),
                runtime
            )
            .unwrap(),
            LoadScreenPreludeStepResult::Pending {
                presentation_needed: true,
                ..
            }
        ));
        assert_eq!(
            runtime.state.single_player.prelude_state,
            LoadScreenPreludeState::Movie
        );
        pump_load_screen_prelude_presentation_with_pump(
            LoadScreenKind::SinglePlayer,
            Some(&mut *presentation_pump),
            runtime,
        )
        .unwrap();
        assert_eq!(
            runtime.state.single_player.prelude_state,
            LoadScreenPreludeState::Movie
        );
        assert!(matches!(
            advance_load_screen_prelude_with_pump(
                LoadScreenKind::SinglePlayer,
                true,
                Some(&mut *presentation_pump),
                runtime
            )
            .unwrap(),
            LoadScreenPreludeStepResult::Finished {
                outcome: LoadScreenPreludeOutcome::Complete,
                presentation_needed: false
            }
        ));
        assert_eq!(
            *observed.borrow(),
            vec![(false, false), (false, false), (true, false),]
        );
        assert_eq!(
            *trace.lock().unwrap(),
            vec![
                ("decompress", 0),
                ("render", 0),
                ("next", 0),
                ("decompress", 1),
                ("render", 1),
                ("next", 1),
                ("close", 2)
            ]
        );
        assert_eq!(
            with_single_player_load_screen_state(|state| state.prelude_state, runtime),
            LoadScreenPreludeState::Complete
        );
        clear_load_screen_presentation_pump();
        reset_single_player_load_screen_audio_state(runtime);
    });
}
