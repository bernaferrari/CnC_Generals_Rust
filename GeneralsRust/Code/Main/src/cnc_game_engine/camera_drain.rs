#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

const REPLAY_FAST_FORWARD_LOGIC_STEPS: usize = 4;

#[inline]
fn replay_logic_step_count(replay_fast_forward: bool) -> usize {
    if replay_fast_forward {
        REPLAY_FAST_FORWARD_LOGIC_STEPS
    } else {
        1
    }
}

/// C++ SuperweaponInfo name drawn in the owning player's color
/// (InGameUI.cpp:245 / 3505). Presentation already stamps enemy/ally + `#RRGGBB`
/// on non-local rows; fill color from the frozen roster when missing.
fn superweapon_timer_strip_name(
    pres: &crate::presentation_frame::PresentationFrame,
    timer: &crate::presentation_frame::PresentationSuperweaponTimer,
) -> String {
    if timer.name.contains(" (Enemy")
        || timer.name.contains(" (Ally")
        || timer.name.contains(" (Neutral")
        || timer.name.contains('#')
    {
        return timer.name.clone();
    }
    let owner_id = match timer.power_key.rsplit_once('#') {
        Some((key, id)) if !key.is_empty() => id.parse::<u32>().unwrap_or(pres.local_player_id),
        _ => pres.local_player_id,
    };
    if owner_id == pres.local_player_id {
        return timer.name.clone();
    }
    let owner = pres.players.iter().find(|p| p.id == owner_id);
    let color = owner.map(|p| p.color_rgb).unwrap_or((200, 200, 200));
    let local_alliance = pres
        .players
        .iter()
        .find(|lp| lp.id == pres.local_player_id)
        .map(|lp| lp.alliance_team)
        .unwrap_or(-1);
    let rel = owner
        .map(|p| {
            if p.alliance_team >= 0 && local_alliance >= 0 && local_alliance == p.alliance_team {
                "Ally"
            } else if p.alliance_team >= 0 && local_alliance >= 0 {
                "Enemy"
            } else {
                "Neutral"
            }
        })
        .unwrap_or("Neutral");
    let (r, g, b) = color;
    format!("{} ({rel} #{r:02X}{g:02X}{b:02X})", timer.name)
}

/// C++ InGameUI.cpp:3644-3650 `time.format(L"%d:%2.2d", min, sec)` even when ready (0:00).
fn superweapon_countdown_text(remaining: f32) -> String {
    let secs = remaining.max(0.0) as u32;
    format!("{}:{:02}", secs / 60, secs % 60)
}

impl CnCGameEngine {
    /// Wave 602: via `host_route_shell_owned_screen_change`.
    pub(super) fn route_shell_owned_screen_change(&mut self, screen: Screen) {
        // Wave 602: thin wrapper — shell screen route via host helper.
        self.host_route_shell_owned_screen_change(screen);
    }

    /// Wave 602: host shell-owned screen route residual (MainMenu/Options/Credits/
    /// LoadGame/Skirmish WND push path).
    pub(super) fn host_route_shell_owned_screen_change(&mut self, screen: Screen) {
        // Wave 602: host shell screen route residual.
        match screen {
            Screen::MainMenu => self.enter_shell_menu_from_runtime_host(None),
            Screen::Options => self.enter_shell_options_from_runtime_host(),
            Screen::Credits => {
                self.enter_shell_screen_from_runtime_host(Some("Credits"), "Menus/CreditsMenu.wnd")
            }
            Screen::LoadGame => {
                self.enter_shell_screen_from_runtime_host(Some("LoadGame"), "Menus/SaveLoad.wnd")
            }
            Screen::Skirmish => self.enter_shell_screen_from_runtime_host(
                Some("Skirmish"),
                "Menus/SkirmishGameOptionsMenu.wnd",
            ),
            _ => {}
        }
    }

    pub(super) fn apply_pending_script_camera_requests(&mut self) {
        // Prefer presentation-frozen camera residual when a frame is installed (InGame).
        // Live take_* path is boot/menu residual only.
        // Wave 572: freeze vs boot split via helpers.
        if let Some(pres) = self.last_presentation_frame.clone() {
            self.apply_presentation_camera_residual(&pres);
            // Drain live queues so peeked presentation fields are not re-applied next frame.
            self.drain_live_camera_request_queues();
            return;
        }
        // Wave 572: boot residual camera via helper (no presentation freeze).
        self.apply_boot_camera_residual();
    }

    /// Wave 572: boot residual camera — live take_* dual-reads when no presentation freeze.
    /// Presentation path uses `apply_presentation_camera_residual` + drain.
    pub(super) fn apply_boot_camera_residual(&mut self) {
        // Wave 572/899: boot residual from host_match only (no live take_* dual-read).
        // InGame uses apply_presentation_camera_residual when freeze is installed.
        if let Some(focus) = self.host_match_camera_follow_position.map(glam::Vec3::from) {
            self.center_camera_on(focus);
        }
    }

