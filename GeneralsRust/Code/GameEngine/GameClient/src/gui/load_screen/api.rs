// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

pub fn select_load_screen(request: LoadScreenRequest) -> Option<LoadScreenKind> {
    match request.mode {
        LoadScreenGameMode::Shell | LoadScreenGameMode::Replay => Some(LoadScreenKind::ShellGame),
        LoadScreenGameMode::SinglePlayer => {
            if request.loading_save_game || !request.has_current_campaign {
                Some(LoadScreenKind::ShellGame)
            } else if request.current_campaign_is_challenge {
                Some(LoadScreenKind::Challenge)
            } else {
                Some(LoadScreenKind::SinglePlayer)
            }
        }
        LoadScreenGameMode::Skirmish
        | LoadScreenGameMode::Lan
        | LoadScreenGameMode::Multiplayer => Some(LoadScreenKind::Multiplayer),
        LoadScreenGameMode::Internet => Some(LoadScreenKind::GameSpy),
        LoadScreenGameMode::None => None,
    }
}

pub fn descriptor_for_kind(kind: LoadScreenKind) -> LoadScreenDescriptor {
    match kind {
        LoadScreenKind::ShellGame => LoadScreenDescriptor {
            kind,
            layout: "Menus/ShellGameLoadScreen.wnd",
            root: "ShellGameLoadScreen.wnd:ParentShellGameLoadScreen",
            primary_progress: "ShellGameLoadScreen.wnd:ProgressLoad",
            progress_prefix: "ShellGameLoadScreen.wnd:ProgressLoad",
            slot_count: 0,
            uses_progress_fudge: false,
        },
        LoadScreenKind::SinglePlayer => LoadScreenDescriptor {
            kind,
            layout: "Menus/SinglePlayerLoadScreen.wnd",
            root: "SinglePlayerLoadScreen.wnd:ParentSinglePlayerLoadScreen",
            primary_progress: "SinglePlayerLoadScreen.wnd:ProgressLoad",
            progress_prefix: "SinglePlayerLoadScreen.wnd:ProgressLoad",
            slot_count: 0,
            uses_progress_fudge: true,
        },
        LoadScreenKind::Challenge => LoadScreenDescriptor {
            kind,
            layout: "Menus/ChallengeLoadScreen.wnd",
            root: "ChallengeLoadScreen.wnd:ParentChallengeLoadScreen",
            primary_progress: "ChallengeLoadScreen.wnd:ProgressLoad",
            progress_prefix: "ChallengeLoadScreen.wnd:ProgressLoad",
            slot_count: 0,
            uses_progress_fudge: true,
        },
        LoadScreenKind::Multiplayer => LoadScreenDescriptor {
            kind,
            layout: "Menus/MultiplayerLoadScreen.wnd",
            root: "MultiplayerLoadScreen.wnd:ParentMultiplayerLoadScreen",
            primary_progress: "MultiplayerLoadScreen.wnd:ProgressLoad0",
            progress_prefix: "MultiplayerLoadScreen.wnd:ProgressLoad",
            slot_count: MAX_LOAD_SCREEN_SLOTS,
            uses_progress_fudge: false,
        },
        LoadScreenKind::GameSpy => LoadScreenDescriptor {
            kind,
            layout: "Menus/GameSpyLoadScreen.wnd",
            root: "GameSpyLoadScreen.wnd:ParentMultiplayerLoadScreen",
            primary_progress: "GameSpyLoadScreen.wnd:ProgressLoad0",
            progress_prefix: "GameSpyLoadScreen.wnd:ProgressLoad",
            slot_count: MAX_LOAD_SCREEN_SLOTS,
            uses_progress_fudge: false,
        },
        LoadScreenKind::MapTransfer => LoadScreenDescriptor {
            kind,
            layout: "Menus/MapTransferScreen.wnd",
            root: "MapTransferScreen.wnd:ParentMapTransferScreen",
            primary_progress: "MapTransferScreen.wnd:ProgressLoad0",
            progress_prefix: "MapTransferScreen.wnd:ProgressLoad",
            slot_count: MAX_LOAD_SCREEN_SLOTS,
            uses_progress_fudge: false,
        },
    }
}

pub fn transformed_progress_percent(descriptor: LoadScreenDescriptor, raw_percent: f32) -> f32 {
    if descriptor.uses_progress_fudge {
        (raw_percent + FRAME_FUDGE_ADD) / FRAME_FUDGE_SCALE
    } else {
        raw_percent
    }
}

pub fn init_load_screen(kind: LoadScreenKind, context: &LoadScreenInitContext) -> bool {
    let descriptor = descriptor_for_kind(kind);
    with_window_manager(|wm| {
        if wm.create_layout_with_windows(descriptor.layout).is_err() {
            return false;
        }

        if let Some(root) = wm.find_window_by_name(descriptor.root) {
            let mut root = root.borrow_mut();
            let _ = root.hide(false);
            let _ = root.bring_to_front();
        }

        initialize_progress_windows(wm, descriptor);
        initialize_kind_windows(wm, descriptor.kind, context);
        true
    })
}

