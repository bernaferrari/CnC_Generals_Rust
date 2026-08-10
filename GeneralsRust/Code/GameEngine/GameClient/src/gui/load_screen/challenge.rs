// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

fn initialize_challenge_windows(wm: &mut WindowManager) {
    with_challenge_load_screen_state(|state| *state = ChallengeLoadScreenState::default());
    with_window_video_manager(|manager| manager.init());

    for name in [
        "ChallengeLoadScreen.wnd:PortraitLeft",
        "ChallengeLoadScreen.wnd:PortraitRight",
        "ChallengeLoadScreen.wnd:CircleAlphaOuter",
        "ChallengeLoadScreen.wnd:CircleAlphaInner",
        "ChallengeLoadScreen.wnd:VersusBackdrop",
        "ChallengeLoadScreen.wnd:OverlayVs",
        "ChallengeLoadScreen.wnd:PortraitMovieLeft",
        "ChallengeLoadScreen.wnd:PortraitMovieRight",
        "ChallengeLoadScreen.wnd:BioNameLeft",
        "ChallengeLoadScreen.wnd:BioBirthplaceLeft",
        "ChallengeLoadScreen.wnd:BioStrategyLeft",
        "ChallengeLoadScreen.wnd:BigNameEntryLeft",
        "ChallengeLoadScreen.wnd:BioNameEntryLeft",
        "ChallengeLoadScreen.wnd:BioBirthplaceEntryLeft",
        "ChallengeLoadScreen.wnd:BioStrategyEntryLeft",
        "ChallengeLoadScreen.wnd:BioNameRight",
        "ChallengeLoadScreen.wnd:BioBirthplaceRight",
        "ChallengeLoadScreen.wnd:BioStrategyRight",
        "ChallengeLoadScreen.wnd:BigNameEntryRight",
        "ChallengeLoadScreen.wnd:BioNameEntryRight",
        "ChallengeLoadScreen.wnd:BioBirthplaceEntryRight",
        "ChallengeLoadScreen.wnd:BioStrategyEntryRight",
    ] {
        hide_window(wm, name, true);
    }

    if let Some((player, opponent)) = current_challenge_persona_text() {
        let movie_label = current_challenge_movie_label();
        with_challenge_load_screen_state(|state| {
            state.player = Some(player.clone());
            state.opponent = Some(opponent.clone());
            state.high_spec_prelude_active = movie_label.is_some();
            state.current_frame = 0;
            state.postlude_audio_played = false;
            state.ambient_loop_handle = 0;
        });
        if let Some(image) = player.portrait_large.as_deref() {
            set_window_image(wm, "ChallengeLoadScreen.wnd:PortraitLeft", 0, image, true);
        }
        if let Some(image) = opponent.portrait_large.as_deref() {
            set_window_image(wm, "ChallengeLoadScreen.wnd:PortraitRight", 0, image, true);
        }
        if let Some(movie_label) = movie_label {
            play_challenge_movie(
                wm,
                "ChallengeLoadScreen.wnd:ParentChallengeLoadScreen",
                &movie_label,
            );
        } else {
            activate_challenge_pieces_min_spec_windows(wm);
            finish_challenge_load_screen_audio_postlude();
        }
    }
}

fn with_challenge_load_screen_state<R>(f: impl FnOnce(&mut ChallengeLoadScreenState) -> R) -> R {
    let state =
        CHALLENGE_LOAD_SCREEN_STATE.get_or_init(|| Mutex::new(ChallengeLoadScreenState::default()));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

pub fn activate_challenge_load_screen_frame(frame: i32) {
    with_window_manager(|wm| activate_challenge_pieces_frame_windows(wm, frame));
}

pub fn activate_challenge_load_screen_min_spec() {
    with_window_manager(activate_challenge_pieces_min_spec_windows);
}

fn update_challenge_load_screen_prelude(wm: &mut WindowManager) {
    let frame = with_challenge_load_screen_state(|state| {
        if !state.high_spec_prelude_active {
            return None;
        }
        state.current_frame += 1;
        Some(state.current_frame)
    });

    if let Some(frame) = frame {
        activate_challenge_pieces_frame_windows(wm, frame);
        with_window_video_manager(|manager| manager.update());
    }
}

fn finish_challenge_load_screen_audio_postlude() {
    let postlude = with_challenge_load_screen_state(|state| {
        if state.postlude_audio_played {
            return None;
        }
        let taunt = {
            let opponent = state.opponent.as_ref()?;
            challenge_taunt_sound(opponent, challenge_taunt_seed()).map(str::to_string)
        };
        state.postlude_audio_played = true;
        state.high_spec_prelude_active = false;
        Some(taunt)
    });

    let Some(taunt) = postlude else {
        return;
    };
    if let Some(taunt) = taunt {
        play_audio_event(&taunt);
    }
    let ambient_handle = add_audio_event("LoadScreenAmbient");
    with_challenge_load_screen_state(|state| {
        state.ambient_loop_handle = ambient_handle;
    });
}

fn reset_challenge_load_screen_audio_state() {
    let ambient_handle = with_challenge_load_screen_state(|state| {
        let handle = state.ambient_loop_handle;
        state.high_spec_prelude_active = false;
        state.current_frame = 0;
        state.postlude_audio_played = false;
        state.ambient_loop_handle = 0;
        handle
    });
    if ambient_handle != 0 {
        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(ambient_handle);
        }
    }
}