    /// Play presentation-frozen script/radar movies (C++ script display residual).
    /// Drains live pending movie queues after apply. Fail-closed: not full BINK parity.
    /// Wave 567: pairs with `apply_boot_movie_residual` for freeze/boot split.
    pub(super) fn apply_presentation_movie_residual(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        #[cfg(feature = "game_client")]
        {
            if let Some(ref name) = pres.pending_movie {
                let started =
                    game_client::core::script_action_handler::play_script_display_movie(name);
                if !started {
                    log::trace!("presentation movie play deferred/failed: {name}");
                }
            }
            if let Some(ref name) = pres.pending_radar_movie {
                // Radar movies use InGameUI path when available.
                let started = game_client::helpers::TheInGameUI::play_movie(name);
                if !started {
                    let _ =
                        game_client::core::script_action_handler::play_script_display_movie(name);
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
    pub(super) fn apply_presentation_popup_music_residual(
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
    pub(super) fn apply_boot_popup_music_residual(&mut self) {
        // Wave 571/899: fail-closed no-op (no popup/music take dual-read).
        // InGame uses apply_presentation_popup_music_residual when freeze is installed.
    }

    pub(super) fn apply_boot_movie_residual(&mut self) {
        // Wave 567/899: fail-closed no-op (no take_pending_* dual-read).
        // InGame uses apply_presentation_movie_residual when freeze is installed.
    }

    /// Apply camera residual frozen on `PresentationFrame` (no live take dual-read).
    /// Wave 572: pairs with `apply_boot_camera_residual` for freeze/boot split.
    pub(super) fn apply_presentation_camera_residual(
        &mut self,
        pres: &crate::presentation_frame::PresentationFrame,
    ) {
        let player_cancel = super::mouse::take_scripted_camera_player_cancel();
        if let Some(focus) = pres.camera_focus {
            if !player_cancel.cancels_move() {
                self.center_camera_on(Vec3::new(focus[0], focus[1], focus[2]));
            }
        }

        // C++ ControlBar setDefault/setLow writes TheTacticalView->setHeight.
        // Rebuild the live frustum so Default bar is the retail 80% rect.
        self.rebuild_tactical_projection();

        // Wave 216: presentation-frozen follow only (no live camera_follow dual-read residual).
        // hq-1kgx8: player scroll breaks LOCK_FOLLOW; skip until the lock is released.
        if self.look_at_player_broke_camera_lock() {
            if pres.camera_follow_position.is_none() {
                self.look_at_clear_player_broke_camera_lock();
            }
        } else if let Some(follow) = pres.camera_follow_position {
            let target = Vec3::new(follow[0], follow[1], follow[2]);
            self.apply_camera_follow_or_tether(target, pres.camera_tether_play);
        } else {
            self.camera_follow_factor = -1.0;
        }

        if pres.camera_zoom_reset {
            // C++ W3DView::resetCamera: zoom/pitch/heading animate over script duration.
            let duration = pres.camera_zoom_reset_duration.max(0.0);
            let (ease_in, ease_out) = pres.camera_zoom_reset_ease;
            if !player_cancel.cancels_set() {
                let zoom = self.compute_default_camera_zoom_for_target(
                    self.camera_target,
                    self.ui_script_default_camera_max_height(),
                );
                if duration <= 0.0 {
                    self.camera_zoom = zoom;
                    self.camera_zoom_target = None;
                    self.camera_zoom_start = self.camera_zoom;
                    self.camera_zoom_duration = 0.0;
                    self.camera_zoom_elapsed = 0.0;
                    self.camera_zoom_ease_in = 0.0;
                    self.camera_zoom_ease_out = 0.0;
                } else {
                    self.camera_zoom_start = self.camera_zoom;
                    self.camera_zoom_target = Some(zoom);
                    self.camera_zoom_duration = duration;
                    self.camera_zoom_elapsed = 0.0;
                    self.camera_zoom_ease_in = ease_in;
                    self.camera_zoom_ease_out = ease_out;
                }
                self.apply_script_camera_pitch_request(CameraPitchRequest {
                    // C++ pitchCamera(1.0, milliseconds, easeIn, easeOut)
                    pitch: 1.0,
                    duration_seconds: duration,
                    ease_in_seconds: ease_in,
                    ease_out_seconds: ease_out,
                });
            }
            // C++ m_mcwpInfo.cameraAngle[2] = 0.0 — animate heading to default.
            if !player_cancel.cancels_rotate() {
                let target_yaw = 0.0;
                if duration <= 0.0 {
                    self.camera_yaw_radians = target_yaw;
                    self.camera_yaw_target = None;
                    self.camera_yaw_start = self.camera_yaw_radians;
                    self.camera_yaw_duration = 0.0;
                    self.camera_yaw_elapsed = 0.0;
                    self.camera_yaw_ease_in = 0.0;
                    self.camera_yaw_ease_out = 0.0;
                    self.apply_camera_orbit_transform();
                } else {
                    self.camera_yaw_start = self.camera_yaw_radians;
                    self.camera_yaw_target = Some(target_yaw);
                    self.camera_yaw_duration = duration;
                    self.camera_yaw_elapsed = 0.0;
                    self.camera_yaw_ease_in = ease_in;
                    self.camera_yaw_ease_out = ease_out;
                }
            }
        }

        if let Some((zoom, duration_seconds)) = pres.camera_zoom {
            if !player_cancel.cancels_set() {
                let (ease_in, ease_out) = pres.camera_zoom_ease;
                if duration_seconds <= 0.0 {
                    self.camera_zoom = zoom;
                    self.camera_zoom_target = None;
                    self.camera_zoom_start = self.camera_zoom;
                    self.camera_zoom_duration = 0.0;
                    self.camera_zoom_elapsed = 0.0;
                    self.camera_zoom_ease_in = 0.0;
                    self.camera_zoom_ease_out = 0.0;
                } else {
                    self.camera_zoom_start = self.camera_zoom;
                    self.camera_zoom_target = Some(zoom);
                    self.camera_zoom_duration = duration_seconds;
                    self.camera_zoom_elapsed = 0.0;
                    self.camera_zoom_ease_in = ease_in;
                    self.camera_zoom_ease_out = ease_out;
                }
            }
        }

        if let Some((pitch, duration_seconds)) = pres.camera_pitch {
            if !player_cancel.cancels_set() {
                let (ease_in, ease_out) = pres.camera_pitch_ease;
                self.apply_script_camera_pitch_request(CameraPitchRequest {
                    pitch,
                    duration_seconds,
                    ease_in_seconds: ease_in,
                    ease_out_seconds: ease_out,
                });
            }
        }

        if let Some((rotations, duration_seconds)) = pres.camera_rotate {
            if !player_cancel.cancels_rotate() {
                let (ease_in, ease_out) = pres.camera_rotate_ease;
                self.apply_script_camera_rotate_request(CameraRotateRequest {
                    rotations,
                    duration_seconds,
                    ease_in_seconds: ease_in,
                    ease_out_seconds: ease_out,
                });
            }
        }

        if let Some(look) = pres.camera_look_toward {
            if !player_cancel.cancels_rotate() {
                let (ease_in, ease_out) = pres.camera_look_toward_ease;
                self.apply_camera_look_toward_request(CameraLookTowardWaypointRequest {
                    position: Vec3::new(look[0], look[1], look[2]),
                    duration_seconds: pres.camera_look_toward_duration,
                    ease_in_seconds: ease_in,
                    ease_out_seconds: ease_out,
                    reverse_rotation: pres.camera_look_toward_reverse_rotation,
                });
            }
        }

        if let Some((thing_template_name, bone_name)) = pres.camera_slave_enable.clone() {
            self.camera_slave_mode = Some(CameraSlaveModeRequest {
                thing_template_name,
                bone_name,
            });
        }

        if pres.camera_slave_disable {
            self.camera_slave_mode = None;
        }

        for intensity in &pres.screen_shakes {
            self.enqueue_script_screen_shake(*intensity);
        }

        for &(position, amplitude, duration_seconds, radius) in &pres.camera_shakers {
            self.enqueue_script_camera_shaker(CameraAddShakerRequest {
                position: Vec3::new(position[0], position[1], position[2]),
                amplitude,
                duration_seconds,
                radius,
            });
        }
    }

    fn apply_camera_follow_or_tether(&mut self, target: Vec3, tether_play: Option<f32>) {
        if self.camera_follow_factor < 0.0 {
            self.camera_follow_factor = 0.05;
        } else {
            self.camera_follow_factor = (self.camera_follow_factor + 0.05).min(1.0);
        }

        let current = self.camera_target;
        let dx = target.x - current.x;
        let dz = target.z - current.z;
        let cell = game_engine::common::global_data::read().partition_cell_size;
        let snap_thresh_sqr = cell * cell;
        let cur_dist_sqr = dx * dx + dz * dz;

        let next = if let Some(play) = tether_play {
            if cur_dist_sqr >= snap_thresh_sqr && cur_dist_sqr > 0.0 {
                let ratio = 1.0 - snap_thresh_sqr / cur_dist_sqr;
                Vec3::new(
                    current.x + dx * ratio * 0.5,
                    target.y,
                    current.z + dz * ratio * 0.5,
                )
            } else {
                let ratio = 0.01 * play;
                Vec3::new(current.x + dx * ratio, target.y, current.z + dz * ratio)
            }
        } else {
            Vec3::new(
                current.x + dx * self.camera_follow_factor,
                target.y,
                current.z + dz * self.camera_follow_factor,
            )
        };
        self.center_camera_on(next);
    }

    /// Consume live camera request queues without applying (presentation already applied).
    /// Wave 596: via `host_drain_live_camera_request_queues`.
    pub(super) fn drain_live_camera_request_queues(&mut self) {
        // Wave 596: thin wrapper — takes live behind host_drain helper.
        self.host_drain_live_camera_request_queues();
    }

    /// Wave 596: host camera request queue drain residual.
    ///
    /// When presentation already applied camera residuals, drop live host camera
    /// request queues so the next frame does not double-apply. All takes stay
    /// behind this single host dual-read surface.
    pub(super) fn host_drain_live_camera_request_queues(&mut self) {
        // Wave 596/865/899: presentation owns camera residual; boot path no longer
        // dual-reads take_* queues (fail-closed no-op). Live queues are not drained
        // via GameLogic observe path.
        let _ = self.last_presentation_frame.as_ref();
    }

    /// Wave 601: via `host_restart_mission_from_ui`.
    pub(super) fn restart_mission_from_ui(&mut self) {
        // Wave 601: thin wrapper — restart via host helper.
        self.host_restart_mission_from_ui();
    }

    /// Wave 605: Menu-state client residual.
    ///
    /// Shell map tick + script FPS, menu commands, script camera, camera, slow-tick
    /// diagnostics, shell prewarm logs, MainMenu UI projection, and GameClient menu
    /// shell / NewGame drain. Returns `true` when a NewGame start consumed the frame.
    pub(super) fn host_tick_menu_client_residuals(&mut self, visual_dt: f32, dt: f32) -> bool {
        if matches!(self.startup_load_state, StartupLoadState::InProgress { .. }) {
            if let Err(err) = self.update_startup_loading() {
                warn!("Menu late startup load failed: {err}");
            }
            if self.current_state != GameState::Menu {
                return false;
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
            self.host_update_shell_with_budget(dt, 1);
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

        self.set_runtime_ui_state_projection(UISystemState::MainMenu);

        #[cfg(feature = "game_client")]
        {
            // Wave 588: Menu GameClient shell + NewGame drain residual via helper.
            if self.host_tick_game_client_menu_shell() {
                return true;
            }
        }
        // Headless / no-game_client builds still drain NewGame if present.
        #[cfg(not(feature = "game_client"))]
        if let Some(request) = self.take_pending_new_game_start_request() {
            if matches!(request.mode, GameMode::Shell) {
                info!("Menu NewGame drain: ignore GAME_SHELL (shell map already live)");
                Self::take_shell_new_game_messages_from_common_stream();
                return false;
            }
            self.start_game_from_ui(request);
            let _ = Self::take_new_game_dispatch_from_common_stream();
            return true;
        }
        false
    }

    /// Wave 604: Loading-state client residual.
    ///
    /// Runs startup loading progress, then projects Loading UI when still in
    /// Loading after the tick (load may transition away mid-call).
    pub(super) fn host_tick_loading_client_residuals(&mut self) -> Result<()> {
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
        if let Some(pending) = self.pending_match_start.take() {
            info!(
                "Loading parked match start: mode={:?} map={}",
                pending.request.mode, pending.request.map
            );
            self.complete_parked_match_start(pending);
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
                Self::take_shell_new_game_messages_from_common_stream();
                gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
            } else {
                info!(
                    "Loading NewGame drain: mode={:?} faction={} map={}",
                    request.mode, request.faction, request.map
                );
                self.start_game_from_ui(request);
                let _ = Self::take_new_game_dispatch_from_common_stream();
                gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
                return Ok(());
            }
        }
        if gamelogic::helpers::TheGameLogic::is_start_new_game_requested() {
            if self.host_is_in_shell_game() {
                // Worker/crate start_new_game(GAME_SHELL) arms this flag.
                // build_start_request_from_pending_globals(None) defaults
                // Skirmish+Defcon6 — that is not a match start.
                info!("Loading start_new_game flag drain: ignore leftover GAME_SHELL flag");
                gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
            } else {
                gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
                if let Some(request) = self.build_start_request_from_pending_globals(None) {
                    info!(
                        "Loading start_new_game flag drain: mode={:?} map={}",
                        request.mode, request.map
                    );
                    self.start_game_from_ui(request);
                    return Ok(());
                }
            }
        }
        // Boot worker (INI + ShellMapMD) must not pin Loading forever, but
        // C++ showShellMap still finishes GAME_SHELL. Do not abandon: keep
        // the receiver so Menu can apply a late Complete (hq-akj0).
        if self.startup_load_should_release_to_menu() {
            info!(
                "Startup load still in progress; releasing Loading → Menu, keeping worker for shell map"
            );
            self.update_shell_loading_progress(1.0, Some("Startup complete"));
            self.startup_last_reported_progress = 1.0;
            self.apply_shell_menu_window_chrome();
            self.transition_to_state(GameState::Menu);
            return Ok(());
        }
        self.set_runtime_ui_state_projection(UISystemState::Loading);
        // After loading completes, the state will transition to InGame
        // This is handled by the initialization code setting pending_state
        Ok(())
    }

    /// Wave 603: paused-state client residual (camera/audio/UI; no logic tick).
    pub(super) fn host_tick_paused_client_residuals(&mut self, visual_dt: f32, dt: f32) {
        // Wave 603: paused client residual.
        // In paused: update UI and camera, but not game logic
        // (matches C++ where TheGameLogic->isGamePaused() prevents update)
        // C++ GameClient.cpp:560-565 — snow + Anim2D still run while paused.
        self.host_update_cpp_snow_and_anim2d();
        self.update_camera(visual_dt);
        self.cleanup_sound_effects();
        self.set_runtime_ui_state_projection(UISystemState::PauseMenu);
        if let Err(err) = self.ui_manager.update(dt) {
            warn!("UI manager update failed in paused state: {}", err);
        }
    }

    /// Wave 603: endgame client residual (Victory/Defeat score screen).
    pub(super) fn host_tick_endgame_client_residuals(&mut self, visual_dt: f32, dt: f32) {
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

    /// Wave 602: InGame logic frame residual (host tick + shadow → dual-tick policy →
    /// presentation finalize → client presentation shell).
    ///
    /// Couples GameWorld shadow when live, advances host logic (with retail FF /
    /// headless budget), optionally dual-ticks the ported crate, then runs the
    /// post-logic shadow session and presentation finalize helpers.
    pub(super) fn host_run_ingame_logic_presentation_frame(&mut self, dt: f32) {
        // Wave 602: InGame logic+presentation residual.
        // Retail m_TiVOFastMode residual: extra logic steps while armed.
        let ff_steps = replay_logic_step_count(self.replay_fast_forward);
        // Headless residual: cap catch-up (4 logic frames ≈ 133ms) so a slow
        // present/update path cannot freeze the host control loop for seconds.
        let headless_step_budget = if self.runtime_host_headless {
            Some(4usize)
        } else {
            None
        };
        // Coupled host→shadow frame: sole-tick systems freeze host percent only
        // while this is set AND the engine owns a live GameWorldShadow that will
        // write back after the host tick. Host-only gates / missing shadow leave
        // host construction/production advancing (fail-open).
        // Wave 904: production path is single-authority by default; dual-tick remains
        // opt-in via GENERALS_ALLOW_DUAL_TICK (verification hardens dual-world failures).
        crate::authoritative_world::set_verification_single_authority(
            crate::authoritative_world::verification_single_authority()
                || std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_none(),
        );
        let couple_shadow = self.gameworld_shadow.is_some();
        // Keep both coupled-frame depth and the TLS shadow handle RAII-owned.
        // This is intentionally lexical around host logic plus post-logic
        // writeback: a panic cannot leave a raw shadow handle live into a later
        // frame.  The handle is dropped before presentation, matching the
        // existing authority boundary.
        let coupled_tick_guard =
            couple_shadow.then(crate::gameworld_shadow::CoupledTickGuard::enter);
        let coupled_shadow_guard = if couple_shadow {
            self.gameworld_shadow
                .as_mut()
                .map(crate::gameworld_shadow::install_coupled_shadow_guard)
        } else {
            None
        };
        // Each replay fast-forward iteration offers one host update, but a host
        // update may produce zero, one, or several fixed 30 Hz logic steps.
        // Advance the coupled GameWorld last-writer boundary for the *actual*
        // number of completed logic steps: production, exit-delay,
        // construction, special-power, and movement must not follow render
        // iteration count. Deferring the boundary until after the four offers
        // made these shadow-owned channels run at one quarter speed.
        for _ in 0..ff_steps {
            // Wave 584: host logic tick residual via helper.
            let fixed_steps = self
                .host_update_logic_frame(dt, headless_step_budget)
                .steps_run;
            for _ in 0..fixed_steps {
                // Consume typed Gather/drop-off observation events immediately after
                // each authoritative logic step. The evidence helper rejects passive,
                // untracked, injected, remote, hidden, and non-offline paths.
                self.host_drain_physical_gather_dropoffs();
                // Wave 682/925: post-logic host→GameWorld residual batch under the
                // coupled shadow tick. Single authority boundary replaces N eager
                // apply dual-borrows.
                if couple_shadow {
                    if let Some(ref mut shadow) = self.gameworld_shadow {
                        crate::gameworld_shadow::eager_apply_all_host_residuals_after_logic(
                            shadow,
                            &mut self.game_logic,
                        );
                    }
                }
                // Wave 597: GameWorld shadow session residual. This stays per
                // completed fixed logic step even though presentation is coalesced
                // after the fast-forward batch.
                self.host_run_gameworld_shadow_after_logic(couple_shadow);
            }
        }
        // Script FPS applied from presentation residual after snapshot build (below).
        // Live take remains for boot path when no frame is produced this tick.

        // Single-authority policy: Main GameLogic is the match host by default.
        // Dual-tick of the ported gamelogic crate is opt-in (GENERALS_ALLOW_DUAL_TICK)
        // and is fatal under GENERALS_VERIFY_SINGLE_AUTHORITY verification builds.
        // Wave 916: AuthorityOnly (default) never touches the dual crate tick residual.
        // Dual-tick crate Ok(()) may still be an empty-world no-op; see tick_gamelogic_crate.
        let policy = crate::authoritative_world::dual_tick_policy();
        if !matches!(
            policy,
            crate::authoritative_world::DualTickPolicy::AuthorityOnly
        ) {
            if let Err(e) = crate::authoritative_world::apply_post_authority_crate_tick(
                policy,
                crate::game_logic::tick_gamelogic_crate,
            ) {
                log::error!("{e}");
                // Verification: refuse to continue a dual-world silent failure.
                if crate::authoritative_world::verification_single_authority() {
                    panic!("{e}");
                }
            }
        }
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

        // Keep active-shadow access available through host writeback, then
        // release it before building the immutable presentation frame.
        drop(coupled_shadow_guard);
        drop(coupled_tick_guard);

        // Wave 589: presentation finalize residual via helper (build + audio + FX).
        self.host_finalize_presentation_after_logic();

        #[cfg(feature = "game_client")]
        {
            // Wave 586: GameClient presentation shell residual via helper.
            self.host_tick_game_client_presentation_shell();
        }
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
    pub(super) fn host_restart_mission_from_ui(&mut self) {
        if let Some(identity) = super::dispatch::last_new_game_identity() {
            info!(
                "UI requested restart: mode={:?} difficulty={} rank={} faction={} map={}",
                identity.dispatch.game_mode,
                identity.dispatch.difficulty_code,
                identity.dispatch.rank_points,
                identity.faction,
                identity.map
            );
            self.host_restart_mission_from_dispatch(identity.dispatch);
            return;
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
        ));
    }

    /// Prefer presentation-frozen game mode when a frame is installed.
    /// Wave 609: via `host_presentation_or_live_game_mode`.
    pub(super) fn presentation_or_live_game_mode(&self) -> GameMode {
        // Wave 609: thin wrapper — UI/presentation residual via host helper.
        self.host_presentation_or_live_game_mode()
    }

    /// Prefer presentation-frozen game mode when a frame is installed.
    pub(super) fn host_presentation_or_live_game_mode(&self) -> GameMode {
        // Wave 609/842: host UI/presentation residual helper.
        // Prefer freeze, then host-owned match mode residual, then boot GameLogic.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.game_mode;
        }
        if let Some(mode) = self.host_match_game_mode {
            return mode;
        }
        // Wave 898: fail-closed boot default (menu/shell).
        GameMode::Shell
    }

    /// Wave 550: presentation freeze owns visual speed residual when installed.
    /// Boot residual without freeze uses host GameLogic probe.
    #[inline]

    /// Wave 844: refresh host-owned sim timing residuals from Main GameLogic.
    /// Called after match load and each presentation finalize (stamp only — consumers
    /// prefer freeze, then these residuals, then boot live probes).
    #[inline]

    /// Wave 848/853/857: single stamp-phase object scan for alive-set, local train
    /// producers, and special-power-ready ids. Prefer presentation freeze when
    /// installed (no host dual-read); otherwise one `get_objects` pass fills all
    /// residual families.
    pub(super) fn host_refresh_local_train_producer_residuals(&mut self) {
        let team = self
            .host_match_local_team
            .or_else(|| {
                self.last_presentation_frame
                    .as_ref()
                    .map(|f| f.local_team())
            })
            .unwrap_or(crate::game_logic::Team::USA);
        let mut barracks = Vec::new();
        let mut any = Vec::new();
        let mut unfinished = Vec::new();
        let mut sample = None;
        let mut alive = std::collections::HashSet::new();
        let mut special_ready = std::collections::HashSet::new();

        if let Some(pres) = self.last_presentation_frame.as_ref() {
            for o in &pres.objects {
                if !o.destroyed && o.health_current > 0.0 {
                    alive.insert(o.id.0);
                    if o.special_power_ready {
                        special_ready.insert(o.id.0);
                    }
                }
                if o.team != team || o.destroyed || o.health_current <= 0.0 {
                    continue;
                }
                if sample.is_none() {
                    let p = o.position;
                    sample = Some([p.x, p.y, p.z]);
                }
                let name = o.template_name.to_ascii_lowercase();
                let is_barracks = name.contains("barracks")
                    || o.building_type
                        == Some(crate::presentation_frame::PresentationBuildingType::Barracks);
                let is_producer = o.can_produce
                    || is_barracks
                    || name.contains("warfactory")
                    || name.contains("airfield")
                    || name.contains("helipad");
                if !is_producer {
                    continue;
                }
                if o.under_construction {
                    unfinished.push(o.id);
                    continue;
                }
                if is_barracks {
                    barracks.push(o.id);
                } else {
                    any.push(o.id);
                }
            }
        } else {
            // Wave 853/857/902: cold residual fail-closed (no get_objects dual-read).
            // InGame stamps via presentation freeze path above.
            let _ = team;
        }
        barracks.sort_by_key(|id| id.0);
        any.sort_by_key(|id| id.0);
        unfinished.sort_by_key(|id| id.0);
        self.host_match_local_barracks_ids = Some(barracks);
        self.host_match_local_producer_ids = Some(any);
        self.host_match_local_unfinished_producer_ids = Some(unfinished);
        self.host_match_local_team_sample_pos = sample;
        self.host_match_alive_object_ids = Some(alive);
        self.host_match_special_power_ready_ids = Some(special_ready);
    }

    pub(super) fn host_refresh_match_sim_residuals_from_logic(&mut self) {
        // Wave 893: prefer presentation freeze for sim timing + replay/team when live.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_visual_speed = Some(pres.visual_speed_multiplier);
            self.host_match_time_frozen = Some(pres.time_frozen_for_simulation);
            self.host_match_total_play_time = Some(pres.total_play_time_seconds);
            self.host_match_logic_frame = Some(pres.frame.0);
            self.host_match_logic_steps = Some((
                pres.logic_steps_run,
                pres.logic_steps_budget_hit,
                pres.logic_steps_accumulated_seconds,
            ));
            self.host_match_in_replay = Some(pres.in_replay_game);
            self.host_match_local_player_id = Some(self.current_player_id);
            self.host_match_in_shell = Some(false);
            self.host_match_local_team = Some(pres.local_team);
            // Wave 894: AI difficulty residual from freeze (no get_difficulty dual-read).
            self.host_match_ai_difficulty = Some(pres.ai_difficulty);
        } else {
            // Wave 902/908/909: cold residual after match-start — keep prior tick stamp or
            // fail-closed zeros until presentation seed / next update snapshot. No live probe.
            self.host_match_visual_speed = self.host_match_visual_speed.or(Some(1.0));
            self.host_match_time_frozen = Some(self.game_paused);
            self.host_match_total_play_time = self.host_match_total_play_time.or(Some(0.0));
            self.host_match_logic_frame = self.host_match_logic_frame.or(Some(0));
            self.host_match_logic_steps = self.host_match_logic_steps.or(Some((0, false, 0.0)));
            self.host_match_in_replay = Some(false);
            self.host_match_local_player_id = Some(self.current_player_id);
            self.host_match_in_shell = Some(false);
            self.host_match_local_team = Some(crate::game_logic::Team::USA);
        }
        // Wave 846: diplomacy / template / sciences host residuals.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_diplomacy_players = Some(pres.players.clone());
            self.host_match_known_template_names = Some(pres.known_template_names.clone());
            // Wave 901: sciences residual from freeze only (no player_unlocked dual-read).
            let mut sciences = std::collections::HashMap::new();
            sciences.insert(pres.local_player_id, pres.local_unlocked_sciences.clone());
            for p in &pres.players {
                sciences.entry(p.id).or_insert_with(Vec::new);
            }
            self.host_match_unlocked_sciences = Some(sciences);
        } else {
            // Wave 901: cold residual fail-closed (boot_player_info already residual-only).
            self.host_match_diplomacy_players = Some(Vec::new());
            self.host_match_known_template_names = Some(Vec::new());
            self.host_match_unlocked_sciences = Some(std::collections::HashMap::new());
        }
        // Wave 847: camera-follow host residual (prefer freeze when present).
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_camera_follow_active = Some(pres.camera_follow_position.is_some());
            self.host_match_camera_follow_position = pres.camera_follow_position;
            // Wave 913: freeze without object-id field — clear id residual when follow inactive.
            if pres.camera_follow_position.is_none() {
                self.host_match_camera_follow_id = Some(None);
            }
        } else {
            // Wave 901/913: cold residual fail-closed (no camera_follow dual-read).
            self.host_match_camera_follow_active = Some(false);
            self.host_match_camera_follow_position = None;
            self.host_match_camera_follow_id = Some(None);
        }
        // Wave 848: stamp local train producers after other match residuals.
        self.host_refresh_local_train_producer_residuals();
        // Wave 858: stamp script camera defaults (freeze first, else host).
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_script_camera_max_height = Some(pres.script_default_camera_max_height);
            self.host_match_script_camera_pitch = Some(pres.script_default_camera_pitch);
        } else {
            // Wave 901: cold residual fail-closed camera defaults.
            self.host_match_script_camera_max_height = Some(1.0);
            self.host_match_script_camera_pitch = Some(1.0);
        }
        // Wave 861/901: skirmish host is never multiplayer (no dual-read).
        self.host_match_in_multiplayer = Some(false);
        // Wave 862: stamp world bounds residual (freeze first).
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_world_bounds = Some(pres.world_env.world_bounds_vec3());
        } else if let Some(b) = self.host_match_world_bounds {
            let _ = b; // keep prior stamp
        } else {
            // Wave 901: cold residual fail-closed.
            self.host_match_world_bounds = Some((glam::Vec3::ZERO, glam::Vec3::ZERO));
        }
        // Wave 863/901: stamp first-opponent residual from diplomacy residual only.
        let local = self
            .host_match_local_player_id
            .unwrap_or(self.current_player_id);
        if let Some(players) = self.host_match_diplomacy_players.as_ref() {
            self.host_match_first_opponent_id =
                Some(players.iter().find(|p| p.id != local).map(|p| p.id));
        } else if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_first_opponent_id =
                Some(pres.players.iter().find(|p| p.id != local).map(|p| p.id));
        } else {
            // Wave 901: cold residual fail-closed.
            self.host_match_first_opponent_id = Some(None);
        }
        // Wave 901: removed Wave 855 clear that wiped camera/mp/bounds/opponent stamps.
        // Wave 849: stamp match outcome residuals from freeze (or clear when none).
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_over = Some(pres.match_over);
            self.host_match_victory_label = pres.victory_label.clone();
            self.host_match_victory_summary = pres.victory_summary.clone();
            if pres.match_over {
                self.host_match_victory_winner = Some(pres.victory_winner_id());
            } else {
                self.host_match_victory_winner = None;
            }
        }
        // Wave 850: stamp selection residual (engine first, then freeze).
        if !self.selected_objects.is_empty() {
            self.host_match_selected_ids = Some(self.selected_objects.clone());
        } else if let Some(pres) = self.last_presentation_frame.as_ref() {
            if !pres.selected.is_empty() {
                self.host_match_selected_ids = Some(pres.selected.clone());
            } else {
                let from_objs: Vec<_> = pres
                    .objects
                    .iter()
                    .filter(|o| o.selected && !o.destroyed && o.health_current > 0.0)
                    .map(|o| o.id)
                    .collect();
                self.host_match_selected_ids = Some(from_objs);
            }
        }
        // Wave 851/853: alive residual stamped inside host_refresh_local_train_producer_residuals
        // (single freeze walk or single host get_objects scan).
        // Wave 852/901: purchasable science residual fail-closed (no can_purchase dual-read).
        {
            let mut map = std::collections::HashMap::new();
            if let Some(players) = self.host_match_diplomacy_players.as_ref() {
                for p in players {
                    map.insert(p.id, std::collections::HashSet::new());
                }
            }
            self.host_match_purchasable_sciences = Some(map);
        }
        // Wave 868/901: local science purchase points from freeze only.
        self.host_match_local_science_purchase_points =
            Some(if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.local_science_purchase_points()
            } else {
                0
            });
        // Wave 921: local supplies residual from freeze only.
        self.host_match_local_supplies =
            Some(if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.local_supplies as u32
            } else {
                0
            });
        // Wave 854/857: special-power-ready residual stamped inside
        // host_refresh_local_train_producer_residuals (single freeze/host scan).
    }

    pub(super) fn presentation_or_boot_visual_speed(&self) -> f32 {
        // Wave 550/844: presentation freeze owns visual speed residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.visual_speed_multiplier;
        }
        if let Some(v) = self.host_match_visual_speed {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        1.0
    }

    /// Wave 551: presentation freeze owns time-frozen residual when installed.
    /// Boot residual without freeze uses host GameLogic probe.
    #[inline]
    pub(super) fn presentation_or_boot_time_frozen(&self) -> bool {
        // Wave 551/844: presentation freeze owns time-frozen residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.time_frozen_for_simulation;
        }
        if let Some(v) = self.host_match_time_frozen {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        false
    }

    /// Wave 552: presentation freeze owns shell-bypass residual when installed
    /// (`fow_shell_bypass`, even if false). Boot residual without freeze uses
    /// host `isInShellGame`.
    #[inline]
    pub(super) fn presentation_or_boot_shell_bypass(&self) -> bool {
        // Wave 552/845: presentation freeze owns shell-bypass residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.fow_shell_bypass;
        }
        if let Some(v) = self.host_match_in_shell {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read). Menu shell residual
        // is stamped into host_match_in_shell on match transitions.
        true
    }

    /// Wave 553: presentation freeze owns total play-time residual when installed
    /// (pipeline freeze preferred, then last frame). Boot residual without freeze
    /// uses host `get_total_play_time`.
    #[inline]
    pub(super) fn presentation_or_boot_total_play_time(&self) -> f32 {
        // Wave 553/844: presentation freeze owns total play-time residual when installed.
        if let Some(pres) = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_ref())
        {
            return pres.total_play_time_seconds;
        }
        if let Some(v) = self.host_match_total_play_time {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        0.0
    }

    /// Wave 553: presentation freeze owns local player id residual when installed.
    /// Boot residual without freeze uses host `local_player_id` probe.
    #[inline]

    /// Wave 569: defeat residual — prefer presentation freeze `defeated_player_ids`,
    /// drain live queue when freeze installed; boot residual takes live.

    /// Wave 570: script message residual — prefer pipeline/last presentation freeze
    /// `new_script_messages`, drain live queue when freeze installed; boot residual takes live.
    /// Wave 607: via `host_take_presentation_or_boot_new_script_messages`.
    pub(super) fn take_presentation_or_boot_new_script_messages(&mut self) -> Vec<String> {
        // Wave 607: thin wrapper — presentation/boot drain via host helper.
        self.host_take_presentation_or_boot_new_script_messages()
    }

    /// Wave 570: script message residual — prefer pipeline/last presentation freeze
    /// `new_script_messages`, drain live queue when freeze installed; boot residual takes live.
    pub(super) fn host_take_presentation_or_boot_new_script_messages(&mut self) -> Vec<String> {
        // Wave 607/900: presentation freeze owns script message residual when installed.
        // No live take drain dual-read; boot fail-closed empty.
        if let Some(pres) = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_ref())
        {
            return pres.new_script_messages.clone();
        }
        // Wave 900: fail-closed boot default.
        Vec::new()
    }

    /// Wave 607: via `host_take_presentation_or_boot_defeat_events`.
    pub(super) fn take_presentation_or_boot_defeat_events(&mut self) -> Vec<u32> {
        // Wave 607: thin wrapper — presentation/boot drain via host helper.
        self.host_take_presentation_or_boot_defeat_events()
    }

    pub(super) fn host_take_presentation_or_boot_defeat_events(&mut self) -> Vec<u32> {
        // Wave 607/900: presentation freeze owns defeat residual when installed.
        // No live take drain dual-read; boot fail-closed empty.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.defeated_player_ids.clone();
        }
        // Wave 900: fail-closed boot default.
        Vec::new()
    }

    /// Wave 569: alliance residual — prefer presentation freeze `alliance_events`,
    /// drain live queue when freeze installed; boot residual takes live.
    /// Wave 607: via `host_take_presentation_or_boot_alliance_events`.
    pub(super) fn take_presentation_or_boot_alliance_events(
        &mut self,
    ) -> Vec<crate::game_logic::AllianceNotification> {
        // Wave 607: thin wrapper — presentation/boot drain via host helper.
        self.host_take_presentation_or_boot_alliance_events()
    }

    /// Wave 569: alliance residual — prefer presentation freeze `alliance_events`,
    /// drain live queue when freeze installed; boot residual takes live.
    pub(super) fn host_take_presentation_or_boot_alliance_events(
        &mut self,
    ) -> Vec<crate::game_logic::AllianceNotification> {
        // Wave 607/900: presentation freeze owns alliance residual when installed.
        // No live take drain dual-read; boot fail-closed empty.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.alliance_events.clone();
        }
        // Wave 900: fail-closed boot default.
        Vec::new()
    }

    pub(super) fn presentation_or_boot_local_player_id(&self) -> Option<u32> {
        // Wave 553/843: presentation freeze owns local player id residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return Some(pres.local_player_id);
        }
        if let Some(id) = self.host_match_local_player_id {
            return Some(id);
        }
        // Wave 895: host current_player_id residual (no GameLogic dual-read).
        Some(self.current_player_id)
    }

    /// Wave 554: presentation freeze owns map-name residual when installed
    /// (even if empty). Boot residual without freeze uses host map probe.
    #[inline]
    pub(super) fn presentation_or_boot_map_name(&self) -> String {
        // Wave 554/860: presentation freeze owns map-name residual when installed.
        // Wave 840/843/860: if freeze still holds shell residual after match load, prefer
        // host_match_map_name (no live GameLogic dual-read). Live probe only when residual cold.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            let freeze = pres.world_env.map_name.clone();
            if matches!(
                self.current_state,
                GameState::InGame | GameState::Paused | GameState::Loading
            ) && Self::map_name_is_shell_residual(&freeze)
            {
                if let Some(host) = self.host_match_map_name.as_ref() {
                    if !Self::map_name_is_shell_residual(host) {
                        return host.clone();
                    }
                    // Wave 860: warm residual still shell — fail-closed to residual, no live dual-read.
                    return host.clone();
                }
                // Wave 896: residual cold under InGame shell freeze — fail-closed to freeze
                // (no get_current_map_name dual-read). Match-start stamps host_match_map_name.
            }
            return freeze;
        }
        if let Some(host) = self.host_match_map_name.as_ref() {
            return host.clone();
        }
        // Wave 896: fail-closed boot default (no dual-read).
        String::new()
    }

    /// Wave 554: presentation freeze owns AI difficulty residual when installed.
    /// Boot residual without freeze uses host difficulty probe.
    #[inline]
    pub(super) fn presentation_or_boot_ai_difficulty(&self) -> crate::ai::AIDifficulty {
        // Wave 554/843: presentation freeze owns AI difficulty residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.ai_difficulty;
        }
        if let Some(d) = self.host_match_ai_difficulty {
            return d;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        crate::ai::AIDifficulty::Medium
    }

    /// Wave 557: presentation freeze owns replay-mode residual when installed.
    /// Boot residual without freeze uses host `isInReplayGame`.
    #[inline]
    pub(super) fn presentation_or_boot_in_replay_game(&self) -> bool {
        // Wave 557/844: presentation freeze owns replay-mode residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.in_replay_game;
        }
        if let Some(v) = self.host_match_in_replay {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        false
    }

    /// Wave 558: presentation freeze owns diplomacy roster residual when installed.
    /// Boot residual without freeze uses host player_* probes (no get_players dual-read).
    /// Wave 573: boot roster build via `boot_player_info_from_host`.
    #[inline]
    pub(super) fn presentation_or_boot_diplomacy_players(
        &self,
    ) -> Vec<crate::presentation_frame::PresentationPlayerInfo> {
        // Wave 558/846: presentation freeze owns diplomacy roster residual when installed.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            return frame.players.clone();
        }
        if let Some(players) = self.host_match_diplomacy_players.as_ref() {
            return players.clone();
        }
        // Wave 895: fail-closed boot default (no dual-read). Diplomacy residual is
        // stamped into host_match_diplomacy_players on match sim refresh.
        Vec::new()
    }

    /// Wave 560: presentation freeze owns logic-frame residual when installed.
    /// Boot residual without freeze uses host `get_frame`.
    #[inline]
    pub(super) fn presentation_or_boot_logic_frame(&self) -> u32 {
        // Wave 560/844: presentation freeze owns logic-frame residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.frame.0;
        }
        if let Some(v) = self.host_match_logic_frame {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        0
    }

    /// Wave 561: presentation freeze owns fixed-step catch-up residual when installed.
    /// Boot residual without freeze uses host `fixed_step_diagnostics().steps_run`.
    #[inline]
    pub(super) fn presentation_or_boot_logic_steps(&self) -> u32 {
        self.presentation_or_boot_fixed_step_diagnostics().0
    }

    /// Wave 564: presentation freeze owns full fixed-step diagnostics residual when
    /// installed (`steps_run`, `budget_hit`, `accumulated_time_seconds`). Boot residual
    /// without freeze uses host `fixed_step_diagnostics`.
    #[inline]
    pub(super) fn presentation_or_boot_fixed_step_diagnostics(&self) -> (u32, bool, f32) {
        // Wave 564/844: presentation freeze owns fixed-step diagnostics residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return (
                pres.logic_steps_run,
                pres.logic_steps_budget_hit,
                pres.logic_steps_accumulated_seconds,
            );
        }
        if let Some(v) = self.host_match_logic_steps {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        (0, false, 0.0)
    }

    /// Wave 563/565: presentation freeze owns template-name residual when installed
    /// (train + construct). Boot residual without freeze uses host `templates.contains_key`.
    #[inline]
    pub(super) fn presentation_or_boot_has_template(&self, name: &str) -> bool {
        // Wave 563/846/859: presentation freeze owns template-name residual when installed.
        // Wave 565: construct residual shares this helper with train.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.has_template_name(name);
        }
        // Wave 859: warm host residual is fail-closed (no live dual-read on miss).
        if let Some(names) = self.host_match_known_template_names.as_ref() {
            return names.binary_search(&name.to_string()).is_ok()
                || names.iter().any(|n| n.eq_ignore_ascii_case(name));
        }
        // Wave 895: fail-closed boot default (no dual-read).
        false
    }

    /// Wave 581: ensure GoldenRanger host template residual for train honesty path.
    pub(super) fn host_ensure_golden_ranger_template(&mut self) {
        // Wave 581/722/872/915/934: mid-command host insert residual (callers must opt in).
        // Prefer residual template table when warm (no live dual-read on hit).
        if let Some(names) = self.host_match_known_template_names.as_ref() {
            if names.binary_search(&"GoldenRanger".to_string()).is_ok()
                || names.iter().any(|n| n.eq_ignore_ascii_case("GoldenRanger"))
            {
                return;
            }
        } else if self.last_presentation_frame.is_some() {
            // Wave 915: presentation freeze without residual table — fail-closed skip
            // mid-match template dual-write (boot residual still inserts below).
            return;
        }
        // Wave 900: residual miss / boot → insert host template (or_insert via insert).
        // No contains_key dual-read probe before write.
        let mut tpl = crate::game_logic::ThingTemplate::new("GoldenRanger");
        tpl.set_health(120.0);
        tpl.set_cost(100, 0);
        tpl.build_time = 0.05;
        tpl.add_kind_of(crate::game_logic::KindOf::Infantry);
        tpl.add_kind_of(crate::game_logic::KindOf::Selectable);
        tpl.add_kind_of(crate::game_logic::KindOf::Attackable);
        let _ = self.host_game_logic_mut().apply_host_support_op(
            crate::game_logic::HostSupportOp::InsertThingTemplate {
                name: "GoldenRanger".into(),
                template: tpl,
            },
        );
        self.host_stamp_known_template_name("GoldenRanger");
    }

    /// Wave 872: keep host_match_known_template_names residual warm after inserts.
    #[inline]
    pub(super) fn host_stamp_known_template_name(&mut self, name: &str) {
        let key = name.to_string();
        let entry = self
            .host_match_known_template_names
            .get_or_insert_with(Vec::new);
        if entry.binary_search(&key).is_err() && !entry.iter().any(|n| n.eq_ignore_ascii_case(name))
        {
            match entry.binary_search(&key) {
                Ok(_) => {}
                Err(i) => entry.insert(i, key),
            }
        }
    }

    /// Wave 581: template residual for train/construct mid-command host inserts.
    /// Prefer freeze known names; if freeze misses, allow live host `templates`
    /// (inserts after last PresentationFrame). Boot without freeze uses host only.
    #[inline]
    /// Wave 610: via `host_presentation_or_live_has_template`.
    pub(super) fn presentation_or_live_has_template(&self, name: &str) -> bool {
        // Wave 610: thin wrapper — residual via host helper.
        self.host_presentation_or_live_has_template(name)
    }

    /// Wave 581: template residual for train/construct mid-command host inserts.
    /// Prefer freeze known names; if freeze misses, allow live host `templates`
    /// (inserts after last PresentationFrame). Boot without freeze uses host only.
    #[inline]
    pub(super) fn host_presentation_or_live_has_template(&self, name: &str) -> bool {
        // Wave 610/846/859: host residual helper.
        // Wave 581: freeze known names OR host residual OR live host insert residual.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            if pres.has_template_name(name) {
                return true;
            }
            // Wave 898: freeze lag miss uses host residual only (no dual-read).
            if let Some(names) = self.host_match_known_template_names.as_ref() {
                return names.binary_search(&name.to_string()).is_ok()
                    || names.iter().any(|n| n.eq_ignore_ascii_case(name));
            }
            return false;
        }
        if let Some(names) = self.host_match_known_template_names.as_ref() {
            return names.binary_search(&name.to_string()).is_ok()
                || names.iter().any(|n| n.eq_ignore_ascii_case(name));
        }
        // Wave 898: fail-closed boot default (no dual-read).
        false
    }

    /// C++ GameClient.cpp:560-565 — TheSnowManager + TheAnim2DCollection
    /// UPDATE at the execute/logic cadence, not every 45 Hz present.
    pub(super) fn host_update_cpp_snow_and_anim2d(&mut self) {
        #[cfg(feature = "game_client")]
        {
            let dt = self.snow_anim2d_dt_for_present();
            self.game_client.update_cpp_snow_and_anim2d(dt);
        }
    }

    fn snow_anim2d_dt_for_present(&self) -> f32 {
        let logic_steps = self
            .host_match_logic_steps
            .map(|(steps, _, _)| steps)
            .unwrap_or(0);
        if logic_steps > 0 {
            return game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL
                * logic_steps as f32;
        }
        if matches!(self.current_state, GameState::InGame) {
            return 0.0;
        }
        snow_anim2d_client_only_dt()
    }

    /// Wave 588: Menu GameClient shell tick + NewGame drain residual.
    ///
    /// Advances Main-injected device state, shell/pre/post UI, then drains
    /// MSG_NEW_GAME *before* `pump_message_stream` so WND Start reaches InGame.
    /// Returns `true` when a match start was applied (caller should `return`).
    ///
    /// Distinct from InGame `host_tick_game_client_presentation_shell` (no FOW/pose;
    /// NewGame intercept is Menu-only). Full `GameClient::update` stays disconnected.
    #[cfg(feature = "game_client")]
    pub(super) fn host_tick_game_client_menu_shell(&mut self) -> bool {
        // Peek/start MSG_NEW_GAME before update_input/propagateMessages
        // can destroy it. WND Skirmish Start posts here.
        if let Some(request) = self.take_pending_new_game_start_request() {
            if matches!(request.mode, GameMode::Shell) {
                info!("Menu NewGame drain: ignore GAME_SHELL (shell map already live)");
                Self::take_shell_new_game_messages_from_common_stream();
            } else {
                info!(
                    "Menu NewGame drain: mode={:?} faction={} map={} skirmish={}",
                    request.mode,
                    request.faction,
                    request.map,
                    request.skirmish.is_some()
                );
                self.start_game_from_ui(request);
                let _ = Self::take_new_game_dispatch_from_common_stream();
                gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
                return true;
            }
        }
        let early_menu_frame = self.menu_world_frames_rendered < 5;
        let t0 = std::time::Instant::now();
        let snow_dt = self.snow_anim2d_dt_for_present();
        {
            let gc = &mut self.game_client;
            if early_menu_frame {
                debug!(
                    "Menu update_internal: calling gc.ensure_shell_visible (menu_frame={})",
                    self.menu_world_frames_rendered
                );
            }
            let _ = gc.ensure_shell_visible();
            let _ = gc.create_frame_tick_message();
            let t1 = std::time::Instant::now();
            if early_menu_frame {
                debug!("Menu update_internal: calling gc.update_input");
            }
            // Wave 587/588: device bookkeeping on Main-injected state (not dual OS poll).
            let _ = gc.update_input();
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
            let _ = gc.update_post_draw_ui();
            let _ = (t1, t2, t3);
        }

        // Peek MSG_NEW_GAME before pump so WND Start reaches InGame, then let
        // `pump_message_stream` deliver the same message to crate GameLogic
        // (`GameLogic::logicMessageDispatcher` / MSG_NEW_GAME).
        let t4 = std::time::Instant::now();
        if let Some(request) = self.take_pending_new_game_start_request() {
            if matches!(request.mode, GameMode::Shell) {
                // C++ MSG_NEW_GAME GAME_SHELL is the shell map, already applied
                // by finalize. Treating it as a match start loads Defcon6.
                info!("Menu NewGame drain: ignore GAME_SHELL (shell map already live)");
                Self::take_shell_new_game_messages_from_common_stream();
            } else {
                info!(
                    "Menu NewGame drain: mode={:?} faction={} map={} skirmish={}",
                    request.mode,
                    request.faction,
                    request.map,
                    request.skirmish.is_some()
                );
                self.start_game_from_ui(request);
                {
                    let gc = &mut self.game_client;
                    if early_menu_frame {
                        debug!("Menu update_internal: calling gc.pump_message_stream");
                    }
                    let _ = gc.pump_message_stream();
                }
                let _ = Self::take_new_game_dispatch_from_common_stream();
                gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
                return true;
            }
        }

        {
            let gc = &mut self.game_client;
            if early_menu_frame {
                debug!("Menu update_internal: calling gc.pump_message_stream");
            }
            let _ = gc.pump_message_stream();
        }

        // Secondary path: crate helpers may flag start after pump.
        if gamelogic::helpers::TheGameLogic::is_start_new_game_requested() {
            let pending = game_engine::common::global_data::read()
                .pending_file
                .clone();
            let pending_shellish = {
                let t = pending.trim().to_ascii_lowercase();
                t.is_empty() || t.contains("shellmap")
            };
            if self.host_is_in_shell_game() && pending_shellish {
                info!("Menu start_new_game flag drain: ignore leftover GAME_SHELL flag");
                gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
            } else {
                gamelogic::helpers::TheGameLogic::clear_start_new_game_request();
                if let Some(request) = self.build_start_request_from_pending_globals(None) {
                    info!(
                        "Menu start_new_game flag drain: mode={:?} map={}",
                        request.mode, request.map
                    );
                    self.start_game_from_ui(request);
                    return true;
                }
            }
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
        false
    }

    /// Wave 590: seed PresentationFrame after match start (no logic advance).
    /// Syncs shadow, builds host+GW frame, applies HUD/ControlBar/UI state.
    #[inline]

    /// Wave 591: render-path UI presentation consumer residual.
    ///
    /// Prefers pipeline freeze, then `last_presentation_frame`. Builds
    /// `GameUIState` from presentation only (no live object walks). Boot/loading
    /// residual without a freeze still uses `host_update_ui_state`.

    /// Wave 592: render-path presentation overlays residual (radar/script/clock/diag).
    ///
    /// Applies HUD radar messages from UI state, presentation-or-boot script
    /// messages, sim clock + fps/diagnostics/asset stats. Does not process UI
    /// events (caller owns that boundary).

    /// Wave 593: render-path UI finalize residual (minimap/radar/victory/last_ui).
    ///
    /// After overlays + `process_ui_events`, stamps minimap texture/coords/viewport,
    /// radar pings/messages from presentation world bounds, match-over/victory
    /// summary, and retains `last_ui_state`.
    pub(super) fn host_finalize_render_ui_state(&mut self, ui_state: &mut crate::ui::GameUIState) {
        // Wave 593: render UI finalize residual.
        ui_state.minimap_texture_id = self.render_pipeline.get_minimap_texture_id();
        ui_state.minimap_coordinates = self.render_pipeline.get_minimap_coordinates().cloned();
        self.update_minimap_viewport(ui_state);
        // Prefer presentation world_env for radar/minimap when a frame is installed.
        let world_bounds = self.presentation_world_bounds();
        self.game_hud
            .update_radar_pings(&ui_state.radar_pings, world_bounds.0, world_bounds.1);
        for msg in &ui_state.radar_messages {
            self.game_hud.push_radar_message(msg);
        }
        for evt in &ui_state.radar_events {
            self.game_hud
                .add_radar_message(&evt.text, evt.position, evt.kind);
        }

        ui_state.match_over = self.match_over;
        if let Some(summary) = &self.victory_summary {
            ui_state.victory_summary = Some(summary.clone());
            ui_state.player_outcome = summary
                .player_results
                .iter()
                .find(|result| result.player_id == self.current_player_id)
                .map(|result| result.outcome);
        } else {
            ui_state.victory_summary = None;
            ui_state.player_outcome = None;
        }
        // Retain presentation-overlaid identity for consumers (was dropped each frame).
        self.last_ui_state = Some(ui_state.clone());
    }

    pub(super) fn host_apply_render_ui_presentation_overlays(
        &mut self,
        ui_state: &mut crate::ui::GameUIState,
    ) {
        // Wave 592: render UI presentation overlays residual.
        if !ui_state.radar_events.is_empty() {
            for evt in &ui_state.radar_events {
                self.game_hud
                    .add_radar_message(&evt.text, evt.position, evt.kind);
            }
        } else {
            for msg in &ui_state.radar_messages {
                self.game_hud.push_radar_message(msg);
            }
        }
        // Wave 570: presentation-or-boot script message residual via helper.
        let new_script_messages: Vec<String> = self.take_presentation_or_boot_new_script_messages();
        for msg in &new_script_messages {
            self.game_hud.push_script_message(msg);
        }
        // Wave 462: prefer pipeline/last presentation sim clock residual.
        // Wave 553: via presentation_or_boot_total_play_time helper.
        ui_state.current_game_time = self.presentation_or_boot_total_play_time();
        ui_state.fps = self.fps;
        ui_state.frame_time_ms = if self.fps > 0.0 {
            1000.0 / self.fps
        } else {
            0.0
        };
        ui_state.performance_score = (ui_state.fps / 60.0).clamp(0.0, 1.5);
        if let Some(diag) = &self.diagnostics_overlay {
            ui_state.diagnostics = Some(diag.clone());
        } else {
            ui_state.diagnostics = Some(DiagnosticsOverlayStats::from_overall(
                ui_state.performance_score * 100.0,
            ));
        }
        ui_state.show_debug_overlay = self.show_debug_info;
        if let Some(manager_arc) = get_asset_manager() {
            if let Ok(manager) = manager_arc.lock() {
                let stats = manager.get_statistics();
                ui_state.assets_loaded = stats.archive_stats.total_files as u64;
                ui_state.asset_memory_mb = 0.0;
                ui_state.asset_cache_usage = 0.0;
            }
        }
    }

    pub(super) fn host_build_render_ui_state_from_presentation(
        &mut self,
    ) -> crate::ui::GameUIState {
        // Wave 591: render UI presentation consumer residual.
        // Wave 462: prefer pipeline freeze, then last_presentation_frame.
        // Boot residual: update_ui_state only when no frame is installed yet.
        // ControlBar selection panel health is presentation-owned.
        // GameUIState is built from PresentationFrame only (no live object walks).
        if let Some(pres) = self
            .render_pipeline
            .presentation_frame()
            .cloned()
            .or_else(|| self.last_presentation_frame.clone())
        {
            let mut ui = crate::ui::GameUIState::default();
            pres.apply_to_ui_state(&mut ui);
            self.apply_presentation_to_huds(&pres);
            // Presentation audio already dispatched; SFX dual-path retired.
            self.sync_eva_messages_from_presentation(&pres);
            #[cfg(feature = "game_client")]
            {
                pres.apply_to_control_bar(&mut self.control_bar);
                let _ = self
                    .control_bar
                    .update(std::time::Duration::from_millis(33));
            }
            // Keep last_presentation aligned with pipeline freeze when it was the source.
            self.last_presentation_frame = Some(pres);
            ui
        } else {
            // Boot/loading residual only.
            self.host_update_ui_state(self.current_player_id)
        }
    }

    pub(super) fn host_sync_shadow_and_build_presentation(
        &mut self,
        with_victory: bool,
    ) -> crate::presentation_frame::PresentationFrame {
        // Wave 926: single host→shadow sync + presentation build boundary.
        if let Some(ref mut shadow) = self.gameworld_shadow {
            shadow.sync_from_host(&self.game_logic);
        }
        let runtime_heightmap = self.presentation_runtime_heightmap_for_frame();
        let local_id = self.current_player_id;
        if with_victory {
            crate::presentation_frame::PresentationFrame::build_with_victory_for_engine_with_runtime_heightmap(
                &mut self.game_logic,
                local_id,
                self.gameworld_shadow.as_ref(),
                runtime_heightmap,
            )
        } else {
            crate::presentation_frame::PresentationFrame::build_for_engine_with_runtime_heightmap(
                &self.game_logic,
                local_id,
                self.gameworld_shadow.as_ref(),
                runtime_heightmap,
            )
        }
    }

    pub(super) fn host_seed_presentation_after_match_start(&mut self) {
        // Wave 590: match-start presentation seed residual.
        self.match_damage_applied = 0.0;
        self.match_kills = 0;
        crate::game_logic::host_damage_log::reset_cumulative();

        // Wave 172/590: match-start seed always syncs GameWorldShadow from host
        // then build_for_engine (GW overlay/rebuild lives inside that freeze).
        if let Some(ref mut shadow) = self.gameworld_shadow {
            shadow.sync_from_host(&self.game_logic);
        }
        let runtime_heightmap = self.presentation_runtime_heightmap_for_frame();
        let mut pres =
            crate::presentation_frame::PresentationFrame::build_for_engine_with_runtime_heightmap(
                &self.game_logic,
                self.current_player_id,
                self.gameworld_shadow.as_ref(),
                runtime_heightmap,
            );
        pres.apply_to_game_hud(&mut self.game_hud);
        #[cfg(feature = "game_client")]
        {
            pres.apply_to_control_bar(&mut self.control_bar);
            let _ = self
                .control_bar
                .update(std::time::Duration::from_millis(33));
        }
        let mut ui = GameUIState::default();
        pres.apply_to_ui_state(&mut ui);
        self.last_ui_state = Some(ui);
        self.last_presentation_frame = Some(pres);
        // C++ does not evaluate victory on the load frame. A just-seeded
        // alpine/empty world would otherwise stamp match_over and jump to Defeat.
        if let Some(pres) = self.last_presentation_frame.as_mut() {
            pres.match_over = false;
        }
        self.host_match_over = Some(false);
        self.host_match_boot_victory_condition = Some(None);
        self.match_over = false;
        #[cfg(feature = "game_client")]
        {
            // A map/load seed can transition directly to InGame before the
            // next logic tick reaches `host_tick_game_client_presentation_shell`.
            // Establish the exact direct bindings now so the first W3D
            // candidate ledger has a current key/status/pose rather than
            // silently skipping its C++ clear-frame boundary for one render.
            let presentation_time_frozen =
                self.presentation_or_boot_time_frozen() || self.game_paused;
            self.host_sync_presentation_direct_drawables(presentation_time_frozen);
        }
        // Wave 844: stamp host sim residuals after match-start seed.
        self.host_refresh_match_sim_residuals_from_logic();
    }

    #[cfg(feature = "game_client")]
    fn presentation_draw_module_names_from_template(template: &str) -> Vec<String> {
        let Some(manager) = crate::assets::get_asset_manager() else {
            return Vec::new();
        };
        let Ok(manager) = manager.lock() else {
            return Vec::new();
        };
        let Some(definition) = manager.get_object_definition(template) else {
            return Vec::new();
        };
        definition
            .draw_modules
            .iter()
            .filter_map(|module| {
                module
                    .declaration
                    .split_whitespace()
                    .next()
                    .map(str::to_string)
            })
            .collect()
    }

    /// Freeze-to-GameClient direct Drawable association boundary shared by the
    /// ordinary presentation shell tick and the initial match/load seed.
    ///
    /// This is deliberately narrower than `host_tick_game_client_presentation_shell`:
    /// it does not consume input, update UI/effects, or advance client visual
    /// time. It only establishes the C++ Object-backed Drawable association,
    /// status, and pose from one immutable presentation frame, so the first
    /// InGame W3D candidate can validate and write its clear-frame state.
    #[cfg(feature = "game_client")]
    fn host_sync_presentation_direct_drawables(&mut self, presentation_time_frozen: bool) {
        let Some(pres) = self.last_presentation_frame.as_ref() else {
            return;
        };
        crate::game_logic::refresh_host_fx_object_poses_from_presentation(pres);

        let logic_frame = pres.frame.0;
        let host_epoch = self.host_direct_visual_world_epoch;
        // Materialize every borrowed presentation fact before mutating
        // GameClient, preserving the immutable host→client boundary.
        let sync_entries = pres
            .direct_host_drawables
            .iter()
            .map(|direct| {
                let o = &direct.object;
                game_client::core::game_client::PresentationDrawableSync {
                    object_id: o.id.0,
                    host_epoch,
                    // The direct-host roster was frozen from actual host
                    // ownership. It deliberately remains resident through
                    // gameplay death/slow death/rubble until that host entry
                    // is removed, matching C++ Drawable lifetime.
                    resident: direct.resident,
                    visual_template_name: direct.visual_template_name.clone(),
                    template_name: o.template_name.clone(),
                    position: [o.position.x, o.position.y, o.position.z],
                    orientation: o.orientation,
                    float_yaw: o.float_yaw,
                    float_pitch: o.float_pitch,
                    destroyed: o.destroyed,
                    model_condition_bits: o.model_condition_bits,
                    body_damage_state: o.body_damage_state,
                    // Wave 970: overlay residual (vet/construct) on Wave 965 kind/stealth/color/health.
                    kind_names: o.kind_of.iter().map(|k| format!("{k:?}")).collect(),
                    team_color: o.team_color,
                    effectively_stealthed: o.effectively_stealthed,
                    // C++ StealthUpdate resolves the look for this viewer:
                    // undetected enemy stealth is invisible, while allied
                    // stealth remains a translucent visible look. Dead
                    // drawables are source-visible even if old status bits
                    // still say stealthed.
                    scene_hidden_by_stealth: pres.local_viewer_hides_stealthed(o),
                    health_current: o.health_current,
                    health_max: o.health_max,
                    selected: o.selected,
                    veterancy_level: match o.veterancy {
                        crate::presentation_frame::PresentationVeterancy::Rookie => 0,
                        crate::presentation_frame::PresentationVeterancy::Veteran => 1,
                        crate::presentation_frame::PresentationVeterancy::Elite => 2,
                        crate::presentation_frame::PresentationVeterancy::Heroic => 3,
                    },
                    under_construction: o.under_construction,
                    construction_percent: o.construction_percent.clamp(0.0, 1.0),
                    // Wave 1115: sold residual for construct-percent fail-closed.
                    sold: o.sold,
                    // Wave 972: icon-pip residual.
                    ammo_pip_total: o.ammo_pip_total.min(255) as u8,
                    ammo_pip_full: o.ammo_pip_full.min(255) as u8,
                    occupant_count: (o.occupant_count as u32).min(255) as u8,
                    max_garrison: (o.max_garrison as u32).min(255) as u8,
                    disabled: o.disabled,
                    // C++ ICON_CARBOMB only when WEAPONSET_CARBOMB && local owner.
                    is_carbomb: o.weapon_set_carbomb
                        && o.owner_player_id == Some(pres.local_player_id),
                    bomb_type: o.bomb_type,
                    bomb_timer_seconds: o.bomb_timer_seconds,
                    weapon_bonus_enthusiastic: o.weapon_bonus_enthusiastic,
                    // Wave 983: healing icon residual.
                    show_healing: o.show_healing,
                    healing_icon_type: o.healing_icon_type,
                    // Wave 984: garrisoned unit ids for contained-flash residual.
                    garrisoned_ids: o.garrisoned_units.iter().map(|id| id.0).collect(),
                    // Wave 1057: emoticon residual for dual icon UI.
                    emoticon_name: o.emoticon_name.clone(),
                    emoticon_frames_left: o.emoticon_frames_left,
                    // Wave 1058: formation residual for dual formation letter.
                    formation_id: o.formation_id,
                    // Wave 1059: caption residual (display_name when distinct from template).
                    caption: {
                        let dn = o.display_name.trim();
                        if !dn.is_empty() && dn != o.template_name {
                            dn.to_string()
                        } else {
                            String::new()
                        }
                    },
                    draw_module_names: Self::presentation_draw_module_names_from_template(
                        if direct.visual_template_name.trim().is_empty() {
                            o.template_name.as_str()
                        } else {
                            direct.visual_template_name.as_str()
                        },
                    ),
                    health_box_width: o.health_box_width,
                    health_box_z_offset: o.health_box_z_offset,
                }
            })
            .collect::<Vec<_>>();
        let direct_sources = pres
            .direct_host_drawables
            .iter()
            .filter(|direct| direct.resident)
            .filter_map(|direct| {
                let object = &direct.object;
                let (raw_status, effectively_dead) =
                    object.drawable_shroud.direct_game_client_status()?;
                Some((
                    object.id.0,
                    raw_status,
                    effectively_dead,
                    [object.position.x, object.position.y, object.position.z],
                    object.orientation,
                    object.float_yaw,
                    object.float_pitch,
                ))
            })
            .collect::<Vec<_>>();

        let (created, updated, pruned) = self.game_client.sync_presentation_drawables(sync_entries);
        if created + updated + pruned > 0 {
            log::trace!(
                "presentation drawable sync created={created} updated={updated} pruned={pruned}"
            );
        }
        // Resolve the complete current binding *after* sync. The same runtime
        // key then guards raw C++ shroud status and frozen pose, so an ordinary
        // update preserves it while a visual replacement rejects predecessor
        // frame data.
        let direct_bindings = direct_sources
            .into_iter()
            .filter_map(
                |(
                    object_id,
                    raw_status,
                    effectively_dead,
                    position,
                    orientation,
                    float_yaw,
                    float_pitch,
                )| {
                    let binding_key = self
                        .game_client
                        .presentation_direct_drawable_state(host_epoch, object_id)?
                        .binding_key;
                    Some((
                        binding_key,
                        raw_status,
                        effectively_dead,
                        position,
                        orientation,
                        float_yaw,
                        float_pitch,
                    ))
                },
            )
            .collect::<Vec<_>>();
        // C++ `GameClient::update` does not recompute bound Drawable shroud
        // state while visual time is frozen. The view/scene phase still runs
        // later against this retained client state, where a real eligible
        // Clear candidate may refresh its clear frame.
        if !presentation_time_frozen {
            let shroud_entries = direct_bindings.iter().map(
                |(binding_key, raw_status, effectively_dead, _, _, _, _)| {
                    game_client::core::game_client::FrozenDirectShroudStatus {
                        binding_key: *binding_key,
                        raw_status: *raw_status,
                        effectively_dead: *effectively_dead,
                    }
                },
            );
            let _ = self
                .game_client
                .apply_frozen_direct_shroud_statuses(logic_frame, shroud_entries);
        }
        // Pose belongs to the identical direct binding rather than a raw
        // ObjectID. Do not filter gameplay-destroyed records here: the direct
        // source roster controls C++ Drawable residency.
        let pose_entries = direct_bindings.into_iter().map(
            |(binding_key, _, _, position, orientation, float_yaw, float_pitch)| {
                game_client::core::game_client::FrozenDirectPresentationPose {
                    binding_key,
                    position,
                    orientation,
                    float_yaw,
                    float_pitch,
                }
            },
        );
        let n = self
            .game_client
            .apply_frozen_direct_presentation_poses(pose_entries);
        if n > 0 {
            log::trace!("presentation pose applied to {n} drawables");
        }
    }

    /// Wave 590: boot/render residual — freeze a PresentationFrame if none installed.
    /// Ensures execute never dual-reads live GameLogic mid-draw.
    #[inline]
    pub(super) fn host_ensure_presentation_frame_for_render(&mut self) {
        // Wave 590: boot presentation seed residual.
        if self.last_presentation_frame.is_some() {
            return;
        }
        // Wave 195/590/926: shadow sync + presentation build via single host boundary.
        let frame = self.host_sync_shadow_and_build_presentation(false);
        self.last_presentation_frame = Some(frame);
    }

    /// Wave 590: pipeline env seed residual (host+GW) when pipeline has no frame.
    #[inline]
    pub(super) fn host_ensure_presentation_env_for_hints(&mut self) {
        // Wave 590: pipeline env seed residual.
        // Wave 466: prefer host+GameWorld shadow freeze when a shadow session exists.
        // Wave 455: presentation-only env boundary — seed via build_for_engine only.
        if self.render_pipeline.presentation_frame().is_some() {
            return;
        }
        // Wave 466: sync shadow then freeze via build_for_engine (not host-only None).
        // Wave 466/474: seed via build_for_engine(host + self.gameworld_shadow).
        if let Some(ref mut shadow) = self.gameworld_shadow {
            shadow.sync_from_host(&self.game_logic);
        }
        let runtime_heightmap = self.presentation_runtime_heightmap_for_frame();
        let env_frame = crate::game_logic::seed_presentation_env_frame_from_host_and_shadow_with_runtime_heightmap(
            &self.game_logic,
            self.current_player_id,
            self.gameworld_shadow.as_ref(),
            runtime_heightmap,
        );
        self.render_pipeline.set_presentation_frame(Some(env_frame));
    }

    /// Wave 600: post-presentation client residual (camera/audio/UI/popup/music).
    ///
    /// After HUD apply: non-Menu camera + SFX cleanup, InGame UI projection,
    /// script camera residual, and presentation-or-boot popup/music/movies.
    pub(super) fn host_tick_post_presentation_client_residuals(&mut self, visual_dt: f32, dt: f32) {
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
            self.set_runtime_ui_state_projection(UISystemState::InGame);
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
    pub(super) fn host_broadcast_match_outcome_residuals(&mut self) {
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
                self.game_hud.push_info_message(&message);
                self.ui_manager.game_hud_mut().push_info_message(&message);
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
                self.game_hud.push_info_message(&message);
                // Wave 566: boot residual via notify_boot_ui_message helper.
                self.notify_boot_ui_message(&message, Some(player.team));
            } else {
                // Fail-closed id-only residual.
                info!("Player {} has been defeated", player_id);
            }
            fow_rendering::reveal_entire_map_for_player(player_id);
            script_events::push_event(ScriptEvent::PlayerDefeated { player_id });
            script_events::push_event(ScriptEvent::RevealMapForPlayer { player_id });
            // C++ VictoryConditions.cpp:201-214 — first local defeat
            // TheRadar->forceOn + SetInGameChatType(EVERYONE).
            if self.presentation_or_boot_local_player_id() == Some(player_id) {
                self.host_game_logic_mut().set_radar_forced(true);
                #[cfg(feature = "game_client")]
                {
                    let _ = game_client::gui::callbacks::ingame_callbacks::set_in_game_chat_type(
                        game_client::gui::callbacks::ingame_callbacks::InGameChatType::Everyone,
                    );
                    self.control_bar
                        .set_control_bar_scheme_by_player_side("Observer");
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
            self.game_hud.push_info_message(&message);
            self.ui_manager.game_hud_mut().push_info_message(&message);
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

            if matches!(event.state, AllianceState::AlliedDefeat) {
                fow_rendering::reveal_entire_map_for_player(event.player_id);
                script_events::push_event(ScriptEvent::RevealMapForPlayer {
                    player_id: event.player_id,
                });
            }
            script_events::push_event(ScriptEvent::AllianceStateChanged {
                player_id: event.player_id,
                state: event.state,
            });
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
    pub(super) fn host_apply_ingame_hud_from_presentation(&mut self, dt: f32) {
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
                pres.apply_to_control_bar(&mut self.control_bar);
                let _ = self
                    .control_bar
                    .update(std::time::Duration::from_millis(33));
            }
        } else {
            // Wave 238: boot residual via ui_local_economy (no &Player dual-read).
            let (money, power, max_power) = self.ui_local_economy();
            self.game_hud.update_resources(money, power, max_power);
        }

        if dt.is_finite() {
            if let Err(err) = self.game_hud.update(dt) {
                warn!("Game HUD update failed: {}", err);
            }
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

    /// Wave 597: GameWorld shadow session after host logic residual.
    ///
    /// Runs `shadow_session_after_host_tick` (or maybe_shadow), seeds
    /// `last_gameworld_presentation_entity_count` from observe-path view, and
    /// ends a coupled shadow tick when requested. Host remains temporary
    /// mid-frame owner; shadow is last-writer for HP/cash/pose.
    pub(super) fn host_run_gameworld_shadow_after_logic(&mut self, couple_shadow: bool) {
        // Wave 597/680/927: GameWorld shadow session residual via single boundary.
        // AFTER host logic + projectiles + path; host temporary mid-frame owner.
        // Keep the generation-checked couple handle live through writeback
        // complete/spawn so `host_authoritative_*` still see GameWorld.
        let from_boundary = crate::gameworld_shadow::run_post_logic_shadow_boundary(
            self.gameworld_shadow.as_mut(),
            &mut self.game_logic,
        );
        // Wave 186: stamp observe-path entity count from presentation_view_from_shadow
        // after the coupled shadow session (status gameworld_presentation_entities).
        self.last_gameworld_presentation_entity_count = self
            .gameworld_shadow
            .as_ref()
            .map(|shadow| {
                crate::gameworld_shadow::presentation_view_from_shadow(shadow, 0)
                    .entities
                    .len()
            })
            .unwrap_or(from_boundary);
        // Wave 621/912: after health writeback, drain destroy-ready log and process
        // die side effects same couple-frame (host still owns ObjectId remove).
        let _ = self
            .game_logic
            .apply_host_support_op(crate::game_logic::HostSupportOp::ProcessDestroyListIfNeeded);
        // Coupled-frame depth and the active shadow handle are owned by the
        // caller's RAII guards so unwinding cannot leak either into a later
        // frame.  They remain live through this writeback boundary only.
        let _ = couple_shadow;
    }

    /// Wave 589: post-logic presentation finalize residual.
    ///
    /// Builds immutable `PresentationFrame` (victory + GameWorld object path when
    /// shadow live), dispatches presentation audio events, mirrors particle FX to
    /// the client, stores `last_presentation_frame`, then applies InGame script FPS.
    ///
    /// Call after host logic + shadow writeback. Borrow-first: no live dual-read
    /// during later render. Fail-closed: not sole GameWorld authority / playable_claim.
    pub(super) fn host_finalize_presentation_after_logic(&mut self) {
        // Wave 985: drain ControlBar host production-pause residual onto BuildingData.
        for (producer_id, paused) in
            game_client::gui::control_bar::take_host_production_pause_requests()
        {
            let _ = self
                .game_logic
                .set_production_paused(crate::game_logic::ObjectId(producer_id), paused);
        }
        // Wave 589: presentation finalize residual.
        // Wave 589/838/926: shadow sync + victory presentation build via single boundary.
        let mut pres = self.host_sync_shadow_and_build_presentation(true);
        // Presentation → audio subsystem directly (no GameLogic dual-write mid-frame).
        let audio_n = pres.dispatch_audio_events_direct();
        if audio_n > 0 {
            log::trace!("presentation audio events dispatched: {audio_n}");
        }
        // Same-frame particle residual: backfill client ParticleSystemManager.
        let fx_n = pres.apply_particle_systems_to_client();
        if fx_n > 0 {
            log::trace!("presentation particle client mirrors: {fx_n}");
        }
        self.last_presentation_frame = Some(pres);
        self.render_pipeline
            .set_presentation_frame(self.last_presentation_frame.clone());
        // Wave 844: keep host sim residuals current for freeze-miss peels.
        self.host_refresh_match_sim_residuals_from_logic();

        // Wave 568: InGame script FPS residual via helper.
        self.apply_ingame_script_fps_limit_residual();
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
    pub(super) fn host_tick_game_client_presentation_shell(&mut self) {
        // C++ GameLogic::update setFrame(TheGameLogic->getFrame()) before client update.
        // Extra presents with the same host frame freeze drawable/FX time.
        #[cfg(feature = "game_client")]
        self.game_client.set_frame(self.game_logic.frame);
        // Wave 587: process Main-injected device state before shell UI residual.
        // inject_game_client_* already wrote THE_MOUSE/THE_KEYBOARD from OS events;
        // update_input only runs Keyboard/Mouse::update bookkeeping (no second OS poll).
        // Wave 586: presentation freeze residual when a frame is installed.
        // C++ `GameClient::update` freezes the per-Drawable update/shroud
        // loop for script/tactical freezes *and* ordinary game pause. The
        // installed presentation frame owns the former; `game_paused` is a
        // host shell state and must remain part of this client-facing gate.
        let presentation_time_frozen = self.presentation_or_boot_time_frozen() || self.game_paused;
        let visual_delta = if presentation_time_frozen {
            0.0
        } else {
            game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL
        };
        self.host_sync_presentation_direct_drawables(presentation_time_frozen);
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            // Presentation cinematic letterbox residual → client display.
            let was_letterbox = self.game_client.letterbox_overlay_enabled();
            self.game_client
                .apply_presentation_cinematic_letterbox(pres.cinematic_letterbox);
            // C++ ScriptActions::doLetterBoxMode HideControlBar(TRUE)/ShowControlBar(FALSE).
            if pres.cinematic_letterbox {
                let _ = game_client::gui::callbacks::control_bar_callbacks::hide_control_bar(true);
                // hide_control_bar no-ops when already hidden; still lift the
                // live 80% tactical frac so letterbox 3D fills the display.
                game_client::display::view::set_tactical_view_height_frac(1.0);
            } else if was_letterbox {
                let _ = game_client::gui::callbacks::control_bar_callbacks::show_control_bar(false);
            }
            // Military caption residual → InGameUI (duration from freeze).
            self.game_client.apply_presentation_military_caption(
                pres.military_caption.as_deref(),
                pres.military_caption_remaining_ms,
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
            let world_anims: Vec<(String, [f32; 3], f32, f32, bool, u32)> = pres
                .world_anims
                .iter()
                .filter(|a| a.is_active_at(pres.frame.0))
                .map(|a| {
                    (
                        a.template.clone(),
                        [a.position.x, a.position.y, a.position.z],
                        a.display_time_seconds,
                        a.z_rise_per_second,
                        a.fades,
                        a.spawn_frame,
                    )
                })
                .collect();
            self.game_client
                .apply_presentation_world_anims(&world_anims);
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
                            kind_names: o.kind_of.iter().map(|k| format!("{k:?}")).collect(),
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
            for (group, ids) in &self.control_groups {
                let g = i8::try_from(*group).unwrap_or(-1);
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
                            kind_names: o.kind_of.iter().map(|k| format!("{k:?}")).collect(),
                            // Wave 979: airborne catalog.
                            // Wave 971: special-power ready residual for host SP targeting.
                            special_power_ready: o.special_power_ready,
                            airborne_target: o.airborne_target
                                || o.kind_of.iter().any(|k| format!("{k:?}") == "Aircraft"),
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
                                crate::game_logic::ObjectId(o.id.0),
                            )
                            .or_else(|| {
                                let dn = o.display_name.trim();
                                if !dn.is_empty() && dn != o.template_name {
                                    Some(dn.to_string())
                                } else {
                                    None
                                }
                            })
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
        if let Err(e) = self.game_client.update_presentation_shell(visual_delta) {
            log::trace!("GameClient presentation shell update failed (non-fatal): {e}");
        }
    }
}

fn snow_anim2d_client_only_dt() -> f32 {
    use std::sync::Mutex;
    use std::time::Instant;
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    let now = Instant::now();
    let Ok(mut last) = LAST.lock() else {
        return 0.0;
    };
    let elapsed = last
        .map(|t| now.saturating_duration_since(t).as_secs_f32())
        .unwrap_or(game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL);
    if elapsed + f32::EPSILON < game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL {
        return 0.0;
    }
    *last = Some(now);
    game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL
}

#[cfg(any(test, feature = "internal"))]
mod replay_fast_forward_probe {
    use super::replay_logic_step_count;
    use crate::cnc_game_engine::CnCGameEngine;
    use crate::command_line::CommandLineArgs;
    use crate::game_logic::{
        BuildingData, BuildingType, GameLogic, KindOf, ObjectId, ProductionItem, ProductionKind,
        Resources, Team, ThingTemplate,
    };
    #[cfg(test)]
    use crate::gameworld_shadow::authority_env_lock;
    use crate::gameworld_shadow::{
        GameWorldShadow, ShadowCoupleGuard, gameworld_production_sole_tick_enabled,
        refresh_gameworld_authority_env_caches,
    };
    use glam::Vec3;
    use std::sync::Arc;
    use winit::{event_loop::EventLoop, window::WindowAttributes};

    struct EnvRestore {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            crate::env_compat::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => crate::env_compat::set_var(self.key, value),
                None => crate::env_compat::remove_var(self.key),
            }
            refresh_gameworld_authority_env_caches();
        }
    }

    fn replay_fixture_engine() -> anyhow::Result<(EventLoop<()>, CnCGameEngine)> {
        let event_loop = EventLoop::new()?;
        let window = Arc::new(
            event_loop.create_window(
                WindowAttributes::default()
                    .with_title("replay fast-forward regression")
                    .with_visible(false),
            )?,
        );
        let args = CommandLineArgs::parse_from_args(vec![
            "replay-fast-forward-test".to_string(),
            "--runtime_host=headless".to_string(),
            "--noaudio".to_string(),
        ])?;
        let engine = pollster::block_on(CnCGameEngine::new(window, Arc::new(args)))?;
        Ok((event_loop, engine))
    }

    fn install_replay_factory(engine: &mut CnCGameEngine) -> ObjectId {
        engine.game_logic = GameLogic::new();
        engine.game_logic.set_production_authority(true);
        engine.gameworld_shadow = Some(GameWorldShadow::new(64));
        engine.last_frame_timing = None;
        engine.last_presentation_frame = None;
        engine.host_match_time_frozen = None;
        engine.game_paused = false;

        let mut factory = ThingTemplate::new("ReplayFastForwardFactory");
        factory.set_health(500.0);
        factory.add_kind_of(KindOf::Structure);
        factory.add_kind_of(KindOf::FSBarracks);
        engine
            .game_logic
            .templates
            .insert("ReplayFastForwardFactory".to_string(), factory);
        let producer_id = engine
            .game_logic
            .create_object(
                "ReplayFastForwardFactory",
                Team::USA,
                Vec3::new(8.0, 0.0, 8.0),
            )
            .expect("producer");
        let producer = engine
            .game_logic
            .host_object_mut(producer_id)
            .expect("producer object");
        let mut building = BuildingData::new(BuildingType::Barracks);
        building.production_queue.push(ProductionItem {
            template_name: "ReplayFastForwardRanger".to_string(),
            progress: 0.0,
            total_time: 10.0,
            construction_frames: 0,
            cost: Resources {
                supplies: 100,
                power: 0,
            },
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        building.exit_delay_remaining_frames = 9;
        building.exit_burst_remaining = 0;
        building.queue_exit_state_initialized = true;
        building.exit_delay_remaining = 9.0 / 30.0;
        producer.building_data = Some(building);
        producer_id
    }

    fn host_queue_state(engine: &CnCGameEngine, producer_id: ObjectId) -> (u32, u32) {
        let building = engine
            .game_logic
            .host_object(producer_id)
            .and_then(|producer| producer.building_data.as_ref())
            .expect("host producer queue");
        let head = building.production_queue.first().expect("queue head");
        (
            head.construction_frames,
            building.exit_delay_remaining_frames,
        )
    }

    fn run_engine_replay_batch(
        engine: &mut CnCGameEngine,
        replay_fast_forward: bool,
        dt: f32,
        outer_frames: usize,
    ) -> (u32, u32, u32) {
        engine.replay_fast_forward = replay_fast_forward;
        let producer_id = install_replay_factory(engine);
        let before = host_queue_state(engine, producer_id);
        for _ in 0..outer_frames {
            // Exercise the actual engine boundary: it decides fixed-step count,
            // drains host residuals, runs GameWorld writeback, and finalizes one
            // presentation frame per outer update.
            engine.host_run_ingame_logic_presentation_frame(dt);
        }
        let after = host_queue_state(engine, producer_id);
        (
            after.0 - before.0,
            before.1 - after.1,
            engine.game_logic.get_frame(),
        )
    }

    pub(super) fn run_replay_fast_forward_engine_probe() -> anyhow::Result<()> {
        #[cfg(test)]
        let _env_guard = authority_env_lock();
        let _shadow_env = EnvRestore::set("GENERALS_GAMEWORLD_SHADOW", "1");
        refresh_gameworld_authority_env_caches();
        let _coupled = ShadowCoupleGuard::enter();

        let (_event_loop, mut engine) = replay_fixture_engine()?;
        // Arm production authority on the engine's GameLogic (hq-e84zk retired
        // the GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY env flag; re-armed on
        // every fresh instance inside install_replay_factory).
        install_replay_factory(&mut engine);
        assert!(
            gameworld_production_sole_tick_enabled(),
            "fixture must take the same coupled production-authority path as the engine"
        );
        drop(_coupled);

        let normal_30 = run_engine_replay_batch(&mut engine, false, 1.0 / 30.0, 1);
        assert_eq!(
            normal_30,
            (1, 1, 1),
            "normal 30 Hz engine frame must advance one queue/exit/fixed step"
        );

        let fast_30 = run_engine_replay_batch(&mut engine, true, 1.0 / 30.0, 1);
        assert_eq!(replay_logic_step_count(true), 4);
        assert_eq!(
            fast_30,
            (4, 4, 4),
            "TiVO fast-forward at 30 Hz must advance four queue/exit/fixed steps"
        );

        let normal_60 = run_engine_replay_batch(&mut engine, false, 1.0 / 60.0, 4);
        assert_eq!(
            normal_60,
            (2, 2, 2),
            "four normal 60 Hz engine frames must complete only two fixed steps"
        );

        let fast_60 = run_engine_replay_batch(&mut engine, true, 1.0 / 60.0, 1);
        assert_eq!(
            fast_60,
            (2, 2, 2),
            "four TiVO 60 Hz offers must advance shadow queues only for the two completed fixed steps"
        );
        Ok(())
    }

    #[cfg(test)]
    #[test]
    fn replay_fast_forward_runs_shadow_queue_per_actual_fixed_step() {
        // The full regression runs through the `internal` probe binary because
        // macOS requires winit event loops on the process main thread, while
        // Rust's unit-test harness executes tests on worker threads.
        assert_eq!(replay_logic_step_count(false), 1);
        assert_eq!(replay_logic_step_count(true), 4);
    }
}

#[cfg(test)]
mod superweapon_countdown_tests {
    use super::superweapon_countdown_text;

    #[test]
    fn ready_and_recharging_use_mm_ss_not_ready_word() {
        assert_eq!(superweapon_countdown_text(0.0), "0:00");
        assert_eq!(superweapon_countdown_text(-1.0), "0:00");
        assert_eq!(superweapon_countdown_text(90.0), "1:30");
        assert_eq!(superweapon_countdown_text(5.9), "0:05");
        assert_ne!(superweapon_countdown_text(0.0), "READY");
    }

    #[test]
    fn look_toward_drain_forwards_frozen_reverse_rotation() {
        let src = include_str!("camera_drain.rs");
        let start = src
            .find("if let Some(look) = pres.camera_look_toward")
            .expect("look-toward drain");
        let body = &src[start..src.len().min(start + 700)];
        assert!(
            body.contains("reverse_rotation: pres.camera_look_toward_reverse_rotation"),
            "LOOK_TOWARD_WAYPOINT reverseRotation must survive presentation drain"
        );
        assert!(
            !body.contains("reverse_rotation: false"),
            "must not hardcode reverse_rotation false at presentation drain"
        );
    }
}

#[cfg(feature = "internal")]
pub(super) fn run_replay_fast_forward_engine_probe() -> anyhow::Result<()> {
    replay_fast_forward_probe::run_replay_fast_forward_engine_probe()
}
