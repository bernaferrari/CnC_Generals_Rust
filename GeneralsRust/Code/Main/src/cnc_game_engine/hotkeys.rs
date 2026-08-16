#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
impl CnCGameEngine {
    /// Apply the CommandMap CREATE/SELECT/ADD/VIEW_TEAM action for a physical
    /// number-row key.  The caller deliberately resolves the group from the
    /// physical key rather than its layout-dependent logical character.
    pub(super) fn handle_control_group_hotkey(&mut self, group_num: u8) {
        let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        let alt_down = self.keys_pressed.contains(&Key::Named(NamedKey::Alt));

        if ctrl_down {
            // CREATE_TEAM residual: assign control group.
            if self.selected_objects.is_empty() {
                self.control_groups.remove(&group_num);
                info!("Cleared control group {}", group_num);
            } else {
                self.control_groups
                    .insert(group_num, self.selected_objects.clone());
                info!(
                    "Assigned {} units to control group {}",
                    self.selected_objects.len(),
                    group_num
                );
            }
        } else if shift_down {
            // ADD_TEAM residual: merge current selection into group.
            if self.selected_objects.is_empty() {
                return;
            }
            let entry = self.control_groups.entry(group_num).or_default();
            for id in &self.selected_objects {
                if !entry.contains(id) {
                    entry.push(*id);
                }
            }
            info!(
                "Added selection to control group {} (now {} units)",
                group_num,
                entry.len()
            );
        } else if alt_down {
            // VIEW_TEAM residual: center camera on group without changing selection.
            let stored = self
                .control_groups
                .get(&group_num)
                .cloned()
                .unwrap_or_default();
            if stored.is_empty() {
                info!("Control group {} is empty (view)", group_num);
                return;
            }
            // Presentation-only poses for InGame control-group camera jump.
            let center = self
                .last_presentation_frame
                .as_ref()
                .and_then(|frame| frame.centroid_of_ids(&stored));
            if let Some(center) = center {
                // Wave 577: host camera jump via helper.
                let clamped = self.host_center_camera_and_request_focus(center);
                info!("VIEW_TEAM{} camera jump to {:?}", group_num, clamped);
            }
        } else {
            // SELECT_TEAM residual: select control group.
            let stored = self
                .control_groups
                .get(&group_num)
                .cloned()
                .unwrap_or_default();
            if stored.is_empty() {
                info!("Control group {} is empty", group_num);
                return;
            }

            // Presentation-only: InGame always has last_presentation_frame.
            let (selection, double_tap_center) = {
                let Some(frame) = self.last_presentation_frame.as_ref() else {
                    return;
                };
                let team = frame.local_team();
                let selection = frame.filter_alive_selectable_ids(&stored, team);
                let center = frame.centroid_of_ids(&selection);
                (selection, center)
            };

            // Wave 583: selection residual via host_set_selection.
            self.host_set_selection(self.current_player_id, selection.clone());
            self.play_sound_effect(SoundType::Select);

            // Double-tap residual: second press of same group within 500ms centers camera.
            let now = Instant::now();
            let double_tap = matches!(
                self.last_control_group_select,
                Some((g, t)) if g == group_num && now.duration_since(t).as_millis() < 500
            );
            self.last_control_group_select = Some((group_num, now));
            if double_tap {
                if let Some(center) = double_tap_center {
                    let clamped = self.clamp_to_world_bounds(center);
                    self.camera_target.x = clamped.x;
                    self.camera_target.z = clamped.z;
                    info!(
                        "Control group {} double-tap camera jump to {:?}",
                        group_num, clamped
                    );
                }
            }
        }
    }

