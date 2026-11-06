use super::*;

impl CnCGameEngine {
    /// Wave 602: via `host_route_shell_owned_screen_change`.
    pub(in crate::cnc_game_engine) fn route_shell_owned_screen_change(
        &mut self,
        screen: Screen,
    ) -> Result<(), String> {
        // Wave 602: thin wrapper — shell screen route via host helper.
        self.host_route_shell_owned_screen_change(screen)
    }

    /// Wave 602: host shell-owned screen route residual (MainMenu/Options/Credits/
    /// LoadGame/Skirmish WND push path).
    pub(in crate::cnc_game_engine) fn host_route_shell_owned_screen_change(
        &mut self,
        screen: Screen,
    ) -> Result<(), String> {
        // Wave 602: host shell screen route residual.
        match screen {
            Screen::MainMenu => self.enter_shell_menu_from_runtime_host(None)?,
            Screen::Options => self.enter_shell_options_from_runtime_host()?,
            Screen::Credits => {
                self.enter_shell_screen_from_runtime_host(Some("Credits"), "Menus/CreditsMenu.wnd")?
            }
            Screen::LoadGame => {
                self.enter_shell_screen_from_runtime_host(Some("LoadGame"), "Menus/SaveLoad.wnd")?
            }
            Screen::Skirmish => self.enter_shell_screen_from_runtime_host(
                Some("Skirmish"),
                "Menus/SkirmishGameOptionsMenu.wnd",
            )?,
            _ => {}
        }
        Ok(())
    }

    pub(in crate::cnc_game_engine) fn apply_pending_script_camera_requests(&mut self) {
        // Prefer presentation-frozen camera residual when a frame is installed (InGame).
        // Live take_* path is boot/menu residual only.
        // Wave 572: freeze vs boot split via helpers.
        if let Some(pres) = self.last_presentation_frame.clone() {
            self.apply_presentation_camera_residual(&pres);
            // Legacy compatibility hook; exact requests were acknowledged above.
            self.drain_live_camera_request_queues();
            return;
        }
        // Wave 572: boot residual camera via helper (no presentation freeze).
        self.apply_boot_camera_residual();
    }

    /// No completed presentation exists in boot. A follow lock is admitted
    /// by the next receiving View update, never synthesized as a LookAt here.
    pub(in crate::cnc_game_engine) fn apply_boot_camera_residual(&mut self) {}

    /// Play presentation-frozen script/radar movies (C++ script display residual).
    /// Drains live pending movie queues after apply. Fail-closed: not full BINK parity.
    /// Wave 567: pairs with `apply_boot_movie_residual` for freeze/boot split.
    pub(in crate::cnc_game_engine) fn apply_presentation_movie_residual(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        #[cfg(feature = "game_client")]
        {
            if let Some(ref name) = pres.pending_movie {
                let started = game_client::core::script_action_handler::play_script_display_movie(
                    self.game_client.script_display_receiver(),
                    name,
                );
                if !started {
                    log::trace!("presentation movie play deferred/failed: {name}");
                }
            }
            if let Some(ref name) = pres.pending_radar_movie {
                // Radar movies use InGameUI path when available.
                let started = self.game_client.play_radar_movie(name);
                if !started {
                    let _ = game_client::core::script_action_handler::play_script_display_movie(
                        self.game_client.script_display_receiver(),
                        name,
                    );
                }
            }
        }
        // Wave 900: no live pending-movie drain dual-read after presentation apply.
    }

    /// Wave 567: boot residual movies when no presentation freeze is installed.
    /// Presentation path uses `apply_presentation_movie_residual` (peek freeze + drain).
    /// Fail-closed: not full BINK parity / playable_claim.

