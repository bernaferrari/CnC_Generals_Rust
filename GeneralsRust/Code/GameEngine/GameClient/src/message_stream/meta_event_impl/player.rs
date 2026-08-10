// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

fn kill_local_player_selection() {
    // Wave 976: host empty dual-world still routes kills through TheGameLogic IDs
    // (selection manager + TheGameLogic::find_object_by_id), not OBJECT_REGISTRY walks.
    let selected_ids = local_selection_object_ids();

    for object_id in selected_ids {
        if let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(mut object) = object_arc.write() {
                object.kill(None, None);
            }
        }
    }
}

fn kill_all_enemy_objects_for_local_player() {
    let Some(local_team) = ThePlayerList().read().ok().and_then(|list| {
        list.get_local_player().and_then(|player| {
            player
                .read()
                .ok()
                .and_then(|guard| guard.get_default_team())
        })
    }) else {
        return;
    };
    let Ok(local_team_guard) = local_team.read() else {
        return;
    };

    // Wave 979: host empty dual-world → kill non-local catalog IDs via TheGameLogic.
    if dual_world_registry_unavailable() {
        use crate::presentation_translator_residual::{
            translator_entry_is_local, with_translator_catalog,
        };
        let mut enemy_ids: Vec<u32> = Vec::new();
        with_translator_catalog(|catalog| {
            for entry in catalog {
                if !translator_entry_is_local(entry) {
                    enemy_ids.push(entry.object_id);
                }
            }
        });
        for object_id in enemy_ids {
            if let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(mut object) = object_arc.write() {
                    object.kill(None, None);
                }
            }
        }
        return;
    }
    for object in OBJECT_REGISTRY.get_all_objects() {
        let Ok(mut object_guard) = object.write() else {
            continue;
        };
        let is_enemy = object_guard
            .get_controlling_player()
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|guard| guard.is_enemy_with_team(&local_team_guard))
            })
            .unwrap_or(false);
        if is_enemy {
            object_guard.kill(None, None);
        }
    }
}

fn first_selected_object_id_for_local_player() -> Option<u32> {
    local_selection_object_ids().into_iter().next()
}

fn adjust_local_selection_veterancy(delta: i32) {
    // Wave 976: host empty dual-world still routes veterancy through TheGameLogic IDs.
    for object_id in local_selection_object_ids() {
        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            continue;
        };
        let Ok(mut object) = object_arc.write() else {
            continue;
        };
        let Some(tracker_arc) = object.get_experience_tracker() else {
            continue;
        };
        let Ok(mut tracker) = tracker_arc.lock() else {
            continue;
        };
        if !tracker.is_trainable() {
            continue;
        }

        let old_level = tracker.get_veterancy_level();
        let new_level = old_level.saturating_add_levels(delta);
        if tracker.set_veterancy_level(new_level).is_some() {
            drop(tracker);
            object.on_veterancy_level_changed(old_level, new_level, true);
        }
    }
}

fn clear_local_player_selection() {
    let local_player_id = get_local_player_id();
    if let Ok(mut manager) = get_selection_manager().write() {
        if manager.get_player_selection_ref(local_player_id).is_none() {
            manager.initialize_player(local_player_id);
        }
        if let Some(selection) = manager.get_player_selection(local_player_id) {
            selection.clear_selection();
        }
    }
}

fn local_player_side_name() -> Option<String> {
    let list = ThePlayerList().read().ok()?;
    let index = list.get_local_player_index();
    if index == PLAYER_INDEX_INVALID || index < 0 {
        return None;
    }
    let player = list.get_player(index)?;
    let guard = player.read().ok()?;
    Some(guard.get_side().to_string())
}

fn local_player_index_u32() -> Option<u32> {
    let list = ThePlayerList().read().ok()?;
    let index = list.get_local_player_index();
    if index == PLAYER_INDEX_INVALID || index < 0 {
        return None;
    }
    Some(index as u32)
}

fn adjust_texture_reduction_factor(delta: i32) {
    let Some(global_data) = get_global_data() else {
        return;
    };
    let mut global = global_data.write();
    global.texture_reduction_factor = (global.texture_reduction_factor + delta).clamp(0, 4);
}

