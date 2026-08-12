#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::input::MouseInputOrigin;
use super::*;

/// Physical input metadata held only across the synchronous right-click command
/// execution boundary. `issued_at` is part of the command's own immutable
/// fingerprint, so an unrelated queued/AI Gather event cannot be mistaken for
/// this physical input edge.
#[derive(Debug, Clone)]
struct PhysicalGatherAttempt {
    command_id: u32,
    issued_at: SystemTime,
    player_id: u32,
    target_id: ObjectId,
}

impl PhysicalGatherAttempt {
    fn from_right_click_command(
        command: &crate::command_system::GameCommand,
        origin: MouseInputOrigin,
        physical_rmb_gesture: bool,
    ) -> Option<Self> {
        if !matches!(origin, MouseInputOrigin::Physical) || !physical_rmb_gesture {
            return None;
        }
        let crate::command_system::CommandType::Gather { target_id } = &command.command_type else {
            return None;
        };
        Some(Self {
            command_id: command.command_id,
            issued_at: command.timestamp.clone(),
            player_id: command.player_id,
            target_id: *target_id,
        })
    }

    fn matches(&self, event: &crate::game_logic::AcceptedGatherCommand) -> bool {
        self.command_id == event.command_id
            && self.issued_at == event.issued_at
            && self.player_id == event.player_id
            && self.target_id == event.target_id
    }
}

