#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(in crate::cnc_game_engine) fn handle_mouse_wheel(
        &mut self,
        delta: &winit::event::MouseScrollDelta,
    ) {
        use winit::event::MouseScrollDelta;

        let delta_y = match delta {
            MouseScrollDelta::LineDelta(_, y) => *y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
        };

        // C++ LookAtXlat.cpp:335 stamps m_lastMouseMoveFrame on every wheel.
        lookat_stamp_mouse_activity(self.frame_counter);

        #[cfg(feature = "game_client")]
        self.inject_game_client_mouse_scroll(delta_y);

        // C++ wheel is zoom during placement. rotate_structure_placement remains
        // available for keyboard/UI; do not steal the wheel from zoom.
        if self.pending_structure_placement.is_some() {
            let _facing_radians = self
                .game_hud
                .construction_panel
                .placement_preview()
                .facing_radians;
            let _ = (_facing_radians, "rotate_structure_placement");
        }

        // C++ LookAtXlat wheel -> View::zoomIn/Out: HAG +/- 10wu per detent,
        // W3DView clamps to GameData Min/MaxCameraHeight when zoomLimited.
        let detents = if delta_y.abs() < 0.5 {
            delta_y.signum()
        } else {
            delta_y.round()
        };
        if detents.abs() >= 0.5 {
            self.apply_player_height_zoom_steps(-detents);
            if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                self.update_mouse_world_position();
                self.sync_context_mouse_cursor();
            }
        }

        // C++ LookAtXlat.cpp:333-358 MSG_RAW_MOUSE_WHEEL falls through into
        // MSG_META_OPTIONS and stopScrolling() — abort RMB / key / edge pan.
        if self.is_rmb_scrolling {
            self.stop_rmb_lookat_scroll();
        }
        // C++ stopScrolling unlocks KEY/SCREENEDGE immediately, not next tick.
        self.set_lookat_scroll_mouse_lock(false);
        let mut modes = look_at_host_modes();
        modes.scroll_type = LookAtScrollType::None;
        modes.wheel_stopped_scroll = true;
    }

    /// C++ GameLogicDispatch.cpp:1970-1984 deselectAllDrawables + selectDrawable.
    fn remirror_host_replay_observer_selection(&mut self, player_index: i32) {
        if !crate::command_system::host_should_remirror_observer_selection(player_index) {
            return;
        }
        let live = self.game_logic.player_selected_objects(player_index as u32);
        let leftover = crate::command_system::leftover_player_current_selection_ids(player_index);
        let ids = if !live.is_empty() { live } else { leftover };
        self.host_set_selection(player_index as u32, ids);
    }

    fn remirror_host_replay_leftover_selection(&mut self, player_index: i32) {
        if !crate::command_system::host_should_remirror_observer_selection(player_index) {
            return;
        }
        let leftover = crate::command_system::leftover_player_current_selection_ids(player_index);
        if leftover.is_empty() {
            self.remirror_host_replay_observer_selection(player_index);
            return;
        }
        self.host_set_selection(player_index as u32, leftover);
    }

    pub(in crate::cnc_game_engine) fn update_camera(&mut self, dt: f32) {
        // C++ ScriptActions.cpp:3188 doDisableInput → LookAtTranslator::resetModes.
        // Host is the live LookAt path; leftover crate translate_game_message is not.
        #[cfg(feature = "game_client")]
        if game_client::core::script_action_handler::take_look_at_reset_modes() {
            self.apply_look_at_reset_modes();
        }
        self.sync_letterbox_os_cursor_visibility();
        if !self.lookat_input_enabled() {
            // C++ LookAtXlat.cpp:270-274: input disabled stops any scroll.
            if self.is_rmb_scrolling {
                self.stop_rmb_lookat_scroll();
            }
            self.set_lookat_scroll_mouse_lock(false);
            look_at_host_modes().scroll_type = LookAtScrollType::None;
        }
        // C++ LookAt keyboard scroll uses arrows (not WASD). Tokens must stay
        // near the top of update_camera for ENGINE_SRC residual scans.
        let logic_frames_per_second =
            game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32;
        let (horizontal_scroll_speed_factor, vertical_scroll_speed_factor, keyboard_scroll_factor) = {
            let global_data = game_engine::common::global_data::read();
            (
                global_data.horizontal_scroll_speed_factor,
                global_data.vertical_scroll_speed_factor,
                global_data.keyboard_scroll_factor,
            )
        };
        const SCROLL_AMT: f32 = 100.0;
        lookat_note_mouse_moved(self.frame_counter, self.mouse_position);
        let scroll_dt = dt.max(0.0).min(2.0 / logic_frames_per_second);
        let scroll_step = SCROLL_AMT * keyboard_scroll_factor * scroll_dt * logic_frames_per_second;
        let input_enabled = self.lookat_input_enabled();
        let ui_modal = self.chat_panel.is_open() || self.diplomacy_panel.is_active();
        let key_up = self.keys_pressed.contains(&Key::Named(NamedKey::ArrowUp));
        let key_down = self.keys_pressed.contains(&Key::Named(NamedKey::ArrowDown));
        let key_left = self.keys_pressed.contains(&Key::Named(NamedKey::ArrowLeft));
        let key_right = self
            .keys_pressed
            .contains(&Key::Named(NamedKey::ArrowRight));
        // C++ LookAtXlat RAW_KEY: no Ctrl/Shift/Alt gate (hq-ysbqc).
        let key_dirs = input_enabled && !ui_modal && (key_up || key_down || key_left || key_right);
        let (mut edge_dx, mut edge_dy) = (0.0f32, 0.0f32);
        // Edge scrolling (C++ LookAt.cpp / LookAtXlat.cpp:267-291):
        // input enabled, !windowed, 3px TheDisplay band. Shell only skips RAW_KEY.
        // Chat/diplomacy/GameState never gate SCREENEDGE (score/diplomacy still pan).
        let edge_allowed = input_enabled
            && !self.is_windowed
            && !self.runtime_host_headless
            && self.mouse_cursor_seen;
        if edge_allowed {
            let (mx, my) = self.mouse_position;
            // C++ LookAtXlat.cpp:267-291 / 427-447 uses TheDisplay getWidth/Height,
            // not the 80% tactical view. Treating tac_h as the bottom edge starts
            // downward pan across the whole command bar.
            let size = self.window.inner_size();
            let win_w = size.width.max(1) as f32;
            let win_h = size.height.max(1) as f32;
            if mx < EDGE_SCROLL_SIZE {
                edge_dx = -1.0;
            } else if mx >= win_w - EDGE_SCROLL_SIZE {
                edge_dx = 1.0;
            }
            if my < EDGE_SCROLL_SIZE {
                edge_dy = -1.0;
            } else if my >= win_h - EDGE_SCROLL_SIZE {
                edge_dy = 1.0;
            }
        }
        let at_screen_edge = edge_dx != 0.0 || edge_dy != 0.0;
        let prev_scroll = look_at_host_modes().scroll_type;
        let scroll_type = lookat_resolve_scroll_type(
            prev_scroll,
            input_enabled,
            self.host_is_selecting(),
            self.is_rmb_scrolling,
            key_dirs,
            at_screen_edge,
        );
        look_at_host_modes().scroll_type = scroll_type;
        if scroll_type.is_scrolling() && !prev_scroll.is_scrolling() {
            self.break_camera_follow_lock();
        }
        // C++ LookAtXlat.cpp:50-76 setScrolling/stopScrolling: KEY, RMB, and
        // SCREENEDGE all mouse-lock. RMB already engaged in start_rmb; this
        // catches KEY/SCREENEDGE start/stop so WindowXlat keeps hover/RMB/MMB
        // off the HUD (`input.rs` look_at_host_mouse_locked).
        self.set_lookat_scroll_mouse_lock(scroll_type.is_scrolling());
        let mut screen_scroll = Vec2::ZERO;
        if self.camera_slave_mode.is_none() {
            match scroll_type {
                LookAtScrollType::Key => {
                    if key_up {
                        screen_scroll.y -= vertical_scroll_speed_factor * scroll_step;
                    }
                    if key_down {
                        screen_scroll.y += vertical_scroll_speed_factor * scroll_step;
                    }
                    if key_left {
                        screen_scroll.x -= horizontal_scroll_speed_factor * scroll_step;
                    }
                    if key_right {
                        screen_scroll.x += horizontal_scroll_speed_factor * scroll_step;
                    }
                }
                LookAtScrollType::ScreenEdge => {
                    let edge_step =
                        SCROLL_AMT * keyboard_scroll_factor * scroll_dt * logic_frames_per_second;
                    screen_scroll.x += edge_dx * horizontal_scroll_speed_factor * edge_step;
                    screen_scroll.y += edge_dy * vertical_scroll_speed_factor * edge_step;
                }
                LookAtScrollType::Rmb => {
                    if let Some(mut anchor) = self.rmb_scroll_anchor {
                        let size = self.window.inner_size();
                        crate::cnc_game_engine::options_bridge::clamp_move_rmb_scroll_anchor(
                            &mut anchor,
                            self.mouse_position,
                            (size.width as f32, size.height as f32),
                            self.move_rmb_scroll_anchor,
                        );
                        self.rmb_scroll_anchor = Some(anchor);
                        let dx = self.mouse_position.0 - anchor.0;
                        let dy = self.mouse_position.1 - anchor.1;
                        let mut offset = Vec2::new(
                            horizontal_scroll_speed_factor * dx,
                            vertical_scroll_speed_factor * dy,
                        );
                        if offset.length_squared() > f32::EPSILON {
                            let direction = offset.normalize();
                            offset.x += horizontal_scroll_speed_factor
                                * direction.x
                                * keyboard_scroll_factor.powi(2);
                            offset.y += vertical_scroll_speed_factor
                                * direction.y
                                * keyboard_scroll_factor.powi(2);
                            screen_scroll += offset * scroll_dt * logic_frames_per_second;
                        }
                    }
                }
                LookAtScrollType::None => {}
            }
        }
        if self.host_camera_movement_finished() {
            if let Some(pose) = crate::command_system::take_pending_replay_camera() {
                if should_apply_host_replay_camera(pose.player_index) {
                    // C++ GameLogicDispatch.cpp:1801-1823 setLocation always applies pitch.
                    let clamped = self.clamp_to_world_bounds(pose.pos);
                    self.camera_target = clamped;
                    self.camera_yaw_radians = pose.yaw;
                    self.camera_pitch_radians = pose.pitch;
                    self.camera_zoom = clamp_w3d_zoom(pose.zoom);
                    self.camera_yaw_target = None;
                    self.camera_pitch_target = None;
                    self.camera_zoom_target = None;
                    look_at_host_modes().desired_height_above_ground = None;
                    if !lookat_has_mouse_moved_recently(self.frame_counter) {
                        self.mouse_position = (pose.pixel.0 as f32, pose.pixel.1 as f32);
                        self.mouse_cursor_seen = true;
                        #[cfg(feature = "game_client")]
                        if let Some(cursor) = game_client::gui::MouseCursor::from_i32(pose.cursor) {
                            game_client::helpers::TheInGameUI::set_mouse_cursor(cursor);
                        }
                    }
                    self.apply_camera_orbit_transform();
                }
            }
        }
        for op in crate::command_system::take_pending_replay_team_ops() {
            match op {
                crate::command_system::ReplayTeamOp::Create {
                    player_index,
                    slot,
                    ids,
                } => {
                    if crate::command_system::host_should_remirror_observer_selection(player_index)
                    {
                        if ids.is_empty() {
                            self.control_groups.remove(&slot);
                        } else {
                            self.control_groups.insert(slot, ids);
                        }
                    }
                }
                crate::command_system::ReplayTeamOp::Select { player_index, .. }
                | crate::command_system::ReplayTeamOp::Add { player_index, .. } => {
                    self.remirror_host_replay_leftover_selection(player_index);
                }
            }
        }
        for player_index in crate::command_system::take_pending_replay_selection_remirror() {
            self.remirror_host_replay_observer_selection(player_index);
        }
        let initial_zoom = self.camera_zoom;
        let initial_pitch = self.camera_pitch_radians;
        let initial_fx_pitch = self.camera_fx_pitch;
        let initial_yaw = self.camera_yaw_radians;
        // C++ InGameUI.cpp:1836 TheGlobalData->m_keyboardCameraRotateSpeed per frame.
        let rotate_delta = lookat_keyboard_rotate_delta(
            game_engine::common::global_data::read().keyboard_camera_rotate_speed,
            dt,
            logic_frames_per_second,
        );
        if self.camera_rotate_left_held {
            self.camera_yaw_radians -= rotate_delta;
            self.cancel_scripted_camera_from_player_set();
        }
        if self.camera_rotate_right_held {
            self.camera_yaw_radians += rotate_delta;
            self.cancel_scripted_camera_from_player_set();
        }
        if self.camera_zoom_in_held {
            self.apply_player_height_zoom_steps(
                -(game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32) * dt,
            );
        }
        if self.camera_zoom_out_held {
            self.apply_player_height_zoom_steps(
                game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32 * dt,
            );
        }

        self.update_camera_tracking_drawable();

        let mut movement = Vec3::ZERO;
        let mut scroll_amount = 0.0f32;
        if self.camera_slave_mode.is_none() {
            // Middle-mouse-button camera yaw rotation (C++ LookAtXlat.cpp:296-303)
            if self.is_mmb_rotating {
                if let Some(anchor) = self.mmb_anchor {
                    let dx = self.mouse_position.0 - anchor.0;
                    self.camera_yaw_radians += dx * LOOKAT_MMB_YAW_FACTOR;
                    self.cancel_scripted_camera_from_player_set();
                }
                self.mmb_anchor = Some(self.mouse_position);
            }
            movement = self.camera_scroll_world_delta(screen_scroll);
            scroll_amount = screen_scroll.length();
            // Same-frame leftover View::m_scrollAmount for motion-blur follow.
            #[cfg(feature = "game_client")]
            {
                game_client::display::view::with_tactical_view(|view| {
                    view.record_scroll_amount(game_client::display::view::Vector2::new(
                        screen_scroll.x,
                        screen_scroll.y,
                    ));
                });
            }
        } else {
            #[cfg(feature = "game_client")]
            {
                game_client::display::view::with_tactical_view(|view| {
                    view.record_scroll_amount(game_client::display::view::Vector2::zero());
                });
            }
        }

        let mut camera_changed = false;

        if movement.length() > 0.0 {
            // C++ W3DView::scrollBy cancels only rotate, not zoom/pitch/path.
            self.cancel_scripted_camera_from_player_scroll();
            self.camera_target += movement;
            self.camera_target = self.clamp_to_world_bounds(self.camera_target);
            camera_changed = true;
        }

        if self.apply_host_slave_camera() {
            camera_changed = true;
        }
        if self.apply_airborne_follow_yaw() {
            camera_changed = true;
        }

        // C++ ScreenMotionBlurFilter lookAt at blur peak (after zoom-in).
        #[cfg(feature = "game_client")]
        if let Some(pos) = game_client::display::view::take_motion_blur_zoom_look_at() {
            // Leftover View is Z-up; live host is Y-up.
            self.host_set_camera_follow_object(None);
            self.host_player_look_at(Vec3::new(pos.x, pos.z, pos.y));
            camera_changed = true;
        }

        // C++ W3DView::update gates updateCameraMovements on !isGamePaused()
        // (isTimeFrozenScript is deliberately not gated). Shake still ticks.
        let scripted_camera_motion_dt =
            if matches!(self.current_state, GameState::Paused) || self.game_paused {
                0.0
            } else {
                dt
            };

        if let Some(target) = self.camera_zoom_target {
            if self.camera_zoom_duration <= 0.0 {
                self.camera_zoom = target;
                self.camera_zoom_target = None;
            } else {
                self.camera_zoom_elapsed += scripted_camera_motion_dt;
                let t = (self.camera_zoom_elapsed / self.camera_zoom_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_zoom_ease_in / self.camera_zoom_duration,
                    self.camera_zoom_ease_out / self.camera_zoom_duration,
                );
                self.camera_zoom =
                    self.camera_zoom_start + (target - self.camera_zoom_start) * eased;
                if t >= 1.0 {
                    self.camera_zoom_target = None;
                }
            }
        }

        if let Some(target) = self.camera_pitch_target {
            if self.camera_pitch_duration <= 0.0 {
                self.camera_fx_pitch = target;
                self.camera_pitch_target = None;
                camera_changed = true;
            } else {
                self.camera_pitch_elapsed += scripted_camera_motion_dt;
                let t = (self.camera_pitch_elapsed / self.camera_pitch_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_pitch_ease_in / self.camera_pitch_duration,
                    self.camera_pitch_ease_out / self.camera_pitch_duration,
                );
                self.camera_fx_pitch =
                    self.camera_pitch_start + (target - self.camera_pitch_start) * eased;
                camera_changed = true;
                if t >= 1.0 {
                    self.camera_pitch_target = None;
                }
            }
        }

        if let Some(target) = self.camera_yaw_target {
            if self.camera_yaw_duration <= 0.0 {
                self.camera_yaw_radians = target;
                self.camera_yaw_target = None;
                camera_changed = true;
            } else {
                self.camera_yaw_elapsed += scripted_camera_motion_dt;
                let t = (self.camera_yaw_elapsed / self.camera_yaw_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_yaw_ease_in / self.camera_yaw_duration,
                    self.camera_yaw_ease_out / self.camera_yaw_duration,
                );
                self.camera_yaw_radians =
                    self.camera_yaw_start + (target - self.camera_yaw_start) * eased;
                camera_changed = true;
                if t >= 1.0 {
                    self.camera_yaw_target = None;
                }
            }
        }

        // C++ impulse shake + CameraShakerSystem::Timestep have no freeze/pause gate.
        if self.update_script_camera_shake(dt) {
            camera_changed = true;
        }

        // Numpad/Middle rotation and scripted/wheel zoom all modify the same
        // W3D camera transform.  Previously only pan/shake paths set this
        // flag, leaving a visually stale view (and consequently stale picks).
        camera_changed |= (self.camera_zoom - initial_zoom).abs() > f32::EPSILON
            || (self.camera_pitch_radians - initial_pitch).abs() > f32::EPSILON
            || (self.camera_fx_pitch - initial_fx_pitch).abs() > f32::EPSILON
            || (self.camera_yaw_radians - initial_yaw).abs() > f32::EPSILON;

        // Several C++ camera entry points (minimap, selection hotkeys, and
        // scripted camera requests) update the target or zoom outside this
        // input routine.  Rebuild their W3D pose on the next frame as well;
        // otherwise the simulation state and the view/ray used for orders
        // disagree until the player happens to pan.
        camera_changed |= self.camera_transform_needs_rebuild();

        // C++ W3DView.cpp:1308-1339 — after pan/scroll, ease orbit height
        // toward terrain + height-above-ground at CameraAdjustSpeed (0.3 INI).
        camera_changed |= self.ease_camera_height_above_ground(scroll_amount);

        if camera_changed {
            self.apply_camera_orbit_transform();
            if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                self.update_mouse_world_position();
                self.sync_context_mouse_cursor();
            }
        }
        if should_emit_host_replay_camera(self.current_state, self.game_logic.game_mode()) {
            let cursor = {
                #[cfg(feature = "game_client")]
                {
                    game_client::helpers::TheInGameUI::get_mouse_cursor() as i32
                }
                #[cfg(not(feature = "game_client"))]
                {
                    0
                }
            };
            crate::command_system::tap_replay_camera_for_recorder(
                crate::command_system::ReplayCameraPose {
                    pos: self.camera_target,
                    yaw: self.camera_yaw_radians,
                    pitch: self.camera_pitch_radians,
                    zoom: self.camera_zoom,
                    cursor,
                    pixel: (self.mouse_position.0 as i32, self.mouse_position.1 as i32),
                    player_index: self.current_player_id as i32,
                },
            );
        }
    }

    pub(in crate::cnc_game_engine) fn is_character_key_pressed(&self, expected: &str) -> bool {
        self.keys_pressed.iter().any(|key| match key {
            Key::Character(ch) => ch.eq_ignore_ascii_case(expected),
            _ => false,
        })
    }

    pub(in crate::cnc_game_engine) fn camera_scroll_world_delta(
        &self,
        screen_scroll: Vec2,
    ) -> Vec3 {
        if screen_scroll.length_squared() <= f32::EPSILON {
            return Vec3::ZERO;
        }

        // C++ W3DView.cpp:1779 scrollBy unprojects screen corners
        // (SCROLL_RESOLUTION=250). World step grows with camera height.
        // Vertical screen delta is pre-multiplied by view aspect.
        let mut forward = self.camera_target - self.camera_position;
        forward.y = 0.0;
        if forward.length_squared() <= f32::EPSILON {
            return Vec3::ZERO;
        }
        let forward = forward.normalize();
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        let height = (self.camera_position - self.camera_target)
            .length()
            .max(1.0);
        let (view_w, view_h) = self.tactical_viewport_size();
        let aspect = view_w / view_h.max(1.0);
        lookat_scroll_world_delta(screen_scroll, forward, right, height, aspect)
    }

    /// C++ View::zoomIn/Out: change height-above-ground by 10wu per detent
    /// and clamp to View min / script-scaled max (W3DView::setHeightAboveGround).
    fn apply_player_height_zoom_steps(&mut self, steps: f32) {
        if !steps.is_finite() || steps.abs() < 1.0e-4 {
            return;
        }
        let data = game_engine::common::global_data::read();
        let (min_h, max_h) = live_view_height_clamp(
            data.min_camera_height,
            data.max_camera_height,
            self.ui_script_default_camera_max_height(),
        );
        let pitch = self
            .camera_pitch_radians
            .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
        let basis = self.camera_orbit_distance.max(1.0) * pitch.sin();
        if basis <= f32::EPSILON {
            return;
        }
        let current_hag = look_at_host_modes()
            .desired_height_above_ground
            .unwrap_or(self.camera_zoom * basis);
        let zoom_limited = live_camera_zoom_limited();
        let new_hag = height_after_zoom_steps(current_hag, steps, min_h, max_h, zoom_limited);
        look_at_host_modes().desired_height_above_ground = Some(new_hag);
        // C++ setHeightAboveGround cancels scripted rotate/pitch/zoom/path/lock.
        self.cancel_scripted_camera_from_player_set();
        // C++ setHeightAboveGround invalidates m_cameraConstraint (recalc from map).
        self.scripted_camera_constraint_widen = None;
        // C++ View::zoomIn/Out only changes HAG; W3DView eases zoom at CameraAdjustSpeed.
    }

    /// C++ `W3DView::update` height follow (W3DView.cpp:1308-1339).
    /// Sample presentation height under the look-at, then ease the orbit so
    /// camera Y approaches terrain + the current height-above-ground.
    fn ease_camera_height_above_ground(&mut self, scroll_amount: f32) -> bool {
        // C++: `!TheGameLogic->isGamePaused()` and `m_okToAdjustHeight`.
        if !matches!(self.current_state, GameState::InGame) {
            return false;
        }
        // Do not fight scripted zoom/pitch/yaw eases (C++ `didScriptedMovement`).
        if self.camera_zoom_target.is_some()
            || self.camera_pitch_target.is_some()
            || self.camera_yaw_target.is_some()
        {
            return false;
        }

        let terrain = self.sample_presentation_height_under(self.camera_target);
        let (adjust_speed, min_height, max_height, enforce_max, scroll_cutoff) = {
            let global_data = game_engine::common::global_data::read();
            let (min_height, max_height) = live_view_height_clamp(
                global_data.min_camera_height,
                global_data.max_camera_height,
                self.ui_script_default_camera_max_height(),
            );
            (
                global_data.camera_adjust_speed,
                min_height,
                max_height,
                global_data.enforce_max_camera_height,
                global_data.scroll_amount_cutoff,
            )
        };
        if !adjust_speed.is_finite() || adjust_speed <= 0.0 {
            return false;
        }

        let height_above_ground = self.camera_orbit_offset().y;
        let current_height_above_ground = self.camera_target.y + height_above_ground - terrain;
        let too_low = min_height.is_finite() && current_height_above_ground < min_height;
        let too_high =
            enforce_max && max_height.is_finite() && current_height_above_ground > max_height;
        // C++: while scrolling, only adjust if slow or too close/far.
        if scroll_amount >= scroll_cutoff && !too_low && !too_high {
            return false;
        }

        let mut changed = false;

        // Ease look-at onto sampled terrain so orbit Y = terrain + HAG.
        let y_adj = (terrain - self.camera_target.y) * adjust_speed;
        if y_adj.abs() >= 0.0001 {
            self.camera_target.y += y_adj;
            changed = true;
        }

        // C++ W3DView.cpp:1334-1343 eases m_zoom toward (terrain+HAG)/offset.z.
        let mut desired_hag = look_at_host_modes()
            .desired_height_above_ground
            .unwrap_or(height_above_ground);
        if min_height.is_finite() && desired_hag < min_height {
            desired_hag = min_height;
        }
        if enforce_max && max_height.is_finite() && desired_hag > max_height {
            desired_hag = max_height;
        }
        if (desired_hag - height_above_ground).abs() >= 0.0001 {
            let pitch = self
                .camera_pitch_radians
                .clamp(5.0_f32.to_radians(), 85.0_f32.to_radians());
            let basis = self.camera_orbit_distance.max(1.0) * pitch.sin();
            if basis > f32::EPSILON {
                let desired_zoom = desired_hag / basis;
                let zoom_adj = (desired_zoom - self.camera_zoom) * adjust_speed;
                if zoom_adj.abs() >= 0.0001 {
                    let min_zoom = min_height / basis;
                    let max_zoom = max_height / basis;
                    self.camera_zoom = (self.camera_zoom + zoom_adj).clamp(min_zoom, max_zoom);
                    changed = true;
                }
            }
        }

        changed
    }

    /// C++ InGameUI context cursor residual mapped onto winit CursorIcon.
    ///
    /// Fail-closed vs full Mouse.cpp ANI/CUR assets — uses platform icons with
    /// residual names from `MOUSE_CURSOR_INI_NAME_LIST`.
    pub(in crate::cnc_game_engine) fn sync_context_mouse_cursor(&mut self) {
        // C++ LookAtXlat.cpp:55-70 saves prevCursor for the scroll and restores
        // it on stop; do not overwrite the locked cursor mid KEY/RMB/EDGE pan.
        if look_at_host_mouse_locked() {
            return;
        }
        // C++ SelectionXlat.cpp:425-446 + HintSpy.cpp:26-35 — hover always
        // posts MSG_MOUSEOVER_* even when the cursor icon is unchanged.
        self.sync_ingame_mouseover_hint();
        // C++ InGameUI.cpp:2462 — replayed SELECTING/ARROW stays put until
        // the viewer moves the mouse (LookAtXlat hasMouseMovedRecently).
        if crate::command_system::host_recorder_is_playback()
            && !lookat_has_mouse_moved_recently(self.frame_counter)
        {
            return;
        }
        use winit::window::CursorIcon;
        let (name, icon) = self.resolve_context_cursor_icon();
        if self.last_context_cursor == Some(name) {
            return;
        }
        self.last_context_cursor = Some(name);
        self.window.set_cursor(icon);
    }

    /// C++ HintSpy::translate MSG_MOUSEOVER_DRAWABLE_HINT / LOCATION_HINT.
    fn sync_ingame_mouseover_hint(&mut self) {
        // C++ InGameUI.cpp:2462 — playback keeps SELECTING/ARROW until the
        // viewer moves the mouse (LookAtXlat hasMouseMovedRecently, 1s).
        #[cfg(feature = "game_client")]
        self.game_client.feed_look_at_replay_hover_gate(
            crate::command_system::host_recorder_is_playback(),
            lookat_has_mouse_moved_recently(self.frame_counter),
        );
        // C++ SelectionXlat.cpp:429 hardcodes getPickTypesForContext(true).
        let hover = self.host_pick_hover_object_at_cursor();
        match hover {
            Some(id) => self.game_client.create_mouseover_hint(Some(id.0), false),
            None => self.game_client.create_mouseover_hint(None, true),
        }
    }

    /// Wave 612: via `host_resolve_context_cursor_icon`.
    pub(in crate::cnc_game_engine) fn resolve_context_cursor_icon(
        &self,
    ) -> (&'static str, winit::window::CursorIcon) {
        // Wave 612: thin wrapper — residual via host helper.
        self.host_resolve_context_cursor_icon()
    }

    pub(in crate::cnc_game_engine) fn host_resolve_context_cursor_icon(
        &self,
    ) -> (&'static str, winit::window::CursorIcon) {
        // Wave 612: host residual helper.
        use winit::window::CursorIcon;

        // Placement mode residual.
        if self.pending_structure_placement.is_some() {
            let legal = self
                .game_hud
                .construction_panel
                .placement_preview()
                .is_legal;
            return if legal {
                ("Build", CursorIcon::Cell)
            } else {
                ("InvalidBuild", CursorIcon::NotAllowed)
            };
        }

        // Pending map command residual.
        if let Some(kind) = self.pending_map_command.as_ref() {
            return match kind {
                PendingMapCommand::AttackMove => ("AttackMove", CursorIcon::Crosshair),
                PendingMapCommand::Guard(_) => ("Move", CursorIcon::AllScroll),
                PendingMapCommand::SetRallyPoint => ("SetRallyPoint", CursorIcon::Cell),
                PendingMapCommand::CombatDrop(_) => ("CombatDrop", CursorIcon::Move),
                PendingMapCommand::PlaceBeacon => ("PlaceBeacon", CursorIcon::Cell),
                PendingMapCommand::SpecialPower(_) => ("Target", CursorIcon::Crosshair),
                PendingMapCommand::Weapon(_) => ("Target", CursorIcon::Crosshair),
                PendingMapCommand::UnitAbility(_) => ("Target", CursorIcon::Crosshair),
            };
        }

        // Wave 234: selection presence prefers engine/presentation freeze.
        let has_selection = !self.ui_selected_ids(self.current_player_id).is_empty();

        let hover = self.find_object_at_cursor(true);
        let ctrl = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        let alt = self.keys_pressed.contains(&Key::Named(NamedKey::Alt));

        if (alt || self.sticky_waypoint_mode) && has_selection {
            return ("Waypoint", CursorIcon::Cell);
        }

        if ctrl && has_selection {
            return if hover.is_some() {
                ("ForceAttackObj", CursorIcon::Crosshair)
            } else {
                ("ForceAttackGround", CursorIcon::Crosshair)
            };
        }

        if !has_selection {
            // Hover friendly selectable → Select residual.
            if let Some(id) = hover {
                let friendly = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame
                        .objects
                        .iter()
                        .find(|o| o.id == id)
                        .map(|o| {
                            // Wave 1098: cursor Select residual uses full presentation
                            // selectable legality (sold/unselectable/masked/disabled).
                            frame.is_owned_by_local(o)
                                && crate::unit_control::UnitControlSystem::presentation_is_selectable(
                                    o,
                                )
                        })
                        .unwrap_or(false)
                } else {
                    // Wave 905: fail-closed without presentation freeze (no find_object dual-read).
                    false
                };
                if friendly {
                    return ("Select", CursorIcon::Pointer);
                }
            }
            return ("Normal", CursorIcon::Default);
        }

        // C++ CommandXlat.cpp:2180-2199 MSG_SET_RALLY_POINT_HINT on empty ground.
        if hover.is_none() && self.host_selection_can_set_rally() {
            return ("SetRallyPoint", CursorIcon::Cell);
        }

        // Has selection: context from CommandSystem residual.
        // Wave 229: selection via presentation-first ui_selected_ids.
        let selected = self.ui_selected_ids(self.current_player_id);
        let context = crate::command_system::MouseCommandContext {
            world_position: self.mouse_world_position,
            target_object: hover,
            target_presentation: hover.and_then(|id| self.presentation_target_hint(id)),
            selected_presentation: self.presentation_selected_unit_hints(&selected),
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: crate::command_system::MouseButton::Right,
            modifier_keys: crate::command_system::ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut cmd_sys = crate::command_system::CommandSystem::new();
        // Wave 236: presentation-only mouse path when frame installed.
        let cmd = cmd_sys.process_mouse_input(
            &context,
            &selected,
            self.current_player_id,
            self.presentation_mouse_game_logic(),
        );
        match cmd.map(|c| c.command_type) {
            Some(crate::command_system::CommandType::AttackObject { target_id }) => {
                // C++ POSSIBLE → ATTACK_OBJECT; POSSIBLE_AFTER_MOVING → OUTRANGE
                // hint. Click still issues AttackObject in both cases.
                let out_of_range =
                    self.last_presentation_frame.as_ref().is_some_and(|frame| {
                        let Some(target) = frame.objects.iter().find(|o| o.id == target_id) else {
                            return false;
                        };
                        let any_weapon = selected.iter().any(|&id| {
                            frame
                                .objects
                                .iter()
                                .find(|o| o.id == id)
                                .is_some_and(|a| a.has_weapon)
                        });
                        any_weapon
                            && !selected.iter().any(|&id| {
                                frame.objects.iter().find(|o| o.id == id).is_some_and(|a| {
                                    presentation_weapon_reaches(a, target.position)
                                })
                            })
                    });
                if out_of_range {
                    ("OutRange", CursorIcon::NotAllowed)
                } else {
                    ("AttackObj", CursorIcon::Crosshair)
                }
            }
            Some(crate::command_system::CommandType::ForceAttackObject { .. }) => {
                ("ForceAttackObj", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::ForceAttackGround { .. }) => {
                ("ForceAttackGround", CursorIcon::Crosshair)
            }
            Some(crate::command_system::CommandType::Enter { .. }) => {
                ("EnterFriendly", CursorIcon::Copy)
            }
            Some(crate::command_system::CommandType::GetRepaired { .. }) => {
                ("GetRepaired", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::Repair { .. }) => {
                ("DoRepair", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::GetHealed { .. }) => {
                ("GetHealed", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::Dock { .. })
            | Some(crate::command_system::CommandType::Gather { .. }) => {
                ("Dock", CursorIcon::AllScroll)
            }
            Some(crate::command_system::CommandType::Hijack { .. })
            | Some(crate::command_system::CommandType::ConvertToCarbomb { .. })
            | Some(crate::command_system::CommandType::Sabotage { .. }) => {
                ("EnterAggressively", CursorIcon::Copy)
            }
            Some(crate::command_system::CommandType::DisableVehicleHack { .. })
            | Some(crate::command_system::CommandType::StealCashHack { .. })
            | Some(crate::command_system::CommandType::HackerDisableBuilding { .. }) => {
                ("Hack", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::OverrideSpecialPowerDestination {
                ..
            }) => ("ParticleUplinkCannon", CursorIcon::Crosshair),
            Some(crate::command_system::CommandType::SetRallyPoint { .. }) => {
                ("SetRallyPoint", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::ResumeConstruction { .. }) => {
                ("ResumeConstruction", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::CaptureBuilding { .. }) => {
                ("CaptureBuilding", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::MoveTo { .. })
            | Some(crate::command_system::CommandType::DoSalvage { .. })
            | Some(crate::command_system::CommandType::AttackMoveTo { .. }) => {
                ("Move", CursorIcon::AllScroll)
            }
            Some(crate::command_system::CommandType::AddWaypoint { .. }) => {
                ("Waypoint", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::Guard { .. }) => {
                ("Move", CursorIcon::AllScroll)
            }
            _ => {
                if hover.is_some() {
                    ("Select", CursorIcon::Pointer)
                } else {
                    ("Move", CursorIcon::AllScroll)
                }
            }
        }
    }

    pub(in crate::cnc_game_engine) fn lookat_input_enabled(&self) -> bool {
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheInGameUI::get_input_enabled()
        }
        #[cfg(not(feature = "game_client"))]
        {
            true
        }
    }

    pub(in crate::cnc_game_engine) fn look_at_player_broke_camera_lock(&self) -> bool {
        look_at_host_modes().camera_follow_lock_broken
    }

    pub(in crate::cnc_game_engine) fn look_at_clear_player_broke_camera_lock(&self) {
        look_at_host_modes().camera_follow_lock_broken = false;
    }

    fn host_camera_movement_finished(&self) -> bool {
        self.camera_zoom_target.is_none()
            && self.camera_pitch_target.is_none()
            && self.camera_yaw_target.is_none()
    }

    pub(in crate::cnc_game_engine) fn cancel_scripted_camera_from_player_set(&mut self) {
        note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::Set);
        self.camera_zoom_target = None;
        self.camera_pitch_target = None;
        self.camera_yaw_target = None;
        self.camera_zoom_duration = 0.0;
        self.camera_pitch_duration = 0.0;
        self.camera_yaw_duration = 0.0;
        self.game_logic.cancel_scripted_camera_from_player_set();
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                view.cancel_scripted_camera_from_player_set();
            });
        }
    }

    pub(in crate::cnc_game_engine) fn cancel_scripted_camera_from_player_look_at(&mut self) {
        note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::LookAt);
        self.camera_yaw_target = None;
        self.camera_yaw_duration = 0.0;
        self.game_logic.cancel_scripted_camera_from_player_look_at();
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                view.cancel_scripted_camera_from_player_look_at();
            });
        }
    }

    fn cancel_scripted_camera_from_player_scroll(&mut self) {
        self.camera_yaw_target = None;
        self.camera_yaw_duration = 0.0;
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                view.cancel_scripted_camera_from_player_scroll();
            });
        }
    }

    fn break_camera_follow_lock(&mut self) {
        look_at_host_modes().camera_follow_lock_broken = true;
        self.host_set_camera_follow_object(None);
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                // C++ InGameUI.cpp:2800-2801 setCameraLock(INVALID) +
                // setCameraLockDrawable(NULL).
                view.set_camera_lock(None);
                view.set_camera_lock_drawable(None);
            });
        }
    }

    /// C++ `W3DView::cameraEnableSlaveMode` + bone matrix slam.
    fn apply_host_slave_camera(&mut self) -> bool {
        let Some(mode) = self.camera_slave_mode.clone() else {
            return false;
        };
        let origin = self
            .last_presentation_frame
            .as_ref()
            .and_then(|frame| frame.first_alive_position_for_template(&mode.thing_template_name));
        let mut eye = None;
        let mut look = origin;
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                view.camera_enable_slave_mode(&mode.thing_template_name, &mode.bone_name);
            });
            let slaved =
                game_client::display::view::with_tactical_view_ref(|view| view.is_camera_slaved());
            if slaved {
                let p = game_client::display::view::with_tactical_view_ref(|view| {
                    view.get_3d_camera_position()
                });
                // Leftover View is C++ Z-up; live host is Y-up.
                eye = Some(Vec3::new(p.x, p.z, p.y));
            }
        }
        let Some(origin) = origin else {
            return false;
        };
        if let Some(eye_pos) = eye {
            self.camera_position = eye_pos;
            self.camera_target = look.unwrap_or(origin);
            self.view_matrix = Mat4::look_at_rh(self.camera_position, self.camera_target, Vec3::Y);
            true
        } else if !mode.bone_name.is_empty() {
            // Bone missing: still parent the camera at the unit, not an orbit look-at.
            self.camera_position = origin + Vec3::Y * 2.0;
            self.camera_target = origin;
            self.view_matrix = Mat4::look_at_rh(self.camera_position, self.camera_target, Vec3::Y);
            true
        } else {
            let clamped = self.clamp_to_world_bounds(origin);
            if (self.camera_target.x - clamped.x).abs() > 0.001
                || (self.camera_target.z - clamped.z).abs() > 0.001
            {
                self.camera_target.x = clamped.x;
                self.camera_target.z = clamped.z;
                true
            } else {
                false
            }
        }
    }

    /// C++ `W3DView.cpp:1224-1247` LOCK_FOLLOW airborne yaw ease.
    fn apply_airborne_follow_yaw(&mut self) -> bool {
        if self.look_at_player_broke_camera_lock() {
            return false;
        }
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return false;
        };
        if frame.camera_follow_position.is_none() {
            return false;
        }
        let follow = frame.camera_follow_position.unwrap();
        let follow = Vec3::new(follow[0], follow[1], follow[2]);
        let Some(obj) = frame.objects.iter().min_by(|a, b| {
            let da = (a.position - follow).length_squared();
            let db = (b.position - follow).length_squared();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        }) else {
            return false;
        };
        let airborne = obj.airborne_target
            || matches!(
                obj.object_type,
                crate::presentation_frame::PresentationObjectType::Aircraft
            );
        if !airborne {
            return false;
        }
        let ground = if obj.ground_height_from_terrain {
            obj.ground_height
        } else {
            self.sample_presentation_height_under(obj.position)
        };
        if obj.position.y <= PATHFIND_CELL_SIZE_F + ground {
            return false;
        }
        let ideal = normalize_signed_angle(obj.orientation - std::f32::consts::FRAC_PI_2);
        let diff = normalize_signed_angle(ideal - self.camera_yaw_radians);
        if diff.abs() < 1.0e-4 {
            return false;
        }
        self.camera_yaw_radians = normalize_signed_angle(self.camera_yaw_radians + diff * 0.1);
        true
    }

    /// C++ W3DDisplay.cpp:1883-1900 draws the software cursor then letterbox
    /// bars on top. Live has only the OS cursor, so hide it over the bars.
    fn sync_letterbox_os_cursor_visibility(&mut self) {
        #[cfg(feature = "game_client")]
        {
            if look_at_host_mouse_locked() {
                return;
            }
            let fade = self.game_client.letterbox_overlay_fade();
            let enabled = self.game_client.letterbox_overlay_enabled();
            let size = self.window.inner_size();
            let width = size.width.max(1) as f32;
            let height = size.height.max(1) as f32;
            let plan =
                game_client::display::display_fx::letterbox_plan(width, height, fade, enabled);
            let my = self.mouse_position.1;
            let over_bar = (plan.draw_top && my < plan.bar_height)
                || (plan.draw_bottom && my >= height - plan.bar_height);
            let hidden = look_at_host_modes().letterbox_os_cursor_hidden;
            if over_bar != hidden {
                self.window.set_cursor_visible(!over_bar);
                look_at_host_modes().letterbox_os_cursor_hidden = over_bar;
            }
        }
    }

    /// C++ `LookAtXlat.cpp:50-76` `setScrolling` / `stopScrolling`.
    ///
    /// Every exclusive scroll type (KEY / RMB / SCREENEDGE) mouse-locks the
    /// tactical view and sets InGameUI scrolling so WindowXlat keeps hover,
    /// RMB, and MMB off HUD gadgets. Idempotent: RMB start/stop may already
    /// have applied the same flags.
    fn set_lookat_scroll_mouse_lock(&mut self, locked: bool) {
        if look_at_host_modes().mouse_locked == locked {
            return;
        }
        if locked {
            let mut modes = look_at_host_modes();
            modes.prev_cursor = self.last_context_cursor;
            modes.mouse_locked = true;
            drop(modes);
            #[cfg(feature = "game_client")]
            {
                game_client::helpers::TheInGameUI::set_scrolling(true);
                game_client::display::view::with_tactical_view(|view| {
                    view.set_mouse_lock(true);
                });
            }
            // C++ InGameUI.cpp:2797 setMouseCursor(SCROLL).
            self.last_context_cursor = Some("Scroll");
            self.window.set_cursor(winit::window::CursorIcon::AllScroll);
        } else {
            {
                let mut modes = look_at_host_modes();
                modes.mouse_locked = false;
                // C++ LookAtXlat.cpp:70 TheMouse->setCursor(prevCursor).
                let _prev = modes.prev_cursor.take();
            }
            #[cfg(feature = "game_client")]
            {
                game_client::helpers::TheInGameUI::set_scrolling(false);
                game_client::display::view::with_tactical_view(|view| {
                    view.set_mouse_lock(false);
                });
            }
            self.last_context_cursor = None;
            self.sync_context_mouse_cursor();
        }
    }

    /// C++ `LookAtXlat.cpp:50-62` `setScrolling(SCROLL_RMB)`.
    pub(in crate::cnc_game_engine) fn start_rmb_lookat_scroll(&mut self) {
        lookat_stamp_mouse_activity(self.frame_counter);
        self.rmb_scroll_anchor = Some(self.mouse_position);
        // C++ :204-206: start only when `!isSelecting() && !m_isScrolling`.
        if self.host_is_selecting() || self.is_rmb_scrolling {
            return;
        }
        if look_at_host_modes().scroll_type.is_scrolling() {
            return;
        }
        if !self.lookat_input_enabled() {
            return;
        }
        let mut modes = look_at_host_modes();
        modes.wheel_stopped_scroll = false;
        modes.scroll_type = LookAtScrollType::Rmb;
        drop(modes);
        self.is_rmb_scrolling = true;
        self.break_camera_follow_lock();
        self.set_lookat_scroll_mouse_lock(true);
    }

    /// C++ `LookAtXlat.cpp:65-76` `stopScrolling`.
    pub(in crate::cnc_game_engine) fn stop_rmb_lookat_scroll(&mut self) {
        lookat_stamp_mouse_activity(self.frame_counter);
        {
            let mut modes = look_at_host_modes();
            if modes.scroll_type == LookAtScrollType::Rmb {
                modes.scroll_type = LookAtScrollType::None;
            }
        }
        self.is_rmb_scrolling = false;
        self.rmb_scroll_anchor = None;
        self.set_lookat_scroll_mouse_lock(false);
    }

    /// C++ `LookAtTranslator::resetModes` — drop scroll/rotate/pitch/FOV flags
    /// so a cinematic `doDisableInput` cannot leave the camera stuck.
    pub(in crate::cnc_game_engine) fn apply_look_at_reset_modes(&mut self) {
        if self.is_rmb_scrolling {
            self.stop_rmb_lookat_scroll();
        }
        // Live host also unlocks KEY/SCREENEDGE so WindowXlat cannot stay
        // suppressed after doDisableInput (C++ resetModes only clears flags).
        self.set_lookat_scroll_mouse_lock(false);
        self.is_mmb_rotating = false;
        self.mmb_anchor = None;
        self.camera_rotate_left_held = false;
        self.camera_rotate_right_held = false;
        self.camera_zoom_in_held = false;
        self.camera_zoom_out_held = false;
        let mut modes = look_at_host_modes();
        modes.mmb_original_anchor = None;
        modes.mmb_press_frame = 0;
        modes.scroll_type = LookAtScrollType::None;
    }

    /// C++ `LookAtXlat.cpp:224-233` middle-button down.
    pub(in crate::cnc_game_engine) fn begin_mmb_lookat_rotate(&mut self) {
        lookat_stamp_mouse_activity(self.frame_counter);
        self.is_mmb_rotating = true;
        self.mmb_anchor = Some(self.mouse_position);
        let mut modes = look_at_host_modes();
        modes.mmb_original_anchor = Some(self.mouse_position);
        modes.mmb_press_frame = self.frame_counter;
    }

    /// C++ `LookAtXlat.cpp:237-254` short MMB click resets angle/pitch/zoom.
    pub(in crate::cnc_game_engine) fn end_mmb_lookat_rotate(&mut self) {
        lookat_stamp_mouse_activity(self.frame_counter);
        let (original, press_frame) = {
            let mut modes = look_at_host_modes();
            (modes.mmb_original_anchor.take(), modes.mmb_press_frame)
        };
        self.is_mmb_rotating = false;
        self.mmb_anchor = None;
        let Some(origin) = original else {
            return;
        };
        let dx = self.mouse_position.0 - origin.0;
        let dy = self.mouse_position.1 - origin.1;
        let frames = self.frame_counter.saturating_sub(press_frame);
        if lookat_mmb_is_short_click(dx, dy, frames) {
            self.reset_camera_pose_in_place();
        }
    }

    /// C++ `InGameUI::resetCamera` + `W3DView::resetCamera`: keep look-at,
    /// restore default angle/pitch/zoom. Does not retarget the command center.
    pub(in crate::cnc_game_engine) fn reset_camera_pose_in_place(&mut self) {
        let defaults = Self::configured_startup_camera_defaults();
        // C++ setAngleAndPitchToDefault: m_pitchAngle = m_defaultPitchAngle.
        self.camera_yaw_radians = defaults.yaw_degrees.to_radians();
        self.camera_pitch_radians = live_home_pitch_radians(
            defaults.pitch_degrees,
            self.ui_script_default_camera_pitch(),
        );
        self.camera_yaw_target = None;
        self.camera_pitch_target = None;
        self.camera_zoom_target = None;
        // C++ W3DView::setAngleAndPitchToDefault / resetCamera: m_FXPitch = 1.0.
        self.camera_fx_pitch = 1.0;
        self.camera_zoom = self.compute_default_camera_zoom_for_target(
            self.camera_target,
            self.ui_script_default_camera_max_height(),
        );
        // C++ resetCamera/setZoom invalidates m_cameraConstraint.
        self.scripted_camera_constraint_widen = None;
        self.apply_camera_orbit_transform();
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.update_mouse_world_position();
            self.sync_context_mouse_cursor();
        }
    }

    /// C++ `LookAtXlat.cpp:550-587` SAVE_VIEW / VIEW_VIEW full `ViewLocation`.
    pub(in crate::cnc_game_engine) fn save_or_recall_camera_view(&mut self, slot: usize) {
        let save = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        self.store_or_apply_camera_view(slot, save);
    }

    /// Explicit SAVE_VIEW (`save`) vs VIEW_VIEW so Keyboard Options remaps
    /// do not depend on the live Ctrl key.
    pub(in crate::cnc_game_engine) fn store_or_apply_camera_view(
        &mut self,
        slot: usize,
        save: bool,
    ) {
        if slot >= 8 {
            return;
        }
        if save {
            let loc = CameraViewLocation {
                pos: self.camera_target,
                yaw: self.camera_yaw_radians,
                pitch: self.camera_pitch_radians,
                zoom: self.camera_zoom,
            };
            look_at_host_modes().views[slot] = Some(loc);
            self.camera_view_bookmarks[slot] = Some(loc.pos);
            let msg = lookat_bookmark_message(slot + 1);
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
        } else if let Some(loc) = look_at_host_modes().views[slot] {
            let clamped = self.clamp_to_world_bounds(loc.pos);
            self.camera_target = clamped;
            self.camera_yaw_radians = loc.yaw;
            self.camera_pitch_radians = loc.pitch;
            self.camera_zoom = clamp_w3d_zoom(loc.zoom);
            self.camera_yaw_target = None;
            self.camera_pitch_target = None;
            self.camera_zoom_target = None;
            look_at_host_modes().desired_height_above_ground = None;
            self.apply_camera_orbit_transform();
            if matches!(self.current_state, GameState::InGame | GameState::Paused) {
                self.update_mouse_world_position();
                self.sync_context_mouse_cursor();
            }
        } else if let Some(pos) = self.camera_view_bookmarks[slot] {
            // Older position-only bookmark: restore look-at, keep current pose extras.
            let _ = self.host_center_camera_and_request_focus(pos);
        }
        // C++ View::setLocation no-ops when !m_valid. Unsaved F1-F8 is silent.
    }

    pub(in crate::cnc_game_engine) fn clear_look_at_host_modes(&mut self) {
        self.apply_look_at_reset_modes();
    }
}
