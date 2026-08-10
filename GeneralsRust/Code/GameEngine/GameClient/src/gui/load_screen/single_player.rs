// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

fn initialize_single_player_windows(wm: &mut WindowManager) {
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

    let movie_label = {
        let campaign_manager = get_campaign_manager();
        let mut movie_label = None;
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
        }
        movie_label
    };

    let Some(movie_label) = movie_label else {
        return;
    };

    if play_single_player_movie(
        wm,
        "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
        &movie_label,
    ) {
        apply_single_player_campaign_images(wm);
        with_single_player_load_screen_state(|state| {
            state.movie_prelude_active = true;
            state.movie_label = movie_label;
        });
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
        state.movie_prelude_active = false;
        state.movie_label.clear();
        if !state.briefing_voice_played {
            state.briefing_voice_handle = briefing_handle;
            state.briefing_voice_played = true;
        }
        state.ambient_loop_handle = ambient_handle;
    });
}

fn update_single_player_load_screen_movie_prelude(wm: &mut WindowManager) -> bool {
    let movie_label = with_single_player_load_screen_state(|state| {
        state
            .movie_prelude_active
            .then(|| state.movie_label.clone())
            .filter(|label| !label.is_empty())
    });
    let Some(movie_label) = movie_label else {
        return false;
    };

    with_window_video_manager(|manager| manager.update());
    if is_single_player_movie_playing(&movie_label) {
        return true;
    }

    hide_window(wm, "SinglePlayerLoadScreen.wnd:Percent", true);
    finish_single_player_load_screen_audio_prelude();
    true
}
