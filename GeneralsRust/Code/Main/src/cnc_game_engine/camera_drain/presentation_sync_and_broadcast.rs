use super::*;

#[cfg(feature = "game_client")]
struct ClientWorldLoan<'a> {
    logic: &'a mut crate::game_logic::GameLogic,
    paused: bool,
}

#[cfg(feature = "game_client")]
impl game_client::core::game_client::NativeClientWorld for ClientWorldLoan<'_> {
    fn visual_time(&mut self) -> game_client::core::GameClientResult<game_client::core::game_client::NativeVisualTime> {
        native_visual_time::observe_receiving(self.logic, self.paused, None)
            .map_err(game_client::core::GameClientError::InvalidOperation)
    }

    fn update_orphaned_ghosts(&mut self) -> game_client::core::GameClientResult<()> {
        self.logic.update_orphaned_host_fog_ghosts()
            .map_err(game_client::core::GameClientError::InvalidOperation)
    }

    fn ambient_time_of_day(&self) -> Option<game_client::system::TimeOfDay> {
        super::native_view_effects::receiving_native_drawable_policy_for(self.logic).time_of_day
    }

    fn ambient_locality(&mut self, object_id: Option<u32>) -> game_engine::common::audio::AudioSubmissionLocality {
        let source = object_id
            .and_then(|id| self.logic.host_object(crate::game_logic::ObjectId(id)))
            .and_then(|object| object.owner_player_id)
            .map(|id| id as i32)
            .unwrap_or(-1);
        self.logic.receiving_audio_locality_for_player(source)
    }
}