impl CnCGameEngine {
    pub(super) fn handle_left_click(&mut self) {
        self.is_dragging = true;
        self.selection_start = Some(self.mouse_world_position);
        self.selection_start_screen = Some(self.mouse_position);

        let mouse_pos = self.mouse_world_position;
        let clicked_object = self.find_object_at_position(mouse_pos, false);

        // Check for double-click
        let now = Instant::now();
        let is_double_click = if let (Some(last_time), Some(last_pos)) =
            (self.last_click_time, self.last_click_position)
        {
            let time_delta = now.duration_since(last_time).as_millis();
            let pos_delta = (mouse_pos - last_pos).length();
            time_delta < 500 && pos_delta < 10.0
        } else {
            false
        };

        self.last_click_time = Some(now);
        self.last_click_position = Some(mouse_pos);

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));

        if is_double_click && clicked_object.is_some() && !ctrl_down {
            // Double-click: select all similar units
            if let Some(object_id) = clicked_object {
                self.select_similar_units(object_id);
            }
        } else {
            // Single-click behavior
            if self.pending_map_command.is_some() {
                let loc = self.mouse_world_position;
                self.commit_pending_map_command(loc, clicked_object);
            } else if ctrl_down && !self.selected_objects.is_empty() {
                // C++ ForceAttack residual: Ctrl+LMB with selection issues force-attack.
                self.issue_force_attack_from_left_click(mouse_pos, clicked_object);
            } else if let Some(object_id) = clicked_object {
                if shift_down {
                    // C++ Shift+select residual: toggle unit in multi-selection.
                    self.toggle_select_object(object_id);
                } else {
                    // Select this object
                    // Wave 1104: belt-and-suspenders local selectable check (pick peels FOW first).
                    let selectable = self.last_presentation_frame.as_ref().is_some_and(|frame| {
                        let local = frame.local_team();
                        frame.objects.iter().any(|o| {
                            o.id == object_id
                                && o.team == local
                                && crate::unit_control::UnitControlSystem::presentation_is_selectable(
                                    o,
                                )
                        })
                    });
                    if !selectable {
                        return;
                    }
                    // Wave 583: selection residual via host_set_selection.
                    self.host_set_selection(self.current_player_id, vec![object_id]);
                    self.play_sound_effect(SoundType::Select);
                }
            } else if let Some(template) = self.pending_structure_placement.clone() {
                // Wall/fence residual: defer commit to left-release so drag can form a line.
                if Self::is_wall_structure_template(&template) {
                    // selection_start already set at top of handle_left_click.
                } else {
                    // C++ structure placement residual: empty-ground click commits DozerConstruct.
                    let loc = self.mouse_world_position;
                    self.place_structure_from_ui(&template, loc);
                }
            } else {
                // Defer empty-ground clear until left-release if this becomes a box drag.
                // Instant clear on mousedown fights drag-select residual.
            }
        }
    }

    /// Shift+click residual: add friendly unit or remove if already selected.
    pub(super) fn toggle_select_object(&mut self, object_id: ObjectId) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let player_team = frame.local_team();

        // Only toggle friendly selectable units (enemy click under Shift still replaces? retail
        // keeps multi-select among friendlies; enemy under Shift is ignored for add).
        let is_friendly_selectable = frame
            .objects
            .iter()
            .find(|o| o.id == object_id)
            .map(|o| {
                o.team == player_team
                    && !o.destroyed
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
            })
            .unwrap_or(false);
        if !is_friendly_selectable {
            return;
        }

        let mut selection = self.selected_objects.clone();
        if let Some(idx) = selection.iter().position(|id| *id == object_id) {
            selection.remove(idx);
        } else {
            selection.push(object_id);
        }
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, selection);
        self.play_sound_effect(SoundType::Select);
    }

    /// Ctrl+LMB ForceAttack residual (object or ground).
    /// Wave 612: via `host_issue_force_attack_from_left_click`.
    pub(super) fn issue_force_attack_from_left_click(
        &mut self,
        location: Vec3,
        target_object: Option<ObjectId>,
    ) {
        // Wave 612: thin wrapper — residual via host helper.
        self.host_issue_force_attack_from_left_click(location, target_object)
    }

    /// Ctrl+LMB ForceAttack residual (object or ground).
    pub(super) fn host_issue_force_attack_from_left_click(
        &mut self,
        location: Vec3,
        target_object: Option<ObjectId>,
    ) {
        // Wave 612: host residual helper.
        // Wave 234: selection prefers engine/presentation freeze.
        let mut selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            return;
        }

        let command_type = if let Some(tid) = target_object {
            crate::command_system::CommandType::ForceAttackObject { target_id: tid }
        } else {
            crate::command_system::CommandType::ForceAttackGround { location }
        };
        self.host_queue_command(crate::command_system::GameCommand {
            command_type,
            player_id: self.current_player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: selected,
            modifier_keys: crate::command_system::ModifierKeys {
                ctrl: true,
                shift: false,
                alt: false,
            },
        });
        self.host_process_commands_with_command_sound();
    }

    pub(super) fn select_similar_units(&mut self, clicked_object_id: ObjectId) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let player_team = frame.local_team();
        let similar_units = frame.similar_unit_ids(clicked_object_id, player_team);
        let template_label = frame
            .objects
            .iter()
            .find(|o| o.id == clicked_object_id)
            .map(|o| o.template_name.clone())
            .unwrap_or_default();

        if !similar_units.is_empty() {
            // Wave 583: selection residual via host_set_selection.
            self.host_set_selection(self.current_player_id, similar_units);
            self.play_sound_effect(SoundType::Select);
            info!(
                "Selected {} similar units ({})",
                self.selected_objects.len(),
                template_label
            );
        }
    }

    pub(super) fn handle_left_release(&mut self) {
        self.is_dragging = false;
        self.selection_start_screen = None;

        let Some(start) = self.selection_start.take() else {
            return;
        };

        let end = self.mouse_world_position;

        // If the mouse didn't move enough, the click selection was already handled on mouse-down.
        let drag_distance = Vec2::new(end.x - start.x, end.z - start.z).length();
        if drag_distance < 5.0 {
            // Wall residual: short click places a single segment.
            if let Some(template) = self.pending_structure_placement.clone() {
                if Self::is_wall_structure_template(&template) {
                    self.place_structure_from_ui(&template, end);
                    return;
                }
            }
            // Click on empty ground (no pending command/placement handled on press): clear selection.
            if self.pending_map_command.is_none()
                && self.pending_structure_placement.is_none()
                && self.find_object_at_position(end, false).is_none()
            {
                let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                if !shift_down {
                    self.selected_objects.clear();
                    // Wave 583: clear selection residual via host_set_selection.
                    self.host_set_selection(self.current_player_id, Vec::new());
                }
            }
            return;
        }

        // Wall/fence drag residual: DozerConstructLine along the drag segment.
        if let Some(template) = self.pending_structure_placement.clone() {
            if Self::is_wall_structure_template(&template) {
                self.place_wall_line_from_ui(&template, start, end);
                return;
            }
        }

        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        let min_z = start.z.min(end.z);
        let max_z = start.z.max(end.z);

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));

        let mut selection: Vec<ObjectId> = if shift_down {
            self.selected_objects.clone()
        } else {
            Vec::new()
        };

        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let player_team = frame.local_team();
        let boxed: Vec<ObjectId> =
            frame.box_select_unit_ids(player_team, min_x, max_x, min_z, max_z);
        for id in boxed {
            if !selection.contains(&id) {
                selection.push(id);
            }
        }

        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, selection);
        self.play_sound_effect(SoundType::Select);
    }

    /// Issue a context-sensitive RMB command. Physical gather evidence is
    /// threaded through this exact command path, but only becomes tracked after
    /// GameLogic reports the Gather command actually accepted its carrier IDs.
    pub(super) fn handle_right_click(
        &mut self,
        origin: MouseInputOrigin,
        physical_rmb_gesture: bool,
    ) {
        let mouse_pos = self.mouse_world_position;

        // Prefer live player selection; fall back to engine selection residual.
        // Wave 234: selection prefers engine/presentation freeze.
        let mut selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            return;
        }

        // C++ context-sensitive right-click residual via CommandSystem:
        // attack / gather / repair / enter / get-repaired / get-healed / move / attack-move.
        let target_object = self.find_object_at_position(mouse_pos, true);
        let ctrl = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control)
            )
        });
        let shift = self.keys_pressed.iter().any(|k| {
            matches!(
                k,
                winit::keyboard::Key::Named(winit::keyboard::NamedKey::Shift)
            )
        });
        let alt = self.sticky_waypoint_mode
            || self.keys_pressed.iter().any(|k| {
                matches!(
                    k,
                    winit::keyboard::Key::Named(winit::keyboard::NamedKey::Alt)
                )
            });

        let context = crate::command_system::MouseCommandContext {
            world_position: mouse_pos,
            target_object,
            target_presentation: target_object.and_then(|id| self.presentation_target_hint(id)),
            selected_presentation: self.presentation_selected_unit_hints(&selected),
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::new(self.mouse_position.0, self.mouse_position.1),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: crate::command_system::MouseButton::Right,
            modifier_keys: crate::command_system::ModifierKeys { ctrl, shift, alt },
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let mut cmd_sys = crate::command_system::CommandSystem::new();
        // Wave 236: presentation-only mouse path when frame installed.
        let command = cmd_sys.process_mouse_input(
            &context,
            &selected,
            self.current_player_id,
            self.presentation_mouse_game_logic(),
        );

        if let Some(mut command) = command {
            if self.sticky_auto_attack {
                if let crate::command_system::CommandType::MoveTo { destination, .. } =
                    command.command_type
                {
                    command.command_type = crate::command_system::CommandType::AttackMoveTo {
                        destination,
                        max_shots: -1,
                    };
                }
            }
            self.host_queue_and_process_right_click_command(command, origin, physical_rmb_gesture);
            return;
        }

        // Fail-closed fallback residual: move if context path produced nothing.
        if self.sticky_auto_attack {
            self.host_command_attack_move(self.current_player_id, mouse_pos);
        } else {
            self.host_command_move(self.current_player_id, mouse_pos);
        }
        self.play_sound_effect(SoundType::Command);
    }

    /// Run the normal synchronous command authority path, then bind a physical
    /// Gather attempt to its executor-confirmed carrier subset. Injected,
    /// runtime-host, and AI commands have no physical attempt and are consumed
    /// without ever entering `physical_gather_carrier_ids`.
    fn host_queue_and_process_right_click_command(
        &mut self,
        command: crate::command_system::GameCommand,
        origin: MouseInputOrigin,
        physical_rmb_gesture: bool,
    ) {
        let physical_attempt =
            PhysicalGatherAttempt::from_right_click_command(&command, origin, physical_rmb_gesture);
        self.host_queue_and_process_command(command);

        // `host_queue_and_process_command` is synchronous. Always consume the
        // transient events, even for nonphysical clicks, so background/AI
        // Gather traffic cannot accumulate or be matched by a later input.
        let accepted_gathers = self.host_game_logic_mut().take_accepted_gather_commands();
        let Some(attempt) = physical_attempt else {
            return;
        };
        if !self.host_physical_gather_evidence_eligible()
            || attempt.player_id != self.local_player_id_for_ui()
        {
            return;
        }

        for event in accepted_gathers {
            if attempt.matches(&event) {
                // `execute_gather` emitted only workers selected by this
                // command whose local-team Gather path was accepted.
                self.physical_gather_carrier_ids.extend(event.carrier_ids);
            }
        }
    }

    /// Consume economy events only from the real ReturningResources deposit
    /// branch. A resource-total delta, passive income, scripted income, or an
    /// untracked/remote carrier is deliberately insufficient.
    pub(super) fn host_drain_physical_gather_dropoffs(&mut self) {
        // Clear any non-input Gather acceptances that arose during simulation;
        // a physical RMB path consumes its own event synchronously above.
        let _ = self.host_game_logic_mut().take_accepted_gather_commands();
        let dropoffs = self.host_game_logic_mut().take_supply_dropoff_events();
        if dropoffs.is_empty() || !self.host_physical_gather_evidence_eligible() {
            return;
        }

        let local_player_id = self.local_player_id_for_ui();
        for dropoff in dropoffs {
            let is_tracked_local_deposit = dropoff.carried_amount > 0
                && dropoff.player_id == local_player_id
                && self
                    .physical_gather_carrier_ids
                    .contains(&dropoff.carrier_id);
            self.interactive_playability
                .note_physical_gather_resources(is_tracked_local_deposit);
        }
    }

    /// A physical Gather proof is valid only in a visible, non-headless,
    /// offline match. This intentionally does not infer input provenance from
    /// `CommandSourceType::FromUser` or a runtime-host command name.
    fn host_physical_gather_evidence_eligible(&self) -> bool {
        !self.runtime_host_headless
            && self.runtime_host_window_visible()
            && matches!(self.current_state, GameState::InGame)
            && matches!(
                self.host_match_game_mode,
                Some(
                    crate::game_logic::GameMode::SinglePlayer
                        | crate::game_logic::GameMode::Skirmish
                )
            )
    }

    pub(super) fn handle_mouse_wheel(&mut self, delta: &winit::event::MouseScrollDelta) {
        use winit::event::MouseScrollDelta;

        let delta_y = match delta {
            MouseScrollDelta::LineDelta(_, y) => *y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
        };

        #[cfg(feature = "game_client")]
        self.inject_game_client_mouse_scroll(delta_y);

        // C++ place-building rotate residual: wheel turns ghost while placement armed.
        if self.pending_structure_placement.is_some() {
            let step = delta_y * std::f32::consts::FRAC_PI_4; // 45 deg per notch
            self.game_hud
                .construction_panel
                .rotate_structure_placement(-step);
            self.ui_manager
                .game_hud_mut()
                .construction_panel
                .rotate_structure_placement(-step);
            return;
        }

        // Zoom camera with mouse wheel
        let zoom_speed = 0.1;
        let new_zoom = (self.camera_zoom - delta_y * zoom_speed).clamp(0.1, 5.0);

        if (new_zoom - self.camera_zoom).abs() > 0.001 {
            self.camera_zoom = new_zoom;
            debug!("Camera zoom changed to {:.2}", self.camera_zoom);
        }
    }

    pub(super) fn update_camera(&mut self, dt: f32) {
        // Retail KP4/KP6 rotate and KP8/KP2 zoom hold residual.
        const ROTATE_RAD_PER_SEC: f32 = 1.2;
        const ZOOM_PER_SEC: f32 = 0.85;
        if self.camera_rotate_left_held {
            self.camera_yaw_radians -= ROTATE_RAD_PER_SEC * dt;
        }
        if self.camera_rotate_right_held {
            self.camera_yaw_radians += ROTATE_RAD_PER_SEC * dt;
        }
        if self.camera_zoom_in_held {
            self.camera_zoom = (self.camera_zoom - ZOOM_PER_SEC * dt).clamp(0.1, 5.0);
        }
        if self.camera_zoom_out_held {
            self.camera_zoom = (self.camera_zoom + ZOOM_PER_SEC * dt).clamp(0.1, 5.0);
        }

        self.update_camera_tracking_drawable();

        let mut movement = Vec3::ZERO;
        if self.camera_slave_mode.is_none() {
            let logic_frames_per_second =
                game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32;
            let (
                horizontal_scroll_speed_factor,
                vertical_scroll_speed_factor,
                keyboard_scroll_factor,
            ) = {
                let global_data = game_engine::common::global_data::read();
                (
                    global_data.horizontal_scroll_speed_factor,
                    global_data.vertical_scroll_speed_factor,
                    global_data.keyboard_scroll_factor,
                )
            };

            // C++ parity (LookAtXlat.cpp): key scrolling uses SCROLL_AMT=100 in screen-space and
            // applies horizontal/vertical/keyboard factors once per logic frame.
            const SCROLL_AMT: f32 = 100.0;
            let scroll_step =
                SCROLL_AMT * keyboard_scroll_factor * dt.max(0.0) * logic_frames_per_second;
            let mut screen_scroll = Vec2::ZERO;
            // C++ LookAt keyboard scroll uses arrows (not WASD).
            // WASD are unit hotkeys: A attack-move, S stop, D deploy, etc.
            let mods_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control))
                || self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                || self.keys_pressed.contains(&Key::Named(NamedKey::Alt));
            let ui_modal = self.chat_panel.is_open() || self.diplomacy_panel.is_active();
            if !mods_down && !ui_modal {
                if self.keys_pressed.contains(&Key::Named(NamedKey::ArrowUp)) {
                    screen_scroll.y -= vertical_scroll_speed_factor * scroll_step;
                }
                if self.keys_pressed.contains(&Key::Named(NamedKey::ArrowDown)) {
                    screen_scroll.y += vertical_scroll_speed_factor * scroll_step;
                }
                if self.keys_pressed.contains(&Key::Named(NamedKey::ArrowLeft)) {
                    screen_scroll.x -= horizontal_scroll_speed_factor * scroll_step;
                }
                if self
                    .keys_pressed
                    .contains(&Key::Named(NamedKey::ArrowRight))
                {
                    screen_scroll.x += horizontal_scroll_speed_factor * scroll_step;
                }
            }

            // Edge scrolling (C++ LookAt.cpp: near screen edge).
            // Enable for windowed + fullscreen so map-panning works without arrows.
            // Headless runtime-host residual: mouse stays at (0,0) without OS cursor
            // events, which would permanently edge-scroll the camera off the map.
            if matches!(self.current_state, GameState::InGame | GameState::Paused)
                && !self.runtime_host_headless
                && !self.chat_panel.is_open()
                && !self.diplomacy_panel.is_active()
            {
                const EDGE_SCROLL_SIZE: f32 = 5.0;
                let (mx, my) = self.mouse_position;
                let size = self.window.inner_size();
                let win_w = size.width as f32;
                let win_h = size.height as f32;

                let mut edge_dx = 0.0f32;
                let mut edge_dy = 0.0f32;

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

                if edge_dx != 0.0 || edge_dy != 0.0 {
                    let edge_step =
                        SCROLL_AMT * keyboard_scroll_factor * dt.max(0.0) * logic_frames_per_second;
                    screen_scroll.x += edge_dx * horizontal_scroll_speed_factor * edge_step;
                    screen_scroll.y += edge_dy * vertical_scroll_speed_factor * edge_step;
                }
            }

            // Right-mouse-button drag scrolling (C++ LookAtXlat.cpp:378-406)
            if self.is_rmb_scrolling {
                if let Some(anchor) = self.rmb_scroll_anchor {
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
                        screen_scroll += offset * dt.max(0.0) * logic_frames_per_second;
                    }
                }
            }

            // Middle-mouse-button camera yaw rotation (C++ LookAtXlat.cpp)
            if self.is_mmb_rotating {
                if let Some(anchor) = self.mmb_anchor {
                    let dx = self.mouse_position.0 - anchor.0;
                    self.camera_yaw_radians += dx * 0.005;
                }
                self.mmb_anchor = Some(self.mouse_position);
            }

            movement = self.camera_scroll_world_delta(screen_scroll);
        }

        let mut camera_changed = false;

        if movement.length() > 0.0 {
            self.camera_target += movement;
            camera_changed = true;
        }

        if let Some(mode) = self.camera_slave_mode.as_ref() {
            // Prefer dual-tick presentation pose so camera follow does not re-read live transforms.
            let target = if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.first_alive_position_for_template(&mode.thing_template_name)
            } else {
                // Presentation required (no live get_objects dual-read).
                None
            };
            if let Some(target) = target {
                let clamped = self.clamp_to_world_bounds(target);
                if (self.camera_target.x - clamped.x).abs() > 0.001
                    || (self.camera_target.z - clamped.z).abs() > 0.001
                {
                    self.camera_target.x = clamped.x;
                    self.camera_target.z = clamped.z;
                    camera_changed = true;
                }
            }
        }

        if let Some(target) = self.camera_zoom_target {
            if self.camera_zoom_duration <= 0.0 {
                self.camera_zoom = target;
                self.camera_zoom_target = None;
            } else {
                self.camera_zoom_elapsed += dt;
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
                self.camera_pitch_radians = target;
                self.camera_pitch_target = None;
                camera_changed = true;
            } else {
                self.camera_pitch_elapsed += dt;
                let t = (self.camera_pitch_elapsed / self.camera_pitch_duration).clamp(0.0, 1.0);
                let eased = Self::parabolic_ease(
                    t,
                    self.camera_pitch_ease_in / self.camera_pitch_duration,
                    self.camera_pitch_ease_out / self.camera_pitch_duration,
                );
                self.camera_pitch_radians =
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
                self.camera_yaw_elapsed += dt;
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

        // Wave 250: prefer presentation freeze residual when a frame is installed.
        let shake_dt = if self.presentation_or_boot_time_frozen() {
            0.0
        } else {
            dt
        };
        if self.update_script_camera_shake(shake_dt) {
            camera_changed = true;
        }

        if camera_changed {
            self.apply_camera_orbit_transform();
        }
    }

    pub(super) fn is_character_key_pressed(&self, expected: &str) -> bool {
        self.keys_pressed.iter().any(|key| match key {
            Key::Character(ch) => ch.eq_ignore_ascii_case(expected),
            _ => false,
        })
    }

    pub(super) fn camera_scroll_world_delta(&self, screen_scroll: Vec2) -> Vec3 {
        if screen_scroll.length_squared() <= f32::EPSILON {
            return Vec3::ZERO;
        }

        // Match C++ key-scroll semantics: "up/down/left/right" are screen-space intents.
        // Convert that intent to world-plane motion relative to current camera facing.
        let mut forward = self.camera_target - self.camera_position;
        forward.y = 0.0;
        if forward.length_squared() <= f32::EPSILON {
            return Vec3::ZERO;
        }
        let forward = forward.normalize();
        let right = Vec3::new(forward.z, 0.0, -forward.x);

        // C++ uses y- for UP and y+ for DOWN, so negate Y when mapping to forward motion.
        (right * screen_scroll.x) + (forward * -screen_scroll.y)
    }

    /// C++ InGameUI context cursor residual mapped onto winit CursorIcon.
    ///
    /// Fail-closed vs full Mouse.cpp ANI/CUR assets — uses platform icons with
    /// residual names from `MOUSE_CURSOR_INI_NAME_LIST`.
    pub(super) fn sync_context_mouse_cursor(&mut self) {
        use winit::window::CursorIcon;
        let (name, icon) = self.resolve_context_cursor_icon();
        if self.last_context_cursor == Some(name) {
            return;
        }
        self.last_context_cursor = Some(name);
        self.window.set_cursor(icon);
    }

    /// Wave 612: via `host_resolve_context_cursor_icon`.
    pub(super) fn resolve_context_cursor_icon(&self) -> (&'static str, winit::window::CursorIcon) {
        // Wave 612: thin wrapper — residual via host helper.
        self.host_resolve_context_cursor_icon()
    }

    pub(super) fn host_resolve_context_cursor_icon(
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
                PendingMapCommand::CombatDrop => ("CombatDrop", CursorIcon::Move),
                PendingMapCommand::PlaceBeacon => ("PlaceBeacon", CursorIcon::Cell),
                PendingMapCommand::SpecialPower(_) => ("Target", CursorIcon::Crosshair),
                PendingMapCommand::Weapon(_) => ("Target", CursorIcon::Crosshair),
                PendingMapCommand::UnitAbility(_) => ("Target", CursorIcon::Crosshair),
            };
        }

        // Wave 234: selection presence prefers engine/presentation freeze.
        let has_selection = !self.ui_selected_ids(self.current_player_id).is_empty();

        let hover = self.find_object_at_position(self.mouse_world_position, true);
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
                let player_team = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame.local_team()
                } else {
                    self.local_team_for_ui()
                };
                let friendly = if let Some(frame) = self.last_presentation_frame.as_ref() {
                    frame
                        .objects
                        .iter()
                        .find(|o| o.id == id)
                        .map(|o| {
                            // Wave 1098: cursor Select residual uses full presentation
                            // selectable legality (sold/unselectable/masked/disabled).
                            o.team == player_team
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
            Some(crate::command_system::CommandType::AttackObject { .. }) => {
                ("AttackObj", CursorIcon::Crosshair)
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
            Some(crate::command_system::CommandType::GetRepaired { .. })
            | Some(crate::command_system::CommandType::Repair { .. }) => {
                ("GetRepaired", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::ResumeConstruction { .. }) => {
                ("ResumeConstruction", CursorIcon::Progress)
            }
            Some(crate::command_system::CommandType::CaptureBuilding { .. }) => {
                ("CaptureBuilding", CursorIcon::Cell)
            }
            Some(crate::command_system::CommandType::MoveTo { .. })
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

    /// Feed Main-owned OS keyboard state into GameClient Keyboard device residual.
    /// Main still owns command translation / hotkeys.
    /// Wave 606: via `host_inject_game_client_key`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_key(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) {
        // Wave 606: thin wrapper — OS key inject via host helper.
        self.host_inject_game_client_key(physical_key, pressed);
    }

    /// Wave 606: host OS→GameClient key inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_key(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) {
        // Wave 606: host OS key inject residual.
        if let Some(code) = Self::to_game_client_key_code(physical_key) {
            game_client::input::keyboard::with_keyboard(|kb| {
                let _ = kb.handle_key_simple(code, pressed);
            });
        }
    }

    /// Map winit physical keys to GameClient KeyCode without sharing winit types across crates.
    #[cfg(feature = "game_client")]
    pub(super) fn to_game_client_key_code(
        physical_key: &winit::keyboard::PhysicalKey,
    ) -> Option<game_client::input::KeyCode> {
        use game_client::input::KeyCode as Gk;
        use winit::keyboard::{KeyCode as Wk, PhysicalKey};
        let PhysicalKey::Code(code) = physical_key else {
            return None;
        };
        Some(match code {
            Wk::Escape => Gk::Escape,
            Wk::Enter | Wk::NumpadEnter => Gk::Enter,
            Wk::Space => Gk::Space,
            Wk::Tab => Gk::Tab,
            Wk::Backspace => Gk::Backspace,
            Wk::Delete => Gk::Delete,
            Wk::Home => Gk::Home,
            Wk::End => Gk::End,
            Wk::PageUp => Gk::PageUp,
            Wk::PageDown => Gk::PageDown,
            Wk::ArrowLeft => Gk::Left,
            Wk::ArrowRight => Gk::Right,
            Wk::ArrowUp => Gk::Up,
            Wk::ArrowDown => Gk::Down,
            Wk::ShiftLeft => Gk::LeftShift,
            Wk::ShiftRight => Gk::RightShift,
            Wk::ControlLeft => Gk::LeftCtrl,
            Wk::ControlRight => Gk::RightCtrl,
            Wk::AltLeft => Gk::LeftAlt,
            Wk::AltRight => Gk::RightAlt,
            Wk::KeyA => Gk::A,
            Wk::KeyB => Gk::B,
            Wk::KeyC => Gk::C,
            Wk::KeyD => Gk::D,
            Wk::KeyE => Gk::E,
            Wk::KeyF => Gk::F,
            Wk::KeyG => Gk::G,
            Wk::KeyH => Gk::H,
            Wk::KeyI => Gk::I,
            Wk::KeyJ => Gk::J,
            Wk::KeyK => Gk::K,
            Wk::KeyL => Gk::L,
            Wk::KeyM => Gk::M,
            Wk::KeyN => Gk::N,
            Wk::KeyO => Gk::O,
            Wk::KeyP => Gk::P,
            Wk::KeyQ => Gk::Q,
            Wk::KeyR => Gk::R,
            Wk::KeyS => Gk::S,
            Wk::KeyT => Gk::T,
            Wk::KeyU => Gk::U,
            Wk::KeyV => Gk::V,
            Wk::KeyW => Gk::W,
            Wk::KeyX => Gk::X,
            Wk::KeyY => Gk::Y,
            Wk::KeyZ => Gk::Z,
            Wk::Digit0 => Gk::Num0,
            Wk::Digit1 => Gk::Num1,
            Wk::Digit2 => Gk::Num2,
            Wk::Digit3 => Gk::Num3,
            Wk::Digit4 => Gk::Num4,
            Wk::Digit5 => Gk::Num5,
            Wk::Digit6 => Gk::Num6,
            Wk::Digit7 => Gk::Num7,
            Wk::Digit8 => Gk::Num8,
            Wk::Digit9 => Gk::Num9,
            Wk::F1 => Gk::F1,
            Wk::F2 => Gk::F2,
            Wk::F3 => Gk::F3,
            Wk::F4 => Gk::F4,
            Wk::F5 => Gk::F5,
            Wk::F6 => Gk::F6,
            Wk::F7 => Gk::F7,
            Wk::F8 => Gk::F8,
            Wk::F9 => Gk::F9,
            Wk::F10 => Gk::F10,
            Wk::F11 => Gk::F11,
            Wk::F12 => Gk::F12,
            _ => return None,
        })
    }

    /// Feed Main-owned OS mouse state into GameClient Mouse device residual.
    /// Main still owns command translation; this keeps client device state honest
    /// for presentation-shell UI without dual OS event ownership.
    /// Wave 606: via `host_inject_game_client_mouse_move`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_mouse_move(&self, x: f32, y: f32) {
        // Wave 606: thin wrapper — OS mouse move inject via host helper.
        self.host_inject_game_client_mouse_move(x, y);
    }

    /// Wave 606: host OS→GameClient mouse-move inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_mouse_move(&self, x: f32, y: f32) {
        // Wave 606: host OS mouse move inject residual.
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_mouse_move(x, y);
        });
    }

    /// Wave 606: via `host_inject_game_client_mouse_button`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_mouse_button(&self, button: MouseButton, pressed: bool) {
        // Wave 606: thin wrapper — OS mouse button inject via host helper.
        self.host_inject_game_client_mouse_button(button, pressed);
    }

    /// C++ WindowXlat: OS button → `TheWindowManager` gadget hit-test.
    /// Returns true when the WND consumed the click (shell active or gadget Used).
    /// C++ WindowXlat RAW_KEY → focused gadget `GWM_CHAR`.
    pub(super) fn dispatch_os_key_to_window_manager(
        &self,
        physical_key: &winit::keyboard::PhysicalKey,
        pressed: bool,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::WindowInputReturnCode;
            let Some(vk) = Self::winit_physical_key_to_wnd_vk(physical_key) else {
                return false;
            };
            // C++ KEY_STATE_DOWN=0x02, KEY_STATE_UP=0x01
            let state = if pressed { 0x02 } else { 0x01 };
            game_client::gui::dispatch_os_key_to_window_manager(vk, state)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (physical_key, pressed);
            false
        }
    }

    pub(super) fn winit_physical_key_to_wnd_vk(
        physical_key: &winit::keyboard::PhysicalKey,
    ) -> Option<u8> {
        use winit::keyboard::{KeyCode as Wk, PhysicalKey};
        let PhysicalKey::Code(code) = physical_key else {
            return None;
        };
        Some(match code {
            Wk::Escape => 0x1B,
            Wk::Enter | Wk::NumpadEnter => 13,
            Wk::Space => 32,
            Wk::Tab => 9,
            Wk::Backspace => 8,
            Wk::Delete => 0x2E,
            Wk::Home => 36,
            Wk::End => 35,
            Wk::PageUp => 33,
            Wk::PageDown => 34,
            Wk::ArrowLeft => 37,
            Wk::ArrowUp => 38,
            Wk::ArrowRight => 39,
            Wk::ArrowDown => 40,
            Wk::KeyA => b'A',
            Wk::KeyB => b'B',
            Wk::KeyC => b'C',
            Wk::KeyD => b'D',
            Wk::KeyE => b'E',
            Wk::KeyF => b'F',
            Wk::KeyG => b'G',
            Wk::KeyH => b'H',
            Wk::KeyI => b'I',
            Wk::KeyJ => b'J',
            Wk::KeyK => b'K',
            Wk::KeyL => b'L',
            Wk::KeyM => b'M',
            Wk::KeyN => b'N',
            Wk::KeyO => b'O',
            Wk::KeyP => b'P',
            Wk::KeyQ => b'Q',
            Wk::KeyR => b'R',
            Wk::KeyS => b'S',
            Wk::KeyT => b'T',
            Wk::KeyU => b'U',
            Wk::KeyV => b'V',
            Wk::KeyW => b'W',
            Wk::KeyX => b'X',
            Wk::KeyY => b'Y',
            Wk::KeyZ => b'Z',
            Wk::Digit0 => b'0',
            Wk::Digit1 => b'1',
            Wk::Digit2 => b'2',
            Wk::Digit3 => b'3',
            Wk::Digit4 => b'4',
            Wk::Digit5 => b'5',
            Wk::Digit6 => b'6',
            Wk::Digit7 => b'7',
            Wk::Digit8 => b'8',
            Wk::Digit9 => b'9',
            _ => return None,
        })
    }

    pub(super) fn dispatch_os_mouse_to_window_manager(
        &self,
        button: MouseButton,
        pressed: bool,
        x: i32,
        y: i32,
        origin: MouseInputOrigin,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::control_bar::{
                with_host_control_bar_input_provenance, HostControlBarInputProvenance,
            };
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            let msg = match (button, pressed) {
                (MouseButton::Left, true) => WindowMessage::LeftDown,
                (MouseButton::Left, false) => WindowMessage::LeftUp,
                (MouseButton::Right, true) => WindowMessage::RightDown,
                (MouseButton::Right, false) => WindowMessage::RightUp,
                (MouseButton::Middle, true) => WindowMessage::MiddleDown,
                (MouseButton::Middle, false) => WindowMessage::MiddleUp,
                _ => return false,
            };
            // `CommandSourceType::FromUser` alone cannot distinguish a real
            // winit event from `inject_winit_equivalent_*`: both deliberately
            // follow the same WND callback path. Scope the actual origin over
            // this synchronous dispatch so publication captures it exactly.
            let provenance = match origin {
                MouseInputOrigin::Physical => {
                    HostControlBarInputProvenance::PhysicalWindowMouseInput
                }
                MouseInputOrigin::Injected => HostControlBarInputProvenance::InjectedOrUnknown,
            };
            with_host_control_bar_input_provenance(provenance, || {
                game_client::gui::dispatch_os_mouse_to_window_manager(msg, x, y)
                    == WindowInputReturnCode::Used
            })
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (button, pressed, x, y, origin);
            false
        }
    }

    pub(super) fn dispatch_os_mouse_move(&self, x: i32, y: i32) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            game_client::gui::dispatch_os_mouse_to_window_manager(WindowMessage::MousePos, x, y)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (x, y);
            false
        }
    }

    pub(super) fn dispatch_os_mouse_wheel(
        &self,
        delta: &winit::event::MouseScrollDelta,
        x: i32,
        y: i32,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::game_window::{WindowInputReturnCode, WindowMessage};
            let lines = match delta {
                winit::event::MouseScrollDelta::LineDelta(_, y) => *y,
                winit::event::MouseScrollDelta::PixelDelta(p) => (p.y as f32) / 16.0,
            };
            if lines.abs() < f32::EPSILON {
                return false;
            }
            let msg = if lines > 0.0 {
                WindowMessage::WheelUp
            } else {
                WindowMessage::WheelDown
            };
            game_client::gui::dispatch_os_mouse_to_window_manager(msg, x, y)
                == WindowInputReturnCode::Used
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (delta, x, y);
            false
        }
    }

    /// Wave 606: host OS→GameClient mouse-button inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_mouse_button(&self, button: MouseButton, pressed: bool) {
        // Wave 606: host OS mouse button inject residual.
        use game_client::input::mouse::MouseButton as GcMouseButton;
        use std::time::Instant;
        let gc_btn = match button {
            MouseButton::Left => GcMouseButton::Left,
            MouseButton::Right => GcMouseButton::Right,
            MouseButton::Middle => GcMouseButton::Middle,
            MouseButton::Back => GcMouseButton::Other(3),
            MouseButton::Forward => GcMouseButton::Other(4),
            MouseButton::Other(n) => GcMouseButton::Other(n as u16),
        };
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_mouse_button(gc_btn, pressed, Instant::now());
        });
    }

    /// Wave 606: via `host_inject_game_client_mouse_scroll`.
    #[cfg(feature = "game_client")]
    pub(super) fn inject_game_client_mouse_scroll(&self, delta_y: f32) {
        // Wave 606: thin wrapper — OS mouse scroll inject via host helper.
        self.host_inject_game_client_mouse_scroll(delta_y);
    }

    /// Wave 606: host OS→GameClient mouse-scroll inject residual.
    #[cfg(feature = "game_client")]
    pub(super) fn host_inject_game_client_mouse_scroll(&self, delta_y: f32) {
        // Wave 606: host OS mouse scroll inject residual.
        game_client::input::mouse::with_mouse(|mouse| {
            let _ = mouse.handle_scroll_lines(delta_y);
        });
    }

    pub(super) fn update_mouse_world_position(&mut self) {
        // Convert screen coordinates to world coordinates using current world bounds.
        // Prefer presentation world_env when installed (no live dual-read for click map).
        // Boot/loading without a frame still uses host GameLogic bounds.
        let size = self.window.inner_size();
        let normalized_x = (self.mouse_position.0 / size.width.max(1) as f32).clamp(0.0, 1.0);
        let normalized_y = (self.mouse_position.1 / size.height.max(1) as f32).clamp(0.0, 1.0);

        let (world_min, world_max) = self.presentation_world_bounds();
        let world_width = (world_max.x - world_min.x).max(1.0);
        let world_height = (world_max.z - world_min.z).max(1.0);
        let world_x = world_min.x + normalized_x * world_width;
        let world_z = world_min.z + normalized_y * world_height;
        self.mouse_world_position = Vec3::new(world_x, 0.0, world_z);
    }

    /// Presentation-only world pick. Returns `None` when no snapshot is installed
    /// (no live GameLogic dual-read residual). InGame always seeds
    /// `last_presentation_frame` before input.

    /// Wave 228: build presentation target hint for RMB classification.

    /// Wave 229: presentation-frozen selected-unit capabilities for RMB classification.
    pub(super) fn presentation_selected_unit_hints(
        &self,
        ids: &[crate::game_logic::ObjectId],
    ) -> Vec<crate::command_system::PresentationSelectedUnitHint> {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(ids.len());
        for &id in ids {
            let Some(o) = frame.objects.iter().find(|x| x.id == id) else {
                continue;
            };
            // Wave 1097: selected-hint residual fail-closed on unusable sources.
            if o.destroyed
                || o.health_current <= 0.0
                || o.sold
                || o.masked
                || o.disabled
                || o.unselectable
            {
                continue;
            }
            let n = o.template_name.to_ascii_lowercase();
            let is_worker =
                crate::presentation_frame::PresentationFrame::presentation_is_worker_like(o)
                    || n.contains("dozer")
                    || n.contains("worker")
                    || n.contains("supplytruck")
                    || n.contains("supply_truck");
            // Gather authorization is an authored capability, not a template
            // naming convention.  C++ marks Chinooks, Supply Trucks, and GLA
            // Workers with KINDOF_HARVESTER.
            let is_resource_collector =
                crate::presentation_frame::PresentationFrame::object_has_kind(
                    o,
                    crate::game_logic::KindOf::Harvester,
                );
            let can_attack = o.has_weapon;
            let can_move = o.is_mobile;
            let is_lotus =
                crate::game_logic::host_hero_abilities::is_black_lotus_template(&o.template_name);
            let is_hero = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Hero,
            ) || n.contains("colonel")
                || n.contains("jarmen")
                || n.contains("lotus");
            let can_capture = n.contains("ranger")
                || n.contains("rebel")
                || n.contains("redguard")
                || crate::game_logic::host_hero_abilities::can_capture_without_upgrade(
                    is_hero, is_lotus,
                );
            let can_repair = is_worker
                || n.contains("dozer")
                || n.contains("worker")
                || n.contains("construction");
            let is_damaged = o.health_max > 0.0 && o.health_current + 0.01 < o.health_max;
            let is_vehicle = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Vehicle,
            ) || o.object_type
                == crate::presentation_frame::PresentationObjectType::Vehicle;
            let is_aircraft = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Aircraft,
            ) || o.object_type
                == crate::presentation_frame::PresentationObjectType::Aircraft;
            let is_infantry = crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Infantry,
            ) || o.object_type
                == crate::presentation_frame::PresentationObjectType::Infantry;
            out.push(crate::command_system::PresentationSelectedUnitHint {
                id,
                is_alive: true,
                is_resource_collector,
                is_worker,
                can_attack,
                can_move,
                can_capture,
                template_name: o.template_name.clone(),
                can_repair,
                is_damaged,
                is_vehicle,
                is_aircraft,
                is_infantry,
            });
        }
        out
    }

    pub(super) fn presentation_target_hint(
        &self,
        id: crate::game_logic::ObjectId,
    ) -> Option<crate::command_system::PresentationTargetHint> {
        let frame = self.last_presentation_frame.as_ref()?;
        // Wave 1097: target-hint residual fail-closed on sold/masked and non-local
        // FOW unless Clear (matches pick peels 1093–1096).
        let o = frame.objects.iter().find(|x| {
            x.id == id
                && !x.destroyed
                && !x.sold
                && !x.masked
                && (x.team == frame.local_team() || x.fow_visibility.visibility_alpha >= 0.95)
        })?;
        let local = frame.local_team();
        let is_neutral = o.team == crate::game_logic::Team::Neutral;
        let is_enemy = o.team != local && !is_neutral;
        let is_structure = o.object_type
            == crate::presentation_frame::PresentationObjectType::Building
            || crate::presentation_frame::PresentationFrame::object_has_kind(
                o,
                crate::game_logic::KindOf::Structure,
            );
        let is_resource = crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::Harvestable,
        ) || crate::presentation_frame::PresentationFrame::object_has_kind(
            o,
            crate::game_logic::KindOf::Resource,
        ) || o.template_name.to_ascii_lowercase().contains("supply");
        let n = o.template_name.to_ascii_lowercase();
        let can_be_entered = n.contains("transport")
            || n.contains("chinook")
            || n.contains("bunker")
            || n.contains("garrison")
            || n.contains("overlord");
        let is_damaged = o.health_max > 0.0 && o.health_current + 0.01 < o.health_max;
        let is_friendly = o.team == local && !is_neutral;
        let provides_heal = n.contains("healpad")
            || n.contains("heal_pad")
            || n.contains("hospital")
            || n.contains("ambulance");
        let provides_aircraft_repair =
            n.contains("airfield") || n.contains("helipad") || n.contains("airstrip");
        let provides_vehicle_repair = n.contains("repair")
            || n.contains("warfactory")
            || n.contains("war_factory")
            || n.contains("armsdealer")
            || n.contains("propaganda")
            || provides_aircraft_repair;
        Some(crate::command_system::PresentationTargetHint {
            id,
            // Wave 1098: is_alive residual excludes sold/masked.
            is_alive: !o.destroyed && o.health_current > 0.0 && !o.sold && !o.masked,
            is_structure,
            is_resource,
            under_construction: o.under_construction,
            sold: o.sold,
            team: o.team,
            is_enemy_of_local: is_enemy,
            is_neutral,
            template_name: o.template_name.clone(),
            can_be_entered,
            is_damaged,
            is_friendly_of_local: is_friendly,
            provides_vehicle_repair: is_structure && provides_vehicle_repair,
            provides_aircraft_repair: is_structure && provides_aircraft_repair,
            provides_heal: is_structure && provides_heal,
        })
    }

    pub(super) fn find_object_at_position(
        &self,
        position: Vec3,
        command_context: bool,
    ) -> Option<ObjectId> {
        const BASE_SELECTION_RADIUS: f32 = 20.0;
        // Wave 222: presentation-only pick (no GameLogic dual-read residual).

        let frame = self.last_presentation_frame.as_ref()?;
        let player_team = Some(frame.local_team());
        let has_selected_units = !self.selected_objects.is_empty();
        let prioritize_enemy_targets = command_context && has_selected_units;

        crate::unit_control::UnitControlSystem::pick_object_id_at_world_from_presentation(
            frame,
            position,
            player_team,
            prioritize_enemy_targets,
            BASE_SELECTION_RADIUS,
        )
    }

    /// Path following is authoritative in `GameLogic::update_movement`.
    /// Retained as a no-op compatibility hook for older call sites.

    /// Legacy render stub -- NOT called from the active render path.
    /// Actual rendering is handled by RenderPipeline::execute() -> ForwardPass::render()
    /// which queues MeshClass instances into the WW3D Renderer and issues real draw calls.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    pub(super) fn render_game_objects<'a>(&'a self, _render_pass: &mut wgpu::RenderPass<'a>) {
        // Presentation-only stub: RenderPipeline is the sole draw path.
        let n = self
            .last_presentation_frame
            .as_ref()
            .map(|f| f.objects.len())
            .unwrap_or(0);
        log::trace!(
            "Legacy stub: presentation has {} objects (RenderPipeline is sole draw path)",
            n
        );
    }

    /// Legacy per-object render stub -- logs model status but does NOT submit draw calls.
    /// The active render path is RenderPipeline::collect_render_items() which builds
    /// RenderItem list and ForwardPass::prepare_mesh_instance() which creates actual
    /// MeshClass instances submitted to the WW3D Renderer.
    #[allow(dead_code)] // Legacy stub: superseded by RenderPipeline, retained for reference
    pub(super) fn render_object<'a>(
        &'a self,
        obj: &Object,
        _render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        let model_name = obj.get_template().get_model_name();

        log::trace!(
            "Render object {} template '{}' model '{}' (cached={})",
            obj.id,
            obj.template_name,
            model_name,
            self.graphics_system.get_model(model_name).is_some()
        );

        let w3d_model = self
            .graphics_system
            .get_model(model_name)
            .or_else(|| self.graphics_system.get_model(&obj.template_name));

        if let Some(w3d_model) = w3d_model {
            let total_vertices: usize = w3d_model
                .meshes
                .iter()
                .map(|mesh| mesh.vertices.len())
                .sum();
            let total_indices: usize = w3d_model.meshes.iter().map(|mesh| mesh.indices.len()).sum();

            log::trace!("Rendering W3D model: {} (template: {}) with {} vertices, {} indices across {} meshes",
                model_name, obj.template_name, total_vertices, total_indices, w3d_model.meshes.len());
            log::trace!("Resolved W3D model '{}' for object {}", model_name, obj.id);
        } else {
            log::debug!(
                "No W3D model resolved for object {} template '{}' (model '{}') -- fallback cube will be used by RenderPipeline",
                obj.id,
                obj.template_name,
                model_name
            );
        }
    }

    #[allow(dead_code)] // Legacy stub: selection_renderer + PresentationFrame own production path
    pub(super) fn render_selection_indicators(&self, _render_pass: &mut wgpu::RenderPass) {
        // Prefer presentation selected residual when installed (no live find_object dual-read).
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            let n = frame
                .objects
                .iter()
                .filter(|o| o.selected && !o.destroyed)
                .count();
            log::trace!(
                "Legacy stub: presentation selected count={n} (selection_renderer is sole path)"
            );
            return;
        }
        // Boot residual only.
        for &object_id in &self.selected_objects {
            let _ = object_id;
        }
    }

    pub(super) fn render_projectiles(&self, _render_pass: &mut wgpu::RenderPass) {
        // Projectiles render from PresentationFrame (host CombatSystem freeze).
    }

    pub(super) fn render_ui(&self, _render_pass: &mut wgpu::RenderPass) {
        if let Err(err) = self.ui_manager.render() {
            log::warn!("UI manager render failed: {}", err);
        }
        log::trace!(
            "UI overlay rendered for {} selected units",
            self.selected_objects.len()
        );
    }
}