    pub(super) fn handle_key_press(&mut self, key: &Key) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            match key {
                Key::Character(c) if c == "m" || c == "M" => {
                    self.toggle_background_music();
                }
                Key::Named(NamedKey::F11) => {
                    let current_fullscreen = self.window.fullscreen().is_some();
                    if let Err(e) = self.set_fullscreen(!current_fullscreen) {
                        error!("Failed to toggle fullscreen: {}", e);
                    } else {
                        info!("Toggled fullscreen mode: {}", !current_fullscreen);
                    }
                }
                Key::Named(NamedKey::Escape) => {
                    info!("Escape pressed in Menu/Loading - exiting");
                    self.request_state_change(GameState::Exiting);
                }
                _ => {}
            }
            return;
        }

        let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));

        // Retail chat residual: when open, keyboard goes to ChatPanel first.
        if matches!(self.current_state, GameState::InGame | GameState::Paused)
            && self.chat_panel.is_open()
        {
            if self.route_key_to_chat_panel(key) {
                return;
            }
        }

        match key {
            Key::Named(NamedKey::Space)
                if self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Center camera on selection residual (Alt+Space).
                self.center_camera_on_selection();
            }
            Key::Named(NamedKey::Space) => {
                // Retail CommandMap VIEW_LAST_RADAR_EVENT KEY_SPACE residual.
                // Pause remains on P.
                self.issue_named_command_from_ui("Command_ViewLastRadarEvent");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Unit attitude Aggressive residual (Alt+A).
                self.issue_named_command_from_ui("Command_AttitudeAggressive");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("s")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Unit attitude Sleep / hold-fire residual (Alt+S).
                self.issue_named_command_from_ui("Command_AttitudeSleep");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("d")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Unit attitude Passive residual (Alt+D).
                self.issue_named_command_from_ui("Command_AttitudePassive");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Control))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // C++ classic A-key AttackMove residual: arm pending map click.
                // Wave 221: selection via presentation-first ui_selected_ids.
                if !self.ui_selected_ids(self.current_player_id).is_empty() {
                    self.pending_map_command = Some(PendingMapCommand::AttackMove);
                    self.pending_structure_placement = None;
                    self.arm_radius_cursor_for_pending("ATTACK_CONTINUE_AREA");
                    let msg = "Attack-move: click destination";
                    self.game_hud.push_info_message(msg);
                    self.ui_manager.game_hud_mut().push_info_message(msg);
                }
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("t")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all attacking friendlies residual (Ctrl+Alt+T).
                self.select_all_friendly_attacking();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("t")
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Control))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // C++ classic T-key ForceAttackGround residual at cursor.
                // Wave 221: selection via presentation-first ui_selected_ids.
                let selected = self.ui_selected_ids(self.current_player_id);
                if selected.is_empty() {
                    // no-op
                } else {
                    let loc = self.mouse_world_position;
                    self.host_queue_and_process_command_silent(
                        crate::command_system::GameCommand {
                            command_type: crate::command_system::CommandType::ForceAttackGround {
                                location: loc,
                            },
                            player_id: self.current_player_id,
                            command_id: 0,
                            timestamp: std::time::SystemTime::now(),
                            selected_units: selected,
                            modifier_keys: crate::command_system::ModifierKeys {
                                ctrl: true,
                                shift: false,
                                alt: false,
                            },
                        },
                    );
                    self.host_process_commands_with_command_sound();
                    let msg = "Force-attack ground";
                    self.game_hud.push_info_message(msg);
                    self.ui_manager.game_hud_mut().push_info_message(msg);
                }
            }
            Key::Named(NamedKey::Home)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Unfinished construction cycle residual (Ctrl+Alt+Home).
                self.cycle_unfinished_construction(1);
            }
            Key::Named(NamedKey::End)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Unfinished construction cycle residual (Ctrl+Alt+End).
                self.cycle_unfinished_construction(-1);
            }
            Key::Named(NamedKey::Home) if !ctrl_down => {
                // SELECT_NEXT_STRUCTURE residual (Home).
                self.cycle_friendly_structure_selection(1);
            }
            Key::Named(NamedKey::End) if !ctrl_down => {
                // SELECT_PREV_STRUCTURE residual (End).
                self.cycle_friendly_structure_selection(-1);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Convenience alias; retail SELECT_ALL is KEY_Q.
                self.select_all_friendly_units();
            }
            Key::Named(NamedKey::Delete) => {
                let shift = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                if ctrl_down && !shift {
                    // Cancel entire production queue residual (Ctrl+Delete).
                    if self.cancel_all_selected_production() {
                        return;
                    }
                }
                if shift {
                    // Debug residual: Shift+Delete destroys selection.
                    if self.selected_objects.is_empty() {
                        return;
                    }
                    for id in self.selected_objects.clone() {
                        self.host_destroy_object(id);
                    }
                    self.selected_objects.clear();
                    // Wave 583: clear selection residual via host_set_selection.
                    self.host_set_selection(self.current_player_id, Vec::new());
                } else if self.cancel_selected_production_queue_head() {
                    // Producer selection: Delete cancels queue head residual.
                } else {
                    // Retail CommandMap DELETE_BEACON KEY_DEL residual.
                    self.issue_named_command_from_ui("Command_RemoveBeacon");
                }
            }
            Key::Named(NamedKey::Tab)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Cycle control groups residual (Ctrl+Shift+Tab).
                let delta = if self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) {
                    -1
                } else {
                    1
                };
                self.cycle_control_group_selection(delta);
            }
            Key::Named(NamedKey::Tab) if !ctrl_down => {
                // Retail CommandMap DIPLOMACY KEY_TAB residual.
                self.toggle_diplomacy_panel_hotkey();
            }
            Key::Named(NamedKey::F1) if ctrl_down => {
                // Debug overlay residual (Ctrl+F1); bare F1 remains camera bookmark.
                self.toggle_debug_info_hotkey();
            }
            Key::Named(NamedKey::F1) => self.handle_camera_view_hotkey(0),
            Key::Named(NamedKey::F2) if ctrl_down => {
                // FPS counter residual (Ctrl+F2); bare F2 remains camera bookmark.
                self.toggle_fps_counter_hotkey();
            }
            Key::Named(NamedKey::F2) => self.handle_camera_view_hotkey(1),
            Key::Named(NamedKey::F3) if ctrl_down => {
                // Move path lines residual (Ctrl+F3); bare F3 remains camera bookmark.
                self.toggle_move_lines_hotkey();
            }
            Key::Named(NamedKey::F3) => self.handle_camera_view_hotkey(2),
            Key::Named(NamedKey::F4) if ctrl_down => {
                // Attack path lines residual (Ctrl+F4); bare F4 remains camera bookmark.
                self.toggle_attack_lines_hotkey();
            }
            Key::Named(NamedKey::F4) => self.handle_camera_view_hotkey(3),
            Key::Named(NamedKey::F5) => self.handle_camera_view_hotkey(4),
            Key::Named(NamedKey::F6) => self.handle_camera_view_hotkey(5),
            Key::Named(NamedKey::F7) => self.handle_camera_view_hotkey(6),
            Key::Named(NamedKey::F8) => self.handle_camera_view_hotkey(7),
            Key::Character(c)
                if (c == "m" || c == "M")
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                self.toggle_background_music();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("v")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Debug residual only — not retail CommandMap.
                self.debug_show_victory(Some(self.current_player_id));
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("v")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Cycle ready special-power structures residual (Ctrl+Alt+V).
                self.cycle_ready_special_power_structure(1);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("v")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Fire ready superweapon / special power residual (map click).
                self.issue_named_command_from_ui("Command_DoSpecialPower");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("l")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Debug residual only — not retail CommandMap.
                let winner = self.host_first_opponent_id(self.current_player_id);
                self.debug_show_victory(winner);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("p")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Cycle busy producers residual (Ctrl+Alt+P).
                self.cycle_busy_producer_selection(1);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("p")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Toggle pause with 'P' key
                self.toggle_pause();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("s")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // C++ MSG_META_STOP residual: stop selected units immediately.
                self.issue_named_command_from_ui("Command_Stop");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("d")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // C&C Generals standard Deploy residual (CommandMap DEPLOY commented but UI uses D).
                self.issue_named_command_from_ui("Command_Deploy");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("u")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select garrisoned structures residual (Ctrl+Alt+U).
                self.select_all_garrisoned_structures();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("u")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Harvester return supplies residual (Alt+U).
                self.issue_named_command_from_ui("Command_ReturnSupplies");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("u")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Evacuate / unload transport or garrison residual.
                self.issue_named_command_from_ui("Command_Evacuate");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("n")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Colonel Burton remote charge detonate residual.
                self.issue_named_command_from_ui("Command_DetonateRemoteDemoCharges");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("r")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all repairing units residual (Ctrl+Alt+R).
                self.select_all_repairing_units();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("r")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Aircraft return-to-base residual (Alt+R).
                self.issue_named_command_from_ui("Command_ReturnToBase");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("r")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Dozer/Worker repair residual: arm structure click.
                self.issue_named_command_from_ui("Command_Repair");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("y")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all patrolling residual (Ctrl+Alt+Y).
                self.select_all_friendly_patrolling();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("y") && !ctrl_down => {
                // Set factory rally point residual.
                self.issue_named_command_from_ui("Command_SetRallyPoint");
            }
            Key::Character(c) if c.eq_ignore_ascii_case("o") && !ctrl_down => {
                // China nuclear plant overcharge residual.
                self.issue_named_command_from_ui("Command_ToggleOvercharge");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("c")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Chinook combat drop residual (Alt+C).
                self.issue_named_command_from_ui("Command_CombatDrop");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("c")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Infantry capture-building residual: arm structure click.
                self.issue_named_command_from_ui("Command_CaptureBuilding");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("g")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all guarding friendlies residual (Ctrl+Alt+G).
                self.select_all_friendly_guarding();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("g")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // GeneralsExperience purchase next science residual (Alt+G).
                self.try_purchase_next_generals_science();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("g")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // C++ Guard residual: arm map-click Guard (location or unit).
                self.issue_named_command_from_ui("Command_Guard");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("x")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Dozer/Worker clear nearest mine residual (Alt+X).
                self.issue_named_command_from_ui("Command_ClearMines");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("x")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail CommandMap SCATTER KEY_X residual.
                self.issue_named_command_from_ui("Command_Scatter");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("z")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Clear path waypoints residual (Alt+Z).
                self.clear_selected_path_waypoints();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("z")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Sticky waypoint mode residual (Alt still force-on while held).
                self.sticky_waypoint_mode = !self.sticky_waypoint_mode;
                let msg = if self.sticky_waypoint_mode {
                    "Waypoint mode: ON"
                } else {
                    "Waypoint mode: OFF"
                };
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("q")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all combat units residual (Ctrl+Alt+Q).
                self.select_all_friendly_combat();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("q") && !ctrl_down => {
                // Retail CommandMap SELECT_ALL KEY_Q residual.
                self.select_all_friendly_units();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("e")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select veteran+ units residual (Ctrl+Alt+E).
                self.select_all_friendly_veterans();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("e")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Resume unfinished construction residual (Alt+E).
                self.resume_selected_construction();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("e")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail CommandMap SELECT_MATCHING_UNITS KEY_E residual.
                self.select_matching_units_hotkey();
            }
            Key::Character(c)
                if (c == "[" || c == "{")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Shrink guard radius residual (Alt+[).
                self.adjust_selected_guard_radius(-15.0);
            }
            Key::Character(c)
                if (c == "]" || c == "}")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Grow guard radius residual (Alt+]).
                self.adjust_selected_guard_radius(15.0);
            }
            Key::Character(c)
                if (c == "[" || c == "{")
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Construction tab previous residual.
                self.cycle_construction_tab(-1);
            }
            Key::Character(c)
                if (c == "]" || c == "}")
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Construction tab next residual.
                self.cycle_construction_tab(1);
            }
            Key::Character(c)
                if (c == "." || c == ">")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Stop all friendly mobile units residual (Ctrl+Shift+.).
                self.stop_all_friendly_units();
            }
            Key::Character(c)
                if (c == "." || c == ">")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Cycle idle military residual (Ctrl+Alt+.).
                self.cycle_idle_military_selection(1);
            }
            Key::Character(c)
                if (c == "," || c == "<")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Cycle idle military residual (Ctrl+Alt+,).
                self.cycle_idle_military_selection(-1);
            }
            Key::Character(c) if (c == "." || c == ">") && !ctrl_down => {
                // Retail-ish next idle worker residual (period key).
                self.cycle_friendly_worker_selection(1);
            }
            Key::Character(c) if c == "," || c == "<" => {
                // Previous idle worker residual (comma key).
                self.cycle_friendly_worker_selection(-1);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("w")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select docked aircraft residual (Ctrl+Alt+W).
                self.select_all_docked_aircraft();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("w")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Cycle primary/secondary weapon residual (Alt+W).
                self.issue_named_command_from_ui("Command_SwitchWeapons");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("i")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // China Hacker HackInternet residual (Alt+I).
                self.issue_named_command_from_ui("Command_HackInternet");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("m")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all moving friendlies residual (Ctrl+Alt+M).
                self.select_all_friendly_moving();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("m")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Ambulance CleanupArea residual (Alt+M).
                self.issue_named_command_from_ui("Command_CleanupArea");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("w")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail CommandMap SELECT_ALL_AIRCRAFT KEY_W residual.
                self.select_all_friendly_aircraft();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("h") && !ctrl_down => {
                // Retail CommandMap VIEW_COMMAND_CENTER KEY_H residual.
                self.issue_named_command_from_ui("Command_ViewCommandCenter");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("h")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select gathering units residual (Ctrl+Alt+H).
                self.select_all_friendly_gathering();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("h")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Toggle health bars residual (Alt+H).
                self.toggle_health_bars_hotkey();
            }
            Key::Character(c) if c.eq_ignore_ascii_case("h") && ctrl_down => {
                // Retail CommandMap SELECT_HERO Ctrl+H residual.
                self.select_hero_units_hotkey();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("i")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select idle harvesters residual (Ctrl+Alt+I).
                self.select_idle_harvesters();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("k")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select stealthed friendlies residual (Ctrl+Alt+K).
                self.select_all_friendly_stealthed();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("j")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select occupied transports residual (Ctrl+Alt+J).
                self.select_all_occupied_transports();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("i")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Select all harvesters / supply collectors residual.
                self.select_all_harvesters();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("i")
                    && ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Select all idle military residual (Ctrl+I).
                self.select_all_idle_military();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("f")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Camera follow primary selection residual (Alt+F).
                self.toggle_camera_follow_selection();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("f")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Retail CommandMap CREATE_FORMATION Ctrl+F residual.
                self.issue_named_command_from_ui("Command_CreateFormation");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("b")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select constructing workers residual (Ctrl+Alt+B).
                self.select_all_constructing_workers();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("b")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt))
                    && !ctrl_down =>
            {
                // Demo tertiary suicide residual (Alt+B).
                self.issue_named_command_from_ui("Command_DemoTertiarySuicide");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("b")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Retail CommandMap PLACE_BEACON Ctrl+B residual.
                self.issue_named_command_from_ui("Command_PlaceBeacon");
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("c")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Retail CommandMap ALL_CHEER Ctrl+C residual.
                self.issue_named_command_from_ui("Command_Cheer");
            }
            Key::Named(NamedKey::ArrowRight)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Damaged structure cycle residual (Ctrl+Alt+Right).
                self.cycle_damaged_structure_selection(1);
            }
            Key::Named(NamedKey::ArrowLeft)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Damaged structure cycle residual (Ctrl+Alt+Left).
                self.cycle_damaged_structure_selection(-1);
            }
            Key::Named(NamedKey::ArrowRight)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // SELECT_NEXT_STRUCTURE residual (Ctrl+Shift+Right).
                self.cycle_friendly_structure_selection(1);
            }
            Key::Named(NamedKey::ArrowLeft)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // SELECT_PREV_STRUCTURE residual (Ctrl+Shift+Left).
                self.cycle_friendly_structure_selection(-1);
            }
            Key::Named(NamedKey::ArrowRight) if ctrl_down => {
                // Retail SELECT_NEXT_UNIT Ctrl+Right residual.
                self.cycle_friendly_selection(1);
            }
            Key::Named(NamedKey::ArrowLeft) if ctrl_down => {
                // Retail SELECT_PREV_UNIT Ctrl+Left residual.
                self.cycle_friendly_selection(-1);
            }
            Key::Named(NamedKey::ArrowUp)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Damaged unit cycle residual (Ctrl+Alt+Up).
                self.cycle_damaged_unit_selection(1);
            }
            Key::Named(NamedKey::ArrowDown)
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Damaged unit cycle residual (Ctrl+Alt+Down).
                self.cycle_damaged_unit_selection(-1);
            }
            Key::Named(NamedKey::ArrowUp) if ctrl_down => {
                // Retail SELECT_NEXT_WORKER Ctrl+Up residual.
                self.cycle_friendly_worker_selection(1);
            }
            Key::Named(NamedKey::ArrowDown) if ctrl_down => {
                // Retail SELECT_PREV_WORKER Ctrl+Down residual.
                self.cycle_friendly_worker_selection(-1);
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all friendlies near camera residual (Ctrl+Alt+A).
                self.select_all_friendly_on_screen();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("a")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Sticky auto-attack residual (Ctrl+Shift+A): RMB move becomes attack-move.
                // Stored on engine; MouseCommandContext path honors via force AttackMove.
                self.sticky_auto_attack = !self.sticky_auto_attack;
                let msg = if self.sticky_auto_attack {
                    "Auto-attack: ON"
                } else {
                    "Auto-attack: OFF"
                };
                self.game_hud.push_info_message(msg);
                self.ui_manager.game_hud_mut().push_info_message(msg);
            }
            Key::Named(NamedKey::F9) => {
                // Retail CommandMap TOGGLE_CONTROL_BAR KEY_F9 residual.
                // C++ CommandXlat.cpp:3144 MSG_META_TOGGLE_CONTROL_BAR → ToggleControlBar.
                #[cfg(feature = "game_client")]
                {
                    let _ = game_client::gui::callbacks::control_bar_callbacks::toggle_control_bar(
                        true,
                    );
                }
                self.game_hud.toggle_visibility();
                self.ui_manager.game_hud_mut().toggle_visibility();
                info!(
                    "Control bar visibility toggled (engine visible={})",
                    self.game_hud.hud_visible()
                );
            }

            Key::Character(c)
                if c.eq_ignore_ascii_case("s")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Select all friendly structures residual (Ctrl+Alt+S).
                self.select_all_friendly_structures();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("s")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) =>
            {
                // Sell selected structures residual (Ctrl+Shift+S).
                self.issue_named_command_from_ui("Command_Sell");
            }
            Key::Character(c) if c.eq_ignore_ascii_case("s") && ctrl_down => {
                self.quick_save_from_hotkey("Ctrl+S");
            }
            Key::Character(c) if c.eq_ignore_ascii_case("l") && ctrl_down => {
                self.quick_load_from_hotkey("Ctrl+L");
            }
            Key::Named(NamedKey::Enter) => {
                // Retail CommandMap CHAT_EVERYONE KEY_ENTER residual.
                self.open_chat_hotkey(crate::ui::ChatTarget::All);
            }
            Key::Named(NamedKey::Backspace) => {
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) {
                    // Retail DEMO_INSTANT_QUIT Shift+Ctrl+Backspace residual.
                    info!("DEMO_INSTANT_QUIT residual — exiting");
                    self.request_state_change(GameState::Exiting);
                } else {
                    // Retail CommandMap CHAT_ALLIES KEY_BACKSPACE residual.
                    self.open_chat_hotkey(crate::ui::ChatTarget::Allies);
                }
            }
            Key::Named(NamedKey::F12) => {
                // Retail CommandMap TAKE_SCREENSHOT KEY_F12 residual.
                self.take_screenshot_hotkey();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("t")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail TOGGLE_CAMERA_TRACKING_DRAWABLE Shift+Alt+Ctrl+T residual.
                self.toggle_camera_tracking_drawable_hotkey();
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("f")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail TOGGLE_FAST_FORWARD_REPLAY KEY_F residual.
                self.toggle_replay_fast_forward_hotkey();
            }
            Key::Named(NamedKey::Escape) => {
                // Outer event-loop Escape handler is unreachable for keyboard
                // because input() consumes the event. Mirror C++ residual here:
                // cancel placement/map-command first, else pause/resume.
                match self.current_state {
                    GameState::InGame => {
                        if self.chat_panel.is_open() {
                            self.chat_panel.close();
                            info!("Escape closed chat residual");
                        } else if self.diplomacy_panel.is_active() {
                            self.diplomacy_panel.close();
                            info!("Escape closed diplomacy panel residual");
                        } else if self.pending_structure_placement.is_some() {
                            self.cancel_structure_placement_from_ui();
                            info!("Escape cancelled structure placement residual");
                        } else if self.pending_map_command.take().is_some() {
                            self.clear_radius_cursor_overlays();
                            let msg = "Cancelled pending command";
                            self.game_hud.push_info_message(msg);
                            self.ui_manager.game_hud_mut().push_info_message(msg);
                            info!("Escape cancelled pending map command residual");
                        } else {
                            // Retail CommandMap `OPTIONS` routes Escape to
                            // MSG_META_OPTIONS -> ToggleQuitMenu(), which
                            // creates Menus/QuitMenu.wnd.  Keep Main's host
                            // simulation pause synchronized with that live
                            // WND instead of replacing it with PauseMenu.
                            #[cfg(feature = "game_client")]
                            if self.host_toggle_retail_quit_menu() {
                                info!("Escape opened/toggled the retail QuitMenu WND");
                            } else {
                                // Fail closed only when the WND could not be
                                // materialised; the old pause surface remains
                                // a usable fallback for a damaged install.
                                info!("Escape QuitMenu WND unavailable - using pause fallback");
                                self.request_state_change(GameState::Paused);
                            }
                            #[cfg(not(feature = "game_client"))]
                            {
                                info!("Escape pressed in InGame state - pausing");
                                self.request_state_change(GameState::Paused);
                            }
                        }
                    }
                    GameState::Paused => {
                        info!("Escape pressed in Paused state - resuming");
                        self.request_state_change(GameState::InGame);
                    }
                    GameState::Menu | GameState::Loading => {
                        info!("Escape pressed in Menu/Loading - exiting");
                        self.request_state_change(GameState::Exiting);
                    }
                    GameState::Victory | GameState::Defeat => {
                        info!("Escape pressed in endgame - returning to menu");
                        self.request_state_change(GameState::Menu);
                    }
                    GameState::Exiting | GameState::Initializing => {}
                }
            }
            Key::Named(NamedKey::F11) => {
                // Toggle fullscreen mode
                let current_fullscreen = self.window.fullscreen().is_some();
                if let Err(e) = self.set_fullscreen(!current_fullscreen) {
                    error!("Failed to toggle fullscreen: {}", e);
                } else {
                    info!("Toggled fullscreen mode: {}", !current_fullscreen);
                }
            }
            _ => {}
        }
    }

    /// Retail SAVE_VIEW / VIEW_VIEW (Ctrl+Fn save, Fn recall) residual.
    pub(super) fn handle_camera_view_hotkey(&mut self, slot: usize) {
        if slot >= self.camera_view_bookmarks.len() {
            return;
        }
        let ctrl = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        if ctrl {
            let pos = self.camera_target;
            self.camera_view_bookmarks[slot] = Some(pos);
            let msg = format!("Saved camera view {}", slot + 1);
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
            info!("SAVE_VIEW{} -> {:?}", slot + 1, pos);
        } else if let Some(pos) = self.camera_view_bookmarks[slot] {
            // Wave 577: host camera jump via helper.
            let clamped = self.host_center_camera_and_request_focus(pos);
            info!("VIEW_VIEW{} -> {:?}", slot + 1, clamped);
        } else {
            let msg = format!("Camera view {} is empty", slot + 1);
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
        }
    }

    /// Retail CHAT_EVERYONE / CHAT_ALLIES residual.
    pub(super) fn open_chat_hotkey(&mut self, target: crate::ui::ChatTarget) {
        let name = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame
                .players
                .iter()
                .find(|p| p.id == self.current_player_id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| format!("Player{}", self.current_player_id))
        } else {
            self.ui_player_name(self.current_player_id)
                .unwrap_or_else(|| format!("Player{}", self.current_player_id))
        };
        self.chat_panel.set_local_player_name(&name);
        self.chat_panel.set_target(target);
        if self.chat_panel.open() {
            let label = match target {
                crate::ui::ChatTarget::All => "Chat (All)",
                crate::ui::ChatTarget::Allies => "Chat (Allies)",
                crate::ui::ChatTarget::Player(_) => "Chat (Whisper)",
            };
            self.game_hud.push_info_message(label);
            self.ui_manager.game_hud_mut().push_info_message(label);
            info!("Opened {label}");
        }
    }

    pub(super) fn route_key_to_chat_panel(&mut self, key: &Key) -> bool {
        use crate::ui::KeyCode;
        // Prefer UI key mapping then character text insert.
        if let Some(ui_key) = Self::to_ui_key_code(key) {
            if self.chat_panel.press_key(ui_key) {
                // Drain sent messages into HUD log residual.
                for ev in self.chat_panel.drain_events() {
                    if let crate::ui::ChatEvent::MessageSent { text, target } = ev {
                        let prefix = match target {
                            crate::ui::ChatTarget::All => "[All]",
                            crate::ui::ChatTarget::Allies => "[Allies]",
                            crate::ui::ChatTarget::Player(_) => "[Whisper]",
                        };
                        let msg = format!("{prefix} {text}");
                        self.game_hud.push_info_message(&msg);
                        self.ui_manager.game_hud_mut().push_info_message(&msg);
                    }
                }
                return true;
            }
        }
        if let Key::Character(s) = key {
            if self.chat_panel.type_text(s) {
                return true;
            }
        }
        false
    }

    /// Retail TOGGLE_CAMERA_TRACKING_DRAWABLE residual.
    pub(super) fn toggle_camera_tracking_drawable_hotkey(&mut self) {
        self.camera_tracking_selection = !self.camera_tracking_selection;
        let msg = if self.camera_tracking_selection {
            "Camera tracking selection: ON"
        } else {
            "Camera tracking selection: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
        info!("{msg}");
    }

    /// Retail TOGGLE_FAST_FORWARD_REPLAY (m_TiVOFastMode) residual.
    pub(super) fn toggle_replay_fast_forward_hotkey(&mut self) {
        // C++ only applies in replay games (or debug). Residual: always toggle flag + HUD.
        self.replay_fast_forward = !self.replay_fast_forward;
        let msg = if self.replay_fast_forward {
            "m_TiVOFastMode: ON"
        } else {
            "m_TiVOFastMode: OFF"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
        info!("{msg}");
    }

    /// Follow selection centroid when camera tracking residual is armed.
    pub(super) fn update_camera_tracking_drawable(&mut self) {
        if !self.camera_tracking_selection {
            return;
        }
        // Wave 226: selection via presentation-first ui_selected_ids.
        let ids = self.ui_selected_ids(self.current_player_id);
        if ids.is_empty() {
            return;
        }
        // Presentation-only poses for InGame camera tracking.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        if let Some(center) = frame.centroid_of_ids(&ids) {
            let clamped = self.clamp_to_world_bounds(center);
            self.camera_target.x = clamped.x;
            self.camera_target.z = clamped.z;
        }
    }

    /// Retail TAKE_SCREENSHOT (KEY_F12) residual.
    pub(super) fn take_screenshot_hotkey(&mut self) {
        let dir = std::env::temp_dir().join("generals_screenshots");
        let _ = std::fs::create_dir_all(&dir);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("screenshot_{stamp}.png"));
        match ww3d_engine::make_screenshot(&path) {
            Ok(()) => {
                let msg = format!("Screenshot: {}", path.display());
                self.game_hud.push_info_message(&msg);
                self.ui_manager.game_hud_mut().push_info_message(&msg);
                info!("{msg}");
            }
            Err(err) => {
                let msg = format!("Screenshot failed: {err:?}");
                self.game_hud.push_info_message(&msg);
                self.ui_manager.game_hud_mut().push_info_message(&msg);
                warn!("{msg}");
            }
        }
    }

    /// Retail DIPLOMACY (KEY_TAB) residual.
    pub(super) fn toggle_diplomacy_panel_hotkey(&mut self) {
        self.sync_diplomacy_panel_from_world();
        self.diplomacy_panel.toggle();
        let msg = if self.diplomacy_panel.is_active() {
            "Diplomacy panel opened"
        } else {
            "Diplomacy panel closed"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
        info!("{msg}");
    }

    pub(super) fn sync_diplomacy_panel_from_world(&mut self) {
        use crate::ui::{DiplomacyPlayerEntry, DiplomacyPlayerStatus, DiplomacyRelation};
        let local_id = self.current_player_id as i32;
        self.diplomacy_panel.set_local_player_id(local_id);
        let mut rows = Vec::new();
        // Wave 558: presentation freeze owns diplomacy roster residual when installed.
        let players = self.presentation_or_boot_diplomacy_players();
        let local_team = players
            .iter()
            .find(|x| x.id == self.current_player_id)
            .map(|x| x.team);
        for p in &players {
            let status = if p.is_alive {
                DiplomacyPlayerStatus::Active
            } else {
                DiplomacyPlayerStatus::Defeated
            };
            let relationship = if p.id == self.current_player_id {
                DiplomacyRelation::Allied
            } else if local_team == Some(p.team) {
                DiplomacyRelation::Allied
            } else {
                DiplomacyRelation::Enemy
            };
            rows.push(DiplomacyPlayerEntry {
                player_id: p.id as i32,
                name: p.name.clone(),
                side: format!("{:?}", p.team),
                team: match p.team {
                    crate::game_logic::Team::USA => 0,
                    crate::game_logic::Team::China => 1,
                    crate::game_logic::Team::GLA => 2,
                    _ => -1,
                },
                status,
                relationship,
                is_muted: false,
            });
        }
        self.diplomacy_panel.set_players(rows);
        // Keep panel layout in sync with window.
        let (w, h) = (
            self.window.inner_size().width,
            self.window.inner_size().height,
        );
        self.diplomacy_panel.resize(w, h);
    }

    /// Retail CAMERA_RESET (KEY_KP5) residual.
    pub(super) fn reset_camera_view_hotkey(&mut self) {
        // Wave 226: prefer presentation freeze for reset focus (team base / friendlies).
        let focus = if let Some(frame) = self.last_presentation_frame.as_ref() {
            frame
                .local_team_base_position
                .or_else(|| {
                    frame.centroid_of_ids(&frame.alive_selectable_friendly_ids(frame.local_team()))
                })
                .unwrap_or(self.camera_target)
        } else if let Some(pos) = self
            .game_logic
            .player_command_center_position(self.current_player_id)
        {
            // Wave 239: boot residual via player_command_center_position probe.
            pos
        } else {
            self.camera_target
        };
        // Wave 577: host camera jump via helper, then default zoom.
        let clamped = self.host_center_camera_and_request_focus(focus);
        self.camera_zoom = self.compute_default_camera_zoom_for_target(
            clamped,
            self.ui_script_default_camera_max_height(),
        );
        self.apply_camera_orbit_transform();
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.update_mouse_world_position();
            self.sync_context_mouse_cursor();
        }
        info!("CAMERA_RESET residual -> {:?}", clamped);
    }
}
