// Split from `gui/load_screen.rs` dump. Included by `load_screen/mod.rs`.

fn initialize_multiplayer_windows(
    wm: &mut WindowManager,
    prefix: &str,
    context: &LoadScreenInitContext,
) {
    let has_team_windows = load_screen_has_team_windows(prefix);
    if let Some(portrait_image) = context
        .local_general_portrait
        .as_deref()
        .or_else(|| multiplayer_local_general_faction_logo(&context.local_template_name, prefix))
        .or_else(|| multiplayer_local_general_faction_logo(&context.local_side_name, prefix))
    {
        set_window_image(
            wm,
            &format!("{prefix}:LocalGeneralPortrait"),
            0,
            portrait_image,
            false,
        );
    }
    set_window_text(
        wm,
        &format!("{prefix}:LocalGeneralFeatures"),
        multiplayer_local_general_text_fallback(
            &context.local_general_features,
            &context.local_side_name,
        ),
    );
    set_window_text(
        wm,
        &format!("{prefix}:LocalGeneralName"),
        multiplayer_local_general_text_fallback(
            &context.local_general_name,
            &context.local_side_name,
        ),
    );
    if prefix == "MultiplayerLoadScreen.wnd" {
        play_multiplayer_load_screen_music(&context.local_load_screen_music);
    }
    initialize_multiplayer_map_preview(
        wm,
        prefix,
        context.map_name.as_deref(),
        &context.start_positions,
    );

    let slots = multiplayer_slot_contexts(context);
    with_multiplayer_load_screen_state(|state| {
        *state = MultiplayerLoadScreenState::default();
        state.local_player_id = context.local_team_number;
        for (compact_slot, slot_context) in slots.iter().enumerate() {
            if slot_context.player_id >= 0
                && (slot_context.player_id as usize) < MAX_LOAD_SCREEN_SLOTS
            {
                state.player_lookup[slot_context.player_id as usize] = compact_slot as i32;
            }
        }
    });
    for slot in 0..MAX_LOAD_SCREEN_SLOTS {
        set_progress_window(wm, &format!("{prefix}:ProgressLoad{slot}"), 0.0);
        if let Some(slot_context) = slots.get(slot) {
            if let Some(progress_image) = multiplayer_progress_bar_image(slot_context) {
                set_window_image(
                    wm,
                    &format!("{prefix}:ProgressLoad{slot}"),
                    6,
                    &progress_image,
                    false,
                );
            }
            set_window_text(
                wm,
                &format!("{prefix}:StaticTextPlayer{slot}"),
                &slot_context.player_name,
            );
            set_window_text(
                wm,
                &format!("{prefix}:StaticTextSide{slot}"),
                &slot_context.side_name,
            );
            if has_team_windows {
                set_window_text(
                    wm,
                    &format!("{prefix}:StaticTextTeam{slot}"),
                    &GameText::fetch(&multiplayer_team_text(slot_context)),
                );
            }
            if let Some(text_color) = slot_context.apparent_text_color {
                let suffixes = if has_team_windows {
                    &["StaticTextPlayer", "StaticTextSide", "StaticTextTeam"][..]
                } else {
                    &["StaticTextPlayer", "StaticTextSide"][..]
                };
                for suffix in suffixes {
                    set_window_enabled_text_color(
                        wm,
                        &format!("{prefix}:{suffix}{slot}"),
                        text_color,
                    );
                }
            }

            let suffixes = if has_team_windows {
                &["StaticTextPlayer", "StaticTextSide", "StaticTextTeam"][..]
            } else {
                &["StaticTextPlayer", "StaticTextSide"][..]
            };
            for suffix in suffixes {
                hide_window(wm, &format!("{prefix}:{suffix}{slot}"), false);
            }
            hide_window(
                wm,
                &format!("{prefix}:ProgressLoad{slot}"),
                slot_context.is_ai,
            );
            continue;
        }

        let suffixes = if has_team_windows {
            &[
                "ProgressLoad",
                "StaticTextPlayer",
                "StaticTextSide",
                "StaticTextTeam",
            ][..]
        } else {
            &["ProgressLoad", "StaticTextPlayer", "StaticTextSide"][..]
        };
        for suffix in suffixes {
            hide_window(wm, &format!("{prefix}:{suffix}{slot}"), true);
        }
    }
}

fn load_screen_has_team_windows(prefix: &str) -> bool {
    !prefix.eq_ignore_ascii_case("GameSpyLoadScreen.wnd")
}

fn initialize_multiplayer_map_preview(
    wm: &mut WindowManager,
    prefix: &str,
    map_name: Option<&str>,
    start_positions: &[Option<usize>],
) {
    let preview_window_name = format!("{prefix}:WinMapPreview");
    let Some(preview) = wm.find_window_by_name(&preview_window_name) else {
        return;
    };

    let preview_image = map_name.and_then(get_map_preview_image);
    let metadata = map_name.and_then(multiplayer_map_metadata);
    let Some(preview_image) = preview_image else {
        preview.borrow_mut().clear_status(WindowStatus::IMAGE);
        update_multiplayer_start_position_buttons(wm, prefix, metadata.as_ref(), start_positions);
        return;
    };

    set_window_image(wm, &preview_window_name, 0, &preview_image, true);
    update_multiplayer_start_position_buttons(wm, prefix, metadata.as_ref(), start_positions);
}