pub fn load_screen_init_context_from_game_info(
    game_info: &crate::game_network::GameInfo,
) -> LoadScreenInitContext {
    let slots: Vec<_> = (0..MAX_LOAD_SCREEN_SLOTS)
        .filter_map(|player_id| {
            let slot = game_info.get_slot(player_id)?;
            slot.is_occupied().then(|| LoadScreenSlotInitContext {
                player_id: player_id as i32,
                player_name: slot.get_name().to_string(),
                side_name: slot.get_apparent_player_template_display_name(),
                team_number: slot.get_team_number(),
                apparent_color: (slot.get_apparent_color() >= 0)
                    .then_some(slot.get_apparent_color()),
                apparent_text_color: multiplayer_apparent_text_color(slot.get_apparent_color()),
                is_ai: slot.is_ai(),
                has_map: slot.has_map(),
                visible: true,
            })
        })
        .collect();
    let start_positions = (0..MAX_LOAD_SCREEN_SLOTS)
        .map(|player_id| {
            let slot = game_info.get_slot(player_id)?;
            let start_pos = slot.get_apparent_start_pos();
            (start_pos >= 0 && slot.get_player_template() > crate::game_network::PLAYERTEMPLATE_MIN)
                .then_some(start_pos as usize)
        })
        .collect();

    let local_player_id = game_info.get_local_slot_num();
    let local_slot = if local_player_id >= 0 {
        slots.iter().find(|slot| slot.player_id == local_player_id)
    } else {
        slots.first()
    };

    if let Some(local_slot) = local_slot {
        let local_template = if local_slot.player_id >= 0 {
            let template_index =
                game_info
                    .get_slot(local_slot.player_id as usize)
                    .and_then(|slot| {
                        let player_template = slot.get_player_template();
                        (player_template >= 0).then_some(player_template as usize)
                    });
            let store = get_player_template_store();
            template_index
                .and_then(|index| store.get_nth_player_template(index).cloned())
                .or_else(|| store.find_template("FactionObserver").cloned())
        } else {
            None
        };
        let local_general_presentation =
            multiplayer_local_general_presentation(local_template.as_ref(), &local_slot.side_name);
        LoadScreenInitContext {
            local_player_name: local_slot.player_name.clone(),
            local_side_name: local_slot.side_name.clone(),
            local_template_name: local_general_presentation.template_name,
            local_general_name: local_general_presentation.name,
            local_general_features: local_general_presentation.features,
            local_general_portrait: local_general_presentation.portrait,
            local_load_screen_music: local_general_presentation.load_screen_music,
            local_team_number: local_slot.player_id,
            shell_game_did_mem_pass: game_engine::common::game_lod::did_mem_pass(),
            map_name: load_screen_map_name_from_game_info(game_info),
            start_positions,
            slots,
        }
    } else {
        LoadScreenInitContext::default()
    }
}

fn load_screen_map_name_from_game_info(
    game_info: &crate::game_network::GameInfo,
) -> Option<String> {
    let map_name = game_info.get_map();
    if map_name.is_empty() {
        return None;
    }

    if !game_info.is_game_in_progress() {
        let local_slot_num = game_info.get_local_slot_num();
        if local_slot_num < 0 {
            return None;
        }

        let Some(local_slot) = game_info.get_slot(local_slot_num as usize) else {
            return None;
        };
        if !local_slot.has_map() {
            return None;
        }
    }

    Some(map_name.to_string())
}

pub fn reset_load_screen(kind: LoadScreenKind) {
    let descriptor = descriptor_for_kind(kind);
    with_window_manager(|wm| {
        if let Some(root) = wm.find_window_by_name(descriptor.root) {
            let _ = wm.destroy_window(root);
            wm.flush_destroy_queue();
        }
    });
    if kind == LoadScreenKind::Challenge {
        reset_challenge_load_screen_audio_state();
    } else if kind == LoadScreenKind::SinglePlayer {
        reset_single_player_load_screen_audio_state();
    } else if kind == LoadScreenKind::MapTransfer {
        reset_map_transfer_load_screen_state();
    } else if descriptor.slot_count > 0 {
        reset_multiplayer_load_screen_state();
    }
}

pub fn update_load_screen(kind: LoadScreenKind, raw_percent: f32) {
    let descriptor = descriptor_for_kind(kind);
    let percent = transformed_progress_percent(descriptor, raw_percent);
    if kind == LoadScreenKind::MapTransfer {
        map_transfer_liteupdate();
    }
    clear_load_screen_cursor_tooltip();
    if kind == LoadScreenKind::MapTransfer {
        finish_load_screen_update();
        return;
    }
    if descriptor.slot_count > 0 {
        let local_player_id = with_multiplayer_load_screen_state(|state| state.local_player_id);
        if raw_percent <= 100.0 {
            report_multiplayer_load_progress(local_player_id, percent);
            let _ = process_load_screen_progress(kind, local_player_id, percent);
        }
        finish_load_screen_update();
        return;
    }
    with_window_manager(|wm| {
        set_progress_window(wm, descriptor.primary_progress, percent);
        if kind == LoadScreenKind::SinglePlayer {
            set_window_text(
                wm,
                "SinglePlayerLoadScreen.wnd:Percent",
                &format!("{}%", percent as i32),
            );
            let _ = update_single_player_load_screen_movie_prelude(wm);
        } else if kind == LoadScreenKind::Challenge {
            update_challenge_load_screen_prelude(wm);
            if raw_percent >= 100.0 {
                finish_challenge_load_screen_audio_postlude();
            }
        }
    });
    finish_load_screen_update();
}