    /// Wave 571: presentation popup/music residual — apply freeze fields then drain live queues.
    /// Callers should follow with `apply_presentation_movie_residual`.
    pub(in crate::cnc_game_engine) fn apply_presentation_popup_music_residual(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        // Wave 571: C++ InGameUI has one active popup layout. Only its final frozen
        // record may own Main's pause; older historical requests must not
        // repeatedly re-pause an acknowledged dialog.
        if let Some(popup) = pres.pending_popup_messages.last() {
            self.host_reconcile_active_popup_pause(Some(popup.pause));
            if popup.pause_music {
                if let Some(sink) = self.background_music.take() {
                    sink.stop();
                }
            }
        } else {
            self.host_reconcile_active_popup_pause(None);
        }
        if pres.pending_music_stop {
            if let Some(sink) = self.background_music.take() {
                sink.stop();
            }
        }
        // Wave 900: no live popup/music drain dual-read after presentation apply.
    }

    /// Wave 571: boot residual popup/music — live take only (no presentation freeze).
    /// Callers should follow with `apply_boot_movie_residual`.
    pub(in crate::cnc_game_engine) fn apply_boot_popup_music_residual(&mut self) {
        // Wave 571/899: fail-closed no-op (no popup/music take dual-read).
        // InGame uses apply_presentation_popup_music_residual when freeze is installed.
    }

    pub(in crate::cnc_game_engine) fn apply_boot_movie_residual(&mut self) {
        // Wave 567/899: fail-closed no-op (no take_pending_* dual-read).
        // InGame uses apply_presentation_movie_residual when freeze is installed.
    }

    /// Apply camera residual frozen on `PresentationFrame` (no live take dual-read).
    /// Wave 572: pairs with `apply_boot_camera_residual` for freeze/boot split.
    pub(in crate::cnc_game_engine) fn apply_presentation_camera_residual(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        let admitted = self.game_logic.acknowledge_frozen_camera_requests(pres);
        // Focus and scalar commands already mutated this session synchronously
        // in authored action order. This transport acknowledges only; replaying
        // a frozen request would restart an axis or move a completed path twice.
        let _ = self.look_at_modes.take_scripted_camera_player_cancel();
        self.rebuild_tactical_projection();

        // Slave mode already changed synchronously on the receiving camera.
        // Frozen request generations acknowledge transport, never replay state.

        for intensity in pres.screen_shakes.iter().take(admitted.screen_shakes) {
            self.enqueue_script_screen_shake(*intensity);
        }

        for &(position, amplitude, duration_seconds, radius) in
            pres.camera_shakers.iter().take(admitted.shakers)
        {
            self.enqueue_script_camera_shaker(CameraAddShakerRequest {
                position: Vec3::new(position[0], position[1], position[2]),
                amplitude,
                duration_seconds,
                radius,
            });
        }
    }

    pub(in crate::cnc_game_engine) fn apply_camera_follow_or_tether(
        &mut self,
        observed_id: Option<crate::game_logic::ObjectId>,
        facts: Option<crate::presentation_frame::follow::FollowTargetFacts>,
        paused: bool,
    ) -> Result<bool, String> {
        use crate::presentation_frame::follow::FollowAdmission;
        let result = self
            .game_logic
            .admit_receiving_camera_follow_with_policy(observed_id, facts, paused, None)
            .map_err(|error| error.to_string())?;
        match result {
            FollowAdmission::TargetGone => {
                self.host_set_camera_follow_object(None);
                Ok(false)
            }
            FollowAdmission::Followed => Ok(true),
            _ => Ok(false),
        }
    }

    /// Compatibility hook retained for historical source mapping. Request admission
    /// now occurs atomically inside apply_presentation_camera_residual.
    /// Wave 596: via `host_drain_live_camera_request_queues`.
    pub(in crate::cnc_game_engine) fn drain_live_camera_request_queues(&mut self) {
        // Wave 596: thin wrapper — takes live behind host_drain helper.
        self.host_drain_live_camera_request_queues();
    }

