// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

fn initialize_progress_windows(wm: &mut WindowManager, descriptor: LoadScreenDescriptor) {
    if descriptor.slot_count == 0 {
        set_progress_window(wm, descriptor.primary_progress, 0.0);
        if descriptor.kind == LoadScreenKind::ShellGame {
            hide_window(wm, descriptor.primary_progress, true);
        }
        hide_window(wm, descriptor.primary_progress, false);
        return;
    }

    for slot in 0..descriptor.slot_count {
        let name = format!("{}{}", descriptor.progress_prefix, slot);
        set_progress_window(wm, &name, 0.0);
    }
}

fn initialize_kind_windows(
    wm: &mut WindowManager,
    kind: LoadScreenKind,
    context: &LoadScreenInitContext,
) {
    match kind {
        LoadScreenKind::ShellGame => {
            initialize_shell_game_windows(wm, context.shell_game_did_mem_pass)
        }
        LoadScreenKind::SinglePlayer => initialize_single_player_windows(wm),
        LoadScreenKind::Challenge => initialize_challenge_windows(wm),
        LoadScreenKind::Multiplayer => {
            initialize_multiplayer_windows(wm, "MultiplayerLoadScreen.wnd", context)
        }
        LoadScreenKind::GameSpy => initialize_gamespy_windows(wm, context),
        LoadScreenKind::MapTransfer => initialize_map_transfer_windows(wm, context),
    }
}

fn initialize_shell_game_windows(wm: &mut WindowManager, did_mem_pass: bool) {
    let is_first_load = with_shell_game_first_load(|first_load| *first_load);

    if is_first_load && did_mem_pass {
        set_window_image(
            wm,
            "ShellGameLoadScreen.wnd:ParentShellGameLoadScreen",
            0,
            "TitleScreen",
            true,
        );
        hide_window(wm, "ShellGameLoadScreen.wnd:StaticTextLegal", false);
        hide_window(wm, "ShellGameLoadScreen.wnd:ProgressLoad", true);
        run_shell_game_legal_hold(wm);
        with_shell_game_first_load(|first_load| *first_load = false);
        hide_window(wm, "ShellGameLoadScreen.wnd:ProgressLoad", false);
    }
}

fn with_shell_game_first_load<R>(f: impl FnOnce(&mut bool) -> R) -> R {
    let state = SHELL_GAME_FIRST_LOAD.get_or_init(|| Mutex::new(true));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn with_multiplayer_load_screen_state<R>(
    f: impl FnOnce(&mut MultiplayerLoadScreenState) -> R,
) -> R {
    let state = MULTIPLAYER_LOAD_SCREEN_STATE
        .get_or_init(|| Mutex::new(MultiplayerLoadScreenState::default()));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn reset_multiplayer_load_screen_state() {
    with_multiplayer_load_screen_state(|state| *state = MultiplayerLoadScreenState::default());
}

fn with_map_transfer_load_screen_state<R>(
    f: impl FnOnce(&mut MapTransferLoadScreenState) -> R,
) -> R {
    let state = MAP_TRANSFER_LOAD_SCREEN_STATE
        .get_or_init(|| Mutex::new(MapTransferLoadScreenState::default()));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn reset_map_transfer_load_screen_state() {
    with_map_transfer_load_screen_state(|state| *state = MapTransferLoadScreenState::default());
}

fn reset_single_player_load_screen_audio_state() {
    let ambient_handle = with_single_player_load_screen_state(|state| {
        let handle = state.ambient_loop_handle;
        state.movie_prelude_active = false;
        state.movie_label.clear();
        state.briefing_voice_played = false;
        state.briefing_voice_handle = 0;
        state.ambient_loop_handle = 0;
        handle
    });
    if ambient_handle != 0 {
        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(ambient_handle);
        }
    }
}

#[cfg(not(test))]
fn shell_game_legal_hold_duration() -> Duration {
    Duration::from_millis(3000)
}

#[cfg(test)]
fn shell_game_legal_hold_duration() -> Duration {
    Duration::ZERO
}

fn run_shell_game_legal_hold(wm: &mut WindowManager) {
    let hold_duration = shell_game_legal_hold_duration();
    if hold_duration.is_zero() {
        wm.update();
        pump_load_screen_presentation();
        return;
    }

    let show_start = Instant::now();
    while show_start.elapsed() < hold_duration {
        wm.update();
        pump_load_screen_presentation();
        std::thread::sleep(SHELL_GAME_LEGAL_UPDATE_INTERVAL);
    }
}
