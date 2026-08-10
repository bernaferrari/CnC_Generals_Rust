// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

fn initialize_map_transfer_windows(wm: &mut WindowManager, context: &LoadScreenInitContext) {
    set_window_text(wm, "MapTransferScreen.wnd:StaticTextCurrentFile", "");
    set_window_text(wm, "MapTransferScreen.wnd:StaticTextTimeout", "");

    let slots = map_transfer_slot_contexts(context);
    with_map_transfer_load_screen_state(|state| {
        *state = MapTransferLoadScreenState::default();
        for (compact_slot, slot_context) in slots.iter().enumerate() {
            if slot_context.player_id >= 0
                && (slot_context.player_id as usize) < MAX_LOAD_SCREEN_SLOTS
            {
                state.player_lookup[slot_context.player_id as usize] = compact_slot as i32;
            }
        }
    });

    for slot in 0..MAX_LOAD_SCREEN_SLOTS {
        set_progress_window(
            wm,
            &format!("MapTransferScreen.wnd:ProgressLoad{slot}"),
            0.0,
        );
        set_window_text(
            wm,
            &format!("MapTransferScreen.wnd:StaticTextProgress{slot}"),
            "",
        );

        if let Some(slot_context) = slots.get(slot) {
            set_window_text(
                wm,
                &format!("MapTransferScreen.wnd:StaticTextPlayer{slot}"),
                &slot_context.player_name,
            );
            if let Some(color) = slot_context.apparent_text_color {
                set_progress_window_fill_color(
                    wm,
                    &format!("MapTransferScreen.wnd:ProgressLoad{slot}"),
                    color,
                );
                set_window_enabled_text_color(
                    wm,
                    &format!("MapTransferScreen.wnd:StaticTextPlayer{slot}"),
                    color,
                );
                set_window_enabled_text_color(
                    wm,
                    &format!("MapTransferScreen.wnd:StaticTextProgress{slot}"),
                    color,
                );
            }

            hide_window(
                wm,
                &format!("MapTransferScreen.wnd:ProgressLoad{slot}"),
                map_transfer_progress_hidden(slot_context),
            );
            hide_window(
                wm,
                &format!("MapTransferScreen.wnd:StaticTextPlayer{slot}"),
                false,
            );
            hide_window(
                wm,
                &format!("MapTransferScreen.wnd:StaticTextProgress{slot}"),
                false,
            );
            continue;
        }

        for suffix in ["ProgressLoad", "StaticTextPlayer", "StaticTextProgress"] {
            hide_window(wm, &format!("MapTransferScreen.wnd:{suffix}{slot}"), true);
        }
    }
}

fn map_transfer_slot_contexts(context: &LoadScreenInitContext) -> Vec<LoadScreenSlotInitContext> {
    context
        .slots
        .iter()
        .filter(|slot| slot.visible && !slot.is_ai)
        .take(MAX_LOAD_SCREEN_SLOTS)
        .cloned()
        .collect()
}

fn map_transfer_progress_hidden(slot: &LoadScreenSlotInitContext) -> bool {
    slot.player_id == 0 || slot.has_map
}

pub fn process_map_transfer_progress(player_id: i32, percentage: i32, state_label: &str) -> bool {
    if !(0..=100).contains(&percentage)
        || player_id < 0
        || player_id as usize >= MAX_LOAD_SCREEN_SLOTS
    {
        return false;
    }

    let update = with_map_transfer_load_screen_state(|state| {
        let translated_slot = state.player_lookup[player_id as usize];
        if translated_slot < 0 || state.old_progress[player_id as usize] == percentage {
            return None;
        }
        state.old_progress[player_id as usize] = percentage;
        Some(translated_slot as usize)
    });
    let Some(translated_slot) = update else {
        return false;
    };

    with_window_manager(|wm| {
        set_progress_window(
            wm,
            &format!("MapTransferScreen.wnd:ProgressLoad{translated_slot}"),
            percentage as f32,
        );
        set_window_text(
            wm,
            &format!("MapTransferScreen.wnd:StaticTextProgress{translated_slot}"),
            &GameText::fetch(state_label),
        );
    });
    true
}

pub fn process_map_transfer_timeout(seconds_left: i32) -> bool {
    let changed = with_map_transfer_load_screen_state(|state| {
        if state.old_timeout == seconds_left {
            return false;
        }
        state.old_timeout = seconds_left;
        true
    });
    if !changed {
        return false;
    }

    let text = format_map_transfer_timeout(seconds_left);
    with_window_manager(|wm| {
        set_window_text(wm, "MapTransferScreen.wnd:StaticTextTimeout", &text);
    });
    true
}

