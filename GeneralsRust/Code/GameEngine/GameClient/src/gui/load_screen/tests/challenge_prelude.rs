// LoadScreen.cpp challenge prelude fixture cases; included in the original logical test module.

#[test]
fn challenge_persona_text_matches_cpp_load_screen_fields() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "challenge_persona_text_matches_cpp_load_screen_fields"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        Language::clear_localized_strings();
        Language::register_localized_string("CHALLENGE:PlayerName", "General Player");
        Language::register_localized_string("CHALLENGE:PlayerRank", "General");
        Language::register_localized_string("CHALLENGE:PlayerStrategy", "Air superiority");
        Language::register_localized_string("CHALLENGE:OpponentName", "General Opponent");
        Language::register_localized_string("CHALLENGE:OpponentRank", "Prince");
        Language::register_localized_string("CHALLENGE:OpponentStrategy", "Ambush");

        let mut generals = ChallengeGenerals::new();
        {
            let positions = generals.challenge_generals_mut();
            positions[0].set_campaign("ChallengeCampaign".to_string());
            positions[0].set_bio_name("CHALLENGE:PlayerName".to_string());
            positions[0].set_bio_rank("CHALLENGE:PlayerRank".to_string());
            positions[0].set_bio_strategy("CHALLENGE:PlayerStrategy".to_string());
            positions[0].set_bio_portrait_large(Some("PlayerPortrait".to_string()));
            positions[0].set_portrait_movie_left_name("PlayerMovieLeft".to_string());
            positions[0].set_portrait_movie_right_name("PlayerMovieRight".to_string());
            positions[0].set_name_sound("PlayerNameSound".to_string());
            positions[0].set_taunt_sound_1("PlayerTaunt1".to_string());
            positions[0].set_taunt_sound_2("PlayerTaunt2".to_string());
            positions[0].set_taunt_sound_3("PlayerTaunt3".to_string());

            positions[1].set_bio_name("CHALLENGE:OpponentName".to_string());
            positions[1].set_bio_rank("CHALLENGE:OpponentRank".to_string());
            positions[1].set_bio_strategy("CHALLENGE:OpponentStrategy".to_string());
            positions[1].set_bio_portrait_large(Some("OpponentPortrait".to_string()));
            positions[1].set_portrait_movie_left_name("OpponentMovieLeft".to_string());
            positions[1].set_portrait_movie_right_name("OpponentMovieRight".to_string());
            positions[1].set_name_sound("OpponentNameSound".to_string());
            positions[1].set_taunt_sound_1("OpponentTaunt1".to_string());
            positions[1].set_taunt_sound_2("OpponentTaunt2".to_string());
            positions[1].set_taunt_sound_3("OpponentTaunt3".to_string());
        }

        let (player, opponent) = challenge_persona_text_for_current_mission(
            "ChallengeCampaign",
            "CHALLENGE:OpponentName",
            &generals,
        )
        .expect("challenge personas");

        assert_eq!(player.big_name, "General Player");
        assert_eq!(player.name, "General Player");
        assert_eq!(player.rank, "General");
        assert_eq!(player.strategy, "Air superiority");
        assert_eq!(player.portrait_large.as_deref(), Some("PlayerPortrait"));
        assert_eq!(player.portrait_movie_left, "PlayerMovieLeft");
        assert_eq!(player.portrait_movie_right, "PlayerMovieRight");
        assert_eq!(player.name_sound, "PlayerNameSound");
        assert_eq!(
            player.taunt_sounds,
            ["PlayerTaunt1", "PlayerTaunt2", "PlayerTaunt3"]
        );

        assert_eq!(opponent.big_name, "General Opponent");
        assert_eq!(opponent.name, "General Opponent");
        assert_eq!(opponent.rank, "Prince");
        assert_eq!(opponent.strategy, "Ambush");
        assert_eq!(opponent.portrait_large.as_deref(), Some("OpponentPortrait"));
        assert_eq!(opponent.portrait_movie_left, "OpponentMovieLeft");
        assert_eq!(opponent.portrait_movie_right, "OpponentMovieRight");
        assert_eq!(opponent.name_sound, "OpponentNameSound");
        assert_eq!(
            opponent.taunt_sounds,
            ["OpponentTaunt1", "OpponentTaunt2", "OpponentTaunt3"]
        );

        Language::clear_localized_strings();
    });
}

