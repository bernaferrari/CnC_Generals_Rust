// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

fn initialize_single_player_windows(wm: &mut WindowManager, did_mem_pass: bool) {
    with_single_player_load_screen_state(|state| *state = SinglePlayerLoadScreenState::default());
    with_window_video_manager(|manager| manager.init());

    set_window_text(wm, "SinglePlayerLoadScreen.wnd:Percent", "0%");
    hide_window(wm, "SinglePlayerLoadScreen.wnd:Percent", true);
    hide_window(wm, "SinglePlayerLoadScreen.wnd:ObjectivesWin", true);

    for line in 0..MAX_OBJECTIVE_LINES {
        set_window_text(
            wm,
            &format!("SinglePlayerLoadScreen.wnd:StaticTextLine{line}"),
            "",
        );
    }

    for cameo in 0..4 {
        hide_window(
            wm,
            &format!("SinglePlayerLoadScreen.wnd:StaticTextCameoText{cameo}"),
            true,
        );
    }

    let (movie_label, voice_length) = {
        let campaign_manager = get_campaign_manager();
        let mut movie_label = None;
        let mut voice_length = 0;
        if let Some(mission) = campaign_manager.get_current_mission() {
            let text = single_player_mission_text(mission);
            with_single_player_load_screen_state(|state| {
                state.mission_text = text.clone();
                state.current_objective_line = 0;
                state.current_objective_width_offset = 0;
                state.current_objective_line_character = 0;
                state.finished_objective_text = false;
            });
            for unit in 0..MAX_DISPLAYED_UNITS {
                set_window_text(
                    wm,
                    &format!("SinglePlayerLoadScreen.wnd:StaticTextCameoText{unit}"),
                    &text.unit_descriptions[unit],
                );
            }
            set_window_text(
                wm,
                "SinglePlayerLoadScreen.wnd:StaticTextCameoText3",
                &text.location,
            );
            movie_label = (!mission.movie_label.trim().is_empty())
                .then(|| mission.movie_label.trim().to_string());
            voice_length = mission.voice_length;
        }
        (movie_label, voice_length)
    };

    // C++ returns before the authored image/audio setup when its movie stream
    // cannot be opened.  Keep an absent mission/movie equally inert instead of
    // manufacturing a fallback prelude.
    let Some(movie_label) = movie_label else {
        return;
    };

    if !play_single_player_movie(
        wm,
        "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
        &movie_label,
    ) {
        with_single_player_load_screen_state(|state| {
            state.prelude_state = LoadScreenPreludeState::Failed;
        });
        return;
    }

    apply_single_player_campaign_images(wm);
    let delay = campaign_voice_delay_duration(voice_length);
    let deadline = Instant::now().checked_add(delay);
    with_single_player_load_screen_state(|state| {
        state.movie_label = movie_label;
        state.movie_prelude_active = did_mem_pass;
        state.prelude_state = if did_mem_pass {
            LoadScreenPreludeState::Movie
        } else {
            LoadScreenPreludeState::VoiceDelay
        };
        state.prelude_duration = delay;
        state.prelude_deadline = deadline;
    });
}

fn campaign_voice_delay_duration(voice_length: i32) -> Duration {
    let duration = Duration::from_secs(voice_length.max(0) as u64);
    // Tests drive the transition through deterministic movie hooks.  A
    // zero-length min-spec gate keeps those tests fast while preserving the
    // production path's authored VoiceLength delay.
    #[cfg(test)]
    {
        let _ = duration;
        Duration::ZERO
    }
    #[cfg(not(test))]
    {
        duration
    }
}

fn apply_single_player_campaign_images(wm: &mut WindowManager) {
    let campaign_manager = get_campaign_manager();
    let Some(campaign) = campaign_manager.get_current_campaign() else {
        return;
    };
    let Some((background, progress)) = single_player_campaign_images(&campaign.name) else {
        return;
    };

    set_window_image(
        wm,
        "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
        0,
        background,
        true,
    );
    set_window_image(
        wm,
        "SinglePlayerLoadScreen.wnd:ProgressLoad",
        6,
        progress,
        false,
    );
}