    /// Wave 596: host camera request queue drain residual.
    ///
    /// Exact matching requests have already been acknowledged before their
    /// effects. This compatibility hook must not discard newer queued values.
    pub(in crate::cnc_game_engine) fn host_drain_live_camera_request_queues(&mut self) {
        // Wave 596/865/899: presentation owns camera residual; boot path no longer
        // dual-reads take_* queues (fail-closed no-op). Live queues are not drained
        // via GameLogic observe path.
        let _ = self.last_presentation_frame.as_ref();
    }

    /// Wave 601: via `host_restart_mission_from_ui`.
    pub(in crate::cnc_game_engine) fn restart_mission_from_ui(&mut self) -> Result<(), String> {
        // Wave 601: thin wrapper — restart via host helper.
        self.host_restart_mission_from_ui()
    }

    /// Wave 605: Menu-state client residual.
    ///
    /// Shell map tick + script FPS, menu commands, script camera, camera, slow-tick
    /// diagnostics, shell prewarm logs, MainMenu UI projection, and GameClient menu
    /// shell / NewGame drain. Returns `true` when a NewGame start consumed the frame.
    pub(in crate::cnc_game_engine) fn host_tick_menu_client_residuals(
        &mut self,
        visual_dt: f32,
        dt: f32,
    ) -> std::result::Result<bool, String> {
        if matches!(self.startup_load_state, StartupLoadState::InProgress { .. }) {
            if let Err(err) = self.update_startup_loading() {
                warn!("Menu late startup load failed: {err}");
            }
            if self.current_state != GameState::Menu {
                return Ok(false);
            }
        }
        // Wave 605: Menu client residual.
        self.cleanup_sound_effects();
        let menu_tick_started = Instant::now();
        let shell_update_started = Instant::now();
        // Wave 552: menu residual via presentation_affirms_shell_or_boot
        // (stale InGame freeze does not suppress shell ticks).
        let in_shell = self.presentation_affirms_shell_or_boot();
        if in_shell && !self.game_paused {
            self.host_update_shell_with_budget(dt, 1)?;
            self.apply_shell_script_fps_limit_residual();
        }
        let shell_elapsed = shell_update_started.elapsed();

        // C++ shell/menu parity: menu-frame script camera requests must still drive
        // the shell-map viewport even when not in InGame state.
        let process_commands_started = Instant::now();
        // Wave 582: shell/menu command residual via helper.
        self.host_process_shell_menu_commands();
        let process_commands_elapsed = process_commands_started.elapsed();
        let script_camera_started = Instant::now();
        self.apply_pending_script_camera_requests();
        let script_camera_elapsed = script_camera_started.elapsed();
        let camera_started = Instant::now();
        self.update_camera(visual_dt);
        let camera_elapsed = camera_started.elapsed();

        let menu_tick_elapsed = menu_tick_started.elapsed();
        if menu_tick_elapsed >= Duration::from_millis(40)
            && self
                .last_slow_menu_tick_log
                .map(|last| last.elapsed() >= Duration::from_secs(2))
                .unwrap_or(true)
        {
            // Wave 564: fixed-step residual prefers presentation freeze.
            let (fixed_steps, budget_hit, acc_s) =
                self.presentation_or_boot_fixed_step_diagnostics();
            warn!(
                "Slow menu tick: total={:?}, shell={:?}, commands={:?}, script_camera={:?}, camera={:?}, state={:?}, frame={}, fixed_steps={}, budget_hit={}, acc_ms={:.2}",
                menu_tick_elapsed,
                shell_elapsed,
                process_commands_elapsed,
                script_camera_elapsed,
                camera_elapsed,
                self.current_state,
                self.frame_counter,
                fixed_steps,
                budget_hit,
                acc_s * 1000.0
            );
            self.last_slow_menu_tick_log = Some(Instant::now());
        }

        if !self.pending_shell_model_prewarm.is_empty()
            && self
                .last_shell_prewarm_log
                .map(|last| last.elapsed() >= Duration::from_millis(2_000))
                .unwrap_or(true)
        {
            let missing_models = self.render_pipeline.debug_last_model_missing();
            let missing_samples = self.render_pipeline.debug_last_missing_model_samples();
            debug!(
                "Shell prewarm progress: pending_models={} render_items={} missing_models={} budget_skips={}",
                self.pending_shell_model_prewarm.len(),
                self.render_pipeline.debug_render_item_count(),
                missing_models,
                self.render_pipeline.debug_last_model_budget_skips()
            );
            if missing_models > 0 && !missing_samples.is_empty() {
                debug!(
                    "Shell prewarm missing model samples: {}",
                    missing_samples.join(", ")
                );
            }
            self.last_shell_prewarm_log = Some(Instant::now());
        }

        self.set_runtime_ui_state_projection(Screen::MainMenu);

        #[cfg(feature = "game_client")]
        {
            // Wave 588: Menu GameClient shell + NewGame drain residual via helper.
            if self.host_tick_game_client_menu_shell()? {
                return Ok(true);
            }
        }
        // Headless / no-game_client builds still drain NewGame if present.
        #[cfg(not(feature = "game_client"))]
        if let Some(request) = self.take_pending_new_game_start_request() {
            if matches!(request.mode, GameMode::Shell) {
                info!("Menu NewGame drain: ignore GAME_SHELL (shell map already live)");
                Self::take_shell_new_game_messages_from_common_stream(
                    self.game_client.message_stream_mut(),
                );
                return Ok(false);
            }
            self.start_game_from_ui(request)?;
            let _ = Self::take_new_game_dispatch_from_common_stream(
                self.game_client.message_stream_mut(),
            );
            return Ok(true);
        }
        Ok(false)
    }

