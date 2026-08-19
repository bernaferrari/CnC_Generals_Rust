// Split from `message_stream/meta_event.rs` dump. Included by `meta_event_impl/mod.rs`.

fn dispatch_map_entry(record: &MetaMapRec) -> Option<GameMessageDisposition> {
    if let Some(meta) = &record.meta {
        if matches!(meta, GameMessageType::MetaToggleFastForwardReplay) {
            if TheGameLogic::is_in_replay_game() {
                if let Some(global_data) = get_global_data() {
                    let enabled = {
                        let mut guard = global_data.write();
                        guard.tivo_fast_mode = !guard.tivo_fast_mode;
                        guard.tivo_fast_mode
                    };
                    TheInGameUI::message(if enabled {
                        "m_TiVOFastMode: ON"
                    } else {
                        "m_TiVOFastMode: OFF"
                    });
                }
            }
            return Some(GameMessageDisposition::DestroyMessage);
        }
        emit_message(GameMessage::new(meta.clone()));
        return Some(GameMessageDisposition::DestroyMessage);
    }

    // Runtime CommandMap currently relies on these aliases. Keep behavior close to C++:
    // consume the key regardless of whether runtime game-state allows the command.
    if record.name.eq_ignore_ascii_case("PLACE_BEACON") {
        if can_enter_place_beacon_mode() {
            const CMD_NEED_TARGET_POS: u32 = 0x0000_0020;
            TheInGameUI::clear_pending_special_power();
            TheInGameUI::set_pending_command(CommandType::PlaceBeacon, CMD_NEED_TARGET_POS, 0);
            TheInGameUI::set_force_attack_mode(false);
            TheInGameUI::set_force_move_to_mode(false);
            TheInGameUI::set_prefer_selection_mode(false);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DELETE_BEACON") {
        if TheGameLogic::is_in_multiplayer_game() && !TheGameLogic::is_in_replay_game() {
            emit_message(GameMessage::new(GameMessageType::RemoveBeacon(
                Coord3D::default(),
            )));
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("TOGGLE_LOWER_DETAILS") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            if let Ok(mut state) = get_lower_detail_toggle_state().write() {
                if state.is_low_details {
                    global.use_shadow_volumes = state.old_use_shadow_volumes;
                    global.use_light_map = state.old_use_light_map;
                    global.use_cloud_map = state.old_use_cloud_map;
                    global.max_particle_count = state.old_max_particle_count;
                    TheGameLogic::set_show_behind_building_markers(
                        state.old_show_behind_building_markers,
                    );
                    TheInGameUI::message("GUI:ReturnGraphicsToPreviousSettings");
                } else {
                    state.old_use_shadow_volumes = global.use_shadow_volumes;
                    global.use_shadow_volumes = false;

                    state.old_use_light_map = global.use_light_map;
                    global.use_light_map = false;

                    state.old_use_cloud_map = global.use_cloud_map;
                    global.use_cloud_map = false;

                    state.old_show_behind_building_markers =
                        TheGameLogic::get_show_behind_building_markers();
                    TheGameLogic::set_show_behind_building_markers(false);

                    state.old_max_particle_count = global.max_particle_count;
                    global.max_particle_count = DROPPED_MAX_PARTICLE_COUNT;

                    TheInGameUI::message("GUI:DetailsSetToLowest");
                }

                state.is_low_details = !state.is_low_details;
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_NO_DRAW") {
        // C++ CommandXlat.cpp handles MSG_NO_DRAW by setting m_noDraw = 2^32 - 1.
        // This keeps CommandMap demo/debug parity without requiring MSG_NO_DRAW typing yet.
        if let Some(global_data) = get_global_data() {
            global_data.write().no_draw = u32::MAX;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("HELP") {
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_LOD_DECREASE") {
        adjust_texture_reduction_factor(-1);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_LOD_INCREASE") {
        adjust_texture_reduction_factor(1);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_DESHROUD") {
        reveal_local_player_map_permanently();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_DESHROUD") {
        if !TheGameLogic::is_in_multiplayer_game() {
            reveal_local_player_map_permanently();
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_ENSHROUD") {
        shroud_local_player_map();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_DUMP_ASSETS") {
        let _ = dump_used_map_assets();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_VTUNE_ON") {
        set_vtune_enabled(true);
        TheInGameUI::message("VTune Gathering is ON");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_VTUNE_OFF") {
        set_vtune_enabled(false);
        TheInGameUI::message("VTune Gathering is OFF");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_BEGIN_ADJUST_PITCH") {
        set_demo_pitch_adjusting(true);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_END_ADJUST_PITCH") {
        set_demo_pitch_adjusting(false);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_BEGIN_ADJUST_FOV") {
        set_demo_fov_adjusting(true);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_END_ADJUST_FOV") {
        set_demo_fov_adjusting(false);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if let Some(extent_adjust) = parse_extent_adjust_alias(&record.name) {
        apply_extent_adjust_to_local_selection(extent_adjust);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_INCR_ANIM_SKATE_SPEED")
    {
        let value = adjust_skate_distance_override(0.25);
        TheInGameUI::message(&format!("Skate Distance Override is now {value:.6}"));
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_DECR_ANIM_SKATE_SPEED")
    {
        let value = adjust_skate_distance_override(-0.25);
        TheInGameUI::message(&format!("Skate Distance Override is now {value:.6}"));
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_HAND_OF_GOD_MODE")
    {
        if !TheGameLogic::is_in_multiplayer_game() {
            let enabled = toggle_shared_bool_state(hand_of_god_mode_state());
            TheInGameUI::message(if enabled {
                "Meta Hand-Of-God Mode is ON"
            } else {
                "Meta Hand-Of-God Mode is OFF"
            });
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("CHEAT_TOGGLE_HAND_OF_GOD_MODE")
    {
        if !TheGameLogic::is_in_multiplayer_game() {
            let enabled = toggle_shared_bool_state(hand_of_god_mode_state());
            TheInGameUI::message(if enabled {
                "Hand-Of-God Mode is ON"
            } else {
                "Hand-Of-God Mode is OFF"
            });
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_HURT_ME_MODE") {
        if !TheGameLogic::is_in_multiplayer_game() {
            let enabled = toggle_shared_bool_state(hurt_me_mode_state());
            TheInGameUI::message(if enabled {
                "Hurt-Me Mode is ON"
            } else {
                "Hurt-Me Mode is OFF"
            });
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_DEBUG_SELECTION") {
        let enabled = toggle_shared_bool_state(debug_selection_mode_state());
        TheInGameUI::message(if enabled {
            "Debug-Selected-Item Mode is ON"
        } else {
            "Debug-Selected-Item Mode is OFF"
        });
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TEST_SURRENDER") {
        let local_player = get_local_player_id() as u32;
        emit_message(GameMessage::new(GameMessageType::SelfDestruct(
            local_player,
        )));
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_ADD_CASH") {
        if !TheGameLogic::is_in_multiplayer_game() {
            let _ = with_local_player_mut(|player| {
                player.get_money_mut().deposit_money(10_000);
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_ADDCASH") {
        if !TheGameLogic::is_in_multiplayer_game() {
            let _ = with_local_player_mut(|player| {
                player.get_money_mut().deposit_money(10_000);
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_INSTANT_BUILD") {
        if !TheGameLogic::is_in_multiplayer_game() {
            #[cfg(any(debug_assertions, feature = "internal"))]
            {
                let _ = with_local_player_mut(|player| {
                    player.toggle_instant_build();
                });
            }
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_INSTANT_BUILD") {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            let _ = with_local_player_mut(|player| {
                player.toggle_instant_build();
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_FREE_BUILD") {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            let _ = with_local_player_mut(|player| {
                player.toggle_free_build();
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_REMOVE_PREREQ") {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            let _ = with_local_player_mut(|player| {
                player.toggle_ignore_prereqs();
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("CHEAT_GIVE_SCIENCEPURCHASEPOINTS")
    {
        if !TheGameLogic::is_in_multiplayer_game() {
            let _ = with_local_player_mut(|player| {
                player.add_science_purchase_points(1);
            });
            TheInGameUI::message("Adding a SciencePurchasePoint");
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_GIVE_SCIENCEPURCHASEPOINTS")
    {
        let _ = with_local_player_mut(|player| {
            player.add_science_purchase_points(1);
        });
        TheInGameUI::message("Adding a SciencePurchasePoint");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_GIVE_ALL_SCIENCES") {
        if !TheGameLogic::is_in_multiplayer_game() {
            let _ = with_local_player_mut(|player| {
                if let Some(science_store) = get_science_store() {
                    for (&science, _) in science_store.iter() {
                        if science != SCIENCE_INVALID && science_store.is_science_grantable(science)
                        {
                            let _ = player.grant_science(science);
                        }
                    }
                }
            });
            TheInGameUI::message("Granting all sciences!");
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_SWITCH_TEAMS") {
        if !TheGameLogic::is_in_multiplayer_game() {
            if TheGameLogic::is_in_game() {
                let _ = switch_to_next_non_neutral_player();
            }
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_GIVE_ALL_SCIENCES") {
        let _ = with_local_player_mut(|player| {
            if let Some(science_store) = get_science_store() {
                for (&science, _) in science_store.iter() {
                    if science != SCIENCE_INVALID && science_store.is_science_grantable(science) {
                        let _ = player.grant_science(science);
                    }
                }
            }
        });
        TheInGameUI::message("Granting all sciences!");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_GIVE_RANKLEVEL") {
        let _ = with_local_player_mut(|player| {
            let _ = player.set_rank_level(player.get_rank_level() + 1);
        });
        TheInGameUI::message("Adding a RankLevel");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TAKE_RANKLEVEL") {
        let _ = with_local_player_mut(|player| {
            let _ = player.set_rank_level(player.get_rank_level() - 1);
        });
        TheInGameUI::message("Subtracting a RankLevel");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_SWITCH_TEAMS") {
        if TheGameLogic::is_in_game() {
            let _ = switch_to_next_non_neutral_player();
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_SWITCH_TEAMS_CHINA_USA")
        || record
            .name
            .eq_ignore_ascii_case("DEMO_SWITCH_TEAMS_BETWEEN_CHINA_USA")
    {
        let _ = switch_local_player_between_sides("America", "China");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_KILL_SELECTION") {
        if !TheGameLogic::is_in_multiplayer_game() {
            kill_local_player_selection();
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_KILL_SELECTION") {
        kill_local_player_selection();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_KILL_ALL_ENEMIES") {
        kill_all_enemy_objects_for_local_player();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_BATTLE_CRY") {
        if get_global_audio_manager().is_some() {
            let Some(audio) = TheAudio::get() else {
                return Some(GameMessageDisposition::DestroyMessage);
            };
            let misc = TheAudio::get_misc_audio();
            let _ = audio.add_misc_audio_event(&misc.battle_cry_sound);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_GIVE_VETERANCY")
        || record.name.eq_ignore_ascii_case("DEMO_TAKE_VETERANCY")
    {
        if !TheGameLogic::is_in_multiplayer_game() {
            let delta = if record.name.eq_ignore_ascii_case("DEMO_GIVE_VETERANCY") {
                1
            } else {
                -1
            };
            adjust_local_selection_veterancy(delta);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_LOCK_CAMERA_TO_PLANES")
    {
        if let Some(object_id) = next_plane_camera_lock_object_id() {
            with_tactical_view(|view| {
                view.set_camera_lock(Some(object_id));
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_LOCK_CAMERA_TO_SELECTION")
    {
        let selected_id = first_selected_object_id_for_local_player();
        with_tactical_view(|view| {
            let mut next_camera_lock = selected_id;
            if next_camera_lock.is_some() && view.camera_lock_id() == next_camera_lock {
                next_camera_lock = None;
                view.force_redraw();
            }
            view.set_camera_lock(next_camera_lock);
        });
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if let Some((is_cheat_alias, script_index)) = parse_runscript_alias(&record.name) {
        if is_cheat_alias && TheGameLogic::is_in_multiplayer_game() {
            return Some(GameMessageDisposition::DestroyMessage);
        }
        run_key_script_alias(script_index);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_SOUND") {
        if let Some(manager) = get_global_audio_manager() {
            if let Ok(mut audio) = manager.lock() {
                if audio.is_on(AudioAffect::Sound) {
                    stop_movies_for_sound_toggle();
                    audio.set_on(false, AudioAffect::All);
                } else {
                    audio.set_on(true, AudioAffect::All);
                }
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_MILITARY_SUBTITLES")
    {
        TheInGameUI::military_subtitle("MSG:Testing", 10_000);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_NEXT_OBJECTIVE_MOVIE")
    {
        if TheGameLogic::is_in_game() {
            let mut next = 1;
            if let Ok(mut objective) = get_objective_movie_index().write() {
                *objective += 1;
                if *objective > 6 {
                    *objective = 1;
                }
                next = *objective;
            }
            let _ = TheInGameUI::play_movie(&format!("DemoObjective{next:02}"));
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if let Some(movie_index) = parse_objective_movie_alias(&record.name) {
        if TheGameLogic::is_in_game() {
            if let Ok(mut objective) = get_objective_movie_index().write() {
                *objective = movie_index;
            }
            let _ = TheInGameUI::play_movie(&format!("DemoObjective{movie_index:02}"));
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_PLAY_CAMEO_MOVIE") {
        if TheGameLogic::is_in_game() {
            const CAMEO_MOVIE: &str = "CameoMovie";
            if !TheInGameUI::is_movie_playing(CAMEO_MOVIE) {
                let _ = TheInGameUI::play_movie(CAMEO_MOVIE);
            } else {
                let target_window = ["ControlBar.wnd:CameoMovieWindow", "ControlBar.wnd:RightHUD"]
                    .into_iter()
                    .find_map(|window_name| {
                        let window_id =
                            game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
                                window_name,
                            ) as i32;
                        crate::gui::with_window_manager_ref(|manager| {
                            manager.get_window_by_id(window_id)
                        })
                    });
                if let Some(window) = target_window {
                    with_window_video_manager(|manager| manager.stop_movie(&window));
                }
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TIME_OF_DAY") {
        if let Some(global_data) = get_global_data() {
            let (next_time_of_day, changed_time_of_day, force_model_refresh) = {
                let mut global = global_data.write();
                let tod = match global.time_of_day {
                    TimeOfDay::Morning => TimeOfDay::Afternoon,
                    TimeOfDay::Afternoon => TimeOfDay::Evening,
                    TimeOfDay::Evening => TimeOfDay::Night,
                    TimeOfDay::Night | TimeOfDay::Invalid => TimeOfDay::Morning,
                };
                let changed = global.set_time_of_day(tod);
                (tod, changed, global.force_models_to_follow_time_of_day)
            };
            if changed_time_of_day {
                refresh_drawable_time_of_day(next_time_of_day);
                if force_model_refresh {
                    refresh_drawable_model_conditions();
                }
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_SHADOW_VOLUMES")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.use_shadow_volumes = !global.use_shadow_volumes;
            global.use_shadow_decals = !global.use_shadow_decals;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_FOGOFWAR") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.fog_of_war_on = !global.fog_of_war_on;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_TRACKMARKS") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.make_track_marks = !global.make_track_marks;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_WATERPLANE") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.use_water_plane = !global.use_water_plane;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_RENDER") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.disable_render = !global.disable_render;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_BEHIND_BUILDINGS")
    {
        let show_markers = TheGameLogic::get_show_behind_building_markers();
        if show_markers {
            TheGameLogic::set_show_behind_building_markers(false);
            TheInGameUI::message("GUI:ShowBehindBuildings");
        } else {
            TheGameLogic::set_show_behind_building_markers(true);
            TheInGameUI::message("GUI:HideBehindBuildings");
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_LETTERBOX") {
        let handled = crate::gui::with_shell_mut(|shell| {
            if shell.is_shell_active() {
                if let Some(layout) = shell.top() {
                    let hide = !layout.is_hidden();
                    layout.hide(hide);
                }
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
        if !handled {
            let _ = toggle_script_display_letter_box();
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_MOTION_BLUR_ZOOM")
    {
        toggle_motion_blur_zoom_filter();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_BW_VIEW") {
        toggle_bw_view_mode();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_RED_VIEW") {
        toggle_bw_color_view(FilterMode::BWRedAndWhite);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_GREEN_VIEW") {
        toggle_bw_color_view(FilterMode::BWGreenAndWhite);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_SUPPLY_CENTER_PLACEMENT")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_supply_center_placement = !global.debug_supply_center_placement;
            TheInGameUI::message(if global.debug_supply_center_placement {
                "Log SupplyCenter Placement is ON"
            } else {
                "Log SupplyCenter Placement is OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_AI_DEBUG") {
        if let Some(global_data) = get_global_data() {
            let debug_level = {
                let mut global = global_data.write();
                global.debug_ai.value = global.debug_ai.value.saturating_add(1);
                if global.debug_ai.value >= 6 {
                    global.debug_ai.value = 0;
                }
                global.debug_ai.value
            };

            if debug_level == 0 {
                TheInGameUI::message("Debug AI Mode is OFF");
            } else {
                TheInGameUI::message(&format!("Debug AI Mode is Level {}", debug_level));
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_CAMERA_DEBUG") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_camera = !global.debug_camera;
            TheInGameUI::message(if global.debug_camera {
                "Debug Camera Mode is On"
            } else {
                "Debug Camera Mode is OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_VISIONDEBUG") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_visibility = !global.debug_visibility;
            TheInGameUI::message(if global.debug_visibility {
                "Debug Vision Mode is On"
            } else {
                "Debug Vision Mode is OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_PROJECTILEDEBUG")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_projectile_path = !global.debug_projectile_path;
            TheInGameUI::message(if global.debug_projectile_path {
                "Debug Projectile Path Mode is On"
            } else {
                "Debug Projectile Path Mode is OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_THREATDEBUG") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_threat_map = !global.debug_threat_map;
            if global.debug_threat_map {
                global.debug_cash_value_map = false;
            }
            TheInGameUI::message(if global.debug_threat_map {
                "Debug Threat Map is On"
            } else {
                "Debug Threat Map is OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_CASHMAPDEBUG") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_cash_value_map = !global.debug_cash_value_map;
            if global.debug_cash_value_map {
                global.debug_threat_map = false;
            }
            TheInGameUI::message(if global.debug_cash_value_map {
                "Debug Cash Value Map is On"
            } else {
                "Debug Cash Value Map is OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_GRAPHICALFRAMERATEBAR")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_show_graphical_framerate = !global.debug_show_graphical_framerate;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_SHOW_EXTENTS") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_collision_extents = !global.show_collision_extents;
            TheInGameUI::message(if global.show_collision_extents {
                "Show Object Extents ON"
            } else {
                "Show Object Extents OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_SHOW_AUDIO_LOCATIONS")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_audio_locations = !global.show_audio_locations;
            TheInGameUI::message(if global.show_audio_locations {
                "Show AudioLocations ON"
            } else {
                "Show AudioLocations OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_SHOW_HEALTH") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_object_health = !global.show_object_health;
            TheInGameUI::message(if global.show_object_health {
                "Object Health ON"
            } else {
                "Object Health OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_METRICS") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_metrics = !global.show_metrics;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_DEBUG_STATS") {
        toggle_script_display_debug_callback(stat_debug_display_callback);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEBUG_DUMP_PLAYER_OBJECTS")
    {
        dump_player_object_counts(false);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEBUG_DUMP_ALL_PLAYER_OBJECTS")
    {
        dump_player_object_counts(true);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEBUG_OBJECT_ID_PERFORMANCE")
    {
        report_object_id_lookup_performance();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEBUG_DRAWABLE_ID_PERFORMANCE")
    {
        report_drawable_id_lookup_performance();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEBUG_SLEEPY_UPDATE_PERFORMANCE")
    {
        let count = TheGameLogic::get_number_sleepy_updates();
        TheInGameUI::message(&format!("Number of Sleepy Modules: {count}."));
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_NETWORK") {
        toggle_demo_network_runtime();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_CYCLE_LOD_LEVEL") {
        cycle_dynamic_lod_level();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_PARTICLEDEBUG")
    {
        toggle_script_display_debug_callback(particle_system_debug_display_callback);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_AUDIODEBUG") {
        toggle_script_display_debug_callback(audio_debug_display_callback);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_AVI") {
        let _ = toggle_script_display_movie_capture();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_MUSIC") {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        if let Ok(mut audio) = manager.lock() {
            if audio.is_music_playing() {
                audio.stop_audio(AudioAffect::Music);
                audio.set_on(false, AudioAffect::Music);
                TheInGameUI::message("Stopping Music");
            } else {
                audio.set_on(true, AudioAffect::Music);
                audio.resume_audio(AudioAffect::Music);
                TheInGameUI::message("Resuming Music");
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_MUSIC_NEXT_TRACK") {
        if let Some(track_name) = cycle_music_track(true) {
            TheInGameUI::message(&format!("Playing Track: {track_name}"));
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_MUSIC_PREV_TRACK") {
        if let Some(track_name) = cycle_music_track(false) {
            TheInGameUI::message(&format!("Playing Track: {track_name}"));
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_PERFORM_STATISTICAL_DUMP")
    {
        if let Some(global_data) = get_global_data() {
            global_data.write().dump_performance_statistics = true;
        }
        TheInGameUI::message(&format!(
            "Statistics dump made on frame: {}",
            TheGameLogic::get_frame()
        ));
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_WIN") {
        TheVictoryConditions::set_local_allied_victory(true);
        if let Ok(list) = ThePlayerList().read() {
            if let Some(local_player) = list.get_local_player() {
                if let Ok(mut guard) = local_player.write() {
                    guard.set_defeated(false);
                }
            }
        }
        let script_engine = get_script_engine();
        if let Ok(mut guard) = script_engine.write() {
            if let Some(engine) = guard.as_mut() {
                engine.start_end_game_timer();
            }
        }
        TheInGameUI::message("Instant Win");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_ZOOM_LOCK") {
        let zoom_limited = with_tactical_view(|view| {
            let next = !view.is_zoom_limited();
            view.set_zoom_limited(next);
            next
        });
        TheInGameUI::message(if zoom_limited {
            "Camera Zoom Limit: ON"
        } else {
            "Camera Zoom Limit: OFF"
        });
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_SPECIAL_POWER_DELAYS")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.special_power_uses_delay = !global.special_power_uses_delay;
            TheInGameUI::message(if global.special_power_uses_delay {
                "Special Power (Superweapon) Delay: ON"
            } else {
                "Special Power (Superweapon) Delay: OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_FEATHER_WATER")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.feather_water -= 1;
            if global.feather_water < 0 {
                global.feather_water = 5;
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_SHOW_HEALTH") {
        if !TheGameLogic::is_in_multiplayer_game() {
            if let Some(global_data) = get_global_data() {
                let mut global = global_data.write();
                global.show_object_health = !global.show_object_health;
                TheInGameUI::message(if global.show_object_health {
                    "Object Health ON"
                } else {
                    "Object Health OFF"
                });
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("CHEAT_TOGGLE_MESSAGE_TEXT")
    {
        if !TheGameLogic::is_in_multiplayer_game() {
            TheInGameUI::toggle_messages();
            if TheInGameUI::is_messages_on() {
                TheInGameUI::message("GUI:MessagesOn");
            }
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_MESSAGE_TEXT") {
        TheInGameUI::toggle_messages();
        if TheInGameUI::is_messages_on() {
            TheInGameUI::message("GUI:MessagesOn");
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("CHEAT_TOGGLE_SPECIAL_POWER_DELAYS")
    {
        if !TheGameLogic::is_in_multiplayer_game() {
            if let Some(global_data) = get_global_data() {
                let mut global = global_data.write();
                global.special_power_uses_delay = !global.special_power_uses_delay;
                TheInGameUI::message(if global.special_power_uses_delay {
                    "Special Power (Superweapon) Delay: ON"
                } else {
                    "Special Power (Superweapon) Delay: OFF"
                });
            }
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    // C++ consumes these command-map keybinds by appending the corresponding
    // message type. Rust keeps input parity by consuming even when full message
    // handlers are not ported yet.
    if is_unimplemented_cpp_command_name(&record.name) {
        return Some(GameMessageDisposition::DestroyMessage);
    }

    None
}

fn can_enter_place_beacon_mode() -> bool {
    if !TheGameLogic::is_in_multiplayer_game() || TheGameLogic::is_in_replay_game() {
        return false;
    }

    let Some(local_player) = ThePlayerList()
        .read()
        .ok()
        .and_then(|list| list.get_local_player().cloned())
    else {
        return false;
    };

    let Ok(local_guard) = local_player.read() else {
        return false;
    };
    if !local_guard.is_player_active() {
        return false;
    }

    let net_min_players = get_global_data()
        .map(|data| data.read().net_min_players)
        .unwrap_or(0);
    let is_multiplayer_session = get_game_engine()
        .map(|engine| engine.lock().is_multiplayer_session())
        .unwrap_or(false);
    if net_min_players != 0 && !is_multiplayer_session {
        return false;
    }

    let Some(template_name) = local_guard
        .get_player_template()
        .map(|template| template.beacon_name.clone())
    else {
        return false;
    };
    if template_name.is_empty() {
        return false;
    }

    let Some(beacon_template) = TheThingFactory::find_template(&template_name) else {
        return false;
    };
    let mut count = [0];
    local_guard.count_objects_by_thing_template(
        std::slice::from_ref(&beacon_template),
        false,
        false,
        &mut count,
    );
    debug!(
        "MSG_META_PLACE_BEACON - Player already has {} beacons active",
        count[0]
    );

    let max_beacons = with_multiplayer_settings(|settings| settings.max_beacons_per_player);
    count[0] < max_beacons
}
