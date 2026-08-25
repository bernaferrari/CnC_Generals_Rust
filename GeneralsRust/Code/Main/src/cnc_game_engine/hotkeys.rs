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
            self.create_control_group(group_num);
        } else if shift_down {
            self.add_control_group_to_selection(group_num);
        } else if alt_down {
            self.view_control_group(group_num);
        } else {
            self.select_control_group(group_num);
        }
    }

    /// C++ `SelectionXlat.cpp:1051` CREATE_TEAM keeps only `isLocallyControlled()`.
    fn locally_controlled_selected_ids(&self) -> Vec<ObjectId> {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return Vec::new();
        };
        self.selected_objects
            .iter()
            .copied()
            .filter(|id| {
                frame
                    .objects
                    .iter()
                    .any(|object| object.id == *id && frame.is_owned_by_local(object))
            })
            .collect()
    }

    /// C++ `Player::removeObjectFromHotkeySquad` then `m_squads[n]->addObject`.
    pub(crate) fn evict_ids_from_other_control_groups(&mut self, ids: &[ObjectId], keep: u8) {
        for (&group, members) in self.control_groups.iter_mut() {
            if group == keep {
                continue;
            }
            members.retain(|id| !ids.contains(id));
        }
        self.control_groups
            .retain(|&group, members| group == keep || !members.is_empty());
    }

    /// C++ `Player::processCreateTeamGameMessage` leftover `m_squads` / Squad xfer.
    pub(crate) fn write_leftover_player_create_team(&self, group_num: u8, ids: &[ObjectId]) {
        crate::command_system::tap_host_team_slot_for_recorder(group_num, 0, ids);
        let object_ids: Vec<u32> = ids.iter().map(|id| id.0).collect();
        let Ok(list) = gamelogic::player::ThePlayerList().read() else {
            return;
        };
        let player_arc = list
            .get_local_player()
            .cloned()
            .or_else(|| list.get_player(self.current_player_id as i32).cloned());
        let Some(player_arc) = player_arc else {
            return;
        };
        drop(list);
        let Ok(mut player) = player_arc.write() else {
            return;
        };
        player.process_create_team_game_message(group_num as i32, &object_ids);
    }

    /// C++ `Player::processSelectTeamGameMessage` / `processAddTeamGameMessage`.
    fn write_leftover_player_select_team(&self, group_num: u8, add: bool) {
        crate::command_system::tap_host_team_slot_for_recorder(
            group_num,
            if add { 2 } else { 1 },
            &[],
        );
        let Ok(list) = gamelogic::player::ThePlayerList().read() else {
            return;
        };
        let player_arc = list
            .get_local_player()
            .cloned()
            .or_else(|| list.get_player(self.current_player_id as i32).cloned());
        let Some(player_arc) = player_arc else {
            return;
        };
        drop(list);
        let Ok(mut player) = player_arc.write() else {
            return;
        };
        if add {
            player.process_add_team_game_message(group_num as i32);
        } else {
            player.process_select_team_game_message(group_num as i32);
        }
    }

    /// C++ `getLiveObjects()` last member: `objlist[numObjs-1]->getDrawable()->getPosition()`.
    /// No `getControllingPlayer()==local` gate — ADD/SELECT double-tap and VIEW_TEAM
    /// center on the last live squad member even after hijack / snipe / capture.
    fn last_live_control_group_position(&self, stored: &[ObjectId]) -> Option<glam::Vec3> {
        let frame = self.last_presentation_frame.as_ref()?;
        let live = frame.filter_live_squad_ids(stored, false);
        let last = *live.last()?;
        frame
            .objects
            .iter()
            .find(|object| object.id == last)
            .map(|object| object.position)
    }

    /// C++ `TheInGameUI->getFirstSelectedDrawable()` + `KINDOF_STRUCTURE`.
    fn first_selected_is_structure(&self) -> bool {
        let Some(first) = self.selected_objects.first() else {
            return false;
        };
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return false;
        };
        frame.objects.iter().any(|object| {
            object.id == *first
                && (object.is_structure
                    || crate::presentation_frame::PresentationFrame::object_has_kind(
                        object,
                        crate::game_logic::KindOf::Structure,
                    )
                    || object.object_type
                        == crate::presentation_frame::PresentationObjectType::Building)
        })
    }

    /// C++ `SelectionXlat.cpp` `now - m_lastGroupSelTime < 20` (30 Hz → 667ms).
    fn note_control_group_double_tap(&mut self, group_num: u8) -> bool {
        const DOUBLE_TAP_MS: u128 = 667;
        let now = Instant::now();
        let tap = matches!(
            self.last_control_group_select,
            Some((g, t)) if g == group_num && now.duration_since(t).as_millis() < DOUBLE_TAP_MS
        );
        self.last_control_group_select = Some((group_num, now));
        tap
    }

    /// C++ `MSG_META_CREATE_TEAM` + `Player::processCreateTeamGameMessage`.
    fn create_control_group(&mut self, group_num: u8) {
        let ids = self.locally_controlled_selected_ids();
        if ids.is_empty() {
            self.control_groups.remove(&group_num);
            self.write_leftover_player_create_team(group_num, &[]);
            info!("Cleared control group {}", group_num);
            return;
        }
        self.evict_ids_from_other_control_groups(&ids, group_num);
        self.write_leftover_player_create_team(group_num, &ids);
        let count = ids.len();
        self.control_groups.insert(group_num, ids);
        info!("Assigned {} units to control group {}", count, group_num);
    }

    /// C++ `MSG_META_ADD_TEAM` — add stored squad to selection; do not rewrite the squad.
    fn add_control_group_to_selection(&mut self, group_num: u8) {
        let stored = self
            .control_groups
            .get(&group_num)
            .cloned()
            .unwrap_or_default();
        let double_tap = self.note_control_group_double_tap(group_num);
        if double_tap {
            if let Some(pos) = self.last_live_control_group_position(&stored) {
                let _ = self.host_center_camera_and_request_focus(pos);
            }
            return;
        }
        if self.first_selected_is_structure() {
            self.host_set_selection(self.current_player_id, Vec::new());
        }
        let live = {
            let Some(frame) = self.last_presentation_frame.as_ref() else {
                return;
            };
            // C++ ADD_TEAM: all getLiveObjects(), no local-owner gate.
            frame.filter_live_squad_ids(&stored, false)
        };
        if live.is_empty() {
            return;
        }
        let mut selection = self.selected_objects.clone();
        for id in live {
            if !selection.contains(&id) {
                selection.push(id);
            }
        }
        // C++ MSG_ADD_TEAM0..9 — m_teamExists only, no pickAndPlay (hq-xbyf3).
        self.host_set_selection_no_sound(self.current_player_id, selection);
        self.play_sound_effect(SoundType::Select);
        self.write_leftover_player_select_team(group_num, true);
    }

    /// C++ `MSG_META_VIEW_TEAM`: `if (group >= 1 && group <= 10) getHotkeySquad(group)`.
    /// VIEW_TEAM0 is a silent no-op; `getHotkeySquad(10)` is NULL.
    fn view_control_group(&mut self, group_num: u8) {
        if group_num < 1 || group_num > 9 {
            return;
        }
        let stored = self
            .control_groups
            .get(&group_num)
            .cloned()
            .unwrap_or_default();
        if stored.is_empty() {
            info!("Control group {} is empty (view)", group_num);
            return;
        }
        if let Some(pos) = self.last_live_control_group_position(&stored) {
            let clamped = self.host_center_camera_and_request_focus(pos);
            info!("VIEW_TEAM residual camera jump to {:?}", clamped);
        }
    }

    /// C++ `MSG_META_SELECT_TEAM` + double-tap lookAt last live member.
    fn select_control_group(&mut self, group_num: u8) {
        let stored = self
            .control_groups
            .get(&group_num)
            .cloned()
            .unwrap_or_default();
        if stored.is_empty() {
            // C++ SelectionXlat.cpp:1107-1110 deselectAllDrawables before
            // recalling the squad, so an empty group clears the current selection.
            info!("Control group {} is empty", group_num);
            self.host_set_selection(self.current_player_id, Vec::new());
            return;
        }
        let selection = {
            let Some(frame) = self.last_presentation_frame.as_ref() else {
                return;
            };
            // C++ SELECT_TEAM: getLiveObjects() + controllingPlayer == local.
            frame.filter_live_squad_ids(&stored, true)
        };
        let double_tap = self.note_control_group_double_tap(group_num);
        if double_tap {
            if let Some(pos) = self.last_live_control_group_position(&stored) {
                let clamped = self.clamp_to_world_bounds(pos);
                self.camera_target.x = clamped.x;
                self.camera_target.z = clamped.z;
                info!(
                    "Control group {} double-tap camera jump to {:?}",
                    group_num, clamped
                );
            }
            return;
        }
        self.host_set_selection(self.current_player_id, selection);
        self.play_sound_effect(SoundType::Select);
        self.write_leftover_player_select_team(group_num, false);
    }

    pub(super) fn handle_key_press(&mut self, key: &Key) {
        self.handle_mapped_key_press(key, None, false);
    }

    /// Live MetaEvent + default-hotkey path. `physical` supplies KEY_KP* /
    /// OEM punctuation VKs; `os_repeat` eats CommandMap autorepeat
    /// (C++ MetaEvent.cpp:463-470).
    pub(super) fn handle_mapped_key_press(
        &mut self,
        key: &Key,
        physical: Option<winit::keyboard::PhysicalKey>,
        os_repeat: bool,
    ) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            if os_repeat {
                return;
            }
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
                    // C++ UseableIn SHELL: Escape is WindowXlat / menu
                    // navigation, never MSG_META_OPTIONS app-quit.
                    info!("Escape pressed in Menu/Loading — GUI navigation only");
                }
                _ => {}
            }
            let _ = self.try_dispatch_command_map_remap(key, physical.as_ref(), true);
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

        // C++ MetaEvent eats KEY_STATE_AUTOREPEAT of known CommandMap keys
        // (no second meta). Chat already had its chance above.
        if os_repeat {
            return;
        }

        // C++ CommandXlat.cpp:2322-2326 — DISABLE_INPUT destroys every meta
        // except OPTIONS / CLEAR_GAME_DATA. Escape still opens QuitMenu.
        if !self.lookat_input_enabled() {
            if matches!(key, Key::Named(NamedKey::Escape)) {
                self.apply_meta_options_interrupt();
                #[cfg(feature = "game_client")]
                if self.host_toggle_retail_quit_menu() {
                    info!("Escape opened/toggled the retail QuitMenu WND");
                } else if matches!(self.current_state, GameState::InGame) {
                    info!("Escape QuitMenu WND unavailable - using pause fallback");
                    self.request_state_change(GameState::Paused);
                }
                #[cfg(not(feature = "game_client"))]
                if matches!(self.current_state, GameState::InGame) {
                    self.request_state_change(GameState::Paused);
                }
            }
            return;
        }

        // C++ MetaEventTranslator walks TheMetaMap so Keyboard Options remaps
        // apply on the next key. Number-row CREATE/SELECT/ADD/VIEW_TEAM stays
        // with FixSelectSquad.
        if self.try_dispatch_command_map_remap(key, physical.as_ref(), false) {
            return;
        }

        match key {
            Key::Named(NamedKey::Space) => {
                // Retail CommandMap VIEW_LAST_RADAR_EVENT KEY_SPACE residual.
                // Pause remains on P. C++ has no Alt+Space center binding.
                if !command_map_binds_host("VIEW_LAST_RADAR_EVENT") {
                    self.issue_named_command_from_ui("Command_ViewLastRadarEvent");
                }
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
                // C++ MSG_META_TOGGLE_ATTACKMOVE flips InGameUI attack-move mode.
                if matches!(
                    self.pending_map_command,
                    Some(PendingMapCommand::AttackMove)
                ) {
                    self.pending_map_command = None;
                    self.clear_radius_cursor_overlays();
                } else if !self.ui_selected_ids(self.current_player_id).is_empty() {
                    self.pending_map_command = Some(PendingMapCommand::AttackMove);
                    self.pending_structure_placement = None;
                    self.arm_radius_cursor_for_pending("ATTACK_CONTINUE_AREA");
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
                    // C++ CommandXlat MSG_DO_FORCE_ATTACK_GROUND is silent.
                }
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
                    if !command_map_binds_host("DELETE_BEACON") {
                        self.issue_named_command_from_ui("Command_RemoveBeacon");
                    }
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
                if !command_map_binds_host("DIPLOMACY") {
                    self.toggle_diplomacy_panel_hotkey();
                }
            }
            Key::Named(NamedKey::F1) => self.handle_camera_view_hotkey(0),
            Key::Named(NamedKey::F2) => self.handle_camera_view_hotkey(1),
            Key::Named(NamedKey::F3) => self.handle_camera_view_hotkey(2),
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
                if !command_map_binds_host("STOP") {
                    self.issue_named_command_from_ui("Command_Stop");
                }
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
                // C++ CommandMap Alt+G / ButtonGeneral: togglePurchaseScience only.
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
                if !command_map_binds_host("SCATTER") {
                    self.issue_named_command_from_ui("Command_Scatter");
                }
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
                // C++ CommandXlat MSG_META_BEGIN/END_WAYPOINTS is hold-Alt, silent.
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
                if !command_map_binds_host("SELECT_ALL") {
                    self.select_all_friendly_units();
                }
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
                if !command_map_binds_host("SELECT_MATCHING_UNITS") {
                    self.select_matching_units_hotkey();
                }
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
                // Retail SELECT_IDLE_WORKER residual (period key).
                self.host_select_next_idle_worker_from_control_bar();
            }
            Key::Character(c) if c == "," || c == "<" => {
                // Previous idle worker residual (comma key) — same C++ idle list.
                self.host_select_next_idle_worker_from_control_bar();
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
                if !command_map_binds_host("SELECT_ALL_AIRCRAFT") {
                    self.select_all_friendly_aircraft();
                }
            }
            Key::Character(c) if c.eq_ignore_ascii_case("h") && !ctrl_down => {
                // Retail CommandMap VIEW_COMMAND_CENTER KEY_H residual.
                if !command_map_binds_host("VIEW_COMMAND_CENTER") {
                    self.issue_named_command_from_ui("Command_ViewCommandCenter");
                }
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
                if !command_map_binds_host("SELECT_HERO") {
                    self.select_hero_units_hotkey();
                }
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
                if !command_map_binds_host("CREATE_FORMATION") {
                    self.issue_named_command_from_ui("Command_CreateFormation");
                }
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
                if !command_map_binds_host("PLACE_BEACON") {
                    self.issue_named_command_from_ui("Command_PlaceBeacon");
                }
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("c")
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Control)) =>
            {
                // Retail CommandMap ALL_CHEER Ctrl+C residual.
                if !command_map_binds_host("ALL_CHEER") {
                    self.issue_named_command_from_ui("Command_Cheer");
                }
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
                // C++ has no Auto-attack HUD overlay; leftover is silent.
            }
            Key::Named(NamedKey::F9) => {
                // Retail CommandMap TOGGLE_CONTROL_BAR KEY_F9 residual.
                // C++ CommandXlat.cpp:3156-3170 no-op in RECORDERMODETYPE_PLAYBACK.
                if !command_map_binds_host("TOGGLE_CONTROL_BAR")
                    && !self.presentation_or_boot_in_replay_game()
                {
                    #[cfg(feature = "game_client")]
                    {
                        let _ =
                            game_client::gui::callbacks::control_bar_callbacks::toggle_control_bar(
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
                if !command_map_binds_host("CHAT_EVERYONE")
                    && self.host_command_xlat_multiplayer_meta()
                {
                    self.open_chat_hotkey(crate::ui::ChatTarget::All);
                }
            }
            Key::Named(NamedKey::Backspace) => {
                if ctrl_down && self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) {
                    // Retail DEMO_INSTANT_QUIT Shift+Ctrl+Backspace residual.
                    info!("DEMO_INSTANT_QUIT residual — exiting");
                    self.request_state_change(GameState::Exiting);
                } else if !command_map_binds_host("CHAT_ALLIES")
                    && self.host_command_xlat_multiplayer_meta()
                {
                    self.open_chat_hotkey(crate::ui::ChatTarget::Allies);
                }
            }
            Key::Named(NamedKey::F12) => {
                // Retail CommandMap TAKE_SCREENSHOT KEY_F12 residual.
                if !command_map_binds_host("TAKE_SCREENSHOT") {
                    self.take_screenshot_hotkey();
                }
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("t")
                    && ctrl_down
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail TOGGLE_CAMERA_TRACKING_DRAWABLE Shift+Alt+Ctrl+T residual.
                if !command_map_binds_host("TOGGLE_CAMERA_TRACKING_DRAWABLE") {
                    self.toggle_camera_tracking_drawable_hotkey();
                }
            }
            Key::Character(c)
                if c.eq_ignore_ascii_case("f")
                    && !ctrl_down
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Shift))
                    && !self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) =>
            {
                // Retail TOGGLE_FAST_FORWARD_REPLAY KEY_F residual.
                if !command_map_binds_host("TOGGLE_FAST_FORWARD_REPLAY") {
                    self.toggle_replay_fast_forward_hotkey();
                }
            }
            Key::Named(NamedKey::Escape) => {
                // C++ MSG_META_OPTIONS always ToggleQuitMenu (CommandXlat.cpp:3091).
                // Placement/pending commands cancel via RMB, not OPTIONS.
                match self.current_state {
                    GameState::InGame => {
                        if self.chat_panel.is_open() {
                            self.chat_panel.close();
                            info!("Escape closed chat residual");
                        } else if self.diplomacy_panel.is_active() {
                            self.diplomacy_panel.close();
                            info!("Escape closed diplomacy panel residual");
                        } else {
                            // Retail CommandMap `OPTIONS` routes Escape to
                            // MSG_META_OPTIONS -> ToggleQuitMenu().
                            self.apply_meta_options_interrupt();
                            #[cfg(feature = "game_client")]
                            if self.host_toggle_retail_quit_menu() {
                                info!("Escape opened/toggled the retail QuitMenu WND");
                            } else {
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
                        self.apply_meta_options_interrupt();
                        info!("Escape pressed in Paused state - resuming");
                        self.request_state_change(GameState::InGame);
                    }
                    GameState::Menu | GameState::Loading => {
                        // C++ shell Escape is WindowXlat / menu callbacks, never app-quit.
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
        // C++ LookAtXlat.cpp:550-587 SAVE_VIEW / VIEW_VIEW stores full ViewLocation.
        self.save_or_recall_camera_view(slot);
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
        let _ = self.chat_panel.open();
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
    /// C++ CommandXlat.cpp:3216-3218 only enables; off is auto-clear on empty
    /// selection (GameClient.cpp:587-597).
    pub(super) fn toggle_camera_tracking_drawable_hotkey(&mut self) {
        self.camera_tracking_selection = true;
        #[cfg(feature = "game_client")]
        game_client::helpers::TheInGameUI::set_camera_tracking_drawable(true);
        info!("Camera tracking selection enabled");
    }

    /// Retail TOGGLE_FAST_FORWARD_REPLAY (m_TiVOFastMode) residual.
    /// C++ MetaEvent.cpp:473-489 — replay-only unless debug cheats; GUI:FF_ON/OFF.
    pub(super) fn toggle_replay_fast_forward_hotkey(&mut self) {
        if !self.presentation_or_boot_in_replay_game() {
            return;
        }
        self.replay_fast_forward = !self.replay_fast_forward;
        if let Ok(mut global) = game_engine::common::global_data::write_safe() {
            global.tivo_fast_mode = self.replay_fast_forward;
        }
        let key = if self.replay_fast_forward {
            "GUI:FF_ON"
        } else {
            "GUI:FF_OFF"
        };
        let msg = host_localized_gui_label(key);
        self.game_hud.push_info_message(&msg);
        self.ui_manager.game_hud_mut().push_info_message(&msg);
        info!("{msg}");
    }

    /// C++ CommandXlat.cpp:3098-3140 MSG_META_TOGGLE_LOWER_DETAILS.
    pub(super) fn toggle_lower_details_hotkey(&mut self) {
        #[cfg(feature = "game_client")]
        {
            let Some(now_low) =
                game_client::message_stream::meta_event::apply_toggle_lower_details()
            else {
                return;
            };
            let key = if now_low {
                "GUI:DetailsSetToLowest"
            } else {
                "GUI:ReturnGraphicsToPreviousSettings"
            };
            let msg = host_localized_gui_label(key);
            self.game_hud.push_info_message(&msg);
            self.ui_manager.game_hud_mut().push_info_message(&msg);
        }
    }

    /// Consult Keyboard Options / CommandMap.ini remaps. Returns true when the
    /// chord was consumed as a remapped meta command.
    fn try_dispatch_command_map_remap(
        &mut self,
        key: &Key,
        physical: Option<&winit::keyboard::PhysicalKey>,
        shell: bool,
    ) -> bool {
        #[cfg(feature = "game_client")]
        {
            let Some(code) = os_key_to_command_map_vk(key, physical) else {
                return false;
            };
            // Number row is CREATE/SELECT/ADD/VIEW_TEAM (FixSelectSquad).
            if (0x30..=0x39).contains(&code) {
                return false;
            }
            let mut mods = 0u32;
            if self.keys_pressed.contains(&Key::Named(NamedKey::Control)) {
                mods |= 1;
            }
            if self.keys_pressed.contains(&Key::Named(NamedKey::Alt)) {
                mods |= 2;
            }
            if self.keys_pressed.contains(&Key::Named(NamedKey::Shift)) {
                mods |= 4;
            }
            let usable = if shell {
                game_client::message_stream::meta_event::COMMAND_MAP_USABLE_SHELL
            } else {
                game_client::message_stream::meta_event::COMMAND_MAP_USABLE_GAME
            };
            let Some(name) =
                game_client::message_stream::meta_event::lookup_command_map_name_usable(
                    code, mods, usable,
                )
            else {
                return false;
            };
            let upper = name.to_ascii_uppercase();
            if upper.contains("CREATE_TEAM")
                || upper.contains("SELECT_TEAM")
                || upper.contains("ADD_TEAM")
                || upper.contains("VIEW_TEAM")
            {
                return false;
            }
            // C++ MetaEvent.cpp:56-71 SAVE_VIEW1..8 / VIEW_VIEW1..8.
            if let Some(slot) = command_map_view_slot("SAVE_VIEW", &upper) {
                self.store_or_apply_camera_view(slot, true);
                return true;
            }
            if let Some(slot) = command_map_view_slot("VIEW_VIEW", &upper) {
                self.store_or_apply_camera_view(slot, false);
                return true;
            }

            match upper.as_str() {
                "TOGGLE_FAST_FORWARD_REPLAY" => {
                    self.toggle_replay_fast_forward_hotkey();
                    true
                }
                "TOGGLE_LOWER_DETAILS" => {
                    self.toggle_lower_details_hotkey();
                    true
                }
                "SELECT_ALL" => {
                    self.select_all_friendly_units();
                    true
                }
                "SELECT_ALL_AIRCRAFT" => {
                    self.select_all_friendly_aircraft();
                    true
                }
                "SELECT_MATCHING_UNITS" => {
                    self.select_matching_units_hotkey();
                    true
                }
                "SELECT_HERO" => {
                    self.select_hero_units_hotkey();
                    true
                }
                "SELECT_NEXT_UNIT" => {
                    self.cycle_friendly_selection(1);
                    true
                }
                "SELECT_PREV_UNIT" => {
                    self.cycle_friendly_selection(-1);
                    true
                }
                "SELECT_NEXT_WORKER" => {
                    self.cycle_friendly_worker_selection(1);
                    true
                }
                "SELECT_PREV_WORKER" => {
                    self.cycle_friendly_worker_selection(-1);
                    true
                }
                "SCATTER" => {
                    self.issue_named_command_from_ui("Command_Scatter");
                    true
                }
                "STOP" => {
                    self.issue_named_command_from_ui("Command_Stop");
                    true
                }
                "DEPLOY" => {
                    self.issue_named_command_from_ui("Command_Deploy");
                    true
                }
                "PLACE_BEACON" => {
                    self.issue_named_command_from_ui("Command_PlaceBeacon");
                    true
                }
                "DELETE_BEACON" => {
                    self.issue_named_command_from_ui("Command_RemoveBeacon");
                    true
                }
                "VIEW_LAST_RADAR_EVENT" => {
                    self.issue_named_command_from_ui("Command_ViewLastRadarEvent");
                    true
                }
                "VIEW_COMMAND_CENTER" => {
                    self.issue_named_command_from_ui("Command_ViewCommandCenter");
                    true
                }
                "CHEER" | "ALL_CHEER" => {
                    self.issue_named_command_from_ui("Command_Cheer");
                    true
                }
                "CREATE_FORMATION" => {
                    self.issue_named_command_from_ui("Command_CreateFormation");
                    true
                }
                "DIPLOMACY" => {
                    self.toggle_diplomacy_panel_hotkey();
                    true
                }
                "TOGGLE_CONTROL_BAR" => {
                    if self.presentation_or_boot_in_replay_game() {
                        return true;
                    }
                    #[cfg(feature = "game_client")]
                    {
                        let _ =
                            game_client::gui::callbacks::control_bar_callbacks::toggle_control_bar(
                                true,
                            );
                    }
                    self.game_hud.toggle_visibility();
                    self.ui_manager.game_hud_mut().toggle_visibility();
                    true
                }
                "TAKE_SCREENSHOT" => {
                    self.take_screenshot_hotkey();
                    true
                }
                "CHAT_EVERYONE" => {
                    if self.host_command_xlat_multiplayer_meta() {
                        self.open_chat_hotkey(crate::ui::ChatTarget::All);
                    }
                    true
                }
                "CHAT_ALLIES" => {
                    if self.host_command_xlat_multiplayer_meta() {
                        self.open_chat_hotkey(crate::ui::ChatTarget::Allies);
                    }
                    true
                }
                "TOGGLE_CAMERA_TRACKING_DRAWABLE" => {
                    self.toggle_camera_tracking_drawable_hotkey();
                    true
                }
                "CAMERA_RESET" => {
                    self.reset_camera_view_hotkey();
                    true
                }
                "OPTIONS" => {
                    // C++ CommandXlat.cpp:3091-3094 MSG_META_OPTIONS → ToggleQuitMenu.
                    #[cfg(feature = "game_client")]
                    if !self.host_toggle_retail_quit_menu()
                        && matches!(self.current_state, GameState::InGame)
                    {
                        info!("OPTIONS QuitMenu WND unavailable - using pause fallback");
                        self.request_state_change(GameState::Paused);
                    }
                    #[cfg(not(feature = "game_client"))]
                    if matches!(self.current_state, GameState::InGame) {
                        self.request_state_change(GameState::Paused);
                    }
                    true
                }
                _ => false,
            }
        }
        #[cfg(not(feature = "game_client"))]
        {
            let _ = (key, physical, shell);
            false
        }
    }

    /// Follow the first selected drawable (C++ GameClient.cpp:587-597).
    pub(super) fn update_camera_tracking_drawable(&mut self) {
        if !self.camera_tracking_selection {
            return;
        }
        // Wave 226: selection via presentation-first ui_selected_ids.
        // C++ `getFirstSelectedDrawable()` is the first list entry, not centroid.
        let ids = self.ui_selected_ids(self.current_player_id);
        let Some(first) = ids.first().copied() else {
            self.camera_tracking_selection = false;
            #[cfg(feature = "game_client")]
            game_client::helpers::TheInGameUI::set_camera_tracking_drawable(false);
            return;
        };
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let Some(obj) = frame.objects.iter().find(|o| o.id == first) else {
            return;
        };
        let pos = glam::Vec3::new(obj.position.x, obj.position.y, obj.position.z);
        let clamped = self.clamp_to_world_bounds(pos);
        self.camera_target.x = clamped.x;
        self.camera_target.z = clamped.z;
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
                // C++ CommandXlat MSG_META_TAKE_SCREENSHOT only takeScreenShot().
                info!("screenshot written to {}", path.display());
            }
            Err(err) => {
                warn!("screenshot failed: {err:?}");
            }
        }
    }

    /// Retail DIPLOMACY (KEY_TAB) residual.
    pub(super) fn toggle_diplomacy_panel_hotkey(&mut self) {
        self.sync_diplomacy_panel_from_world();
        self.diplomacy_panel.toggle();
        // C++ CommandXlat MSG_META_DIPLOMACY only ToggleDiplomacy(FALSE).
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
        // C++ InGameUI.cpp:4139-4143 resetCamera at the *current* look-at.
        // Do not retarget the command center / team base.
        let stay = self.camera_target;
        self.reset_camera_pose_in_place();
        info!("CAMERA_RESET residual in-place -> {:?}", stay);
    }
}

fn command_map_binds_host(name: &str) -> bool {
    #[cfg(feature = "game_client")]
    {
        game_client::message_stream::meta_event::command_map_binds(name)
    }
    #[cfg(not(feature = "game_client"))]
    {
        let _ = name;
        false
    }
}

/// Retail CommandMap SAVE_VIEW1..8 / VIEW_VIEW1..8 slot (0-based).
fn command_map_view_slot(prefix: &str, name: &str) -> Option<usize> {
    let rest = name.strip_prefix(prefix)?;
    let slot: usize = rest.parse().ok()?;
    (1..=8).contains(&slot).then_some(slot - 1)
}

fn host_localized_gui_label(key: &str) -> String {
    #[cfg(feature = "game_client")]
    {
        let (text, exists) = game_client::game_text::GameText::fetch_with_exists(key);
        if exists {
            return text;
        }
    }
    crate::localization::localize(key, key)
}

fn os_key_to_command_map_vk(
    key: &winit::keyboard::Key,
    physical: Option<&winit::keyboard::PhysicalKey>,
) -> Option<u32> {
    use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
    if let Some(PhysicalKey::Code(code)) = physical {
        let from_physical = match code {
            KeyCode::Numpad0 => Some(0x60),
            KeyCode::Numpad1 => Some(0x61),
            KeyCode::Numpad2 => Some(0x62),
            KeyCode::Numpad3 => Some(0x63),
            KeyCode::Numpad4 => Some(0x64),
            KeyCode::Numpad5 => Some(0x65),
            KeyCode::Numpad6 => Some(0x66),
            KeyCode::Numpad7 => Some(0x67),
            KeyCode::Numpad8 => Some(0x68),
            KeyCode::Numpad9 => Some(0x69),
            KeyCode::NumpadDecimal => Some(0x6E),
            KeyCode::NumpadMultiply => Some(0x6A),
            KeyCode::NumpadSubtract => Some(0x6D),
            KeyCode::NumpadAdd => Some(0x6B),
            KeyCode::NumpadDivide => Some(0x6F),
            KeyCode::NumpadEnter => Some(0x0D),
            KeyCode::Minus => Some(0xBD),
            KeyCode::Equal => Some(0xBB),
            KeyCode::BracketLeft => Some(0xDB),
            KeyCode::BracketRight => Some(0xDD),
            KeyCode::Semicolon => Some(0xBA),
            KeyCode::Quote => Some(0xDE),
            KeyCode::Backquote => Some(0xC0),
            KeyCode::Backslash => Some(0xDC),
            KeyCode::Comma => Some(0xBC),
            KeyCode::Period => Some(0xBE),
            KeyCode::Slash => Some(0xBF),
            KeyCode::Insert => Some(0x2D),
            _ => None,
        };
        if from_physical.is_some() {
            return from_physical;
        }
    }
    match key {
        Key::Character(c) => {
            let raw = c.as_str();
            let ch = raw.chars().next()?.to_ascii_uppercase();
            if ch.is_ascii_alphanumeric() {
                Some(ch as u32)
            } else {
                match raw {
                    "," | "<" => Some(0xBC),
                    "." | ">" => Some(0xBE),
                    "-" | "_" => Some(0xBD),
                    "=" | "+" => Some(0xBB),
                    "[" | "{" => Some(0xDB),
                    "]" | "}" => Some(0xDD),
                    ";" | ":" => Some(0xBA),
                    "'" | "\"" => Some(0xDE),
                    "`" | "~" => Some(0xC0),
                    "\\" | "|" => Some(0xDC),
                    "/" | "?" => Some(0xBF),
                    _ => None,
                }
            }
        }
        Key::Named(NamedKey::Space) => Some(0x20),
        Key::Named(NamedKey::Enter) => Some(0x0D),
        Key::Named(NamedKey::Escape) => Some(0x1B),
        Key::Named(NamedKey::Tab) => Some(0x09),
        Key::Named(NamedKey::Backspace) => Some(0x08),
        Key::Named(NamedKey::Delete) => Some(0x2E),
        Key::Named(NamedKey::Home) => Some(0x24),
        Key::Named(NamedKey::End) => Some(0x23),
        Key::Named(NamedKey::PageUp) => Some(0x21),
        Key::Named(NamedKey::PageDown) => Some(0x22),
        Key::Named(NamedKey::ArrowLeft) => Some(0x25),
        Key::Named(NamedKey::ArrowUp) => Some(0x26),
        Key::Named(NamedKey::ArrowRight) => Some(0x27),
        Key::Named(NamedKey::ArrowDown) => Some(0x28),
        Key::Named(NamedKey::F1) => Some(0x70),
        Key::Named(NamedKey::F2) => Some(0x71),
        Key::Named(NamedKey::F3) => Some(0x72),
        Key::Named(NamedKey::F4) => Some(0x73),
        Key::Named(NamedKey::F5) => Some(0x74),
        Key::Named(NamedKey::F6) => Some(0x75),
        Key::Named(NamedKey::F7) => Some(0x76),
        Key::Named(NamedKey::F8) => Some(0x77),
        Key::Named(NamedKey::F9) => Some(0x78),
        Key::Named(NamedKey::F10) => Some(0x79),
        Key::Named(NamedKey::F11) => Some(0x7A),
        Key::Named(NamedKey::F12) => Some(0x7B),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::os_key_to_command_map_vk;
    use winit::keyboard::{Key, KeyCode, PhysicalKey};

    #[test]
    fn os_key_maps_numpad_and_oem_punctuation_to_command_map_vks() {
        let kp5 = PhysicalKey::Code(KeyCode::Numpad5);
        assert_eq!(
            os_key_to_command_map_vk(&Key::Character("5".into()), Some(&kp5)),
            Some(0x65),
            "Numpad5 is KEY_KP5, not KEY_5"
        );
        assert_eq!(
            os_key_to_command_map_vk(&Key::Character("5".into()), None),
            Some(0x35),
            "logical 5 without physical stays KEY_5"
        );
        let comma = PhysicalKey::Code(KeyCode::Comma);
        assert_eq!(
            os_key_to_command_map_vk(&Key::Character(",".into()), Some(&comma)),
            Some(0xBC)
        );
        assert_eq!(
            os_key_to_command_map_vk(&Key::Character("-".into()), None),
            Some(0xBD)
        );
        assert_eq!(
            os_key_to_command_map_vk(&Key::Character("[".into()), None),
            Some(0xDB)
        );
    }

    #[test]
    fn escape_options_stops_rmb_scroll_and_cancels_drag() {
        let src = include_str!("hotkeys.rs");
        assert!(src.contains("self.apply_meta_options_interrupt()"));
        let mouse = super::ENGINE_SRC;
        assert!(mouse.contains("fn apply_meta_options_interrupt"));
        assert!(mouse.contains("self.stop_rmb_lookat_scroll()"));
        assert!(mouse.contains("self.cancel_area_select_from_control_bar()"));
    }

    #[test]
    fn command_map_view_slot_parses_save_and_view() {
        assert_eq!(
            super::command_map_view_slot("SAVE_VIEW", "SAVE_VIEW1"),
            Some(0)
        );
        assert_eq!(
            super::command_map_view_slot("SAVE_VIEW", "SAVE_VIEW8"),
            Some(7)
        );
        assert_eq!(
            super::command_map_view_slot("VIEW_VIEW", "VIEW_VIEW4"),
            Some(3)
        );
        assert_eq!(
            super::command_map_view_slot("VIEW_VIEW", "VIEW_LAST_RADAR_EVENT"),
            None
        );
        assert_eq!(
            super::command_map_view_slot("SAVE_VIEW", "SAVE_VIEW9"),
            None
        );
        assert_eq!(super::command_map_view_slot("SAVE_VIEW", "SAVE_VIEW"), None);
    }

    #[test]
    fn select_all_is_not_hardcoded_ctrl_a() {
        let src = include_str!("hotkeys.rs");
        let prod = src.split("#[cfg(test)]").next().expect("prod");
        assert!(prod.contains("Retail CommandMap SELECT_ALL KEY_Q residual"));
        let q = prod
            .find("Retail CommandMap SELECT_ALL KEY_Q residual")
            .expect("Q residual");
        let window = &prod[q..prod.len().min(q + 250)];
        assert!(
            window.contains("select_all_friendly_units"),
            "Q residual must still select all when CommandMap is unbound"
        );
        assert!(
            !prod.contains("Convenience alias"),
            "live host must not invent an unguarded Ctrl+A SELECT_ALL"
        );
        assert!(
            !prod.contains("eq_ignore_ascii_case(\"a\")")
                || !{
                    let mut i = 0;
                    let mut invented = false;
                    while let Some(at) = prod[i..].find("eq_ignore_ascii_case(\"a\")") {
                        let at = i + at;
                        let win = &prod[at..prod.len().min(at + 220)];
                        if win.contains("NamedKey::Control")
                            && win.contains("select_all_friendly_units")
                            && !win.contains("NamedKey::Alt")
                            && !win.contains("NamedKey::Shift")
                        {
                            invented = true;
                            break;
                        }
                        i = at + 1;
                    }
                    invented
                },
            "Ctrl+A must not call select_all_friendly_units"
        );
    }
}