    /// Wave 604: Loading-state client residual.
    ///
    /// Runs startup loading progress, then projects Loading UI when still in
    /// Loading after the tick (load may transition away mid-call).
    pub(in crate::cnc_game_engine) fn host_tick_loading_client_residuals(&mut self) -> Result<()> {
        // Wave 604: loading client residual.
        // In loading: minimal updates, mainly for loading screen animations
        // C++ GameClient.cpp:560-565 — snow + Anim2D in every client state.
        self.host_update_cpp_snow_and_anim2d();
        self.update_startup_loading()?;
        if self.current_state != GameState::Loading {
            // Loading completed and transitioned this frame; avoid re-applying loading UI.
            return Ok(());
        }
        // Finish a parked UI start after status has published `state=Loading`.
        // Must run before boot Loading→Menu release so a live match start is
        // not discarded. Still calls `host_load_map_or_default` (does not skip).
        if let Some(mut pending) = self.pending_match_start.take() {
            #[cfg(feature = "game_client")]
            if !self
                .advance_cpp_load_screen_prelude(&mut pending)
                .map_err(anyhow::Error::msg)?
            {
                self.pending_match_start = Some(pending);
                return Ok(());
            }
            info!(
                "Loading parked match start: mode={:?} map={}",
                pending.request.mode, pending.request.map
            );
            self.complete_parked_match_start(pending)
                .map_err(anyhow::Error::msg)?;
            return Ok(());
        }
        if self
            .game_client
            .message_stream()
            .get_messages()
            .iter()
            .any(|message| message.skirmish_launch().is_some())
        {
            self.pump_native_client_commands()
                .map_err(anyhow::Error::msg)?;
            return Ok(());
        }
        // NewGame / start-new-game must drain on the load screen, not only Menu.
        // Windowed sit-through was stuck in Loading because WND/start posted
        // NewGame while boot load was still InProgress and Menu never ticked.
        if let Some(request) = self.take_pending_new_game_start_request() {
            if matches!(request.mode, GameMode::Shell) {
                // C++ MSG_NEW_GAME GAME_SHELL is the shell map. Do not start a
                // match and do not return — fall through so the worker can
                // release Loading → Menu (hq-akj0).
                info!("Loading NewGame drain: ignore GAME_SHELL (shell map, not a match)");
                Self::take_shell_new_game_messages_from_common_stream(
                    self.game_client.message_stream_mut(),
                );
            } else {
                info!(
                    "Loading NewGame drain: mode={:?} faction={} map={}",
                    request.mode, request.faction, request.map
                );
                self.start_game_from_ui(request)
                    .map_err(anyhow::Error::msg)?;
                let _ = Self::take_new_game_dispatch_from_common_stream(
                    self.game_client.message_stream_mut(),
                );
                return Ok(());
            }
        }

        // Boot worker (INI + ShellMapMD) must not pin Loading forever, but
        // C++ showShellMap still finishes GAME_SHELL. Do not abandon: keep
        // the receiver so Menu can apply a late Complete (hq-akj0).
        if self.startup_load_should_release_to_menu() {
            info!(
                "Startup load still in progress; releasing Loading → Menu, keeping worker for shell map"
            );
            self.update_shell_loading_progress(1.0, Some("Startup complete"))
                .map_err(anyhow::Error::msg)?;
            self.startup_last_reported_progress = 1.0;
            self.apply_shell_menu_window_chrome();
            self.transition_to_state(GameState::Menu)
                .map_err(anyhow::Error::msg)?;
            return Ok(());
        }
        self.set_runtime_ui_state_projection(Screen::Loading);
        // After loading completes, the state will transition to InGame
        // This is handled by the initialization code setting pending_state
        Ok(())
    }