fn reveal_local_player_map_permanently() {
    let Some(player_id) = local_player_index_u32() else {
        return;
    };
    if let Ok(mut shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() {
        let _ = shroud.reveal_map_for_player_permanently(player_id);
    }
}

fn shroud_local_player_map() {
    let Some(player_id) = local_player_index_u32() else {
        return;
    };
    if let Ok(mut shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() {
        let _ = shroud.undo_reveal_map_for_player_permanently(player_id);
        let _ = shroud.shroud_map_for_player(player_id);
    }
}

fn apply_local_player_switch_side_effects(initialize_shortcut_bar: bool) {
    clear_local_player_selection();
    if let Some(side) = local_player_side_name() {
        if initialize_shortcut_bar {
            TheControlBar::init_special_power_shortcut_bar_for_player(&side);
        }
        TheControlBar::set_control_bar_scheme_by_player(&side);
    }
}

fn set_local_player_index_with_refresh(index: i32, initialize_shortcut_bar: bool) {
    {
        let Ok(mut list) = ThePlayerList().write() else {
            return;
        };
        list.set_local_player_index(index);
    }
    set_local_player_id(index);
    if let Ok(mut shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() {
        shroud.refresh_shroud_for_local_player();
    }
    apply_local_player_switch_side_effects(initialize_shortcut_bar);
}

fn switch_to_next_non_neutral_player() -> bool {
    let Ok(mut list) = ThePlayerList().write() else {
        return false;
    };

    let player_count = list.get_player_count() as i32;
    if player_count <= 0 {
        return false;
    }

    let current = list.get_local_player_index();
    if current == PLAYER_INDEX_INVALID || current < 0 || current >= player_count {
        return false;
    }

    let neutral_index = list.iter().enumerate().find_map(|(idx, player)| {
        let guard = player.read().ok()?;
        if guard.get_player_type() == PlayerType::Neutral {
            Some(idx as i32)
        } else {
            None
        }
    });

    let mut target = current;
    if player_count > 1 {
        let mut idx = current;
        loop {
            idx += 1;
            if idx >= player_count {
                idx = 0;
            }

            if idx == current {
                break;
            }
            if neutral_index == Some(idx) {
                continue;
            }

            target = idx;
            break;
        }
    }

    drop(list);
    set_local_player_index_with_refresh(target, true);
    target != current
}

fn switch_local_player_between_sides(side_a: &str, side_b: &str) -> bool {
    let Ok(list) = ThePlayerList().read() else {
        return false;
    };

    let current = list.get_local_player_index();
    if current == PLAYER_INDEX_INVALID || current < 0 {
        return false;
    }

    let Some(current_player) = list.get_player(current) else {
        return false;
    };
    let Ok(current_guard) = current_player.read() else {
        return false;
    };
    let target_side = if current_guard.get_side().eq_ignore_ascii_case(side_a) {
        side_b
    } else if current_guard.get_side().eq_ignore_ascii_case(side_b) {
        side_a
    } else {
        return false;
    };
    drop(current_guard);

    let target_index = list.iter().enumerate().find_map(|(idx, player)| {
        let guard = player.read().ok()?;
        if guard.get_side().eq_ignore_ascii_case(target_side) {
            Some(idx as i32)
        } else {
            None
        }
    });
    drop(list);

    if let Some(index) = target_index {
        set_local_player_index_with_refresh(index, false);
        true
    } else {
        false
    }
}

fn stop_movies_for_sound_toggle() {
    let _ = stop_script_display_movie();
    with_window_video_manager(|manager| manager.stop_all_movies());
}

fn cycle_music_track(next: bool) -> Option<String> {
    let manager = get_global_audio_manager()?;
    let mut audio = manager.lock().ok()?;

    let mut current = audio.get_music_track_name().to_string();
    let script_engine = get_script_engine();
    let mut script_guard = script_engine.write().ok();
    if let Some(engine) = script_guard.as_deref_mut().and_then(Option::as_mut) {
        let script_current = engine.get_current_track_name();
        if !script_current.is_empty() {
            current = script_current.to_string();
            audio.set_music_track_name(current.clone());
        }
    }

    audio.set_music_track_name(current);
    let next_track = if next {
        audio.next_music_track()
    } else {
        audio.prev_music_track()
    };

    if next_track.is_empty() {
        return None;
    }
    drop(audio);

    if let Some(engine) = script_guard.as_deref_mut().and_then(Option::as_mut) {
        if let Some(action_handler) = engine.action_handler() {
            let _ = action_handler.music_set_track(&next_track, false, false);
        }
        engine.set_current_track_name(next_track.clone());
    }
    Some(next_track)
}

fn map_meta_time_of_day_to_logic_time_of_day(time_of_day: TimeOfDay) -> LogicTimeOfDay {
    match time_of_day {
        TimeOfDay::Morning => LogicTimeOfDay::Morning,
        TimeOfDay::Afternoon => LogicTimeOfDay::Day,
        TimeOfDay::Evening => LogicTimeOfDay::Evening,
        TimeOfDay::Night => LogicTimeOfDay::Night,
        TimeOfDay::Invalid => LogicTimeOfDay::Day,
    }
}

fn refresh_drawable_time_of_day(time_of_day: TimeOfDay) {
    let mapped = map_meta_time_of_day_to_logic_time_of_day(time_of_day);
    let mut applied = 0usize;
    // Wave 981: empty dual-world → host presentation drawable TOD residual.
    if dual_world_registry_unavailable() {
        queue_host_drawable_tod_residual(time_of_day);
        let _ = (mapped, applied);
        return;
    }
    for object in OBJECT_REGISTRY.get_all_objects() {
        let drawable = object.read().ok().and_then(|guard| guard.get_drawable());
        let Some(drawable) = drawable else {
            continue;
        };
        let mut drawable_guard = match drawable.write() {
            Ok(guard) => guard,
            Err(_) => continue,
        };
        let _ = drawable_guard.set_time_of_day(mapped);
        applied += 1;
    }
    // Host residual: registry empty is fine — Main presentation shell owns drawable TOD.
    let _ = applied;
}

fn refresh_drawable_model_conditions() {
    let clear = ModelConditionFlags::empty();
    let set = ModelConditionFlags::empty();
    // Wave 988: empty dual-world → queue NIGHT/SNOW residual for presentation shell.
    // C++ forceModelsToFollowTimeOfDay/Weather residual on TOD demo cycle.
    if dual_world_registry_unavailable() {
        let (is_night, is_snow) = if let Some(global_data) = get_global_data() {
            let g = global_data.read();
            let night = matches!(g.time_of_day, TimeOfDay::Night);
            let snow = matches!(g.weather, game_engine::common::ini::Weather::Snowy);
            (night, snow)
        } else {
            (false, false)
        };
        queue_host_model_condition_weather_residual(is_night, is_snow);
        let _ = (clear, set);
        return;
    }
    for object in OBJECT_REGISTRY.get_all_objects() {
        if let Ok(mut object_guard) = object.write() {
            let _ = object_guard.clear_and_set_model_condition_flags(clear, set);
        }
    }
}
