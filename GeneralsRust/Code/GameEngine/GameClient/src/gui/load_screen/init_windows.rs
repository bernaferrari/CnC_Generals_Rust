// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

fn initialize_progress_windows(
    wm: &mut WindowManager,
    descriptor: LoadScreenDescriptor,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    if descriptor.slot_count == 0 {
        set_progress_window(wm, descriptor.primary_progress, 0.0, effects);
        if descriptor.kind == LoadScreenKind::ShellGame {
            hide_window(wm, descriptor.primary_progress, true);
        }
        if descriptor.kind != LoadScreenKind::ShellGame {
            hide_window(wm, descriptor.primary_progress, false);
        }
        return;
    }

    for slot in 0..descriptor.slot_count {
        let name = format!("{}{}", descriptor.progress_prefix, slot);
        set_progress_window(wm, &name, 0.0, effects);
    }
}

fn initialize_kind_windows(
    wm: &mut WindowManager,
    kind: LoadScreenKind,
    context: &LoadScreenInitContext,
    runtime: &mut LoadScreenOperation<'_>,
) -> bool {
    match kind {
        LoadScreenKind::ShellGame => {
            prepare_shell_game_windows(wm, context.shell_game_did_mem_pass, runtime.first_load)
        }
        LoadScreenKind::SinglePlayer => {
            initialize_single_player_windows(wm, context.shell_game_did_mem_pass, runtime);
            false
        }
        LoadScreenKind::Challenge => {
            initialize_challenge_windows(wm, context.shell_game_did_mem_pass, runtime);
            false
        }
        LoadScreenKind::Multiplayer => {
            initialize_multiplayer_windows(wm, "MultiplayerLoadScreen.wnd", context, runtime);
            false
        }
        LoadScreenKind::GameSpy => {
            initialize_gamespy_windows(wm, context, runtime);
            false
        }
        LoadScreenKind::MapTransfer => {
            initialize_map_transfer_windows(wm, context, runtime);
            false
        }
    }
}

fn initialize_shell_game_windows(
    wm: &mut WindowManager,
    did_mem_pass: bool,
    first_load: &mut bool,
    runtime: &mut LoadScreenOperation<'_>,
) {
    if prepare_shell_game_windows(wm, did_mem_pass, first_load) {
        run_shell_game_legal_hold(wm, runtime);
        *first_load = false;
        hide_window(wm, "ShellGameLoadScreen.wnd:ProgressLoad", false);
    }
}

fn prepare_shell_game_windows(
    wm: &mut WindowManager,
    did_mem_pass: bool,
    first_load: &bool,
) -> bool {
    if !*first_load || !did_mem_pass {
        return false;
    }
    set_window_image(
        wm,
        "ShellGameLoadScreen.wnd:ParentShellGameLoadScreen",
        0,
        "TitleScreen",
        true,
    );
    hide_window(wm, "ShellGameLoadScreen.wnd:StaticTextLegal", false);
    hide_window(wm, "ShellGameLoadScreen.wnd:ProgressLoad", true);
    true
}

fn with_multiplayer_load_screen_state<R>(
    f: impl FnOnce(&mut MultiplayerLoadScreenState) -> R,
    runtime: &mut LoadScreenOperation<'_>,
) -> R {
    f(&mut runtime.state.multiplayer)
}

fn reset_multiplayer_load_screen_state(runtime: &mut LoadScreenOperation<'_>) {
    with_multiplayer_load_screen_state(
        |state| *state = MultiplayerLoadScreenState::default(),
        runtime,
    );
}

fn with_map_transfer_load_screen_state<R>(
    f: impl FnOnce(&mut MapTransferLoadScreenState) -> R,
    runtime: &mut LoadScreenOperation<'_>,
) -> R {
    f(&mut runtime.state.map_transfer)
}

fn reset_map_transfer_load_screen_state(runtime: &mut LoadScreenOperation<'_>) {
    with_map_transfer_load_screen_state(
        |state| *state = MapTransferLoadScreenState::default(),
        runtime,
    );
}

fn reset_single_player_load_screen_audio_state(runtime: &mut LoadScreenOperation<'_>) {
    let ambient_handle = with_single_player_load_screen_state(
        |state| {
            let handle = state.ambient_loop_handle.take();
            state.prelude_state = LoadScreenPreludeState::NotRequired;
            state.prelude_deadline = None;
            state.prelude_duration = Duration::ZERO;
            state.movie_prelude_active = false;
            state.movie_label.clear();
            state.briefing_voice_played = false;
            state.briefing_voice_handle = 0;
            state.ambient_loop_handle = None;
            handle
        },
        runtime,
    );
    if let Some(handle) = ambient_handle {
        let _ = runtime.effects.stop_menu_audio(handle);
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

fn run_shell_game_legal_hold(wm: &mut WindowManager, runtime: &mut LoadScreenOperation<'_>) {
    let duration = shell_game_legal_hold_duration();
    let begin = Instant::now();
    loop {
        wm.update(&mut runtime.effects);
        pump_load_screen_presentation(None, runtime)
            .expect("no presentation recipient in standalone legal hold");
        gamelogic::system::game_logic::set_fp_mode();
        if begin.elapsed() >= duration {
            break;
        }
        std::thread::sleep(SHELL_GAME_LEGAL_UPDATE_INTERVAL);
    }
}

fn run_shell_game_legal_hold_with_pump(
    mut presentation_pump: Option<&mut LoadScreenPresentationPump<'_>>,
    runtime: &mut LoadScreenOperation<'_>,
) -> Result<(), String> {
    let windows = runtime.windows;
    let duration = shell_game_legal_hold_duration();
    let begin = Instant::now();
    loop {
        windows.with_manager(|wm| wm.update(&mut runtime.effects));
        let result = pump_load_screen_presentation(presentation_pump.as_deref_mut(), runtime);
        gamelogic::system::game_logic::set_fp_mode();
        result?;
        if begin.elapsed() >= duration {
            break;
        }
        std::thread::sleep(SHELL_GAME_LEGAL_UPDATE_INTERVAL);
    }
    Ok(())
}