fn with_single_player_load_screen_state<R>(
    f: impl FnOnce(&mut SinglePlayerLoadScreenState) -> R,
) -> R {
    let state = SINGLE_PLAYER_LOAD_SCREEN_STATE
        .get_or_init(|| Mutex::new(SinglePlayerLoadScreenState::default()));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn finish_single_player_load_screen_audio_prelude() {
    let should_play_briefing =
        with_single_player_load_screen_state(|state| !state.briefing_voice_played);
    let briefing_voice = if should_play_briefing {
        let campaign_manager = get_campaign_manager();
        campaign_manager
            .get_current_mission()
            .map(|mission| mission.briefing_voice.sound_file.clone())
    } else {
        None
    };
    let briefing_handle = briefing_voice
        .as_deref()
        .filter(|event| !event.is_empty())
        .map(add_audio_event)
        .unwrap_or(0);
    let ambient_handle = add_audio_event("LoadScreenAmbient");

    with_single_player_load_screen_state(|state| {
        if !state.briefing_voice_played {
            state.briefing_voice_handle = briefing_handle;
            state.briefing_voice_played = true;
        }
        state.ambient_loop_handle = ambient_handle;
    });
}

fn single_player_prelude_outcome(state: LoadScreenPreludeState) -> LoadScreenPreludeOutcome {
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

fn complete_single_player_load_screen_prelude(
    wm: &mut WindowManager,
    outcome: LoadScreenPreludeOutcome,
) {
    let should_play_audio = with_single_player_load_screen_state(|state| {
        if state.prelude_state == LoadScreenPreludeState::Failed {
            return false;
        }
        state.prelude_state = match outcome {
            LoadScreenPreludeOutcome::Complete => LoadScreenPreludeState::Complete,
            LoadScreenPreludeOutcome::Skipped => LoadScreenPreludeState::Skipped,
            LoadScreenPreludeOutcome::Failed => LoadScreenPreludeState::Failed,
            LoadScreenPreludeOutcome::NotRequired => LoadScreenPreludeState::NotRequired,
        };
        state.movie_prelude_active = false;
        state.movie_label.clear();
        state.prelude_deadline = None;
        matches!(
            outcome,
            LoadScreenPreludeOutcome::Complete | LoadScreenPreludeOutcome::Skipped
        )
    });

    hide_window(wm, "SinglePlayerLoadScreen.wnd:Percent", true);
    if should_play_audio {
        finish_single_player_load_screen_audio_prelude();
    }
}

fn advance_single_player_load_screen_prelude(wm: &mut WindowManager) -> LoadScreenPreludeStep {
    let (prelude_state, movie_label, deadline, duration) =
        with_single_player_load_screen_state(|state| {
            (
                state.prelude_state,
                state.movie_label.clone(),
                state.prelude_deadline,
                state.prelude_duration,
            )
        });

    match prelude_state {
        LoadScreenPreludeState::NotRequired
        | LoadScreenPreludeState::Complete
        | LoadScreenPreludeState::Failed
        | LoadScreenPreludeState::Skipped => {
            LoadScreenPreludeStep::Finished(single_player_prelude_outcome(prelude_state))
        }
        LoadScreenPreludeState::Movie => {
            with_window_video_manager(|manager| manager.update());
            if is_single_player_movie_playing(&movie_label) {
                LoadScreenPreludeStep::Pending(LOAD_SCREEN_PRELUDE_MOVIE_IDLE_INTERVAL)
            } else {
                complete_single_player_load_screen_prelude(wm, LoadScreenPreludeOutcome::Complete);
                LoadScreenPreludeStep::Finished(LoadScreenPreludeOutcome::Complete)
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
                set_progress_window(wm, "SinglePlayerLoadScreen.wnd:ProgressLoad", progress);
            }
            if completed {
                complete_single_player_load_screen_prelude(wm, LoadScreenPreludeOutcome::Complete);
                LoadScreenPreludeStep::Finished(LoadScreenPreludeOutcome::Complete)
            } else {
                LoadScreenPreludeStep::Pending(LOAD_SCREEN_PRELUDE_MIN_SPEC_UPDATE_INTERVAL)
            }
        }
    }
}

fn skip_single_player_load_screen_prelude(wm: &mut WindowManager) -> LoadScreenPreludeOutcome {
    let state = with_single_player_load_screen_state(|state| state.prelude_state);
    if matches!(
        state,
        LoadScreenPreludeState::Movie | LoadScreenPreludeState::VoiceDelay
    ) {
        complete_single_player_load_screen_prelude(wm, LoadScreenPreludeOutcome::Skipped);
        LoadScreenPreludeOutcome::Skipped
    } else {
        single_player_prelude_outcome(state)
    }
}