#[test]
fn challenge_init_with_movie_waits_for_frame_activation_like_cpp_high_spec() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "challenge_init_with_movie_waits_for_frame_activation_like_cpp_high_spec"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    setup_current_challenge_for_tests("ChallengeIntro", fixture.client_mut());
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        clear_challenge_movie_play_hook();
        clear_challenge_movie_advance_hook();
        register_challenge_movie_play_hook(|movie_name| movie_name == "ChallengeIntro");
        let frame = Arc::new(AtomicUsize::new(0));
        let hook_frame = Arc::clone(&frame);
        register_challenge_movie_advance_hook(move |movie_name| {
            assert_eq!(movie_name, "ChallengeIntro");
            Some(LoadScreenMovieAdvance {
                frame_index: hook_frame.fetch_add(1, Ordering::SeqCst) as i32 + 1,
                frame_count: 200,
                completed: false,
            })
        });

        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm, &mut runtime.effects);

        initialize_challenge_windows(&mut wm, true, runtime);

        for name in CHALLENGE_BIO_LABEL_WINDOWS
            .iter()
            .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
            .copied()
        {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(window.borrow().is_hidden(), "{name}");
        }

        for _ in 0..FRAME_TITLES_START {
            let _ = advance_challenge_load_screen_prelude(&mut wm, runtime);
        }

        for name in CHALLENGE_BIO_LABEL_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(!window.borrow().is_hidden(), "{name}");
        }

        clear_challenge_movie_play_hook();
        clear_challenge_movie_advance_hook();
        Language::clear_localized_strings();
    });
}

#[test]
fn challenge_init_resets_window_video_manager_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "challenge_init_resets_window_video_manager_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    setup_current_challenge_for_tests("ChallengeIntro", fixture.client_mut());
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let owner = operation_windows.clone();
        (|| {
            clear_challenge_movie_play_hook();
            clear_challenge_movie_advance_hook();
            register_challenge_movie_play_hook(|_| false);

            let mut wm = WindowManager::new();
            challenge_test_windows(&mut wm, &mut runtime.effects);
            with_window_video_manager(runtime.videos(), |manager| {
                manager.set_global_flags_for_tests(true, true)
            });

            initialize_challenge_windows(&mut wm, true, runtime);

            let flags = with_window_video_manager(runtime.videos(), |manager| {
                manager.global_flags_for_tests()
            });
            assert_eq!(flags, (false, false));
            assert_eq!(
                with_challenge_load_screen_state(|state| state.prelude_state, runtime),
                LoadScreenPreludeState::Failed
            );
            runtime
                .windows
                .with_manager(|receiving_wm| *receiving_wm = wm);
            assert_eq!(
                run_load_screen_prelude_with_pump(
                    LoadScreenKind::Challenge,
                    1,
                    Some(&mut *presentation_pump),
                    runtime
                )
                .unwrap(),
                LoadScreenPreludeOutcome::Failed
            );
            update_load_screen_with_pump(
                LoadScreenKind::Challenge,
                100.0,
                Some(&mut *presentation_pump),
                runtime,
            )
            .unwrap();
            let (postlude_audio_played, ambient_handle) = with_challenge_load_screen_state(
                |state| {
                    (
                        state.postlude_audio_played,
                        state.ambient_loop_handle.clone(),
                    )
                },
                runtime,
            );
            assert!(!postlude_audio_played);
            assert_eq!(ambient_handle, None);
            clear_challenge_movie_play_hook();
            clear_challenge_movie_advance_hook();
            reset_challenge_load_screen_audio_state(runtime);
            Language::clear_localized_strings();
        })();
    });
}

#[test]
fn challenge_init_without_movie_returns_without_synthetic_reveal_like_cpp() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "challenge_init_without_movie_returns_without_synthetic_reveal_like_cpp"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    setup_current_challenge_for_tests("", fixture.client_mut());
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        clear_challenge_movie_play_hook();
        clear_challenge_movie_advance_hook();

        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm, &mut runtime.effects);

        initialize_challenge_windows(&mut wm, true, runtime);

        for name in CHALLENGE_BIO_LABEL_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(window.borrow().is_hidden(), "{name}");
        }
        assert!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioStrategyEntryRight")
                .expect("right strategy")
                .borrow()
                .get_text()
                .is_empty()
        );
        let (prelude_state, postlude_played, ambient_handle) = with_challenge_load_screen_state(
            |state| {
                (
                    state.prelude_state,
                    state.postlude_audio_played,
                    state.ambient_loop_handle.clone(),
                )
            },
            runtime,
        );
        assert_eq!(prelude_state, LoadScreenPreludeState::Failed);
        assert!(!postlude_played);
        assert_eq!(ambient_handle, None);

        Language::clear_localized_strings();
    });
}