fn challenge_taunt_seed() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() as usize)
        .unwrap_or(0)
}

fn challenge_taunt_sound(persona: &ChallengePersonaText, seed: usize) -> Option<&str> {
    Some(persona.taunt_sounds[seed % persona.taunt_sounds.len()].as_str())
}

fn activate_challenge_pieces_frame_windows(wm: &mut WindowManager, frame: i32) {
    let personas = with_challenge_load_screen_state(|state| {
        let player = state.player.clone()?;
        let opponent = state.opponent.clone()?;
        Some((player, opponent))
    });
    let Some((player, opponent)) = personas else {
        return;
    };

    match frame {
        FRAME_TITLES_START => {
            for name in CHALLENGE_BIO_LABEL_WINDOWS {
                hide_window(wm, name, false);
            }
        }
        FRAME_TELETYPE_START => {
            with_challenge_load_screen_state(ChallengeLoadScreenState::reset_teletype_positions);
            for name in CHALLENGE_BIO_ENTRY_WINDOWS {
                hide_window(wm, name, false);
                set_window_text(wm, name, "");
            }
        }
        FRAME_PORTRAITS_START => {
            play_challenge_movie(
                wm,
                "ChallengeLoadScreen.wnd:PortraitMovieLeft",
                &player.portrait_movie_left,
            );
            play_challenge_movie(
                wm,
                "ChallengeLoadScreen.wnd:PortraitMovieRight",
                &opponent.portrait_movie_right,
            );
            hide_window(wm, "ChallengeLoadScreen.wnd:PortraitMovieLeft", false);
            hide_window(wm, "ChallengeLoadScreen.wnd:PortraitMovieRight", false);
            play_audio_event(&player.name_sound);
        }
        FRAME_OUTER_CIRCLE_ALPHA_SHOW => {
            hide_window(wm, "ChallengeLoadScreen.wnd:CircleAlphaOuter", false);
        }
        FRAME_INNER_CIRCLE_ALPHA_SHOW => {
            hide_window(wm, "ChallengeLoadScreen.wnd:CircleAlphaInner", false);
        }
        FRAME_INNER_BACKDROP_ALPHA_SHOW => {
            hide_window(wm, "ChallengeLoadScreen.wnd:VersusBackdrop", false);
        }
        FRAME_VS_ANIM_START => {
            hide_window(wm, "ChallengeLoadScreen.wnd:VersusBackdrop", false);
            hide_window(wm, "ChallengeLoadScreen.wnd:OverlayVs", false);
            play_challenge_movie(wm, "ChallengeLoadScreen.wnd:OverlayVs", "VSSmall");
            play_audio_event("Taunts_GCAnnouncer12");
        }
        FRAME_RIGHT_VOICE => {
            play_audio_event(&opponent.name_sound);
        }
        _ => {}
    }

    if frame > FRAME_TELETYPE_START && frame % TELETYPE_UPDATE_FREQ == 0 {
        with_challenge_load_screen_state(|state| {
            state.text_pos_name_left = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioNameEntryLeft",
                &player.name,
                state.text_pos_name_left,
            );
            state.text_pos_big_name_left = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BigNameEntryLeft",
                &player.big_name,
                state.text_pos_big_name_left,
            );
            state.text_pos_birthplace_left = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioBirthplaceEntryLeft",
                &player.rank,
                state.text_pos_birthplace_left,
            );
            state.text_pos_strategy_left = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioStrategyEntryLeft",
                &player.strategy,
                state.text_pos_strategy_left,
            );
            state.text_pos_name_right = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioNameEntryRight",
                &opponent.name,
                state.text_pos_name_right,
            );
            state.text_pos_big_name_right = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BigNameEntryRight",
                &opponent.big_name,
                state.text_pos_big_name_right,
            );
            state.text_pos_birthplace_right = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioBirthplaceEntryRight",
                &opponent.rank,
                state.text_pos_birthplace_right,
            );
            state.text_pos_strategy_right = update_teletype_text(
                wm,
                "ChallengeLoadScreen.wnd:BioStrategyEntryRight",
                &opponent.strategy,
                state.text_pos_strategy_right,
            );
        });
    }
}