impl CnCGameEngine {
    /// Admit logic-origin material commands only after resolving this world's
    /// current complete Drawable association. The pending command remains on
    /// its host object until the receiving owner accepts the whole batch.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_prepare_presentation_materials(
        &mut self,
    ) -> Result<Vec<game_client::core::game_client::NativeMaterialObservation>, String> {
        super::presentation_materials::prepare(
            &mut self.game_logic,
            self.last_presentation_frame.as_deref(),
            &mut self.game_client,
            self.host_direct_visual_world_epoch,
        )
    }

    /// Wave 590: boot/render residual — freeze a PresentationFrame if none installed.
    /// Ensures execute never dual-reads live GameLogic mid-draw.
    #[inline]
    pub(in crate::cnc_game_engine) fn host_ensure_presentation_frame_for_render(
        &mut self,
    ) -> Result<(), String> {
        // Wave 590: boot presentation seed residual.
        if self.last_presentation_frame.is_some() {
            return Ok(());
        }
        // Wave 195/590/926: shadow sync + presentation build via single host boundary.
        let frame = self.host_sync_shadow_and_build_presentation(false)?;
        self.last_presentation_frame = Some(frame.into());
        Ok(())
    }

    /// Wave 590: pipeline env seed residual (host+GW) when pipeline has no frame.
    #[inline]
    pub(in crate::cnc_game_engine) fn host_ensure_presentation_env_for_hints(
        &mut self,
    ) -> Result<(), String> {
        // Wave 590: pipeline env seed residual.
        // Wave 466: prefer host+GameWorld shadow freeze when a shadow session exists.
        // Wave 455: presentation-only env boundary — seed via build_for_engine only.
        if self.render_pipeline.presentation_frame().is_some() {
            return Ok(());
        }
        // Wave 466: sync shadow then freeze via build_for_engine (not host-only None).
        // Wave 466/474: seed via build_for_engine(host + self.gameworld_shadow).
        self.game_logic.owned_script_engine.as_ref().ok_or(
            "Cannot freeze native camera fade: receiving interpreter is unavailable or lent",
        )?;
        if let Some(ref mut shadow) = self.gameworld_shadow {
            shadow.sync_from_host(&self.game_logic);
        }
        let runtime_heightmap = self.presentation_runtime_heightmap_for_frame();
        let mut env_frame =
            crate::presentation_frame::PresentationFrame::try_publish_for_engine_with_runtime_heightmap(
                &mut self.game_logic,
                self.current_player_id,
                self.gameworld_shadow.as_ref(),
                runtime_heightmap,
                false,
            )?;
        #[cfg(feature = "game_client")]
        {
            env_frame.dynamic_lights = self.game_client.dynamic_lights().to_vec();
        }
        #[cfg(feature = "game_client")]
        env_frame.scene_lines.extend(
            self.game_client
                .render_bridge()
                .snapshot_visible_scene_lines()
                .into_iter()
                .map(crate::presentation_frame::PresentationSceneLine::from),
        );
        self.render_pipeline
            .set_presentation_frame(Some(env_frame.into()));
        Ok(())
    }

    /// Wave 600: post-presentation client residual (camera/audio/UI/popup/music).
    ///
    /// After HUD apply: non-Menu camera + SFX cleanup, InGame UI projection,
    /// script camera residual, and presentation-or-boot popup/music/movies.
    pub(in crate::cnc_game_engine) fn host_tick_post_presentation_client_residuals(
        &mut self,
        visual_dt: f32,
        dt: f32,
    ) {
        // Wave 600: post-presentation client residual.
        // C++ GameClient.cpp:560-565 — snow + Anim2D every client frame.
        self.host_update_cpp_snow_and_anim2d();
        // Update camera
        if self.current_state != GameState::Menu {
            self.update_camera(visual_dt);
        }

        // Update audio
        if self.current_state != GameState::Menu {
            self.cleanup_sound_effects();
        }
        if self.current_state == GameState::InGame {
            self.set_runtime_ui_state_projection(Screen::GameHUD);
            if let Err(err) = self.ui_manager.update(dt) {
                warn!("UI manager update failed in playing state: {}", err);
            }
        }

        // Commands already drained in the logic-frame block (before shadow).
        // Script camera requests still apply here after presentation build.
        if self.current_state == GameState::InGame {
            self.apply_pending_script_camera_requests();
        }

        // Wave 571: presentation-or-boot popup/music residual via helpers.
        if let Some(pres) = self.last_presentation_frame.clone() {
            self.apply_presentation_popup_music_residual(&pres);
            // Presentation movie residual: play via GameClient script display, then drain.
            self.apply_presentation_movie_residual(&pres);
        } else {
            self.apply_boot_popup_music_residual();
            // Wave 567: boot residual movies via helper (no presentation frame).
            self.apply_boot_movie_residual();
        }
    }

    /// Wave 599: match outcome broadcast residual (defeat/alliance/victory).
    ///
    /// Presentation-or-boot defeat and alliance event drains, FOW/script side
    /// effects, and victory screen when freeze/boot says the match ended.
    pub(in crate::cnc_game_engine) fn host_broadcast_match_outcome_residuals(&mut self) {
        // Wave 599: match outcome broadcast residual.
        // Broadcast defeat notifications so UI/systems mirror C++ VictoryConditions flow.
        // Wave 569: presentation-or-boot defeat residual via helper.
        let defeated_players: Vec<u32> = self.take_presentation_or_boot_defeat_events();
        for player_id in defeated_players {
            // Prefer presentation roster when installed (no live get_player dual-read).
            let roster = self
                .last_presentation_frame
                .as_ref()
                .and_then(|f| f.player_info(player_id).cloned());
            if let Some(player) = roster {
                let message = localization::localize_with_args(
                    "hud.message.player_defeated",
                    "{player} has been defeated!",
                    &[("player", player.name.as_str())],
                );
                info!("Player {} ({}) has been defeated", player.name, player_id);
                self.host_push_info_message(&message);
                // Wave 539: presentation freeze → HUD radar + audio (no GameLogic dual-write).
                self.notify_presentation_ui_message(&message);
            } else if self.last_presentation_frame.is_some() {
                // Wave 542: freeze installed but roster miss — fail-closed id-only
                // (no GameLogic dual-write mid-frame).
                info!("Player {} has been defeated", player_id);
            } else if let Some(player) = self.ui_player_info(player_id) {
                // Wave 237: defeat UI prefers presentation roster helper (boot residual only; no presentation freeze).
                let message = localization::localize_with_args(
                    "hud.message.player_defeated",
                    "{player} has been defeated!",
                    &[("player", player.name.as_str())],
                );
                info!("Player {} ({}) has been defeated", player.name, player_id);
                self.host_push_info_message(&message);
                // Wave 566: boot residual via notify_boot_ui_message helper.
                self.notify_boot_ui_message(&message, Some(player.team));
            } else {
                // Fail-closed id-only residual.
                info!("Player {} has been defeated", player_id);
            }
            // C++ VictoryConditions.cpp:201-214 — first local defeat
            // TheRadar->forceOn + SetInGameChatType(EVERYONE).
            if self.presentation_or_boot_local_player_id() == Some(player_id) {
                self.host_game_logic_mut().set_radar_forced(true);
                #[cfg(feature = "game_client")]
                {
                    self.game_client.set_in_game_chat_type_owned(
                        game_client::gui::callbacks::ingame_callbacks::InGameChatType::Everyone,
                        false, // This exact local player was just eliminated.
                    );
                    self.set_owned_control_bar_scheme("Observer");
                }
            }
        }

        // Wave 569: presentation-or-boot alliance residual via helper.
        let alliance_events: Vec<crate::game_logic::AllianceNotification> =
            self.take_presentation_or_boot_alliance_events();
        // Wave 553: prefer presentation local_player residual when installed.
        let local_player_id = self.presentation_or_boot_local_player_id();
        let mut observer_notified = false;
        for event in alliance_events {
            let is_local = local_player_id == Some(event.player_id);
            if !is_local && local_player_id.is_some() {
                continue;
            }
            if !is_local && observer_notified {
                continue;
            }

            let (key, fallback) = match event.state {
                AllianceState::AlliedVictory if is_local => {
                    ("hud.message.allied_victory", "Your alliance has triumphed!")
                }
                AllianceState::AlliedDefeat if is_local => (
                    "hud.message.allied_defeat",
                    "Your alliance has been defeated!",
                ),
                AllianceState::AlliedVictory => (
                    "hud.message.observer_allied_victory",
                    "An alliance has won the battle.",
                ),
                AllianceState::AlliedDefeat => (
                    "hud.message.observer_allied_defeat",
                    "An alliance has been defeated.",
                ),
                AllianceState::Active => continue,
            };

            let message = localization::localize(key, fallback);
            self.host_push_info_message(&message);
            // Wave 538: when presentation freeze is installed, route radar/UI SFX
            // through HUD + audio subsystem (no GameLogic dual-write mid-frame).
            // Boot/menu residual still uses host queue/play_ui_sound.
            if self.last_presentation_frame.is_some() {
                // Wave 538/539: shared presentation UI notify residual.
                self.notify_presentation_ui_message(&message);
            } else {
                // Wave 566: boot residual via notify_boot_ui_message helper.
                let team = self.ui_player_team(event.player_id);
                self.notify_boot_ui_message(&message, team);
            }
            if !is_local {
                observer_notified = true;
            }
        }

        // C++ VictoryConditions.cpp:128-160 only sets m_endFrame +
        // m_singleAllianceRemaining. Game continues as observer; MultiplayerScripts.scb
        // MULTIPLAYER_ALLIED_VICTORY/DEFEAT drive movies/end timers. ScoreScreen comes
        // from MSG_CLEAR_GAME_DATA (QuitMenu / ScriptEngine timer), not evaluate.
        // Live leftover TheVictoryConditions is written by evaluate(); do not pause
        // or request_state_change(Victory/Defeat) via presentation_or_boot_victory_winner.
    }

    /// Wave 598: InGame HUD presentation residual.
    ///
    /// Prefers last presentation freeze for HUD/EVA/ControlBar; boot residual
    /// without freeze uses `ui_local_economy`. Also advances HUD/diplomacy/chat
    /// panels and placement cursors for the InGame state only.
    pub(in crate::cnc_game_engine) fn host_apply_ingame_hud_from_presentation(&mut self, dt: f32) {
        // Wave 598: InGame HUD presentation residual.
        if self.current_state != GameState::InGame {
            return;
        }
        // Update HUD + ControlBar selection panel from presentation when available
        // (resources + minimap + selection health). ControlBar health is snapshot-owned.
        if let Some(pres) = self.last_presentation_frame.clone() {
            self.apply_presentation_to_huds(&pres);
            // Presentation audio already dispatched via dispatch_audio_events_direct
            // (BuildingComplete/UnitReady/UpgradeComplete). Do not dual-play engine SFX.
            self.sync_eva_messages_from_presentation(&pres);
            #[cfg(feature = "game_client")]
            {
                self.update_owned_control_bar_animations();
                self.control_bar.set_receiving_upgrade_definitions(
                    &self.game_logic.upgrade_definitions,
                    self.game_logic
                        .receiving_module_content
                        .as_ref()
                        .map(|content| content.action_definitions_shared()),
                );
                {
                    let facts = self.receiving_menu_world_facts();
                    let bar = &mut self.control_bar;
                    self.game_client
                        .with_window_event_effects(Some(facts), |effects| {
                            effects.with_manager(|manager, effects| {
                                pres.apply_to_control_bar(bar, manager, effects)
                            })
                        })
                };
                let _ = self
                    .update_receiving_observer_control_bar(std::time::Duration::from_millis(33));
            }
        } else {
            // Wave 238: boot residual via ui_local_economy (no &Player dual-read).
            let (money, power, max_power) = self.ui_local_economy();
            self.game_hud_mut()
                .update_resources(money, power, max_power);
        }

        if dt.is_finite() {
            // UIManager advances this HUD once in the post-presentation pass.
            self.diplomacy_panel.update(dt);
            self.chat_panel.update(dt);
            self.sync_pending_structure_placement_cursor();
            self.sync_pending_map_command_radius_cursor();
        } else {
            warn!(
                "Skipping Game HUD update due to non-finite delta time: {}",
                dt
            );
        }
    }

    /// Complete each actual logic step before observing it. Optional authority
    /// experiments retain their separate scoped publication path.
    pub(super) fn host_run_fast_forward_loop(
        &mut self,
        dt: f32,
        ff_steps: usize,
        headless_step_budget: Option<usize>,
        mut owned_observer: Option<&mut crate::gameworld_shadow::GameWorldShadow>,
    ) -> std::result::Result<(), String> {
        for _ in 0..ff_steps {
            let (_, interrupted) = self.drive_native_command_phase(
                dt,
                headless_step_budget,
                false,
                owned_observer.as_deref_mut(),
            )?;
            self.dispatch_game_logic_audio_events();
            if interrupted {
                break;
            }
        }
        Ok(())
    }

    /// Wave 597: GameWorld shadow session residual via single boundary.
    ///
    /// Compatibility boundary for explicit authority experiments and probes.
    /// Ordinary match frames pass their observation mirror directly.

    /// Wave 589: post-logic presentation finalize residual.
    ///
    /// Builds immutable `PresentationFrame` (victory + GameWorld object path when
    /// shadow live), dispatches presentation audio events, mirrors particle FX to
    /// the client, stores `last_presentation_frame`, then applies InGame script FPS.
    ///
    /// Call after host logic + shadow writeback. Borrow-first: no live dual-read
    /// during later render. Fail-closed: not sole GameWorld authority / playable_claim.
    pub(in crate::cnc_game_engine) fn host_finalize_presentation_after_logic(
        &mut self,
    ) -> Result<(), String> {
        self.game_logic.owned_script_engine.as_ref().ok_or(
            "Cannot freeze native camera fade: receiving interpreter is unavailable or lent",
        )?;
        // Native ControlBar pause requests are admitted once by
        // host_tick_control_bar_bridge; never poll a standalone Core UI queue.
        // Wave 589: presentation finalize residual.
        // Wave 589/838/926: shadow sync + victory presentation build via single boundary.
        // Admit actual client bindings into the receiving canonical registry
        // before freezing observations. Presentation frames never emit systems.
        let fx_n = self
            .game_logic
            .combat_particles_mut()
            .backfill_client_systems(self.game_client.particle_systems_mut());
        let mut pres = self.host_sync_shadow_and_build_presentation(true)?;
        // Presentation → audio subsystem directly (no GameLogic dual-write mid-frame).
        let audio_observer = self.receiving_audio_observer_target();
        let audio_n = pres.dispatch_audio_events_direct(
            self.subsystem_manager
                .get_mut::<crate::subsystem_manager::AudioManagerSubsystem>()
                .expect("initialized engine audio subsystem"),
            &self.game_logic,
            audio_observer,
        );
        if audio_n > 0 {
            log::trace!("presentation audio events dispatched: {audio_n}");
        }
        // Returned IDs already belong to the receiving canonical registry.
        if fx_n > 0 {
            log::trace!("presentation particle client mirrors: {fx_n}");
        }
        self.last_presentation_frame = Some(pres.into());
        self.render_pipeline
            .set_presentation_frame(self.last_presentation_frame.clone());
        // Wave 844: keep host sim residuals current for freeze-miss peels.
        self.host_refresh_match_sim_residuals_from_logic();

        // Wave 568: InGame script FPS residual via helper.
        self.apply_ingame_script_fps_limit_residual();
        Ok(())
    }

    /// Wave 586/587: GameClient presentation shell tick residual.
    ///
    /// Host path:
    /// 1. advances client device residual from Main-injected THE_MOUSE/THE_KEYBOARD
    ///    (`update_input` = device frame bookkeeping only — **not** a second OS poll)
    /// 2. applies frozen presentation FOW/pose/cinematic residual
    /// 3. `update_presentation_shell` (local drawable modules + UI/message pump)
    ///
    /// Full `GameClient::update()` stays disconnected on purpose:
    /// - Main owns OS intake → inject_* → commands (no dual OS event ownership)
    /// - Main owns audio dispatch from PresentationFrame (no dual `update_audio`)
    /// - Main `RenderPipeline` is sole 3D present (no dual `draw_display`)
    /// - full `update()` also `finish_frame_timing` sleeps — would double-pace host frames
    /// Wave 876: `update_presentation_shell` no longer sleeps; Main sole frame pace.
    /// Dual-world registry path remains available inside GameClient when
    /// `OBJECT_REGISTRY` is populated (opt-in bridge); production host keeps it empty.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_tick_game_client_presentation_shell(&mut self) {
        if let Err(error) = self.native_display.require_shell_slot() {
            log::error!("Native Client prefix rejected: {error}");
            return;
        }
        self.game_client
            .admit_draw_icon_ui(self.game_logic.get_draw_icon_ui());
        let lighting = self.game_logic.lighting_rules();
        let weather_is_snowy = self
            .game_logic
            .weather_state()
            .current_weather
            .to_ascii_lowercase()
            .contains("snow");
        self.game_client
            .set_session_model_environment(lighting, weather_is_snowy);
        // C++ GameLogic::update setFrame(TheGameLogic->getFrame()) before client update.
        // Extra presents with the same host frame freeze drawable/FX time.
        // The actual client frame is admitted before each logic step, including catch-up.
        // Presentation must retain the last executed frame rather than the next logic frame.
        // Wave 587: process Main-injected device state before shell UI residual.
        // inject_game_client_* already wrote this client's devices from OS events;
        // update_input only runs Keyboard/Mouse::update bookkeeping (no second OS poll).
        if let Err(err) = self.game_client.update_input() {
            log::trace!("GameClient input update failed: {err}");
        }
        if let Some(time_of_day) = self.game_client.take_requested_session_time_of_day() {
            self.game_logic.set_receiving_time_of_day(time_of_day);
        }
        // Camera/script actions may have changed since the last frozen frame.
        // The receiving session owns this gate; a snapshot cannot override it.
        let presentation_time_frozen =
            native_visual_time::simulation_frozen(&self.game_logic, self.game_paused);
        let logic_frame = self.host_match_logic_frame;
        let new_logic_frame =
            logic_frame.is_some() && logic_frame != self.host_visual_logic_frame_applied;
        let visual_delta = if presentation_time_frozen {
            0.0
        } else if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            let steps = match (logic_frame, self.host_visual_logic_frame_applied) {
                (Some(now), Some(prev)) if now > prev => now - prev,
                (Some(_), None) => 1,
                _ => 0,
            };
            if new_logic_frame {
                self.host_visual_logic_frame_applied = logic_frame;
            }
            steps as f32 * game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL
        } else {
            self.snow_anim2d_dt_for_present()
        };
        if let Err(error) = self.host_sync_presentation_direct_drawables(presentation_time_frozen) {
            self.native_display
                .fail_shell(format!("Drawable association failed: {error}"));
            return;
        }
        let material_observations = match self.host_prepare_presentation_materials() {
            Ok(observations) => observations,
            Err(error) => {
                self.native_display
                    .fail_shell(format!("Drawable material admission failed: {error}"));
                return;
            }
        };
        let (authored_visibility, delivered_synchronously) = self
            .game_logic
            .session_view
            .take_letterbox_visibility_output();
        let has_authored_visibility = delivered_synchronously || !authored_visibility.is_empty();
        for hidden in authored_visibility {
            if hidden {
                self.hide_owned_control_bar(true);
            } else {
                self.show_owned_control_bar(false);
            }
        }
        if let Some(letterbox) = self
            .last_presentation_frame
            .as_ref()
            .map(|frame| frame.cinematic_letterbox)
        {
            // Presentation cinematic letterbox residual → client display.
            let was_letterbox = self.game_client.letterbox_overlay_enabled();
            self.game_client
                .apply_presentation_cinematic_letterbox(letterbox);
            if !has_authored_visibility && was_letterbox != letterbox {
                // Display::enableLetterBox changes View's zoom limit only on
                // the transition; a manual toggle survives ordinary frames.
                self.game_logic
                    .session_view
                    .camera
                    .native_camera
                    .set_zoom_limited(!letterbox);
            }
            // Deliver a transition once; rendering never writes View height.
            if !has_authored_visibility && letterbox && !was_letterbox {
                self.hide_owned_control_bar(true);
            } else if !has_authored_visibility && !letterbox && was_letterbox {
                self.show_owned_control_bar(false);
            }
        }
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            // Military caption residual → InGameUI (duration from freeze).
            self.game_client.apply_presentation_military_caption(
                pres.military_caption.as_deref(),
                pres.military_caption_remaining_ms,
                pres.frame.0,
            );
            // Wave 1060: floating cash/text residual → InGameUI (keep vanish phase).
            let floating: Vec<(String, [f32; 3], (u8, u8, u8, u8), u32, u32)> = pres
                .floating_texts
                .iter()
                .filter(|ft| ft.is_active_at(pres.frame.0))
                .map(|ft| {
                    (
                        ft.text.clone(),
                        [ft.position.x, ft.position.y, ft.position.z],
                        ft.color_rgba,
                        ft.spawn_frame,
                        ft.timeout_frame.saturating_sub(ft.spawn_frame).max(1),
                    )
                })
                .collect();
            self.game_client
                .apply_presentation_floating_texts(&floating);
            let world_anims = pres
                .world_anims
                .iter()
                .map(|animation| animation.native())
                .collect::<Vec<_>>();
            self.game_client.apply_presentation_world_anims(
                self.host_direct_visual_world_epoch,
                pres.frame.0,
                self.game_logic.is_paused(),
                &world_anims,
            );
            let sw_timers: Vec<(String, String, bool)> = if !pres.superweapon_display_enabled {
                Vec::new()
            } else {
                pres.superweapon_timers
                    .iter()
                    .filter(|t| t.unlocked)
                    .map(|t| {
                        // C++ InGameUI.cpp:3648-3650 name + mm:ss even when ready (0:00).
                        (
                            superweapon_timer_strip_name(pres, t),
                            superweapon_countdown_text(t.remaining),
                            t.ready,
                        )
                    })
                    .collect()
            };
            self.game_client
                .apply_presentation_superweapon_timers(&sw_timers);

            // Cinematic text residual → W3DDisplay caption (not HUD chat).
            self.game_client.apply_presentation_cinematic_text(
                pres.cinematic_text.as_deref(),
                pres.cinematic_text_remaining_ms,
                pres.cinematic_font.as_deref(),
            );
            // Wave 964: selection residual for InGameUI host empty dual-world path.
            let sel_units: Vec<game_client::gui::ingame_ui::PresentationSelectedUnitResidual> = {
                let selected: std::collections::HashSet<_> =
                    pres.selected.iter().copied().collect();
                pres.objects
                    .iter()
                    .filter(|o| selected.contains(&o.id) && !o.destroyed)
                    .map(|o| {
                        let health_pct = if o.health_max > 0.0 {
                            (o.health_current / o.health_max).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        game_client::gui::ingame_ui::PresentationSelectedUnitResidual {
                            object_id: o.id.0,
                            template_name: o.template_name.clone(),
                            position: [o.position.x, o.position.y, o.position.z],
                            health_pct,
                            kind_names: Self::presentation_kind_names(
                                &self.kind_name_cache,
                                &o.kind_of,
                            ),
                            // Wave 1040: selection HUD legality residual.
                            destroyed: o.destroyed,
                            sold: o.sold,
                            unselectable: o.unselectable,
                            masked: o.masked,
                            effectively_stealthed: o.effectively_stealthed,
                            team_name: format!("{:?}", o.team),
                        }
                    })
                    .collect()
            };
            self.game_client
                .apply_presentation_selection_residual(sel_units);

            // Wave 966: full unit catalog residual for host select-similar.
            // Wave 1055: reverse map object_id → control group (lowest group wins).
            let mut object_hotkey_group: std::collections::HashMap<u32, i8> =
                std::collections::HashMap::new();
            for (group, ids) in self
                .game_logic
                .player_hotkey_squads(self.current_player_id)
                .iter()
                .enumerate()
            {
                let g = i8::try_from(group).unwrap_or(-1);
                if g < 0 {
                    continue;
                }
                for id in ids {
                    object_hotkey_group.entry(id.0).or_insert(g);
                }
            }
            let catalog: Vec<game_client::gui::ingame_ui::PresentationUnitCatalogEntry> = {
                use crate::unit_control::UnitControlSystem;
                pres.objects
                    .iter()
                    .filter(|o| !o.destroyed)
                    .map(
                        |o| game_client::gui::ingame_ui::PresentationUnitCatalogEntry {
                            object_id: o.id.0,
                            template_name: o.template_name.clone(),
                            team_name: format!("{:?}", o.team),
                            selectable: UnitControlSystem::presentation_is_selectable(o),
                            position: [o.position.x, o.position.y, o.position.z],
                            // Wave 1024: orientation residual for dual-world pose peel.
                            orientation: o.orientation,
                            // Wave 1026: disabled residual for dual-world command availability.
                            disabled: o.disabled,
                            // Wave 1028: under-construction residual for dual-world ControlBar.
                            under_construction: o.under_construction,
                            construction_percent: o.construction_percent.clamp(0.0, 1.0),
                            // Wave 1030: garrison residual for dual-world structure inventory.
                            max_garrison: (o.max_garrison as u32).min(u16::MAX as u32) as u16,
                            occupant_count: (o.occupant_count as u32).min(u16::MAX as u32) as u16,
                            // Wave 1031: OCL timer residual for dual-world ControlBar OclTimer.
                            ocl_timer_seconds: o.ocl_timer_seconds,
                            // Wave 1033: sold residual for dual-world ControlBar clear.
                            sold: o.sold,
                            script_unsellable: o.script_unsellable,
                            // Wave 1034: unselectable residual for dual-world selection.
                            unselectable: o.unselectable,
                            // Wave 1035: destroyed/masked residual for dual-world selection.
                            destroyed: o.destroyed,
                            masked: o.masked,
                            // Wave 1036: effectively stealthed residual for dual-world selection.
                            effectively_stealthed: o.effectively_stealthed,
                            // Wave 1041: disguise residual for dual portrait/template peel.
                            disguised: o.disguised,
                            disguise_as_template: o.disguise_as_template.clone(),
                            disguise_as_team: o.disguise_as_team.map(|t| format!("{t:?}")),
                            kind_names: Self::presentation_kind_names(
                                &self.kind_name_cache,
                                &o.kind_of,
                            ),
                            // Wave 979: airborne catalog.
                            // Wave 971: special-power ready residual for host SP targeting.
                            special_power_ready: o.special_power_ready,
                            airborne_target: o.airborne_target
                                || o.kind_of.iter().any(|k| k.name() == "Aircraft"),
                            // Wave 981: FOW → command-hint shroud residual.
                            shroud_status: {
                                use crate::fow_rendering::ObjectVisibility;
                                use gamelogic::common::ObjectShroudStatus;
                                let v = o.fow_visibility;
                                if v.visibility_alpha >= 0.95 {
                                    ObjectShroudStatus::Clear
                                } else if v.is_explored >= 0.5 || v.visibility_alpha > 0.05 {
                                    if v.visibility_alpha >= 0.5 {
                                        ObjectShroudStatus::PartialClear
                                    } else {
                                        ObjectShroudStatus::Fogged
                                    }
                                } else {
                                    ObjectShroudStatus::Shrouded
                                }
                            },
                            // Wave 982: producer/slaver residual for IgnoredInGui mouseover.
                            slaver_object_id: o.producer_id.map(|id| id.0),
                            // Wave 1011: health residual for dual-world portrait.
                            health_current: o.health_current,
                            health_maximum: if o.health_max > 0.0 {
                                o.health_max
                            } else {
                                1.0
                            },
                            // Wave 1012: veterancy chevron residual.
                            veterancy_overlay: {
                                use crate::presentation_frame::PresentationVeterancy as PV;
                                match o.veterancy {
                                    PV::Veteran => Some("SSChevron1L".to_string()),
                                    PV::Elite => Some("SSChevron2L".to_string()),
                                    PV::Heroic => Some("SSChevron3L".to_string()),
                                    PV::Rookie => None,
                                }
                            },
                            // Wave 1013: production queue head residual.
                            production_progress: o
                                .production_queue
                                .first()
                                .map(|p| p.progress_ratio),
                            production_template: o
                                .production_queue
                                .first()
                                .map(|p| p.template_name.clone()),
                            production_paused: o.production_paused,
                            // Wave 1015: command-set residual for dual-world ControlBar.
                            command_set_name: if !o.command_set_name.is_empty() {
                                o.command_set_name.clone()
                            } else {
                                o.command_set_override.clone()
                            },
                            // Wave 1055: host control-group residual for dual group numerals.
                            hotkey_group: object_hotkey_group.get(&o.id.0).copied().unwrap_or(-1),
                            caption: crate::command_executor::host_beacon_caption(
                                &self.game_logic,
                                crate::game_logic::ObjectId(o.id.0),
                            )
                            .unwrap_or_default(),
                            supply_boxes: if o.dock_kind
                                == crate::game_logic::DockKind::SupplyWarehouse
                            {
                                Some(o.drawable_supply_boxes as i32)
                            } else {
                                None
                            },
                        },
                    )
                    .collect()
            };
            self.game_client.apply_presentation_unit_catalog(catalog);
            // Wave 968: local team residual for host mouseover/ownership path.
            if let Some(local_team) = self.ui_local_player_team_name() {
                self.game_client
                    .apply_presentation_local_team_name(local_team);
            }
        }
        // PRES_SHELL_ONLY_DRAWABLE_TICK: client modules via update_drawables_local.
        // Wave 862: presentation pose/shroud/caption residual already applied above.
        // Do not call full update_drawables — double-ticks and overwrites presentation FOW.
        // Boot/loading without freeze still uses the same shell tick (empty registry early-out).
        let frames = game_client::core::game_client::NativeParticleFrames {
            logic_frame: self.game_logic.frame,
            client_frame: self.game_client.get_frame(),
        };
        let native_mode = self.receiving_menu_world_facts().game_mode;
        let mut world = ClientWorldLoan {
            logic: &mut self.game_logic,
            paused: self.game_paused,
        };
        match self
            .game_client
            .try_begin_presentation_shell_with_owned_visual_time_and_ambient(
                visual_delta,
                frames,
                &material_observations,
                native_mode,
                &mut world,
            ) {
            Ok(step) => {
                if let Err(error) = self.native_display.queue_shell(
                    step,
                    material_observations,
                    self.host_direct_visual_world_epoch,
                    frames.client_frame,
                ) {
                    self.native_display.fail_shell(error);
                }
            }
            Err(error) => self
                .native_display
                .fail_shell(format!("Client prefix failed: {error}")),
        }
    }
}
