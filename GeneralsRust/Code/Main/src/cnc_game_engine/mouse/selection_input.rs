#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    /// C++ `GameWinBlockInput` `GWM_LEFT_UP` (`GameWindow.cpp:1480-1491`):
    /// release over the control bar cancels the marquee without applying
    /// the area selection.
    pub(in crate::cnc_game_engine) fn cancel_area_select_from_control_bar(&mut self) {
        self.is_dragging = false;
        self.selection_start = None;
        self.selection_start_screen = None;
        self.left_click_release_behavior = LeftMouseReleaseBehavior::Selection;
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheInGameUI::set_selecting(false);
        }
    }

    /// C++ `LookAtXlat` `MSG_META_OPTIONS` `stopScrolling` + `SelectionXlat`
    /// `m_leftMouseButtonIsDown = FALSE`.
    pub(in crate::cnc_game_engine) fn apply_meta_options_interrupt(&mut self) {
        let _ = super::selection_hud::meta_options_clears_lookat_and_drag();
        self.stop_rmb_lookat_scroll();
        self.cancel_area_select_from_control_bar();
    }

    pub(in crate::cnc_game_engine) fn host_quit_menu_blocks_world_selection(&self) -> bool {
        if self.quit_menu_host_active {
            return true;
        }
        #[cfg(feature = "game_client")]
        {
            return game_client::helpers::TheInGameUI::is_quit_menu_visible()
                || game_client::gui::callbacks::is_quit_menu_visible();
        }
        #[cfg(not(feature = "game_client"))]
        {
            false
        }
    }

    /// C++ `SelectionXlat.cpp:953-961` RMB-down click-vs-scroll samples.
    pub(in crate::cnc_game_engine) fn note_rmb_deselect_anchor(&mut self) {
        self.rmb_deselect_down_at = Some(Instant::now());
        self.rmb_deselect_down_screen = Some(self.mouse_position);
        self.rmb_deselect_down_camera = Some(self.camera_position);
    }

    /// C++ `SelectionXlat.cpp:982-1000`: pixel + time + camera 3D gates.
    pub(in crate::cnc_game_engine) fn rmb_release_is_deselect_click(&self) -> bool {
        let Some(anchor) = self.rmb_deselect_down_screen else {
            return false;
        };
        let dx = self.mouse_position.0 - anchor.0;
        let dy = self.mouse_position.1 - anchor.1;
        let elapsed_ms = self
            .rmb_deselect_down_at
            .map(|started| started.elapsed().as_millis())
            .unwrap_or(u128::MAX);
        let camera_delta = self
            .rmb_deselect_down_camera
            .map(|down| (self.camera_position - down).length())
            .unwrap_or(f32::MAX);
        host_rmb_release_is_click(dx, dy, elapsed_ms, camera_delta)
    }

    pub(in crate::cnc_game_engine) fn host_is_selecting(&self) -> bool {
        host_is_selecting_now(
            self.is_dragging,
            self.pending_structure_placement.is_some(),
            self.selection_start_screen,
            self.mouse_position,
        )
    }

    pub(in crate::cnc_game_engine) fn sync_host_selecting_flag(&self) {
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheInGameUI::set_selecting(self.host_is_selecting());
        }
    }

    fn apply_structure_placement_angle(&mut self, angle: f32) {
        self.game_hud
            .construction_panel
            .rotate_structure_placement(angle);
        self.ui_manager
            .game_hud_mut()
            .construction_panel
            .rotate_structure_placement(angle);
        game_client::helpers::TheInGameUI::set_placement_angle(angle);
    }

    /// C++ `W3DView::screenToTerrain` analog through the live WGPU camera.
    /// Placement drag/confirm reprojects the original screen pixel so a camera
    /// pan during rotate does not spin the ghost around a stale world point.
    fn screen_to_terrain(&self, screen: (f32, f32)) -> Option<Vec3> {
        let (view_w, view_h) = self.tactical_viewport_size();
        let (world_min, world_max) = self.presentation_world_bounds();
        let world_env = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_ref())
            .map(|frame| &frame.world_env);
        unproject_mouse_ray(
            self.view_matrix,
            self.projection_matrix,
            screen,
            view_w,
            view_h,
        )
        .and_then(|(near, far)| {
            raycast_frozen_terrain(near, far, world_min, world_max, world_env).or_else(|| {
                raycast_ground_plane_clamped(near, far, world_min, world_max, world_env)
            })
        })
    }

    /// C++ `PlaceEventTranslator.cpp:68-75` — `findObjectByID` of the pending
    /// place source. Dead/sold/missing builder cancels instead of anchoring.
    fn pending_place_builder_is_gone(&self) -> bool {
        let builder_id = game_client::helpers::TheInGameUI::get_pending_place_source_object_id();
        if builder_id == 0 {
            return true;
        }
        let id = crate::game_logic::ObjectId(builder_id);
        let Some(object) = self.presentation_ro(id) else {
            return !self.presentation_or_boot_object_alive(id);
        };
        object.destroyed || object.health_current <= 0.0 || object.sold
    }

    /// C++ PlaceEventTranslator RAW_MOUSE_POSITION: `setPlacementEnd` after 5px.
    pub(in crate::cnc_game_engine) fn update_anchored_placement_from_cursor(&mut self) {
        if self.pending_structure_placement.is_none() || !self.is_dragging {
            return;
        }
        let Some(start) = self.selection_start_screen else {
            return;
        };
        let dx = self.mouse_position.0 - start.0;
        let dy = self.mouse_position.1 - start.1;
        if !placement_screen_drag_exceeds_threshold(dx, dy) {
            return;
        }
        let end = game_client::message_stream::game_message::ICoord2D::new(
            self.mouse_position.0 as i32,
            self.mouse_position.1 as i32,
        );
        game_client::helpers::TheInGameUI::set_placement_end(Some(end));
        let start_world = self
            .screen_to_terrain(start)
            .or(self.selection_start)
            .unwrap_or(self.mouse_world_position);
        let end_world = self.mouse_world_position;
        let wdx = end_world.x - start_world.x;
        let wdz = end_world.z - start_world.z;
        if wdx.abs() <= f32::EPSILON && wdz.abs() <= f32::EPSILON {
            return;
        }
        self.apply_structure_placement_angle(wdz.atan2(wdx));
    }

    pub(in crate::cnc_game_engine) fn handle_left_click(&mut self) {
        if !self.lookat_input_enabled() {
            return;
        }
        self.is_dragging = true;
        self.selection_start = Some(self.mouse_world_position);
        self.selection_start_screen = Some(self.mouse_position);
        self.left_click_release_behavior = LeftMouseReleaseBehavior::Selection;
        let mouse_pos = self.mouse_world_position;
        // C++ `SelectionXlat.cpp:469` / `:431`: a pick miss is no drawable.
        // Ground click never invents the first locally-owned selectable.
        let clicked_object = self.find_object_at_cursor(false);

        // C++ GameClient.cpp:276-280 attach order (lower number first):
        // PlaceEventTranslator 30, GUICommandTranslator 40, SelectionTranslator
        // 50, CommandTranslator 70.  Both Place and GUI own LMB in every
        // mouse layout and must outrank a stale double-click selection.
        if let Some(template) = self.pending_structure_placement.clone() {
            let _ = template;
            // C++ PlaceEventTranslator.cpp:68-75: missing builder
            // `placeBuildAvailable(NULL,NULL)` and does not anchor.
            if self.pending_place_builder_is_gone() {
                self.cancel_structure_placement_from_ui();
            } else {
                let start = game_client::message_stream::game_message::ICoord2D::new(
                    self.mouse_position.0 as i32,
                    self.mouse_position.1 as i32,
                );
                game_client::helpers::TheInGameUI::set_placement_start(Some(start));
                return;
            }
        }
        if self.pending_map_command.is_some() {
            // C++ CommandXlat issueMoveToLocationCommand / evaluateContextCommand:
            // waypoint mode (Alt or sticky) outranks any armed GUI command.
            let waypoint =
                self.sticky_waypoint_mode || self.keys_pressed.contains(&Key::Named(NamedKey::Alt));
            if waypoint {
                let mut selected = self.ui_selected_ids(self.current_player_id);
                if selected.is_empty() {
                    selected = self.selected_objects.clone();
                }
                if !selected.is_empty() {
                    self.host_queue_and_process_command_silent(
                        crate::command_system::GameCommand {
                            command_type: crate::command_system::CommandType::AddWaypoint {
                                destination: mouse_pos,
                            },
                            player_id: self.current_player_id,
                            command_id: 0,
                            timestamp: std::time::SystemTime::now(),
                            selected_units: selected,
                            modifier_keys: crate::command_system::ModifierKeys {
                                ctrl: false,
                                shift: false,
                                alt: true,
                            },
                        },
                    );
                }
                self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
                return;
            }
            self.commit_pending_map_command(mouse_pos, clicked_object);
            self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
            return;
        }

        // C++ Mouse.cpp:209-214 — OS double-click (GetDoubleClickTime + SM_CXDOUBLECLK).
        let now = Instant::now();
        let is_double_click = if let (Some(last_time), Some(last_screen)) =
            (self.last_click_time, self.last_click_position)
        {
            let time_delta = now.duration_since(last_time).as_millis();
            let dx = self.mouse_position.0 - last_screen.0;
            let dy = self.mouse_position.1 - last_screen.1;
            is_os_style_double_click(
                time_delta,
                dx,
                dy,
                os_double_click_time_ms(),
                OS_DOUBLE_CLICK_SLOP_PX,
            )
        } else {
            false
        };

        self.last_click_time = Some(now);
        self.last_click_position = Some(self.mouse_position);

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        let alt_down = self.keys_pressed.contains(&Key::Named(NamedKey::Alt));

        if is_double_click && !ctrl_down {
            // C++ SelectionXlat.cpp:453-521 DESTROYs LEFT_DOUBLE_CLICK only when
            // the pick is mass-selectable and locally controlled. Otherwise
            // KEEP_MESSAGE so CommandXlat.cpp:3698-3713 can issue DoGuardPosition
            // (enemy, building, or terrain) when UseDoubleClickAttackMove is on.
            let picked = self.find_object_at_cursor(false);
            if let Some(object_id) = picked {
                if self.presentation_double_click_consumes(object_id) {
                    self.select_similar_units_for_double_click(object_id, alt_down);
                    self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
                    return;
                }
            }
            if self.host_try_double_click_guard_command(false) {
                self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
                return;
            }
        }

        let had_selection = !self.ui_selected_ids(self.current_player_id).is_empty();
        if !self.use_alternate_mouse && ctrl_down && had_selection {
            // Classic layout: CommandXlat consumes Ctrl+LMB as force attack.
            // Alternate layout leaves force attack on its RMB context route.
            self.issue_force_attack_from_left_click(mouse_pos, clicked_object);
            self.left_click_release_behavior = LeftMouseReleaseBehavior::Suppress;
            return;
        }

        let target_is_locally_selectable = clicked_object
            .is_some_and(|object_id| self.is_locally_selectable_click_target(object_id));
        if !self.use_alternate_mouse
            && classic_left_context_action_allowed(
                had_selection,
                shift_down,
                target_is_locally_selectable,
            )
        {
            // In C++'s classic layout, LMB first gives CommandXlat an
            // opportunity to issue a context command.  Defer until release so
            // a box drag remains SelectionXlat-owned. Shift is only a
            // selection override for a locally selectable target: C++
            // SelectionInfo still lets Shift+LMB issue a context action on
            // terrain, enemy, allied, civilian, or crate targets. If the
            // context probe yields no command, `handle_left_release` falls
            // through to ordinary selection of the clicked drawable.
            self.left_click_release_behavior = LeftMouseReleaseBehavior::ContextCommand;
            return;
        }

        // C++ SelectionXlat.cpp:890-898: RAW LMB down only sets the
        // select-feedback anchor. Point selection commits on
        // MSG_MOUSE_LEFT_CLICK (non-drag release at the up pixel).
    }

    /// Apply the selection half of C++ `SelectionXlat` for a point click that
    /// was not consumed by a command/placement path.
    fn select_left_click_target(&mut self, object_id: ObjectId, shift_down: bool) {
        if shift_down && self.is_locally_selectable_click_target(object_id) {
            // C++ Shift+select residual: toggle only locally owned units.
            // Enemy/civilian/allied point clicks always replace (SelectionXlat.cpp:679-693).
            self.toggle_select_object(object_id);
            return;
        }

        if !self.is_point_selectable_click_target(object_id) {
            return;
        }
        // C++ SelectionXlat.cpp:734-739 always posts MSG_CREATE_SELECTED_GROUP.
        // host_set_selection skips SelectObjects when residuals already match,
        // so re-click must still pickAndPlay VoiceSelect (hq-bb8in).
        let replay_voice = self.selected_objects.as_slice() == [object_id]
            && self.host_match_selected_ids.as_deref() == Some(&[object_id][..]);
        // Wave 583: selection residual via host_set_selection.
        self.host_set_selection(self.current_player_id, vec![object_id]);
        if replay_voice {
            self.host_game_logic_mut()
                .queue_create_selected_group_voice(&[object_id]);
        }
        // C++ SelectionXlat.cpp:704-705: successful LMB select resets last group.
        self.last_control_group_select = None;
        self.play_sound_effect(SoundType::Select);
    }

    /// Whether the frozen object is a locally owned target that SelectionXlat
    /// may select. This mirrors the point-selection predicate used both for a
    /// normal LMB selection and for classic Shift+LMB's prefer-selection
    /// override.
    fn is_locally_selectable_click_target(&self, object_id: ObjectId) -> bool {
        // Wave 1104: belt-and-suspenders local selectable check (pick peels FOW first).
        self.last_presentation_frame.as_ref().is_some_and(|frame| {
            frame.objects.iter().any(|o| {
                o.id == object_id
                    && frame.is_owned_by_local(o)
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
            })
        })
    }

    /// C++ `CanSelectDrawable` + lone enemy/civilian/allied point select
    /// (`SelectionXlat.cpp:181-189`, `679-693`). Drag-select stays local-only.
    fn is_point_selectable_click_target(&self, object_id: ObjectId) -> bool {
        self.last_presentation_frame.as_ref().is_some_and(|frame| {
            frame.objects.iter().any(|o| {
                o.id == object_id
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(o)
                    && !frame.box_pick_hides_non_local(o)
            })
        })
    }

    /// Shift+click residual: add friendly unit or remove if already selected.
    pub(in crate::cnc_game_engine) fn toggle_select_object(&mut self, object_id: ObjectId) {
        // Presentation-only: InGame always has last_presentation_frame.
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        // Only toggle friendly selectable units (enemy click under Shift still replaces? retail
        // keeps multi-select among friendlies; enemy under Shift is ignored for add).
        let is_friendly_selectable = frame
            .objects
            .iter()
            .find(|o| o.id == object_id)
            .map(|o| {
                frame.is_owned_by_local(o)
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
            // C++ MSG_REMOVE_FROM_SELECTED_GROUP — no pickAndPlay (hq-xbyf3).
            self.host_set_selection_no_sound(self.current_player_id, selection);
        } else {
            selection.push(object_id);
            // C++ MSG_CREATE_SELECTED_GROUP (addToGroup) — VoiceSelect.
            self.host_set_selection(self.current_player_id, selection);
        }
        // C++ SelectionXlat.cpp:704-705 after DESTROY_MESSAGE LMB select.
        self.last_control_group_select = None;
        self.play_sound_effect(SoundType::Select);
    }

    /// Ctrl+LMB ForceAttack residual (object or ground).
    /// Wave 612: via `host_issue_force_attack_from_left_click`.
    pub(in crate::cnc_game_engine) fn issue_force_attack_from_left_click(
        &mut self,
        location: Vec3,
        target_object: Option<ObjectId>,
    ) {
        // Wave 612: thin wrapper — residual via host helper.
        self.host_issue_force_attack_from_left_click(location, target_object)
    }

    /// Ctrl+LMB ForceAttack residual (object or ground).
    pub(in crate::cnc_game_engine) fn host_issue_force_attack_from_left_click(
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

        if !self.host_selection_can_force_attack(location, target_object) {
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

    fn host_note_right_double_click(&mut self) -> bool {
        let now = Instant::now();
        let mouse_pos = self.mouse_world_position;
        let is_double = if let (Some(last_time), Some(last_pos)) =
            (self.last_right_click_time, self.last_right_click_position)
        {
            let time_delta = now.duration_since(last_time).as_millis();
            let pos_delta = (mouse_pos - last_pos).length();
            time_delta < 500 && pos_delta < 10.0
        } else {
            false
        };
        self.last_right_click_time = Some(now);
        self.last_right_click_position = Some(mouse_pos);
        is_double
    }

    /// C++ CommandXlat.cpp:3635-3713 double-click attack-move → MSG_DO_GUARD_POSITION.
    fn host_try_double_click_guard_command(&mut self, right_click: bool) -> bool {
        let double_click_attack_move =
            game_engine::common::global_data::read().double_click_attack_move;
        if !double_click_attack_move {
            return false;
        }
        let should_issue_guard = if right_click {
            self.use_alternate_mouse
        } else {
            !self.use_alternate_mouse
        };
        if !should_issue_guard {
            return false;
        }

        let mut selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        if !selected.is_empty() {
            self.host_queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::Guard {
                    target: crate::command_system::GuardTarget::Position(self.mouse_world_position),
                    mode: crate::game_logic::GuardMode::Normal,
                },
                player_id: self.current_player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: selected,
                modifier_keys: crate::command_system::ModifierKeys {
                    ctrl: false,
                    shift: false,
                    alt: false,
                },
            });
            self.host_process_commands_with_command_sound();
        }
        #[cfg(feature = "game_client")]
        game_client::helpers::TheInGameUI::trigger_double_click_attack_move_guard_hint();
        true
    }

    /// C++ `canAnyForceAttack` / `canObjectForceAttack` (CommandXlat.cpp:152-267).
    fn host_selection_can_force_attack(
        &self,
        location: Vec3,
        target_object: Option<ObjectId>,
    ) -> bool {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return false;
        };
        let selected = self.ui_selected_ids(self.current_player_id);
        for id in selected {
            let Some(attacker) = frame.objects.iter().find(|o| o.id == id) else {
                continue;
            };
            if presentation_object_can_force_attack(frame, attacker, target_object, location) {
                return true;
            }
        }
        false
    }

    pub(in crate::cnc_game_engine) fn select_similar_units(&mut self, clicked_object_id: ObjectId) {
        // C++ `InGameUI::selectUnitsMatchingCurrentSelection` (`InGameUI.cpp:4900-4916`):
        // screen first, then map. `selectMatchingAcrossRegion` (`:4671-4750`) unions
        // every locally-controlled selected template (`isEquivalentTo` + carbomb)
        // and ADDS (`MSG_CREATE_SELECTED_GROUP_NO_SOUND` createNewGroup=false).
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let player_team = frame.local_team();
        let current = self.ui_selected_ids(self.current_player_id);
        let mut seeds: Vec<ObjectId> = current
            .iter()
            .copied()
            .filter(|&id| {
                frame
                    .objects
                    .iter()
                    .any(|object| object.id == id && frame.is_owned_by_local(object))
            })
            .collect();
        if seeds.is_empty() {
            if frame
                .objects
                .iter()
                .any(|object| object.id == clicked_object_id && frame.is_owned_by_local(object))
            {
                seeds.push(clicked_object_id);
            } else {
                return;
            }
        }

        let (vw, vh) = self.tactical_viewport_size();
        let viewport = glam::Vec2::new(vw, vh);
        let mut screen = Vec::new();
        for seed in &seeds {
            screen = union_object_ids(
                screen,
                frame.similar_unit_ids_across_screen(
                    *seed,
                    player_team,
                    self.view_matrix,
                    self.projection_matrix,
                    viewport,
                ),
            );
        }
        let screen_added = screen.iter().any(|id| !current.contains(id));
        let matching = if screen_added {
            screen
        } else {
            let mut map_wide = Vec::new();
            for seed in &seeds {
                map_wide = union_object_ids(map_wide, frame.similar_unit_ids(*seed, player_team));
            }
            map_wide
        };

        let added = matching.iter().any(|id| !current.contains(id));
        if !added {
            return;
        }
        let selection = union_object_ids(current, matching);
        self.host_set_selection(self.current_player_id, selection);
        self.play_sound_effect(SoundType::Select);
    }

    /// C++ `MSG_MOUSE_LEFT_DOUBLE_CLICK` (`SelectionXlat.cpp:466,486-517`).
    /// Structures are not mass-selectable; ALT selects the same template map-wide.
    /// Shift snapshots the current group and re-adds it (`createNewGroup=false`).
    pub(in crate::cnc_game_engine) fn select_similar_units_for_double_click(
        &mut self,
        clicked_object_id: ObjectId,
        across_map: bool,
    ) {
        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        let previous = if shift_down {
            self.ui_selected_ids(self.current_player_id)
        } else {
            Vec::new()
        };
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return;
        };
        let player_team = frame.local_team();
        let (vw, vh) = self.tactical_viewport_size();
        let similar_units = frame.similar_unit_ids_for_double_click(
            clicked_object_id,
            player_team,
            across_map,
            self.view_matrix,
            self.projection_matrix,
            glam::Vec2::new(vw, vh),
        );
        let template_label = frame
            .objects
            .iter()
            .find(|o| o.id == clicked_object_id)
            .map(|o| o.template_name.clone())
            .unwrap_or_default();

        if similar_units.is_empty() {
            // C++ InGameUI::selectMatchingAcrossScreen/Map: empty seed → GUI:NothingSelected.
            self.game_hud.push_info_message("GUI:NothingSelected");
            self.ui_manager
                .game_hud_mut()
                .push_info_message("GUI:NothingSelected");
            return;
        }
        let selection = if shift_down {
            union_object_ids(similar_units, previous)
        } else {
            similar_units
        };
        // C++ selectSingleDrawableWithoutSound + MSG_CREATE_SELECTED_GROUP_NO_SOUND.
        self.host_set_selection_no_sound(self.current_player_id, selection);
        let msg = if across_map {
            "GUI:SelectedAcrossMap"
        } else {
            "GUI:SelectedAcrossScreen"
        };
        self.game_hud.push_info_message(msg);
        self.ui_manager.game_hud_mut().push_info_message(msg);
        info!(
            "Selected {} similar units ({})",
            self.selected_objects.len(),
            template_label
        );
    }

    /// C++ `isMassSelectable` + `isLocallyControlled` (`SelectionXlat.cpp:475-484`).
    fn presentation_double_click_consumes(&self, object_id: ObjectId) -> bool {
        self.last_presentation_frame.as_ref().is_some_and(|frame| {
            frame.objects.iter().any(|o| {
                o.id == object_id
                    && frame.is_owned_by_local(o)
                    && crate::presentation_frame::PresentationFrame::presentation_is_mass_selectable(
                        o,
                    )
            })
        })
    }

    pub(in crate::cnc_game_engine) fn handle_left_release(
        &mut self,
        origin: MouseInputOrigin,
        physical_lmb_gesture: bool,
    ) {
        if !self.lookat_input_enabled() {
            self.is_dragging = false;
            self.selection_start = None;
            self.selection_start_screen = None;
            self.left_click_release_behavior = LeftMouseReleaseBehavior::Selection;
            self.sync_host_selecting_flag();
            return;
        }
        self.is_dragging = false;
        self.sync_host_selecting_flag();
        let release_behavior = std::mem::replace(
            &mut self.left_click_release_behavior,
            LeftMouseReleaseBehavior::Selection,
        );
        let selection_start_screen = self.selection_start_screen.take();

        let Some(start) = self.selection_start.take() else {
            return;
        };

        let end = self.mouse_world_position;
        let selection_end_screen = glam::Vec2::new(self.mouse_position.0, self.mouse_position.1);
        let (drag_dx, drag_dy) = selection_start_screen
            .map(|start_screen| {
                (
                    selection_end_screen.x - start_screen.0,
                    selection_end_screen.y - start_screen.1,
                )
            })
            .unwrap_or((0.0, 0.0));
        let drag_distance_screen = (drag_dx * drag_dx + drag_dy * drag_dy).sqrt();

        // A map-target, structure placement, force attack, or double-click
        // selection already consumed the press edge.  C++'s higher-priority
        // translators suppress the corresponding release so it cannot also
        // clear selection or issue a second world action.
        if release_behavior == LeftMouseReleaseBehavior::Suppress {
            return;
        }

        if release_behavior == LeftMouseReleaseBehavior::ContextCommand
            && is_point_click_drag(drag_dx, drag_dy)
        {
            let had_selection = !self.ui_selected_ids(self.current_player_id).is_empty()
                || !self.selected_objects.is_empty();
            let issued = self.handle_left_context_click(origin, physical_lmb_gesture);
            self.interactive_playability.note_gameplay_order(
                matches!(origin, MouseInputOrigin::Physical) && !self.runtime_host_headless,
                had_selection && issued,
            );

            if !issued {
                // Classic C++ `SelectionInfo::contextCommandForNewSelection`
                // returns false for a drawable with no actionable context
                // command.  That exact fallthrough is what lets LMB replace
                // selection instead of moving selected units into a friendly
                // object under the cursor.
                if let Some(object_id) = self.find_object_at_cursor(false) {
                    let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
                    self.select_left_click_target(object_id, shift_down);
                }
            }
            return;
        }

        // C++ starts an area selection from a pixel delta and dispatches an
        // IRegion2D on release.  Terrain-ray distance changes with camera
        // pitch and must not decide whether a mouse drag was a click.
        if is_point_click_drag(drag_dx, drag_dy) {
            if let Some(template) = self.pending_structure_placement.clone() {
                if Self::is_wall_structure_template(&template) {
                    self.place_structure_from_ui(&template, end);
                    return;
                }
            }
        }

        if let Some(template) = self.pending_structure_placement.clone() {
            // C++ PlaceEventTranslator.cpp:155-160 / InGameUI handleBuildPlacements:
            // confirm re-projects the screen anchor, not the LMB-down world point.
            let start_world = selection_start_screen
                .and_then(|s| self.screen_to_terrain(s))
                .unwrap_or(start);
            let end_world = end;
            let dx = end_world.x - start_world.x;
            let dz = end_world.z - start_world.z;
            // C++ PlaceEventTranslator.cpp:307-323 — 5px Euclidean screen, not 1wu.
            if placement_screen_drag_exceeds_threshold(drag_dx, drag_dy)
                && (dx.abs() > f32::EPSILON || dz.abs() > f32::EPSILON)
            {
                let end_px = game_client::message_stream::game_message::ICoord2D::new(
                    self.mouse_position.0 as i32,
                    self.mouse_position.1 as i32,
                );
                game_client::helpers::TheInGameUI::set_placement_end(Some(end_px));
                self.apply_structure_placement_angle(dz.atan2(dx));
            }
            if Self::is_wall_structure_template(&template)
                && drag_distance_screen > DRAG_TOLERANCE_PX
            {
                self.place_wall_line_from_ui(&template, start_world, end_world);
            } else {
                self.place_structure_from_ui(&template, start_world);
            }
            return;
        }

        let shift_down = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        let ctrl_down = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        let alt_down = self.keys_pressed.contains(&Key::Named(NamedKey::Alt));

        // Movement within DragTolerance is a POINT CLICK, not a box.
        // C++ SelectionXlat.cpp:575-587 commits on MSG_MOUSE_LEFT_CLICK
        // at the *up* pixel. Do not box_select / host_set_selection([])
        // (hq-r5dmm). Do not force-select CanSelectDrawable rejects
        // (hq-587py).
        if is_point_click_drag(drag_dx, drag_dy) {
            if let Some(object_id) = self.find_object_at_cursor(false) {
                self.select_left_click_target(object_id, shift_down);
            } else if alternate_mouse_blank_click_deselects(
                self.use_alternate_mouse,
                shift_down,
                ctrl_down,
                alt_down,
            ) {
                // C++ `SelectionXlat.cpp:930-943` — issuing GUI click is
                // protected by armed command; the *next* blank LMB is
                // protected by the one-click prevent flag.
                if !self.host_consume_prevent_left_click_deselection() {
                    self.host_set_selection(self.current_player_id, Vec::new());
                }
            }
            return;
        }

        // Box path (drag > Mouse DragTolerance).
        let (garrison_target, boxed, add_to_group, current) = {
            let Some(frame) = self.last_presentation_frame.as_ref() else {
                return;
            };
            let current = self.ui_selected_ids(self.current_player_id);
            let (has_enemy, has_civilian, has_ally, has_local_structure, has_local_infantry) =
                current_selection_box_counts(frame, &current);
            let add_to_group = shift_down
                && !box_selection_must_replace(
                    has_enemy,
                    has_civilian,
                    has_ally,
                    has_local_structure,
                );
            let player_team = frame.local_team();
            let (vw, vh) = self.tactical_viewport_size();
            let viewport = glam::Vec2::new(vw, vh);
            let start_screen = selection_start_screen
                .map(|start_screen| glam::Vec2::new(start_screen.0, start_screen.1));
            let garrison_target = if infantry_garrison_context_takes_region(
                self.use_alternate_mouse,
                has_enemy || has_civilian || has_ally,
                has_local_infantry,
                start_screen
                    .map(|start_screen| {
                        frame
                            .garrisonable_building_ids_in_screen_rect(
                                self.view_matrix,
                                self.projection_matrix,
                                start_screen,
                                selection_end_screen,
                                viewport,
                            )
                            .len()
                    })
                    .unwrap_or(0),
            ) {
                start_screen.and_then(|start_screen| {
                    frame
                        .garrisonable_building_ids_in_screen_rect(
                            self.view_matrix,
                            self.projection_matrix,
                            start_screen,
                            selection_end_screen,
                            viewport,
                        )
                        .into_iter()
                        .next()
                })
            } else {
                None
            };
            let boxed: Vec<ObjectId> = start_screen
                .map(|start_screen| {
                    frame.box_select_unit_ids_in_screen_rect(
                        player_team,
                        self.view_matrix,
                        self.projection_matrix,
                        start_screen,
                        selection_end_screen,
                        viewport,
                    )
                })
                .unwrap_or_default()
                .into_iter()
                .filter(|&id| !self.host_object_id_blocked_by_opaque_hud(id))
                .collect();
            (garrison_target, boxed, add_to_group, current)
        };

        if let Some(target_id) = garrison_target {
            // C++ `SelectionInfo.cpp:203-205` / `SelectionXlat.cpp:601-610`:
            // leave the click as context garrison-enter instead of replacing
            // the infantry selection with an empty box.
            self.host_queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::Enter { target_id },
                player_id: self.current_player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: current,
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            self.host_process_commands_with_command_sound();
            return;
        }

        if boxed.is_empty() && !add_to_group {
            // C++ empty region `break` — classic empty drag keeps the army.
            return;
        }

        let boxed_any = !boxed.is_empty();
        let selection = if add_to_group {
            union_object_ids(current, boxed)
        } else {
            boxed
        };
        if selection.is_empty() {
            return;
        }
        self.host_set_selection(self.current_player_id, selection);
        // C++ CommandXlat.cpp:326-329 — VoiceSelect via pickAndPlay on
        // MSG_CREATE_SELECTED_GROUP. host_set_selection → select_objects already
        // queues that line. Do not layer an invented UnitSelect beep.
        self.last_control_group_select = None;
    }

    /// Issue a context-sensitive command through Alternate Mouse's RMB route.
    ///
    /// The actual command implementation is shared with the classic-layout
    /// LMB route below; only the physical button policy differs.
    pub(in crate::cnc_game_engine) fn handle_right_click(
        &mut self,
        origin: MouseInputOrigin,
        physical_rmb_gesture: bool,
    ) -> bool {
        if !self.lookat_input_enabled() {
            return false;
        }
        if self.host_note_right_double_click() && self.host_try_double_click_guard_command(true) {
            return true;
        }
        self.handle_context_click(origin, physical_rmb_gesture)
    }

    /// Issue a context-sensitive command through the classic-layout LMB
    /// route.  C++ `CommandXlat` treats this as the same logical context
    /// command as Alternate Mouse's RMB path.
    pub(in crate::cnc_game_engine) fn handle_left_context_click(
        &mut self,
        origin: MouseInputOrigin,
        physical_lmb_gesture: bool,
    ) -> bool {
        self.handle_context_click(origin, physical_lmb_gesture)
    }

    /// Issue one logical C++ `evaluateContextCommand` action.  Physical gather
    /// evidence is threaded through this exact command path, but only becomes
    /// tracked after GameLogic reports the Gather command actually accepted its
    /// carrier IDs.
    fn handle_context_click(
        &mut self,
        origin: MouseInputOrigin,
        physical_context_gesture: bool,
    ) -> bool {
        let mouse_pos = self.mouse_world_position;

        let mut selected = self.ui_selected_ids(self.current_player_id);
        if selected.is_empty() {
            selected = self.selected_objects.clone();
        }
        if selected.is_empty() {
            return false;
        }

        // C++ context-sensitive click residual via CommandSystem:
        // attack / gather / repair / enter / get-repaired / get-healed / move / attack-move.
        let target_object = self.find_object_at_cursor(true);
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
        // C++ waypoint mode outranks Ctrl force-attack. Do not fail-closed
        // the RMB when Alt/sticky waypoint is on.
        if ctrl && !alt && !self.host_selection_can_force_attack(mouse_pos, target_object) {
            return false;
        }

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
            // CommandSystem's `Right` arm represents its logical context
            // command, not the literal OS button.  C++ chooses the physical
            // button in CommandXlat before evaluating this shared action.
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
            let is_force_attack = matches!(
                command.command_type,
                crate::command_system::CommandType::ForceAttackObject { .. }
                    | crate::command_system::CommandType::ForceAttackGround { .. }
            );
            if is_force_attack && !self.host_selection_can_force_attack(mouse_pos, target_object) {
                // C++ evaluateForceAttack DO_COMMAND posts nothing when not possible.
                return false;
            }
            let crate_pickup = target_object
                .and_then(|id| self.presentation_target_hint(id))
                .is_some_and(|hint| hint.is_crate && !hint.is_salvage_crate);
            let target_had_no_context_action = target_object.is_some()
                && matches!(
                    &command.command_type,
                    crate::command_system::CommandType::MoveTo { .. }
                )
                && !crate_pickup
                && !matches!(
                    &command.command_type,
                    crate::command_system::CommandType::DoSalvage { .. }
                );
            if target_had_no_context_action {
                // C++ `evaluateContextCommand` returns MSG_INVALID rather
                // than a move when a drawable was under the cursor but had no
                // actionable relationship.  Classic LMB then falls back to
                // selection; Alternate RMB simply does nothing.
                // Crate clicks are the exception: MSG_DO_SALVAGE / move-to-crate.
                return false;
            }
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
            self.host_queue_and_process_context_click_command(
                command,
                origin,
                physical_context_gesture,
            );
            return true;
        }

        // A drawable with no accepted context action is deliberately not
        // converted to a move.  The only C++ fallback move is terrain (no
        // drawable), and that distinction is what lets classic LMB select a
        // friendly object after a failed context probe.
        if target_object.is_some() {
            return false;
        }

        // Fail-closed terrain fallback residual: move if the context path did
        // not synthesize a command. Factories set rally instead (CommandXlat.cpp:2180).
        if self.host_selection_can_set_rally() {
            self.host_queue_and_process_command_silent(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::SetRallyPoint {
                    location: mouse_pos,
                },
                player_id: self.current_player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: self.ui_selected_ids(self.current_player_id),
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            // C++ MSG_SET_RALLY_POINT is silent in pickAndPlay (no voice slot).
            return true;
        }
        if self.sticky_auto_attack {
            self.host_command_attack_move(self.current_player_id, mouse_pos);
        } else {
            self.host_command_move(self.current_player_id, mouse_pos);
        }
        // C++ VoiceMove from command_move / command_attack_move pickAndPlay.
        // Do not layer an invented UnitCommand beep.
        true
    }

    /// C++ SelectionXlat.cpp:1007-1023 sees a right-button click before
    /// CommandXlat.  An armed GUI command is cancelled without deselect;
    /// a pending place still deselects the builder (place source != 0)
    /// in both mouse layouts.  That click must never become a context
    /// command merely because Main owns direct OS input.
    pub(in crate::cnc_game_engine) fn cancel_world_mouse_targeting(&mut self) -> bool {
        if self.pending_map_command.take().is_some() {
            // C++ SelectionXlat.cpp:1007-1013 — RMB cancel is silent.
            self.clear_radius_cursor_overlays();
            return true;
        }
        if self.pending_structure_placement.is_some() {
            self.deselect_world_selection_from_right_click();
            self.cancel_structure_placement_from_ui();
            return true;
        }
        false
    }

    /// C++ SelectionXlat deselects on a short classic-layout RMB click when
    /// no GUI/build target mode owns that click.  RMB drag remains LookAtXlat
    /// scrolling and is filtered by the caller before this method runs.
    pub(in crate::cnc_game_engine) fn deselect_world_selection_from_right_click(&mut self) {
        if !self.ui_selected_ids(self.current_player_id).is_empty() {
            self.host_set_selection(self.current_player_id, Vec::new());
        }
    }

    /// Run the normal synchronous command authority path, then bind a physical
    /// Gather attempt to its executor-confirmed carrier subset. Injected,
    /// runtime-host, and AI commands have no physical attempt and are consumed
    /// without ever entering `physical_gather_carrier_ids`.
    fn host_queue_and_process_context_click_command(
        &mut self,
        command: crate::command_system::GameCommand,
        origin: MouseInputOrigin,
        physical_context_gesture: bool,
    ) {
        let physical_attempt = PhysicalGatherAttempt::from_context_click_command(
            &command,
            origin,
            physical_context_gesture,
        );
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
    pub(in crate::cnc_game_engine) fn host_drain_physical_gather_dropoffs(&mut self) {
        // Clear any non-input Gather acceptances that arose during simulation;
        // a physical context-click path consumes its own event synchronously above.
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
}