fn multiplayer_map_metadata(map_name: &str) -> Option<MapMetaData> {
    let cache = get_map_cache_manager();
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.update_cache();
    cache.find_map(map_name)
}

fn map_start_waypoint_name(index: usize) -> String {
    format!("Player_{}_Start", index + 1)
}

fn update_multiplayer_start_position_buttons(
    wm: &mut WindowManager,
    prefix: &str,
    metadata: Option<&MapMetaData>,
    start_positions: &[Option<usize>],
) {
    let preview_window_name = format!("{prefix}:WinMapPreview");
    let Some(preview) = wm.find_window_by_name(&preview_window_name) else {
        return;
    };

    let Some(metadata) = metadata.filter(|metadata| metadata.is_multiplayer) else {
        for slot in 0..MAX_LOAD_SCREEN_SLOTS {
            hide_window(wm, &format!("{prefix}:ButtonMapStartPosition{slot}"), true);
        }
        return;
    };

    position_multiplayer_start_position_buttons(wm, prefix, &preview, metadata);
    apply_multiplayer_start_position_labels(wm, prefix, metadata, start_positions);
}

fn position_multiplayer_start_position_buttons(
    wm: &mut WindowManager,
    prefix: &str,
    preview: &Rc<RefCell<GameWindow>>,
    metadata: &MapMetaData,
) {
    let preview = preview.borrow();
    let (map_x, map_y) = preview.get_screen_position();
    let (map_w, map_h) = preview.get_size();
    let extent = metadata.extent;
    let (ul, lr) = find_draw_positions(map_x, map_y, map_w, map_h, extent);
    let extent_width = (extent.hi.x - extent.lo.x).max(1.0);
    let extent_height = (extent.hi.y - extent.lo.y).max(1.0);
    drop(preview);

    let mut placed_buttons: Vec<(i32, i32, i32, i32)> = Vec::new();
    for slot in 0..MAX_LOAD_SCREEN_SLOTS {
        let button_name = format!("{prefix}:ButtonMapStartPosition{slot}");
        let Some(button) = wm.find_window_by_name(&button_name) else {
            continue;
        };
        let waypoint = if (slot as i32) < metadata.num_players {
            metadata.get_waypoint(&map_start_waypoint_name(slot))
        } else {
            None
        };
        let mut button = button.borrow_mut();
        if let Some(coord) = waypoint {
            let ratio_x = (coord.x - extent.lo.x) / extent_width;
            let ratio_y = (extent.hi.y - coord.y) / extent_height;
            let draw_x = ul.x as f32 + (lr.x - ul.x) as f32 * ratio_x;
            let draw_y = ul.y as f32 + (lr.y - ul.y) as f32 * ratio_y;
            let (btn_w, btn_h) = button.get_size();
            let mut new_x = draw_x.round() as i32 - btn_w / 2 - map_x;
            let mut new_y = draw_y.round() as i32 - btn_h / 2 - map_y;
            let gadget_size = btn_w.max(btn_h);
            for (x, y, w, h) in &placed_buttons {
                if new_x >= *x && new_x < *x + *w && new_y >= *y && new_y < *y + *h {
                    if new_y + gadget_size + 1 < map_h {
                        new_y += gadget_size + 1;
                    } else {
                        new_x += gadget_size + 1;
                    }
                }
            }
            let _ = button.set_position(new_x, new_y);
            let _ = button.hide(false);
            let _ = button.enable(true);
            placed_buttons.push((new_x, new_y, btn_w, btn_h));
        } else {
            let _ = button.hide(true);
        }
    }
}

fn apply_multiplayer_start_position_labels(
    wm: &mut WindowManager,
    prefix: &str,
    metadata: &MapMetaData,
    start_positions: &[Option<usize>],
) {
    for slot in 0..MAX_LOAD_SCREEN_SLOTS {
        set_window_text(wm, &format!("{prefix}:ButtonMapStartPosition{slot}"), "");
    }

    let max_players = metadata.num_players.max(0) as usize;
    for (player_index, start_pos) in start_positions.iter().enumerate() {
        let Some(start_pos) = start_pos else {
            continue;
        };
        if *start_pos < max_players {
            set_window_text(
                wm,
                &format!("{prefix}:ButtonMapStartPosition{start_pos}"),
                &GameText::fetch(&format!("NUMBER:{}", player_index + 1)),
            );
        }
    }
}

fn initialize_gamespy_windows(wm: &mut WindowManager, context: &LoadScreenInitContext) {
    initialize_multiplayer_windows(wm, "GameSpyLoadScreen.wnd", context);

    let slots = multiplayer_slot_contexts(context);
    for slot in 0..MAX_LOAD_SCREEN_SLOTS {
        let slot_context = slots.get(slot);
        hide_window(
            wm,
            &format!("GameSpyLoadScreen.wnd:WinPlayer{slot}"),
            slot_context.is_none(),
        );
        let hide_stats = slot_context.map(|slot| slot.is_ai).unwrap_or(true);
        for suffix in gamespy_stats_suffixes() {
            hide_window(
                wm,
                &format!("GameSpyLoadScreen.wnd:{suffix}{slot}"),
                hide_stats,
            );
        }
    }
}