#[test]
fn challenge_prelude_driver_pumps_authored_frames_before_completion() {
    if matches!(
        crate::test_process::run_bounded(
            module_path!(),
            "challenge_prelude_driver_pumps_authored_frames_before_completion"
        ),
        crate::test_process::TestProcess::ParentVerified
    ) {
        return;
    }
    let mut fixture = LoadScreenFixture::new();
    setup_current_challenge_for_tests("ChallengeIntro", fixture.client_mut());
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        let owner = operation_windows.clone();
        (|| {
            reset_challenge_load_screen_audio_state(runtime);
            clear_challenge_movie_play_hook();
            clear_challenge_movie_advance_hook();
            clear_load_screen_presentation_pump();
            Language::clear_localized_strings();

            register_challenge_movie_play_hook(|movie_name| movie_name == "ChallengeIntro");
            let advances = Arc::new(AtomicUsize::new(0));
            let hook_advances = Arc::clone(&advances);
            register_challenge_movie_advance_hook(move |movie_name| {
                assert_eq!(movie_name, "ChallengeIntro");
                let advance = hook_advances.fetch_add(1, Ordering::SeqCst);
                Some(if advance == 0 {
                    LoadScreenMovieAdvance {
                        frame_index: FRAME_TITLES_START,
                        frame_count: 200,
                        completed: false,
                    }
                } else {
                    LoadScreenMovieAdvance {
                        frame_index: FRAME_RIGHT_VOICE,
                        frame_count: 200,
                        completed: true,
                    }
                })
            });

            let mut wm = WindowManager::new();
            challenge_test_windows(&mut wm, &mut runtime.effects);
            initialize_challenge_windows(&mut wm, true, runtime);
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
                    LoadScreenKind::Challenge,
                    8,
                    Some(&mut *presentation_pump),
                    runtime
                )
                .unwrap(),
                LoadScreenPreludeOutcome::Complete
            );
            assert_eq!(advances.load(Ordering::SeqCst), 2);
            assert_eq!(presentation_pumps.get(), 2);
            runtime.windows.with_manager(|wm| {
                for name in CHALLENGE_BIO_LABEL_WINDOWS {
                    assert!(!window_hidden(wm, name), "{name}");
                }
            });
            let (
                prelude_state,
                current_frame,
                high_spec_active,
                postlude_audio_played,
                ambient_handle,
            ) = with_challenge_load_screen_state(
                |state| {
                    (
                        state.prelude_state,
                        state.current_frame,
                        state.high_spec_prelude_active,
                        state.postlude_audio_played,
                        state.ambient_loop_handle.clone(),
                    )
                },
                runtime,
            );
            assert_eq!(prelude_state, LoadScreenPreludeState::Complete);
            assert_eq!(current_frame, FRAME_RIGHT_VOICE);
            assert!(!high_spec_active);
            assert!(postlude_audio_played);
            assert_eq!(
                ambient_handle,
                accepted_handle(&audio_trace, "LoadScreenAmbient")
            );

            clear_load_screen_presentation_pump();
            clear_challenge_movie_play_hook();
            clear_challenge_movie_advance_hook();
            reset_challenge_load_screen_audio_state(runtime);
            Language::clear_localized_strings();
        })();
    });
}

