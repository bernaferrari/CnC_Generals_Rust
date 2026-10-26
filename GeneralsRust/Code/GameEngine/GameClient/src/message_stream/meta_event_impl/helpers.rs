// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

fn parse_objective_movie_alias(name: &str) -> Option<i32> {
    let upper = name.to_ascii_uppercase();
    let suffix = upper.strip_prefix("DEMO_PLAY_OBJECTIVE_MOVIE")?;
    let value = suffix.parse::<i32>().ok()?;
    if (1..=6).contains(&value) {
        Some(value)
    } else {
        None
    }
}

fn parse_runscript_alias(name: &str) -> Option<(bool, i32)> {
    let upper = name.to_ascii_uppercase();
    if let Some(suffix) = upper.strip_prefix("CHEAT_RUNSCRIPT") {
        let value = suffix.parse::<i32>().ok()?;
        return (1..=9).contains(&value).then_some((true, value));
    }

    if let Some(suffix) = upper.strip_prefix("DEMO_RUNSCRIPT") {
        let value = suffix.parse::<i32>().ok()?;
        return (1..=9).contains(&value).then_some((false, value));
    }

    None
}

fn audio_debug_display_callback(
    _display: &mut DebugDisplay,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

fn particle_system_debug_display_callback(
    _display: &mut DebugDisplay,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

fn stat_debug_display_callback(
    _display: &mut DebugDisplay,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

fn toggle_script_display_debug_callback(
    receiver: &mut crate::core::script_action_handler::ScriptDisplayReceiver,
    target: DebugDisplayCallback,
) {
    let active = get_script_display_debug_callback(receiver);
    let same_callback = active
        .map(|callback| callback as usize == target as usize)
        .unwrap_or(false);
    let _ = set_script_display_debug_callback(
        receiver,
        if same_callback { None } else { Some(target) },
    );
}

fn toggle_demo_network_runtime() {
    #[cfg(not(feature = "network"))]
    {
        if let Some(network) = game_network::get_network() {
            network.toggle_network_on();
        }
    }

    #[cfg(feature = "network")]
    {
        let _ = game_network::get_network();
    }
}

fn set_vtune_enabled(enabled: bool) {
    VTUNE_ENABLED.set(enabled);
}

#[cfg(test)]
fn is_vtune_enabled_for_tests() -> bool {
    VTUNE_ENABLED.get()
}

fn adjust_skate_distance_override(delta: f32) -> f32 {
    SKATE_DISTANCE_OVERRIDE.with(|value| {
        let next = value.get() + delta;
        value.set(next);
        next
    })
}

#[cfg(test)]
fn set_skate_distance_override_for_tests(value: f32) {
    SKATE_DISTANCE_OVERRIDE.set(value);
}

fn dump_used_map_assets() -> std::io::Result<()> {
    let mut names = Vec::new();
    with_drawable_manager_ref(|manager| {
        for drawable_id in manager.get_all_drawable_ids() {
            let Some(drawable) = manager.get_drawable(drawable_id) else {
                continue;
            };
            let Some(name) = drawable.get_template_name() else {
                continue;
            };
            if !name.is_empty() {
                names.push(name.to_string());
            }
        }
    });
    names.sort();
    names.dedup();

    let mut output = String::new();
    for name in names {
        output.push_str(&name);
        output.push('\n');
    }
    fs::write("UsedMapAssets.txt", output)
}

fn cycle_dynamic_lod_level(ui: &mut crate::message_stream::ReceivingUi<'_>) {
    let next = CYCLE_LOD_LEVEL_STATE.with(|level| {
        let next = match level.get() {
            DynamicGameLODLevel::VeryHigh => DynamicGameLODLevel::High,
            DynamicGameLODLevel::High => DynamicGameLODLevel::Medium,
            DynamicGameLODLevel::Medium => DynamicGameLODLevel::Low,
            _ => DynamicGameLODLevel::VeryHigh,
        };
        level.set(next);
        next
    });

    game_engine::common::game_lod::set_dynamic_lod_from_string(next.to_str());
    let message = format!("Dynamic Game Detail {}", next.to_str());
    ui.message(&message);
}

#[cfg(test)]
fn set_cycle_lod_level_state_for_tests(level: DynamicGameLODLevel) {
    CYCLE_LOD_LEVEL_STATE.set(level);
}

fn next_plane_camera_lock_object_id() -> Option<u32> {
    let mut candidates: Vec<u32> = Vec::new();
    // Wave 345: scan dual-world registry only when populated.
    if !dual_world_registry_unavailable() {
        for object in OBJECT_REGISTRY.get_all_objects() {
            let Ok(object_guard) = object.read() else {
                continue;
            };
            if !object_guard.is_above_terrain() {
                continue;
            }
            if object_guard.is_kind_of(KindOf::Projectile) {
                continue;
            }
            candidates.push(object_guard.get_id());
        }
    }
    // Wave 979: host empty dual-world → presentation catalog airborne residual.
    // Wave 1071: fail-closed on destroyed/sold/masked/FOW/non-local stealth residuals.
    if candidates.is_empty() {
        with_translator_catalog(|catalog| {
            for entry in catalog {
                if translator_entry_has_kind(entry, "Projectile") {
                    continue;
                }
                if !(entry.airborne_target || translator_entry_has_kind(entry, "Aircraft")) {
                    continue;
                }
                if entry.destroyed || entry.sold || entry.masked || entry.unselectable {
                    continue;
                }
                if entry.shroud_status >= 2 && !translator_entry_is_local(entry) {
                    continue;
                }
                if entry.effectively_stealthed && !translator_entry_is_local(entry) {
                    continue;
                }
                candidates.push(entry.object_id);
            }
        });
    }
    // Fallback: local selection via TheGameLogic when catalog has no airborne.
    if candidates.is_empty() {
        for object_id in local_selection_object_ids() {
            let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };
            if !object_guard.is_above_terrain() {
                continue;
            }
            if object_guard.is_kind_of(KindOf::Projectile) {
                continue;
            }
            candidates.push(object_id);
        }
    }

    if candidates.is_empty() {
        return None;
    }

    let previous = LAST_PLANE_LOCK_OBJECT_ID.with(Cell::get);

    let next = if let Some(previous_id) = previous {
        if let Some(index) = candidates.iter().position(|id| *id == previous_id) {
            candidates[(index + 1) % candidates.len()]
        } else {
            candidates[0]
        }
    } else {
        candidates[0]
    };

    LAST_PLANE_LOCK_OBJECT_ID.set(Some(next));

    Some(next)
}

#[cfg(test)]
fn set_last_plane_lock_object_id_for_tests(object_id: Option<u32>) {
    LAST_PLANE_LOCK_OBJECT_ID.set(object_id);
}

fn toggle_bw_color_view(mode: FilterMode) {
    with_tactical_view(|view| {
        if view.get_view_filter_type() == FilterType::BlackAndWhite {
            view.set_view_filter_mode(FilterMode::Null);
            view.set_view_filter(FilterType::Null);
            view.set_fade_parameters(30, -1);
            return;
        }

        view.set_view_filter_mode(mode);
        view.set_view_filter(FilterType::BlackAndWhite);
        view.set_fade_parameters(30, 1);
    });
}

fn toggle_bw_view_mode() {
    let mode = BW_VIEW_MODE_STATE.with(Cell::get);
    match mode {
        0 => {
            game_engine::common::global_data::write().writable.wireframe = true;
            with_tactical_view(|view| view.set_3d_wireframe_mode(true));
            BW_VIEW_MODE_STATE.set(1);
        }
        1 => {
            let mut should_disable_wireframe = false;
            with_tactical_view(|view| {
                if view.get_view_filter_type() == FilterType::Crossfade {
                    view.set_view_filter_mode(FilterMode::Null);
                    view.set_view_filter(FilterType::Null);
                    view.set_fade_parameters(0, -1);
                } else {
                    if let Ok(mut script_engine_guard) = get_script_engine().write() {
                        if let Some(script_engine) = script_engine_guard.as_mut() {
                            script_engine.do_freeze_time();
                        }
                    }
                    view.set_view_filter_mode(FilterMode::CrossfadeFbMask);
                    view.set_view_filter(FilterType::Crossfade);
                    view.set_fade_parameters(60, -1);
                    view.set_3d_wireframe_mode(false);
                    should_disable_wireframe = true;
                    BW_VIEW_MODE_STATE.set(2);
                }
            });
            if should_disable_wireframe {
                game_engine::common::global_data::write().writable.wireframe = false;
            }
        }
        _ => {
            if let Ok(mut script_engine_guard) = get_script_engine().write() {
                if let Some(script_engine) = script_engine_guard.as_mut() {
                    script_engine.do_unfreeze_time();
                }
            }
            BW_VIEW_MODE_STATE.set(0);
        }
    }
}

fn toggle_motion_blur_zoom_filter() {
    with_tactical_view(|view| {
        if view.get_view_filter_type() == FilterType::MotionBlur {
            view.set_view_filter_mode(FilterMode::Null);
            view.set_view_filter(FilterType::Null);
            return;
        }

        let saturate = MOTION_BLUR_ZOOM_SATURATE.with(|state| {
            let current = state.get();
            state.set(!current);
            current
        });

        let mut mode = if saturate {
            FilterMode::MBInAndOutSaturate
        } else {
            FilterMode::MBInAndOutAlpha
        };
        if view.camera_lock_id().is_some() {
            mode = FilterMode::MBPanAlpha;
        }

        let mut filter_pos = *view.position();
        filter_pos.x += 200.0;
        filter_pos.y += 200.0;
        view.set_view_filter_pos(&filter_pos);
        view.set_view_filter_mode(mode);
        view.set_view_filter(FilterType::MotionBlur);
    });
}

fn run_key_script_alias(script_index: i32) {
    let script_name = format!("KEY_F{script_index}");
    let script_engine = gamelogic::scripting::engine::get_script_engine();
    let Ok(mut engine_guard) = script_engine.write() else {
        return;
    };
    let Some(engine) = engine_guard.as_mut() else {
        return;
    };
    let _ = engine.execute_subroutine_by_name(&script_name);
}

fn local_selection_object_ids() -> Vec<u32> {
    let selection_manager = get_selection_manager();
    selection_manager
        .read()
        .ok()
        .and_then(|manager| {
            manager
                .get_player_selection_ref(get_local_player_id())
                .map(|selection| selection.get_selected_objects())
        })
        .unwrap_or_default()
}

fn dump_player_object_counts(
    include_all_objects: bool,
    ui: &mut crate::message_stream::ReceivingUi<'_>,
) {
    let Ok(player_list) = ThePlayerList().read() else {
        return;
    };

    ui.message("*******************************");
    ui.message("Dumping player object counts");

    for i in 0..player_list.get_player_count() {
        let Some(player_arc) = player_list.get_player(i as i32).cloned() else {
            continue;
        };
        let Ok(player_guard) = player_arc.read() else {
            continue;
        };
        if !player_guard.is_playable_side() {
            continue;
        }

        let mut object_count = 0;
        let mut object_lines: Vec<String> = Vec::new();
        let _ = player_guard.iterate_objects(|object_arc| {
            let Ok(object_guard) = object_arc.read() else {
                return Ok(());
            };
            if object_guard.is_effectively_dead() {
                return Ok(());
            }

            object_count += 1;
            if include_all_objects || object_count <= 5 {
                object_lines.push(format!(
                    "Object {} ({})",
                    object_guard.get_id(),
                    object_guard.get_template().get_name()
                ));
            }
            Ok(())
        });

        ui.message(&format!(
            "Player {i} ({}) has {object_count} non-dead objects",
            player_guard.get_player_display_name()
        ));

        if object_count > 0 && (include_all_objects || object_count <= 5) {
            ui.message("Objects are:");
            for line in object_lines {
                ui.message(&line);
            }
        }
    }
}

fn report_object_id_lookup_performance(ui: &mut crate::message_stream::ReceivingUi<'_>) {
    // Wave 345/1005: empty dual-world → presentation catalog residual metrics.
    if dual_world_registry_unavailable() {
        let n = crate::presentation_translator_residual::translator_catalog_len();
        // C++ debug residual: report presentation-known object count instead of
        // dual-world OBJECT_REGISTRY lookup timing.
        ui.message(&format!(
            "Dual-world residual: presentation catalog knows {n} object ids (no OBJECT_REGISTRY lookup timing)."
        ));
        return;
    }

    for number_lookups in [10_000_u32, 100_000_u32, 1_000_000_u32] {
        let start = Instant::now();
        for test_index in 1..number_lookups {
            black_box(TheGameLogic::find_object_by_id(test_index));
        }
        let elapsed = start.elapsed().as_secs_f64();
        let next_index = TheGameLogic::get_object_id_counter();
        ui.message(&format!(
            "Time to run {number_lookups} ObjectID lookups is {elapsed:.6}. Next index is {next_index}."
        ));
    }
}

fn report_drawable_id_lookup_performance(ui: &mut crate::message_stream::ReceivingUi<'_>) {
    // Wave 1006: dual-world residual — report presentation drawable count instead of
    // hammering empty dual-world id space timing.
    if dual_world_registry_unavailable() {
        let n = TheGameClient::get()
            .map(|c| c.drawable_count())
            .unwrap_or(0);
        ui.message(&format!(
            "Dual-world residual: presentation shell has {n} drawables (no DrawableID lookup timing)."
        ));
        return;
    }

    let maybe_client = TheGameClient::get();
    for number_lookups in [10_000_u32, 100_000_u32, 1_000_000_u32] {
        let start = Instant::now();
        for test_index in 1..number_lookups {
            let value = maybe_client.and_then(|client| client.find_drawable_by_id(test_index));
            black_box(value);
        }
        let elapsed = start.elapsed().as_secs_f64();
        let next_index = Drawable::get_drawable_id_counter();
        ui.message(&format!(
            "Time to run {number_lookups} DrawableID lookups is {elapsed:.6}. Next index is {next_index}."
        ));
    }
}

/// Called only with the presentation recipient supplied by the driving client.
pub(crate) fn execute_display_input_action(
    action: game_engine::common::message_stream::DisplayInputAction,
    receiver: &mut crate::core::script_action_handler::ScriptDisplayReceiver,
    owner: &crate::gui::window_manager::WindowManagerOwner,
    videos: Option<&mut crate::gui::window_video_manager::WindowVideoManager>,
) -> Option<bool> {
    use game_engine::common::message_stream::{
        DisplayDebugKind as Kind, DisplayInputAction as Action,
    };
    Some(match action {
        Action::IsMoviePlaying => {
            crate::core::script_action_handler::is_script_display_movie_playing(receiver)
        }
        Action::StopMovie => stop_script_display_movie(receiver),
        Action::StopRadarMovie => {
            crate::gui::window_video_manager::stop_radar_movie_for_owner(owner, videos?);
            true
        }
        Action::NextObjectiveMovie => videos?.advance_objective_movie(owner),
        Action::PlayObjectiveMovie(index) => videos?.play_objective_movie(owner, index),
        Action::ToggleCameoMovie => videos?.toggle_cameo_movie(owner),
        Action::ToggleLetterbox => toggle_script_display_letter_box(receiver),
        Action::ToggleMovieCapture => toggle_script_display_movie_capture(receiver),
        Action::ToggleDebug(kind) => {
            let target = match kind {
                Kind::Statistics => stat_debug_display_callback,
                Kind::Particles => particle_system_debug_display_callback,
                Kind::Audio => audio_debug_display_callback,
            };
            toggle_script_display_debug_callback(receiver, target);
            true
        }
    })
}

fn dispatch_display_input(
    action: game_engine::common::message_stream::DisplayInputAction,
    dispatch: &mut game_engine::common::message_stream::WindowInputDispatchFn<'_>,

    ui: &mut crate::message_stream::ReceivingUi<'_>,
) -> bool {
    matches!(
        ui.dispatch(
            dispatch,
            game_engine::common::message_stream::WindowInputDispatch::Display(action)
        ),
        Some(game_engine::common::message_stream::WindowInputDispatchResult::Display(true))
    )
}

/// A translator's synchronous GUI operation. The caller owns the actual window
/// manager loan; callbacks receive only that same instance's Effects.
pub(crate) fn execute_ui_input_action(
    action: game_engine::common::message_stream::UiInputAction,
    manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    game_ending: bool,
) -> bool {
    use game_engine::common::message_stream::UiInputAction as Action;
    match action {
        Action::DemoMilitarySubtitle => {
            let frame = effects.logic_frame();
            effects
                .ui_mut()
                .military_subtitle("MSG:Testing", 10_000, frame, manager);
            true
        }
        Action::ToggleDiplomacy { immediate } => {
            crate::gui::callbacks::diplomacy::toggle_diplomacy(immediate, manager, effects, None)
                .is_ok()
        }
        Action::ToggleControlBar => {
            crate::gui::callbacks::control_bar_callbacks::toggle_control_bar(
                false, manager, effects,
            )
            .is_ok()
        }
        Action::ToggleQuitMenu => {
            effects.with_receiving_manager(manager, |effects| {
                crate::gui::callbacks::quit_menu::toggle_quit_menu_receiving(
                    game_ending,
                    effects,
                    None,
                );
            });
            true
        }
        Action::ToggleShellTopVisibility => {
            let owner = effects
                .window_manager_owner
                .expect("Shell operation requires receiving owner");
            let (active, layout) = owner
                .with_shell_ref(|shell| (shell.is_shell_active(), shell.top_layout_receipt()))
                .unwrap_or((false, None));
            if active {
                if let Some(layout) = layout {
                    let hide = !layout.borrow().is_hidden();
                    layout.borrow().hide_with_manager(manager, hide);
                }
            }
            active
        }
        Action::ActivateHotKey(key) => {
            let Some(key) = char::from_u32(key) else {
                return false;
            };
            let activation = manager.hot_keys.activation(&key.to_string());
            activation.is_some_and(|activation| activation.execute(manager, effects))
        }
    }
}

pub(crate) fn dispatch_ui_input(
    action: game_engine::common::message_stream::UiInputAction,
    dispatch: &mut game_engine::common::message_stream::WindowInputDispatchFn<'_>,

    ui: &mut crate::message_stream::ReceivingUi<'_>,
) -> bool {
    match ui.dispatch(
        dispatch,
        game_engine::common::message_stream::WindowInputDispatch::Ui(action),
    ) {
        Some(game_engine::common::message_stream::WindowInputDispatchResult::Ui(handled)) => {
            handled
        }
        _ => panic!("GUI translator operation requires its receiving UI driver"),
    }
}
