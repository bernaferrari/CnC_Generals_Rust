// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

fn initialize_challenge_windows(wm: &mut WindowManager, did_mem_pass: bool) {
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

    let Some((player, opponent)) = current_challenge_persona_text() else {
        return;
    };
    let movie_label = current_challenge_movie_label();
    let voice_length = current_challenge_voice_length();
    with_challenge_load_screen_state(|state| {
        state.player = Some(player.clone());
        state.opponent = Some(opponent.clone());
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

    // C++ calls VideoPlayer::open even for an empty/missing movie label, then
    // returns if the stream/buffer cannot be constructed. Do not preserve the
    // former synthetic min-spec fallback: a missing authored background is a
    // failed prelude, not a license to reveal portraits or start taunt/ambient.
    let movie_label = movie_label.unwrap_or_default();
    if !play_challenge_background_movie(
        wm,
        "ChallengeLoadScreen.wnd:ParentChallengeLoadScreen",
        &movie_label,
    ) {
        // Matches C++'s video-buffer/open failure return: do not create a
        // synthetic min-spec reveal or ambient/taunt audio.
        with_challenge_load_screen_state(|state| {
            state.prelude_state = LoadScreenPreludeState::Failed;
        });
        return;
    }

    let delay = campaign_voice_delay_duration(voice_length);
    let deadline = Instant::now().checked_add(delay);
    with_challenge_load_screen_state(|state| {
        state.background_movie_label = movie_label;
        state.high_spec_prelude_active = did_mem_pass;
        state.prelude_state = if did_mem_pass {
            LoadScreenPreludeState::Movie
        } else {
            LoadScreenPreludeState::VoiceDelay
        };
        state.prelude_duration = delay;
        state.prelude_deadline = deadline;
    });

    if !did_mem_pass {
        // C++ seeks/renders the final background frame, then reveals the
        // min-spec portrait composition while its authored VoiceLength delay
        // continues to pump WindowManager/display.
        activate_challenge_pieces_min_spec_windows(wm);
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

fn challenge_prelude_outcome(state: LoadScreenPreludeState) -> LoadScreenPreludeOutcome {
    match state {
        LoadScreenPreludeState::NotRequired => LoadScreenPreludeOutcome::NotRequired,
        LoadScreenPreludeState::Complete => LoadScreenPreludeOutcome::Complete,
        LoadScreenPreludeState::Failed => LoadScreenPreludeOutcome::Failed,
        LoadScreenPreludeState::Skipped => LoadScreenPreludeOutcome::Skipped,
        LoadScreenPreludeState::Movie | LoadScreenPreludeState::VoiceDelay => {
            LoadScreenPreludeOutcome::Complete
        }
    }
}

fn complete_challenge_load_screen_prelude(outcome: LoadScreenPreludeOutcome) {
    let should_play_audio = with_challenge_load_screen_state(|state| {
        if state.prelude_state == LoadScreenPreludeState::Failed {
            return false;
        }
        state.prelude_state = match outcome {
            LoadScreenPreludeOutcome::Complete => LoadScreenPreludeState::Complete,
            LoadScreenPreludeOutcome::Skipped => LoadScreenPreludeState::Skipped,
            LoadScreenPreludeOutcome::Failed => LoadScreenPreludeState::Failed,
            LoadScreenPreludeOutcome::NotRequired => LoadScreenPreludeState::NotRequired,
        };
        state.high_spec_prelude_active = false;
        state.background_movie_label.clear();
        state.prelude_deadline = None;
        matches!(
            outcome,
            LoadScreenPreludeOutcome::Complete | LoadScreenPreludeOutcome::Skipped
        )
    });
    if should_play_audio {
        finish_challenge_load_screen_audio_postlude();
    }
}

fn activate_challenge_pieces_through_windows(
    wm: &mut WindowManager,
    previous_frame: i32,
    frame: i32,
) {
    if frame <= previous_frame {
        return;
    }
    for frame in (previous_frame + 1)..=frame {
        activate_challenge_pieces_frame_windows(wm, frame);
    }
}

fn advance_challenge_load_screen_prelude(wm: &mut WindowManager) -> LoadScreenPreludeStep {
    let (prelude_state, movie_label, current_frame, deadline, duration) =
        with_challenge_load_screen_state(|state| {
            (
                state.prelude_state,
                state.background_movie_label.clone(),
                state.current_frame,
                state.prelude_deadline,
                state.prelude_duration,
            )
        });

    match prelude_state {
        LoadScreenPreludeState::NotRequired
        | LoadScreenPreludeState::Complete
        | LoadScreenPreludeState::Failed
        | LoadScreenPreludeState::Skipped => {
            LoadScreenPreludeStep::Finished(challenge_prelude_outcome(prelude_state))
        }
        LoadScreenPreludeState::Movie => {
            let advance = advance_challenge_background_movie(&movie_label);
            let Some(advance) = advance else {
                // A successfully-opened one-shot disappearing before an
                // observed frame is still a completed/skipped presentation,
                // not the open failure handled during initialization.
                complete_challenge_load_screen_prelude(LoadScreenPreludeOutcome::Complete);
                return LoadScreenPreludeStep::Finished(LoadScreenPreludeOutcome::Complete);
            };

            let frame = advance.frame_index.max(current_frame);
            if advance.frame_index > current_frame {
                activate_challenge_pieces_through_windows(wm, current_frame, frame);
                with_challenge_load_screen_state(|state| state.current_frame = frame);
            }
            if advance.completed {
                complete_challenge_load_screen_prelude(LoadScreenPreludeOutcome::Complete);
                LoadScreenPreludeStep::Finished(LoadScreenPreludeOutcome::Complete)
            } else {
                LoadScreenPreludeStep::Pending(LOAD_SCREEN_PRELUDE_MOVIE_IDLE_INTERVAL)
            }
        }
        LoadScreenPreludeState::VoiceDelay => {
            let now = Instant::now();
            let completed = deadline.map(|deadline| now >= deadline).unwrap_or(true);
            if !duration.is_zero() {
                let start = deadline
                    .and_then(|deadline| deadline.checked_sub(duration))
                    .unwrap_or(now);
                let elapsed = now.saturating_duration_since(start);
                let progress =
                    (30.0 * elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 30.0);
                set_progress_window(wm, "ChallengeLoadScreen.wnd:ProgressLoad", progress);
            }
            if completed {
                complete_challenge_load_screen_prelude(LoadScreenPreludeOutcome::Complete);
                LoadScreenPreludeStep::Finished(LoadScreenPreludeOutcome::Complete)
            } else {
                LoadScreenPreludeStep::Pending(LOAD_SCREEN_PRELUDE_MIN_SPEC_UPDATE_INTERVAL)
            }
        }
    }
}

fn skip_challenge_load_screen_prelude() -> LoadScreenPreludeOutcome {
    let state = with_challenge_load_screen_state(|state| state.prelude_state);
    if matches!(
        state,
        LoadScreenPreludeState::Movie | LoadScreenPreludeState::VoiceDelay
    ) {
        complete_challenge_load_screen_prelude(LoadScreenPreludeOutcome::Skipped);
        LoadScreenPreludeOutcome::Skipped
    } else {
        challenge_prelude_outcome(state)
    }
}

fn finish_challenge_load_screen_audio_postlude() {
    let postlude = with_challenge_load_screen_state(|state| {
        // C++ returns from init when the background stream/buffer cannot be
        // created. A later generic 100% map-progress callback must not turn
        // that failed screen into a taunt/ambient audio path.
        if state.postlude_audio_played || state.prelude_state == LoadScreenPreludeState::Failed {
            return None;
        }
        let taunt = {
            let opponent = state.opponent.as_ref()?;
            challenge_taunt_sound(opponent, challenge_taunt_seed()).map(str::to_string)
        };
        state.postlude_audio_played = true;
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

fn pump_challenge_load_screen_audio() {
    if let Some(audio) = TheAudio::get() {
        audio.update();
    }
}

fn reset_challenge_load_screen_audio_state() {
    let ambient_handle = with_challenge_load_screen_state(|state| {
        let handle = state.ambient_loop_handle;
        state.prelude_state = LoadScreenPreludeState::NotRequired;
        state.prelude_deadline = None;
        state.prelude_duration = Duration::ZERO;
        state.background_movie_label.clear();
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
            let _ = play_challenge_movie(
                wm,
                "ChallengeLoadScreen.wnd:PortraitMovieLeft",
                &player.portrait_movie_left,
            );
            let _ = play_challenge_movie(
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
            let _ = play_challenge_movie(wm, "ChallengeLoadScreen.wnd:OverlayVs", "VSSmall");
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
    let _ = play_challenge_movie(wm, "ChallengeLoadScreen.wnd:OverlayVs", "VSSmall");
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