    /// Wave 603: paused-state client residual (camera/audio/UI; no logic tick).
    pub(in crate::cnc_game_engine) fn host_tick_paused_client_residuals(
        &mut self,
        visual_dt: f32,
        dt: f32,
    ) {
        // Wave 603: paused client residual.
        // In paused: update UI and camera, but not game logic
        // (matches C++ where TheGameLogic->isGamePaused() prevents update)
        // C++ GameClient.cpp:560-565 — snow + Anim2D still run while paused.
        self.host_update_cpp_snow_and_anim2d();
        self.update_camera(visual_dt);
        self.cleanup_sound_effects();
        self.set_runtime_ui_state_projection(Screen::PauseMenu);
        if let Err(err) = self.ui_manager.update(dt) {
            warn!("UI manager update failed in paused state: {}", err);
        }
    }

    /// Wave 603: endgame client residual (Victory/Defeat score screen).
    pub(in crate::cnc_game_engine) fn host_tick_endgame_client_residuals(
        &mut self,
        visual_dt: f32,
        dt: f32,
    ) {
        // Wave 603: endgame client residual.
        // End-of-match screen: keep UI alive, game logic frozen.
        // C++ shows the score screen then transitions to Menu on user input.
        self.host_update_cpp_snow_and_anim2d();
        self.update_camera(visual_dt);
        self.cleanup_sound_effects();
        if let Err(err) = self.ui_manager.update(dt) {
            warn!("UI manager update failed in endgame state: {}", err);
        }
    }