fn activate_challenge_pieces_min_spec_windows(wm: &mut WindowManager) {
    let personas = with_challenge_load_screen_state(|state| {
        let player = state.player.clone()?;
        let opponent = state.opponent.clone()?;
        Some((player, opponent))
    });
    let Some((player, opponent)) = personas else {
        return;
    };

    for name in CHALLENGE_BIO_LABEL_WINDOWS
        .iter()
        .chain(CHALLENGE_BIO_ENTRY_WINDOWS.iter())
    {
        hide_window(wm, name, false);
    }

    set_challenge_bio_entry_text(wm, "Left", &player);
    set_challenge_bio_entry_text(wm, "Right", &opponent);

    if let Some(image) = player.portrait_large.as_deref() {
        set_window_image(wm, "ChallengeLoadScreen.wnd:PortraitLeft", 0, image, true);
    }
    if let Some(image) = opponent.portrait_large.as_deref() {
        set_window_image(wm, "ChallengeLoadScreen.wnd:PortraitRight", 0, image, true);
    }
    hide_window(wm, "ChallengeLoadScreen.wnd:PortraitLeft", false);
    hide_window(wm, "ChallengeLoadScreen.wnd:PortraitRight", false);
    hide_window(wm, "ChallengeLoadScreen.wnd:CircleAlphaOuter", false);
    hide_window(wm, "ChallengeLoadScreen.wnd:CircleAlphaInner", false);
    hide_window(wm, "ChallengeLoadScreen.wnd:VersusBackdrop", false);
    hide_window(wm, "ChallengeLoadScreen.wnd:OverlayVs", false);
    play_challenge_movie(wm, "ChallengeLoadScreen.wnd:OverlayVs", "VSSmall");
}

fn set_challenge_bio_entry_text(
    wm: &mut WindowManager,
    side: &str,
    persona: &ChallengePersonaText,
) {
    set_window_text(
        wm,
        &format!("ChallengeLoadScreen.wnd:BigNameEntry{side}"),
        &persona.big_name,
    );
    set_window_text(
        wm,
        &format!("ChallengeLoadScreen.wnd:BioNameEntry{side}"),
        &persona.name,
    );
    set_window_text(
        wm,
        &format!("ChallengeLoadScreen.wnd:BioBirthplaceEntry{side}"),
        &persona.rank,
    );
    set_window_text(
        wm,
        &format!("ChallengeLoadScreen.wnd:BioStrategyEntry{side}"),
        &persona.strategy,
    );
}

fn update_teletype_text(
    wm: &mut WindowManager,
    window_name: &str,
    full_text: &str,
    current_text_pos: usize,
) -> usize {
    let Some(window) = wm.find_window_by_name(window_name) else {
        return current_text_pos;
    };
    let Some(next_char) = full_text.chars().nth(current_text_pos) else {
        return current_text_pos;
    };
    let mut window = window.borrow_mut();
    let mut current = window.get_text().to_string();
    current.push(next_char);
    let _ = window.set_text(&current);
    current_text_pos + 1
}

fn play_challenge_movie(wm: &mut WindowManager, window_name: &str, movie_name: &str) {
    if movie_name.is_empty() {
        return;
    }
    if let Some(window) = wm.find_window_by_name(window_name) {
        with_window_video_manager(|manager| {
            manager.play_movie(
                window,
                movie_name.to_string(),
                WindowVideoPlayType::ShowLastFrame,
            )
        });
    }
}