fn clear_load_screen_cursor_tooltip() {
    with_mouse(|mouse| mouse.set_cursor_tooltip(String::new(), None, None, None));
}

fn finish_load_screen_update() {
    // C++ LoadScreen::update does this last: pump windows/display and restore FP mode.
    with_window_manager(|wm| wm.update());
    pump_load_screen_presentation();
    gamelogic::system::game_logic::set_fp_mode();

    #[cfg(test)]
    if let Some(hook) = load_screen_finish_update_hook() {
        hook();
    }
}

pub fn register_load_screen_presentation_pump(
    hook: impl Fn() + 'static,
) -> Option<LoadScreenPresentationPump> {
    LOAD_SCREEN_PRESENTATION_PUMP.with(|state| state.borrow_mut().replace(Rc::new(hook)))
}

pub fn clear_load_screen_presentation_pump() -> Option<LoadScreenPresentationPump> {
    LOAD_SCREEN_PRESENTATION_PUMP.with(|state| state.borrow_mut().take())
}

pub(crate) fn pump_load_screen_presentation() {
    let hook = LOAD_SCREEN_PRESENTATION_PUMP.with(|state| state.borrow().clone());
    if let Some(hook) = hook {
        hook();
    }
}

pub fn register_map_transfer_liteupdate_hook(
    hook: impl Fn() + Send + Sync + 'static,
) -> Option<MapTransferLiteupdateHook> {
    let hook = Arc::new(hook);
    let state = MAP_TRANSFER_LITEUPDATE_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.replace(hook)
}

pub fn clear_map_transfer_liteupdate_hook() -> Option<MapTransferLiteupdateHook> {
    let state = MAP_TRANSFER_LITEUPDATE_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.take()
}

pub fn register_multiplayer_load_progress_hook(
    hook: impl Fn(i32, i32) + Send + Sync + 'static,
) -> Option<MultiplayerLoadProgressHook> {
    let hook = Arc::new(hook);
    let state = MULTIPLAYER_LOAD_PROGRESS_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.replace(hook)
}

pub fn clear_multiplayer_load_progress_hook() -> Option<MultiplayerLoadProgressHook> {
    let state = MULTIPLAYER_LOAD_PROGRESS_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.take()
}

#[cfg(test)]
fn register_load_screen_finish_update_hook(
    hook: impl Fn() + Send + Sync + 'static,
) -> Option<LoadScreenFinishUpdateHook> {
    let state = LOAD_SCREEN_FINISH_UPDATE_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.replace(Arc::new(hook))
}

#[cfg(test)]
fn clear_load_screen_finish_update_hook() -> Option<LoadScreenFinishUpdateHook> {
    let state = LOAD_SCREEN_FINISH_UPDATE_HOOK.get_or_init(|| Mutex::new(None));
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.take()
}

#[cfg(test)]
fn load_screen_finish_update_hook() -> Option<LoadScreenFinishUpdateHook> {
    let state = LOAD_SCREEN_FINISH_UPDATE_HOOK.get_or_init(|| Mutex::new(None));
    let guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.clone()
}

fn map_transfer_liteupdate() {
    let hook = {
        let state = MAP_TRANSFER_LITEUPDATE_HOOK.get_or_init(|| Mutex::new(None));
        let guard = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.clone()
    };
    if let Some(hook) = hook {
        hook();
    }
}

fn report_multiplayer_load_progress(player_id: i32, percentage: f32) {
    if !(0.0..=100.0).contains(&percentage) {
        return;
    }
    let hook = {
        let state = MULTIPLAYER_LOAD_PROGRESS_HOOK.get_or_init(|| Mutex::new(None));
        let guard = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.clone()
    };
    if let Some(hook) = hook {
        hook(player_id, percentage as i32);
    }
}

pub fn process_load_screen_progress(kind: LoadScreenKind, player_id: i32, percentage: f32) -> bool {
    if kind == LoadScreenKind::MapTransfer {
        return false;
    }

    let descriptor = descriptor_for_kind(kind);
    if descriptor.slot_count == 0 || !(0.0..=100.0).contains(&percentage) {
        return false;
    }

    let compact_slot = with_multiplayer_load_screen_state(|state| {
        if player_id < 0 || player_id as usize >= MAX_LOAD_SCREEN_SLOTS {
            None
        } else {
            let compact_slot = state.player_lookup[player_id as usize];
            (compact_slot >= 0).then_some(compact_slot as usize)
        }
    });
    let Some(compact_slot) = compact_slot else {
        return false;
    };

    with_window_manager(|wm| {
        set_progress_window(
            wm,
            &format!("{}{}", descriptor.progress_prefix, compact_slot),
            percentage,
        );
    });
    true
}
