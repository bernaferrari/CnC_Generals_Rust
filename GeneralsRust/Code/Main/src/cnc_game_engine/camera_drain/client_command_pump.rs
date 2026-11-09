use super::*;

impl CnCGameEngine {
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn apply_receiving_rank_science_action(
        &mut self,
        action: game_engine::common::message_stream::ReceivingRankScienceAction,
    ) -> Result<(), String> {
        use game_engine::common::message_stream::{ReceivingRankScienceAction as Action, ReceivingRankScienceUi};
        let Some(id) = self.game_logic.local_player_id() else { return Ok(()) };
        let resets_rank = self.game_logic.get_player(id)
            .is_some_and(|player| {
                let old = player.rank_level.max(1);
                let requested = match action {
                    Action::GiveRank => old.saturating_add(1),
                    Action::TakeRank => old.saturating_sub(1).max(1),
                    _ => return false,
                };
                let count = self.game_logic.world_services.common().ini.rank.read().get_rank_level_count();
                count > 0 && requested.min(self.game_logic.rank_level_limit().max(1) as u32)
                    .min(count as u32) < old
            });
        let default_general_name = if resets_rank {
            Some(self.game_client.game_text_store().write()
                .map_err(|error| format!("receiving GameText lock poisoned: {error}"))?
                .fetch_owned("SCIENCE:GeneralName"))
        } else {
            None
        };
        let owner = self.game_client.window_manager_owner();
        let bar = &mut self.control_bar;
        let client = &mut self.game_client;
        self.game_logic.apply_receiving_rank_science(id, action, default_general_name, None, &mut |event| {
            match event {
                ReceivingRankScienceUi::ScienceAdded => bar.mark_ui_dirty(),
                ReceivingRankScienceUi::RankChanged { purchase_points }
                | ReceivingRankScienceUi::PurchasePointsChanged { purchase_points } => {
                    if let Some(ui) = client.in_game_ui_subsystem_mut() {
                        owner.with_manager(|manager| {
                            bar.on_receiving_player_rank_or_points_changed(manager, ui, purchase_points);
                        });
                    }
                }
            }
        })?;
        let text = match action {
            Action::AddPurchasePoint => "Adding a SciencePurchasePoint",
            Action::GrantAllSciences => "Granting all sciences!",
            Action::GiveRank => "Adding a RankLevel",
            Action::TakeRank => "Subtracting a RankLevel",
            Action::SetRank(_) => return Ok(()),
        };
        if let Some(ui) = self.game_client.in_game_ui_subsystem_mut() {
            ui.message(text, self.game_logic.get_frame());
        }
        Ok(())
    }

    /// C++ GameClient.cpp:560-565 — TheSnowManager + TheAnim2DCollection
    /// UPDATE at the execute/logic cadence, not every 45 Hz present.
    pub(in crate::cnc_game_engine) fn host_update_cpp_snow_and_anim2d(&mut self) {
        #[cfg(feature = "game_client")]
        {
            let dt = self.snow_anim2d_dt_for_present();
            self.game_client.update_cpp_snow_and_anim2d(dt);
        }
    }

    pub(super) fn snow_anim2d_dt_for_present(&mut self) -> f32 {
        self.snow_anim2d_dt_for_present_with_now(Instant::now)
    }