    /// Wave 602: InGame logic frame residual (host tick + shadow →
    /// presentation finalize → client presentation shell).
    ///
    /// Couples GameWorld shadow when live, advances host logic (with retail FF /
    /// headless budget), then runs the post-logic shadow session and
    /// presentation finalize helpers.
    pub(in crate::cnc_game_engine) fn host_run_ingame_logic_presentation_frame(
        &mut self,
        dt: f32,
    ) -> Result<(), String> {
        // Wave 602: InGame logic+presentation residual.
        // Retail m_TiVOFastMode residual: extra logic steps while armed.
        let ff_steps = replay_logic_step_count(self.replay_fast_forward);
        // Windowed passes None: one C++-equivalent logic step per presented
        // frame, dropping any excess backlog after a stall.
        // Some(1) or Some(2) keep the carry, so every later frame runs that
        // many steps and the match stays in slow motion. Headless keeps Some(4).
        let step_budget = if self.runtime_host_headless {
            Some(4usize)
        } else {
            None
        };
        // A golden capture stops logic exactly on its freeze frame.
        let step_budget = match self.golden_remaining_logic_steps() {
            Some(remaining) => Some(step_budget.unwrap_or(1).min(remaining)),
            None => step_budget,
        };
        // The observer only copies completed Main state at each fixed step.
        let mut observer_slot = self.gameworld_shadow.take();
        let result =
            self.host_run_fast_forward_loop(dt, ff_steps, step_budget, observer_slot.as_mut());
        // Restore the real observer owner on success and failure.
        self.gameworld_shadow = observer_slot.take();
        result?;
        // Script FPS applied from presentation residual after snapshot build (below).
        // Live take remains for boot path when no frame is produced this tick.

        // Wave 916: AuthorityOnly is the only policy — no dual crate tick runs here.
        // C++ parity: when script time-freeze is active, gameplay simulation should not
        // advance outside script evaluation.
        // Host side systems (projectiles) run *before* shadow session + PresentationFrame
        // so damage logs and end-of-frame identity include this frame's side systems.
        // Path following already ran inside GameLogic::update_movement.
        // Projectiles + path following are owned by GameLogic::update_simulation
        // (update_combat drain/step + update_movement). Engine must not run a
        // second mid-frame CombatSystem/PathfindingSystem mover.
        // Wave 250: prefer presentation freeze residual when a frame is installed.
        let time_frozen = self.presentation_or_boot_time_frozen();
        if !time_frozen {
            // Hit SFX residual: prefer presentation audio events; legacy direct
            // Hit playback removed with dual CombatSystem step.
            let _ = dt;
        }

        // The scoped couple publication ended with the fast-forward batch
        // above; release coupled-frame depth before the presentation shell.

        // Wave 589: presentation finalize residual via helper (build + audio + FX).
        self.host_finalize_presentation_after_logic()?;

        #[cfg(feature = "game_client")]
        {
            // Wave 586: GameClient presentation shell residual via helper.
            self.host_tick_game_client_presentation_shell();
        }
        Ok(())
    }

    /// Wave 601: host restart-mission residual.
    ///
    /// Presentation freeze owns map/faction when installed; boot residual uses
    /// host probes. Starts a new match through `start_game_from_ui`.
    /// Wave 601: host restart-mission residual.
    ///
    /// C++ `restartMissionMenu` (QuitMenu.cpp:211-216) re-posts MSG_NEW_GAME
    /// with gameMode / difficulty / rankPoints. Keep the last Challenge
    /// PlayerTemplate instead of `without_player_template`.
    pub(in crate::cnc_game_engine) fn host_restart_mission_from_ui(
        &mut self,
    ) -> Result<(), String> {
        if let Some(identity) = self.last_new_game_identity() {
            info!(
                "UI requested restart: mode={:?} difficulty={} rank={} faction={} map={}",
                identity.dispatch.game_mode,
                identity.dispatch.difficulty_code,
                identity.dispatch.rank_points,
                identity.faction,
                identity.map
            );
            self.host_restart_mission_from_dispatch(identity.dispatch)?;
            return Ok(());
        }
        let map = self.presentation_or_boot_map_name();
        let mode = self.presentation_or_live_game_mode();
        let faction = if let Some(pres) = self.last_presentation_frame.as_ref() {
            pres.local_team.get_name().to_string()
        } else {
            self.ui_local_player_team_name()
                .unwrap_or_else(|| "USA".to_string())
        };
        info!(
            "UI requested restart without prior NewGame identity: mode={:?}, faction={}, map={}",
            mode, faction, map
        );
        self.start_game_from_ui(HostStartRequest::without_player_template(
            mode, faction, map, None,
        ))?;
        Ok(())
    }
}