pub fn set_map_transfer_current_filename(filename: &str) {
    let text = format_map_transfer_current_file(filename);
    with_window_manager(|wm| {
        set_window_text(wm, "MapTransferScreen.wnd:StaticTextCurrentFile", &text);
    });
}

fn format_map_transfer_timeout(seconds_left: i32) -> String {
    replace_first_percent_d(
        &replace_first_percent_d(&GameText::fetch("MapTransfer:Timeout"), seconds_left / 60),
        seconds_left % 60,
    )
}

fn format_map_transfer_current_file(filename: &str) -> String {
    GameText::fetch("MapTransfer:CurrentFile").replace("%s", &map_transfer_leaf_name(filename))
}

fn map_transfer_leaf_name(filename: &str) -> String {
    let trimmed = filename.trim_end_matches(['\\', '/']);
    let slash = trimmed.rfind('\\').max(trimmed.rfind('/'));
    slash
        .map(|index| trimmed[index + 1..].to_string())
        .unwrap_or_else(|| trimmed.to_string())
}

fn replace_first_percent_d(template: &str, value: i32) -> String {
    template.replacen("%d", &value.to_string(), 1)
}

fn multiplayer_slot_contexts(context: &LoadScreenInitContext) -> Vec<LoadScreenSlotInitContext> {
    let slots: Vec<_> = context
        .slots
        .iter()
        .filter(|slot| slot.visible)
        .take(MAX_LOAD_SCREEN_SLOTS)
        .cloned()
        .collect();

    if slots.is_empty() {
        vec![LoadScreenSlotInitContext {
            player_id: context.local_team_number,
            player_name: context.local_player_name.clone(),
            side_name: context.local_side_name.clone(),
            team_number: context.local_team_number,
            apparent_color: None,
            apparent_text_color: None,
            is_ai: false,
            has_map: true,
            visible: true,
        }]
    } else {
        slots
    }
}

fn multiplayer_team_text(slot: &LoadScreenSlotInitContext) -> String {
    format!("Team:{}", slot.team_number + 1)
}

fn multiplayer_progress_bar_image(slot: &LoadScreenSlotInitContext) -> Option<String> {
    let image_name = slot
        .apparent_color
        .filter(|color| *color >= 0)
        .map(|color| format!("LoadingBar_ProgressCenter{color}"))?;
    if mapped_image_exists(&image_name) || !mapped_image_exists("LoadingBar_Progress") {
        Some(image_name)
    } else {
        Some("LoadingBar_Progress".to_string())
    }
}

fn multiplayer_apparent_text_color(apparent_color: i32) -> Option<u32> {
    with_multiplayer_settings(|settings| settings.get_color_value(apparent_color))
}

fn mapped_image_exists(image_name: &str) -> bool {
    get_mapped_image_collection()
        .try_read()
        .and_then(|collection| collection.find_image_by_name(image_name).map(|_| ()))
        .is_some()
}

fn gamespy_stats_suffixes() -> [&'static str; 4] {
    [
        "StaticTextTotalDisconnects",
        "StaticTextWinLoss",
        "WinRank",
        "WinOfficer",
    ]
}

fn multiplayer_local_general_faction_logo(side_name: &str, prefix: &str) -> Option<&'static str> {
    let gamespy = prefix.eq_ignore_ascii_case("GameSpyLoadScreen.wnd");
    let side = side_name.trim();
    if side.eq_ignore_ascii_case("USA")
        || side.eq_ignore_ascii_case("America")
        || side.eq_ignore_ascii_case("FactionAmerica")
    {
        Some(if gamespy {
            "SAFactionLogo144_US"
        } else {
            "SAFactionLogoLg_US"
        })
    } else if side.eq_ignore_ascii_case("GLA") || side.eq_ignore_ascii_case("FactionGLA") {
        Some(if gamespy {
            "SUFactionLogo144_GLA"
        } else {
            "SUFactionLogoLg_GLA"
        })
    } else if side.eq_ignore_ascii_case("China") || side.eq_ignore_ascii_case("FactionChina") {
        Some(if gamespy {
            "SNFactionLogo144_China"
        } else {
            "SNFactionLogoLg_China"
        })
    } else {
        None
    }
}