    /// The host branch observes wall time only outside a running/paused match.
    /// Keeping observation here allows deterministic tests of the real API.
    pub(super) fn snow_anim2d_dt_for_present_with_now(&mut self, now: impl FnOnce() -> Instant) -> f32 {
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            let now = self.host_match_logic_frame;
            let steps = match (now, self.host_snow_logic_frame_applied) {
                (Some(now), Some(prev)) if now > prev => now - prev,
                (Some(_), None) => 1,
                _ => 0,
            };
            if now.is_some() && now != self.host_snow_logic_frame_applied {
                self.host_snow_logic_frame_applied = now;
            }
            return steps as f32 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
        }
        snow_anim2d_client_only_dt(&mut self.host_snow_client_only_last_tick, now())
    }

    /// Route surviving client messages into the driving world's existing queue.
    /// Menu and runtime-host start paths share the native receiving boundary.
    #[cfg(all(test, feature = "game_client"))]
    pub(in crate::cnc_game_engine) fn pump_receiving_client_commands(
        client: &mut game_client::core::GameClient,
        pending: &mut crate::command_system::ReplayPendingState,
    ) -> Result<(), String> {
        client
            .pump_message_stream_to_receiving_owner(
                pending.recorder_owned(),
                &mut |messages| {
                    pending
                        .receive_client_messages(messages)
                        .map_err(game_client::core::GameClientError::InvalidOperation)
                },
                0,
                0,
            )
            .map_err(|error| error.to_string())
    }

    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn with_receiving_client_translation<R>(
        &mut self,
        operation: impl FnOnce(
            &mut game_client::core::GameClient,
            &mut super::translation_audio_phase::NativeTranslationPhases<'_, '_>,
            &mut crate::subsystem_manager::SubsystemManager,
        ) -> R,
    ) -> Result<
        (
            R,
            Vec<game_client::gui::callbacks::replay_menu::ingress::ReplayMenuStep>,
            Option<game_engine::common::message_stream::ReceivingRankScienceAction>,
        ),
        String,
    > {
        self.menu_audio.require_bound()?;
        let menu_audio_locality = self.game_logic.receiving_audio_locality_for_player(-1);
        let menu_audio_view = super::audio_view::CompletedAudioView::capture(&self.game_logic);
        let menu_world = self.receiving_menu_world_facts();
        let control_bar_observation = self.receiving_control_bar_callback_observation();
        let diplomacy_world = self.receiving_diplomacy_world_facts().ok();
        let playback = self
            .game_logic
            .replay_pending
            .recorder_owned()
            .is_playback();
        let replay = matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        );
        let game_ending = self.game_logic.owned_script_game_ending();
        let tooltips_disabled = self.receiving_tooltips_disabled();
        let observer_roster = self.receiving_observer_roster();
        let camera_rules = self.game_logic.camera_input_rules();
        self.control_bar.set_receiving_upgrade_definitions(
            &self.game_logic.upgrade_definitions,
            self.game_logic
                .receiving_module_content
                .as_ref()
                .map(|content| content.action_definitions_shared()),
        );
        let upgrade_observer = self.game_logic.receiving_upgrade_tooltip_observer();
        let pick_objects = self.game_logic.translator_pick_object_view();
        let mut replay_operation =
            game_client::gui::callbacks::replay_menu::ingress::ReplayMenuOperation::new(
                !self.game_logic.isInNetworkGame(),
                self.game_client.window_manager_owner().downgrade(),
            );
        let mut steps = Vec::new();
        let rank_action = std::cell::Cell::new(None);
        let result = {
            let pending = &mut self.game_logic.replay_pending;
            let mut replay_version = |filename: &str| {
                pending
                    .replay_version_differs_owned(filename.to_owned())
                    .unwrap_or(false)
            };
            let render_pipeline = &mut self.render_pipeline;
            let mut terrain_lod = |reduction| render_pipeline.apply_texture_lod(reduction);
            let preferences = self.ui_manager.options_preferences_mut();
            let control_bar = &mut self.control_bar;
            let viewport = &mut self.game_logic.session_view.camera;
            let mut driver = |menu_audio: &mut dyn game_client::gui::menu_audio::MenuAudioEffects, owner: &game_client::gui::window_manager::WindowManagerOwner,
                              campaign: &mut game_client::gui::campaign_manager::CampaignManager,
                              main_menu: &mut game_client::gui::shell::main_menu::MainMenu,
                              menu_host: &mut game_client::gui::campaign_launch_host_bridge::MenuHostMailbox,
                              menus: &mut game_client::gui::callbacks::menu_callbacks::MenuCallbackRecipients<'_>,
                              display: &mut game_client::core::script_action_handler::ScriptDisplayReceiver,
                              videos: &mut game_client::gui::window_video_manager::ClientWindowVideos,
                              ime: &mut game_client::gui::ime_manager::ImeManager,
                              ui: &mut game_client::core::subsystems::InGameUISubsystem,
                              cursor_mouse: Option<&mut game_client::input::Mouse>,
                              request,
                              messages: &mut Vec<game_engine::common::message_stream::GameMessage>| {
                if let game_engine::common::message_stream::WindowInputDispatch::RankScience(action) = request {
                    rank_action.set(Some(action));
                    return Some(game_engine::common::message_stream::WindowInputDispatchResult::RankScienceAccepted);
                }
                let mut context = game_client::gui::window_manager::ControlBarInputContext {
            control_bar_observation:Some(control_bar_observation.clone()),
            diplomacy_world: diplomacy_world.as_ref(),
            menu_audio: Some(menu_audio),
            input_provenance: game_client::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
            ui: Some(ui), logic_frame: Some(menu_world.logic_frame), client_frame: Some(menu_world.client_frame),
            menus: Some(menus.reborrow()),
            ime:Some(ime),
                    upgrade_tooltip: game_client::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Receiving(&upgrade_observer),
                    replay_menu: Some(&mut replay_operation), replay_version: Some(&mut replay_version),
                    observer_roster: observer_roster.as_ref(),
                    camera_options: Some(game_client::gui::window_manager::CameraOptionsInputContext::new(&mut *preferences, camera_rules).with_terrain_lod(&mut terrain_lod)),
                    control_bar: &mut *control_bar, viewport: &mut *viewport,
                    replay, game_ending, tooltips_disabled, cursor_mouse,
                    window_manager_owner: Some(owner), campaign_manager: Some(campaign), main_menu: Some(main_menu),
                    menu_world: Some(menu_world), menu_host: Some(menu_host), script_display: Some(display), window_videos: Some(&mut videos.challenge),
            in_game_movies: Some(&mut videos.in_game),
                    emitted_messages: Some(messages),
                };
                let result = game_client::gui::window_manager::dispatch_translated_window_input_for_owner(owner, request, &mut context);
                drop(context);
                if let Some(step) = replay_operation.take() { steps.push(step); }
                result
            };
            let audio = self.menu_audio.lend(
                &mut self.game_logic.session_view.audio_random,
                self.game_logic.queued_audio_events.policy(),
                menu_audio_locality,
                menu_audio_view,
            );
            let eva_speaker = &mut self.eva_recipient.speaker;
            let mut phases = super::translation_audio_phase::NativeTranslationPhases {
                dispatch: &mut driver,
                audio,
                facts: menu_world,
                playback,
                pick_objects,
                eva_speaker,
            };
            operation(
                &mut self.game_client,
                &mut phases,
                &mut self.subsystem_manager,
            )
        };
        Ok((result, steps, rank_action.get()))
    }

    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn pump_native_client_commands(&mut self) -> Result<(), String> {
        use game_engine::common::message_stream::PropagationProgress;
        let mut cursor = None;
        loop {
            let facts = self.receiving_menu_world_facts();
            let (progress, steps, rank_action) =
                self.with_receiving_client_translation(|client, phases, _manager| {
                    crate::subsystem_manager::ReceivingClientPhaseDriver::propagate(
                        phases,
                        client,
                        cursor.take(),
                    )
                })?;
            if let Some(action) = rank_action {
                self.apply_receiving_rank_science_action(action)?;
            }
            match progress.map_err(|error| error.to_string())? {
                PropagationProgress::Complete(messages) => {
                    // OptionsMenu WND callbacks can finish without emitting a
                    // translated message. Drain their typed LOD effect at the
                    // same completed-client boundary as emitted commands.
                    self.host_tick_options_bridge();
                    let result = self
                        .game_logic
                        .replay_pending
                        .receive_client_messages(messages);
                    for step in steps {
                        step.complete(self)?;
                    }
                    return result;
                }
                PropagationProgress::HostBoundary(next) => {
                    for step in steps {
                        step.complete(self)?;
                    }
                    let reset = self.complete_receiving_menu_host_requests()?;
                    if self.is_quitting() {
                        return Ok(());
                    }
                    cursor = Some(if reset {
                        next.after_world_reset()
                    } else {
                        next
                    });
                }
            }
        }
    }

    /// Wave 588: Menu GameClient shell tick + NewGame drain residual.
    ///
    /// Advances Main-injected device state, shell/pre/post UI, then drains
    /// MSG_NEW_GAME before receiving command propagation so WND Start reaches InGame.
    /// Returns `true` when a match start was applied (caller should `return`).
    ///
    /// Distinct from InGame `host_tick_game_client_presentation_shell` (no FOW/pose;
    /// NewGame intercept is Menu-only). Full `GameClient::update` stays disconnected.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_tick_game_client_menu_shell(
        &mut self,
    ) -> Result<bool, String> {
        // Peek/start MSG_NEW_GAME before update_input/propagateMessages
        // can destroy it. WND Skirmish Start posts here.
        if let Some(request) = self.take_pending_new_game_start_request() {
            if matches!(request.mode, GameMode::Shell) {
                info!("Menu NewGame drain: ignore GAME_SHELL (shell map already live)");
                Self::take_shell_new_game_messages_from_common_stream(
                    self.game_client.message_stream_mut(),
                );
            } else {
                info!(
                    "Menu NewGame drain: mode={:?} faction={} map={} skirmish={}",
                    request.mode,
                    request.faction,
                    request.map,
                    request.skirmish.is_some()
                );
                self.start_game_from_ui(request)?;
                let _ = Self::take_new_game_dispatch_from_common_stream(
                    self.game_client.message_stream_mut(),
                );
                return Ok(true);
            }
        }
        let early_menu_frame = self.menu_world_frames_rendered < 5;
        let t0 = std::time::Instant::now();
        let snow_dt = self.snow_anim2d_dt_for_present();
        let post_draw_menu_world = self.receiving_menu_world_facts();
        let shell_menu_world = self.receiving_menu_world_facts();
        let lighting = self.game_logic.lighting_rules();
        let weather_is_snowy = self
            .game_logic
            .weather_state()
            .current_weather
            .to_ascii_lowercase()
            .contains("snow");
        let requested_time_of_day;
        {
            let gc = &mut self.game_client;
            gc.set_session_model_environment(lighting, weather_is_snowy);
            if early_menu_frame {
                debug!(
                    "Menu update_internal: calling gc.ensure_shell_visible (menu_frame={})",
                    self.menu_world_frames_rendered
                );
            }
            let _ = gc.ensure_shell_visible(Some(shell_menu_world));
            let _ = gc.create_frame_tick_message();
            let t1 = std::time::Instant::now();
            if early_menu_frame {
                debug!("Menu update_internal: calling gc.update_input");
            }
            // Wave 587/588: device bookkeeping on Main-injected state (not dual OS poll).
            let _ = gc.update_input();
            requested_time_of_day = gc.take_requested_session_time_of_day();
            // C++ GameClient.cpp:560-565 — snow + Anim2D at execute cadence.
            gc.update_cpp_snow_and_anim2d(snow_dt);
            let t2 = std::time::Instant::now();
            if early_menu_frame {
                debug!("Menu update_internal: calling gc.update_pre_draw_ui");
            }
            let _ = gc.update_pre_draw_ui();
            // C++ GameClient.cpp:719-741 — TerrainVisual + DisplayStringManager.
            gc.update_terrain_visual();
            let _ = gc.update_display_string_manager();
            let t3 = std::time::Instant::now();
            if early_menu_frame {
                debug!("Menu update_internal: calling gc.update_post_draw_ui");
            }
            let _ = gc.update_post_draw_ui(Some(post_draw_menu_world));
            let _ = (t1, t2, t3);
        }
        if let Some(time_of_day) = requested_time_of_day {
            self.game_logic.set_receiving_time_of_day(time_of_day);
        }

        self.complete_receiving_menu_host_requests()?;
        if self.is_quitting() {
            return Ok(true);
        }

        // Peek MSG_NEW_GAME before pump so WND Start reaches InGame, then
        // deliver survivors to the current Main command owner exactly once.
        let t4 = std::time::Instant::now();
        if let Some(request) = self.take_pending_new_game_start_request() {
            if matches!(request.mode, GameMode::Shell) {
                // C++ MSG_NEW_GAME GAME_SHELL is the shell map, already applied
                // by finalize. Treating it as a match start loads Defcon6.
                info!("Menu NewGame drain: ignore GAME_SHELL (shell map already live)");
                Self::take_shell_new_game_messages_from_common_stream(
                    self.game_client.message_stream_mut(),
                );
            } else {
                info!(
                    "Menu NewGame drain: mode={:?} faction={} map={} skirmish={}",
                    request.mode,
                    request.faction,
                    request.map,
                    request.skirmish.is_some()
                );
                self.start_game_from_ui(request)?;
                {
                    if early_menu_frame {
                        debug!("Menu update_internal: receiving client command pump");
                    }
                    self.pump_native_client_commands()?;
                }
                let _ = Self::take_new_game_dispatch_from_common_stream(
                    self.game_client.message_stream_mut(),
                );
                return Ok(true);
            }
        }

        {
            if early_menu_frame {
                debug!("Menu update_internal: receiving client command pump");
            }
            self.pump_native_client_commands()?;
        }

        let t5 = std::time::Instant::now();
        let menu_gc_elapsed = t0.elapsed();
        if menu_gc_elapsed >= std::time::Duration::from_millis(50) || early_menu_frame {
            debug!(
                "Menu GC update: total={:?} newgame_scan={:?} pump_tail={:?} frame={}",
                menu_gc_elapsed,
                t4.duration_since(t0),
                t5.duration_since(t4),
                self.frame_counter,
            );
        }
        Ok(false)
    }
}