#[test]
fn challenge_postlude_audio_fires_once_and_selects_opponent_taunt() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        cache_challenge_test_personas(runtime);

        assert_eq!(
            challenge_taunt_sound(
                &with_challenge_load_screen_state(|state| state.opponent.clone().unwrap(), runtime),
                0
            ),
            Some("OpponentTaunt1")
        );
        assert_eq!(
            challenge_taunt_sound(
                &with_challenge_load_screen_state(|state| state.opponent.clone().unwrap(), runtime),
                4
            ),
            Some("OpponentTaunt2")
        );
        let sparse_taunts = ChallengePersonaText {
            taunt_sounds: [
                String::new(),
                "SparseOpponentTaunt2".to_string(),
                String::new(),
            ],
            ..ChallengePersonaText::default()
        };
        assert_eq!(challenge_taunt_sound(&sparse_taunts, 0), Some(""));
        assert_eq!(
            challenge_taunt_sound(&sparse_taunts, 1),
            Some("SparseOpponentTaunt2")
        );
        assert_eq!(challenge_taunt_sound(&sparse_taunts, 2), Some(""));

        finish_challenge_load_screen_audio_postlude(runtime);
        let first = with_challenge_load_screen_state(
            |state| {
                (
                    state.postlude_audio_played,
                    state.high_spec_prelude_active,
                    state.ambient_loop_handle.clone(),
                )
            },
            runtime,
        );
        assert!(first.0);
        assert!(!first.1);

        finish_challenge_load_screen_audio_postlude(runtime);
        let second =
            with_challenge_load_screen_state(|state| state.ambient_loop_handle.clone(), runtime);
        assert_eq!(second, first.2);
    });
}

#[test]
fn challenge_frame_activation_matches_cpp_teletype_gates() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        cache_challenge_test_personas(runtime);
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm, &mut runtime.effects);

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TITLES_START, runtime);
        for name in CHALLENGE_BIO_LABEL_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(!window.borrow().is_hidden(), "{name}");
        }
        for name in CHALLENGE_BIO_ENTRY_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(window.borrow().is_hidden(), "{name}");
        }

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TELETYPE_START, runtime);
        for name in CHALLENGE_BIO_ENTRY_WINDOWS {
            let window = wm.find_window_by_name(name).expect(name);
            let window = window.borrow();
            assert!(!window.is_hidden(), "{name}");
            assert_eq!(window.get_text(), "");
        }

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TELETYPE_START + 1, runtime);
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioNameEntryLeft")
                .expect("left name")
                .borrow()
                .get_text(),
            ""
        );

        activate_challenge_pieces_frame_windows(&mut wm, FRAME_TELETYPE_START + 2, runtime);
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioNameEntryLeft")
                .expect("left name")
                .borrow()
                .get_text(),
            "G"
        );
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioBirthplaceEntryRight")
                .expect("right rank")
                .borrow()
                .get_text(),
            "P"
        );
    });
}

#[test]
fn challenge_min_spec_activation_matches_cpp_final_reveal() {
    let mut fixture = LoadScreenFixture::new();
    fixture.with_runtime(|runtime, presentation_pump, audio_trace| {
        let operation_windows = runtime.windows.clone();

        cache_challenge_test_personas(runtime);
        let mut wm = WindowManager::new();
        challenge_test_windows(&mut wm, &mut runtime.effects);

        activate_challenge_pieces_min_spec_windows(&mut wm, runtime);

        for name in CHALLENGE_BIO_LABEL_WINDOWS
            .iter()
            .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
            .copied()
            .chain(
                [
                    "ChallengeLoadScreen.wnd:PortraitLeft",
                    "ChallengeLoadScreen.wnd:PortraitRight",
                    "ChallengeLoadScreen.wnd:CircleAlphaOuter",
                    "ChallengeLoadScreen.wnd:CircleAlphaInner",
                    "ChallengeLoadScreen.wnd:VersusBackdrop",
                    "ChallengeLoadScreen.wnd:OverlayVs",
                ]
                .into_iter(),
            )
        {
            let window = wm.find_window_by_name(name).expect(name);
            assert!(!window.borrow().is_hidden(), "{name}");
        }

        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BigNameEntryLeft")
                .expect("left big name")
                .borrow()
                .get_text(),
            "General Player"
        );
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioBirthplaceEntryRight")
                .expect("right rank")
                .borrow()
                .get_text(),
            "Prince"
        );
        assert_eq!(
            wm.find_window_by_name("ChallengeLoadScreen.wnd:BioStrategyEntryRight")
                .expect("right strategy")
                .borrow()
                .get_text(),
            "Ambush"
        );

        let left_portrait = wm
            .find_window_by_name("ChallengeLoadScreen.wnd:PortraitLeft")
            .expect("left portrait");
        let left_portrait = left_portrait.borrow();
        assert_eq!(
            left_portrait
                .get_enabled_draw_data(0)
                .and_then(|draw| draw.image)
                .map(|image| image.name),
            Some("PlayerPortrait".to_string())
        );
    });
}
