// Mouse/keyboard input, cursors, and command/mouseover hints.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

impl InGameUI {
    pub fn handle_mouse_input(
        &mut self,
        mouse: &MouseState,
        keyboard: &KeyboardState,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let mouse_pos = Vec2::new(mouse.position().0, mouse.position().1);
        let left_button = mouse.button_state(MouseButton::Left);
        let right_button = mouse.button_state(MouseButton::Right);
        let add_to_selection = keyboard.is_ctrl_pressed() || keyboard.is_shift_pressed();

        // Check if clicking on minimap
        if self.minimap.contains_point(mouse_pos) {
            if left_button.just_pressed() {
                // Click on minimap - move camera
                let world_pos = self.minimap.minimap_to_world(mouse_pos);
                log::debug!("Minimap click at world position: {:?}", world_pos);
                with_tactical_view(|view| {
                    view.look_at(&Point3::new(world_pos.x, world_pos.y, 0.0));
                });
            }
            return Ok(());
        }

        if self.handle_pending_special_power(mouse_pos, left_button, right_button)? {
            return Ok(());
        }

        // Handle selection box
        match left_button {
            ButtonState::JustPressed => {
                // Start selection box
                self.selection_box.start_at(mouse_pos);

                // Check for double-click
                if self.selection_state.detect_double_click(mouse_pos) {
                    log::debug!("Double-click detected at {:?}", mouse_pos);
                    if let Some(clicked_id) = self.pick_object_at_screen(mouse_pos) {
                        self.select_similar_units(clicked_id, add_to_selection)?;
                    }
                }
            }
            ButtonState::Pressed => {
                // Update selection box
                if self.selection_box.active {
                    self.selection_box.update(mouse_pos);
                }
            }
            ButtonState::JustReleased
                // Finish selection box
                if self.selection_box.active => {
                    if self.selection_box.is_significant() {
                        // Perform box selection
                        let rect = self.selection_box.get_rect();
                        log::debug!("Selection box: {:?}", rect);
                        let selection_type = if add_to_selection {
                            SelectionType::Add
                        } else {
                            SelectionType::Replace
                        };
                        self.perform_box_selection(rect, selection_type)?;
                    } else {
                        // Single click selection
                        let selection_type = if keyboard.is_ctrl_pressed() {
                            SelectionType::Toggle
                        } else if keyboard.is_shift_pressed() {
                            SelectionType::Add
                        } else {
                            SelectionType::Replace
                        };
                        self.perform_click_selection(mouse_pos, selection_type)?;
                    }
                    self.selection_box.finish();
                }
            _ => {}
        }

        // Handle building placement
        if self.placement_preview.is_some() {
            if let Some(world_pos) = self.screen_to_world(mouse_pos) {
                if let Some(preview) = self.placement_preview.as_mut() {
                    preview.position = Vec3::new(world_pos.x, world_pos.y, world_pos.z);
                    let validator = FoundationValidator::new_strict();
                    preview.is_legal = validator
                        .validate_placement(
                            &world_pos,
                            &preview.template_name,
                            preview.rotation,
                            self.player_id as ObjectID,
                        )
                        .is_ok();
                    TheInGameUI::set_placement_angle(preview.rotation);
                }
            }

            if TheInGameUI::is_placement_anchored() {
                if let Some(preview) = self.placement_preview.as_ref() {
                    if let Some(template) = TheThingFactory::find_template(&preview.template_name) {
                        if template.is_kind_of(KindOf::Barrier) {
                            if let Some((start, _)) = TheInGameUI::get_placement_points() {
                                let current =
                                    MsgICoord2D::new(mouse_pos.x as i32, mouse_pos.y as i32);
                                let dx = (current.x - start.x) as f32;
                                let dy = (current.y - start.y) as f32;
                                if (dx * dx + dy * dy).sqrt() >= PLACEMENT_DRAG_DISTANCE {
                                    TheInGameUI::set_placement_end(Some(current));
                                }
                            }
                        }
                    }
                }
            }

            if mouse.button_state(MouseButton::Left).just_pressed() {
                let (is_legal, template_name, rotation) = match self.placement_preview.as_ref() {
                    Some(preview) => (
                        preview.is_legal,
                        preview.template_name.clone(),
                        preview.rotation,
                    ),
                    None => (false, String::new(), 0.0),
                };

                if is_legal {
                    let template = match TheThingFactory::find_template(&template_name) {
                        Some(template) => template,
                        None => return Ok(()),
                    };
                    let build_id = template.get_id();
                    let is_line_build = template.is_kind_of(KindOf::LineBuild);

                    if is_line_build {
                        let start = MsgICoord2D::new(mouse_pos.x as i32, mouse_pos.y as i32);
                        if !TheInGameUI::is_placement_anchored() {
                            TheInGameUI::set_placement_start(Some(start));
                            return Ok(());
                        }
                        TheInGameUI::set_placement_end(Some(start.clone()));
                        if let Some((start, end)) = TheInGameUI::get_placement_points() {
                            let dx = (end.x - start.x) as f32;
                            let dy = (end.y - start.y) as f32;
                            if (dx * dx + dy * dy).sqrt() < PLACEMENT_DRAG_DISTANCE {
                                return Ok(());
                            }
                            let Some(start_world) =
                                self.screen_to_world(Vec2::new(start.x as f32, start.y as f32))
                            else {
                                return Ok(());
                            };
                            let Some(end_world) =
                                self.screen_to_world(Vec2::new(end.x as f32, end.y as f32))
                            else {
                                return Ok(());
                            };
                            let _ = append_message_to_stream(GameMessageType::DozerConstructLine(
                                build_id,
                                MsgCoord3D::new(start_world.x, start_world.y, start_world.z),
                                MsgCoord3D::new(end_world.x, end_world.y, end_world.z),
                                rotation,
                            ));
                        }
                    } else if let Some(world_pos) = self.screen_to_world(mouse_pos) {
                        let _ = append_message_to_stream(GameMessageType::DozerConstruct(
                            build_id,
                            MsgCoord3D::new(world_pos.x, world_pos.y, world_pos.z),
                            rotation,
                        ));
                    }

                    TheInGameUI::place_build_available(None, None);
                    TheInGameUI::set_placement_start(None);
                    self.placement_preview = None;
                }
            }
        }

        Ok(())
    }

    fn handle_pending_special_power(
        &mut self,
        mouse_pos: Vec2,
        left_button: ButtonState,
        right_button: ButtonState,
    ) -> Result<bool> {
        let Some(pending) = TheInGameUI::get_pending_special_power() else {
            return Ok(false);
        };

        if right_button.just_pressed() {
            TheInGameUI::clear_pending_special_power();
            return Ok(true);
        }

        if !left_button.just_pressed() {
            return Ok(true);
        }

        let options = SpecialPowerCommandOption::from_bits_truncate(pending.options);
        let mut issued = false;

        if options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        ) {
            if let Some(target_id) = self.pick_object_at_screen(mouse_pos) {
                if self.is_valid_special_power_target(
                    pending.source_object_id,
                    pending.power_id,
                    target_id,
                    pending.options,
                ) {
                    let _ = append_message_to_stream(GameMessageType::DoSpecialPowerAtObject(
                        pending.power_id,
                        target_id,
                        pending.options,
                        pending.source_object_id,
                    ));
                    issued = true;
                }
            }
        }

        if !issued
            && options.intersects(
                SpecialPowerCommandOption::NEED_TARGET_POS
                    | SpecialPowerCommandOption::ATTACK_OBJECTS_POSITION,
            )
        {
            if let Some(world_pos) = self.screen_to_world(mouse_pos) {
                let _ = append_message_to_stream(GameMessageType::DoSpecialPowerAtLocation(
                    pending.power_id,
                    MsgCoord3D::new(world_pos.x, world_pos.y, world_pos.z),
                    0.0,
                    0,
                    pending.options,
                    pending.source_object_id,
                ));
                issued = true;
            }
        }

        if issued {
            let reselection_required = get_special_power_store()
                .and_then(|store| {
                    store
                        .find_special_power_template_by_id(pending.power_id)
                        .map(|template| template.is_shortcut_power())
                })
                .unwrap_or(false)
                && self.source_has_overridable_special_power_destination(pending.source_object_id);
            if reselection_required {
                let _ = append_message_to_stream(GameMessageType::CreateSelectedGroupNoSound(
                    true,
                    vec![pending.source_object_id],
                ));
            }
            TheInGameUI::clear_pending_special_power();
        } else if options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER
                | SpecialPowerCommandOption::NEED_TARGET_POS
                | SpecialPowerCommandOption::ATTACK_OBJECTS_POSITION,
        ) {
            let _ = append_message_to_stream(GameMessageType::DoInvalidHint);
        }

        Ok(true)
    }

    fn source_has_overridable_special_power_destination(&self, source_object_id: ObjectID) -> bool {
        // Wave 971: host empty dual-world → presentation special-power ready residual.
        // Full overridable-destination module walk requires dual-world; ready residual is
        // the host-safe approximation for pending SP destination override UI.
        if dual_world_registry_unavailable() {
            // Wave 1088/1091: SP override-destination source residual fail-closed on
            // destroyed/sold/disabled/UC/unselectable/masked (matches is_valid SP source).
            return self.presentation_unit_catalog.iter().any(|u| {
                u.object_id == source_object_id
                    && u.special_power_ready
                    && !u.destroyed
                    && !u.sold
                    && !u.disabled
                    && !u.under_construction
                    && !u.unselectable
                    && !u.masked
            });
        }

        if source_object_id == 0 {
            return false;
        }
        let Some(source_obj) = OBJECT_REGISTRY.get_object(source_object_id) else {
            return false;
        };
        let Ok(source_guard) = source_obj.read() else {
            return false;
        };
        if source_guard.is_effectively_dead() {
            return false;
        }

        for behavior_arc in source_guard.get_behavior_modules() {
            let Ok(mut behavior_lock) = behavior_arc.lock() else {
                continue;
            };
            let Some(update) = behavior_lock.get_special_power_update_interface() else {
                continue;
            };
            if update.does_special_power_have_overridable_destination_active()
                || update.does_special_power_have_overridable_destination()
            {
                return true;
            }
        }

        false
    }

    fn is_valid_special_power_target(
        &self,
        source_object_id: ObjectID,
        power_id: u32,
        target_id: ObjectID,
        options_bits: u32,
    ) -> bool {
        // Wave 971: host empty dual-world → presentation catalog residual.
        if dual_world_registry_unavailable() {
            let options = SpecialPowerCommandOption::from_bits_truncate(options_bits);
            let needs_object = options.intersects(
                SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                    | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                    | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                    | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
            );
            if !needs_object {
                return true;
            }
            let Some(source) = self
                .presentation_unit_catalog
                .iter()
                .find(|u| u.object_id == source_object_id)
            else {
                return false;
            };
            if !source.special_power_ready {
                return false;
            }
            // Wave 1038: source must be alive/usable residual.
            // Wave 1066: disabled source residual fail-closed (C++ disabled modules).
            // Wave 1067: under-construction source residual fail-closed.
            // Wave 1091: unselectable/masked source residual fail-closed.
            if source.destroyed
                || source.sold
                || source.disabled
                || source.under_construction
                || source.unselectable
                || source.masked
            {
                return false;
            }
            let Some(target) = self
                .presentation_unit_catalog
                .iter()
                .find(|u| u.object_id == target_id)
            else {
                return false;
            };
            // Wave 1038: C++ target legality residual — skip dead/sold/unselectable/masked/stealthed.
            // Wave 1071: disabled target residual fail-closed for SP object targeting.
            if target.destroyed
                || target.sold
                || target.unselectable
                || target.masked
                || target.disabled
            {
                return false;
            }
            // Enemy/neutral effectively stealthed targets fail closed (SelectionInfo parity).
            if target.effectively_stealthed {
                let local = crate::presentation_translator_residual::translator_local_team_name();
                if local.is_empty() || target.team_name != local {
                    return false;
                }
            }
            // Wave 1065: FOW fogged/black non-local SP targets fail-closed.
            {
                let local = crate::presentation_translator_residual::translator_local_team_name();
                let fogged = matches!(
                    target.shroud_status,
                    ObjectShroudStatus::PartialClear
                        | ObjectShroudStatus::Fogged
                        | ObjectShroudStatus::Shrouded
                );
                if fogged && (local.is_empty() || target.team_name != local) {
                    return false;
                }
            }
            // Relationship residual from team names (fail-open when Neutral options present).
            // Wave 1045: disguised targets present apparent team to non-allied casters
            // (C++ InGameUI disguise residual; allies of real owner still see true team).
            let target_team = if target.disguised && source.team_name != target.team_name {
                target
                    .disguise_as_team
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(target.team_name.as_str())
            } else {
                target.team_name.as_str()
            };
            let same_team = source.team_name == target_team;
            if options.contains(SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT)
                && !options.contains(SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT)
                && same_team
            {
                return false;
            }
            if options.contains(SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT)
                && !options.contains(SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT)
                && !same_team
            {
                return false;
            }
            // Wave 989: ALLOW_SURRENDER is off in retail ZH (no CAN_SURRENDER/PRISON KindOf).
            // Prisoner-required SP targeting residual is fail-closed on host empty dual-world.
            if options.contains(SpecialPowerCommandOption::NEED_TARGET_PRISONER) {
                let _ = target;
                return false;
            }
            let _ = power_id;
            return true;
        }

        let options = SpecialPowerCommandOption::from_bits_truncate(options_bits);
        let needs_object = options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        );
        if !needs_object {
            return true;
        }

        let target = OBJECT_REGISTRY.get_object(target_id);
        let Some(target) = target else {
            return false;
        };
        let Ok(target_guard) = target.read() else {
            return false;
        };
        if target_guard.is_effectively_dead() {
            return false;
        }

        let Some(source_obj) = OBJECT_REGISTRY.get_object(source_object_id) else {
            return false;
        };
        let Ok(source_guard) = source_obj.read() else {
            return false;
        };
        if source_guard.is_effectively_dead() {
            return false;
        }

        let Some(store) = get_special_power_store() else {
            return false;
        };
        let Some(template) = store.find_special_power_template_by_id(power_id) else {
            return false;
        };

        ActionManager::can_do_special_power_at_object(
            &source_guard,
            &target_guard,
            CommandSourceType::FromPlayer,
            template,
            options_bits,
            false,
        )
    }

    /// Perform box selection
    pub fn set_mouse_cursor(&mut self, cursor: MouseCursor) {
        self.current_cursor = cursor;
        if self.mouse_mode == MouseMode::GuiCommand
            && cursor != MouseCursor::Arrow
            && cursor != MouseCursor::Scroll
        {
            self.mouse_mode_cursor = cursor;
        }
    }

    pub fn get_mouse_cursor(&self) -> MouseCursor {
        self.current_cursor
    }

    pub fn set_mouse_mode(&mut self, mode: MouseMode) {
        self.mouse_mode = mode;
        if mode != MouseMode::GuiCommand {
            self.mouse_mode_cursor = MouseCursor::Arrow;
        }
    }

    pub fn get_mouse_mode(&self) -> MouseMode {
        self.mouse_mode
    }

    pub fn get_mouse_mode_cursor(&self) -> MouseCursor {
        self.mouse_mode_cursor
    }

    // ── Scroll / select state ────────────────────────────────────────────
    // C++: InGameUI::setScrolling() (InGameUI.cpp:2787)
    // C++: InGameUI::setSelecting() (InGameUI.cpp:2824)

    pub fn set_scrolling(&mut self, scrolling: bool) {
        if self.is_scrolling == scrolling {
            return;
        }
        if scrolling {
            self.set_mouse_cursor(MouseCursor::Scroll);
        } else {
            self.set_mouse_cursor(MouseCursor::Arrow);
        }
        self.is_scrolling = scrolling;
        if !scrolling {
            self.scroll_amount_x = 0.0;
            self.scroll_amount_y = 0.0;
        }
    }

    pub fn is_scrolling(&self) -> bool {
        self.is_scrolling
    }

    pub fn set_selecting(&mut self, selecting: bool) {
        if self.is_selecting == selecting {
            return;
        }
        self.is_selecting = selecting;
    }

    pub fn is_selecting(&self) -> bool {
        self.is_selecting
    }

    pub fn set_scroll_amount(&mut self, x: f32, y: f32) {
        self.scroll_amount_x = x;
        self.scroll_amount_y = y;
    }

    pub fn get_scroll_amount(&self) -> (f32, f32) {
        (self.scroll_amount_x, self.scroll_amount_y)
    }

    pub fn set_moused_over_drawable_id(&mut self, id: u32) {
        self.moused_over_drawable_id = id;
    }

    pub fn get_moused_over_drawable_id(&self) -> u32 {
        self.moused_over_drawable_id
    }

    pub fn set_recorder_playback_active(&mut self, active: bool) {
        self.recorder_playback_active = active;
    }

    pub fn set_look_at_mouse_moved_recently(&mut self, moved_recently: bool) {
        self.look_at_mouse_moved_recently = moved_recently;
    }

    // ── Hint system ──────────────────────────────────────────────────────
    // C++: InGameUI::createMoveHint() (InGameUI.cpp:2141)
    // C++: InGameUI::createAttackHint() (InGameUI.cpp:2176)
    // C++: InGameUI::expireHint() (InGameUI.cpp:3812)

    pub fn set_input_enabled_and_clear_modes(&mut self, enabled: bool) {
        if !enabled {
            self.set_selecting(false);
        }

        if !enabled {
            // C++: Clear all special modes when input is disabled (cinematic safety)
            self.force_attack_mode = false;
            self.force_move_to_mode = false;
            self.waypoint_mode = false;
            self.prefer_selection_mode = false;
            self.camera_rotating_left = false;
            self.camera_rotating_right = false;
            self.camera_zooming_in = false;
            self.camera_zooming_out = false;
        }
        self.enabled = enabled;
    }
    fn ignored_gui_slaver_id_from_presentation(&self, object_id: ObjectID) -> Option<ObjectID> {
        // Wave 1000: host empty dual-world → presentation catalog slaver residual.
        let entry = self
            .presentation_unit_catalog
            .iter()
            .find(|u| u.object_id == object_id)?;
        let ignored = entry
            .kind_names
            .iter()
            .any(|k| k == "IgnoredInGui" || k.eq_ignore_ascii_case("ignoredingui"));
        if !ignored {
            return None;
        }
        entry.slaver_object_id
    }

    fn ignored_gui_slaver_id_for_object(object: &Object) -> Option<ObjectID> {
        // Wave 273/1000/1007: empty dual-world cannot resolve Object modules;
        // peel slaver via presentation translator catalog (global residual).
        if dual_world_registry_unavailable() {
            let entry =
                crate::presentation_translator_residual::translator_catalog_entry(object.get_id())?;
            let ignored = entry
                .kind_names
                .iter()
                .any(|k| k == "IgnoredInGui" || k.eq_ignore_ascii_case("ignoredingui"));
            if !ignored {
                return None;
            }
            return entry.slaver_object_id;
        }

        if !object.is_kind_of(KindOf::IgnoredInGui) {
            return None;
        }

        for behavior in object.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
                continue;
            };
            let Some(slaved) = behavior.get_slaved_update_interface() else {
                continue;
            };
            let Some(slaver_id) = slaved.slaver_id() else {
                continue;
            };
            if OBJECT_REGISTRY.get_object(slaver_id).is_some() {
                return Some(slaver_id);
            }
        }
        None
    }

    fn mouseover_drawable_id_for_object(drawable_id: u32, object: &Object) -> u32 {
        Self::ignored_gui_slaver_id_for_object(object).unwrap_or(drawable_id)
    }

    fn mouseover_drawable_id_for_lookup(&self, drawable_id: u32, object: Option<&Object>) -> u32 {
        // Wave 1000: dual-world mouseover remaps IgnoredInGui via presentation catalog.
        if dual_world_registry_unavailable() {
            let Some(entry) = self
                .presentation_unit_catalog
                .iter()
                .find(|u| u.object_id == drawable_id)
            else {
                return Self::INVALID_DRAWABLE_ID;
            };
            // Wave 1088: mouseover lookup residual fail-closed on destroyed/sold/masked
            // before IgnoredInGui→slaver remap (matches create_mouseover_hint peels).
            if entry.destroyed || entry.sold || entry.masked {
                return Self::INVALID_DRAWABLE_ID;
            }
            let ignored = entry
                .kind_names
                .iter()
                .any(|k| k == "IgnoredInGui" || k.eq_ignore_ascii_case("ignoredingui"));
            if ignored {
                return entry.slaver_object_id.unwrap_or(drawable_id);
            }
            return drawable_id;
        }
        object
            .map(|object| Self::mouseover_drawable_id_for_object(drawable_id, object))
            .unwrap_or(Self::INVALID_DRAWABLE_ID)
    }

    fn disguised_player_index_for_object(object: &Object) -> Option<i32> {
        if !object.is_kind_of(KindOf::Disguiser) {
            return None;
        }

        for behavior in object.get_behavior_modules() {
            let Ok(behavior) = behavior.lock() else {
                continue;
            };
            if let Some(index) = behavior.get_disguised_player_index() {
                return Some(index);
            }
        }
        None
    }

    fn disguise_visible_player_index_for_object(
        object: &Object,
        local_player: Option<&Player>,
    ) -> Option<i32> {
        let disguised_index = Self::disguised_player_index_for_object(object)?;
        let local_player = local_player?;
        if !local_player.is_player_active() {
            return None;
        }

        let real_player = object.get_controlling_player()?;
        let real_player = real_player.read().ok()?;
        let local_team = local_player.get_default_team()?;
        let local_team = local_team.read().ok()?;
        if real_player.get_relationship_with_team(&local_team) == Relationship::Allies {
            return None;
        }

        Some(disguised_index)
    }

    fn command_hint_after_shroud_projection(
        hint_type: CommandHintType,
        target_shroud: Option<ObjectShroudStatus>,
    ) -> CommandHintType {
        if matches!(
            hint_type,
            CommandHintType::AttackObject | CommandHintType::AttackObjectAfterMoving
        ) && target_shroud == Some(ObjectShroudStatus::Shrouded)
        {
            CommandHintType::MoveTo
        } else {
            hint_type
        }
    }

    fn consume_double_click_attack_move_guard_hint(timer: &mut u32) -> bool {
        if *timer == 0 {
            return false;
        }

        *timer -= 1;
        *timer > 0
    }

    fn default_command_hint_blocked_by_source(source_locally_controlled: Option<bool>) -> bool {
        source_locally_controlled == Some(false)
    }

    fn move_to_cursor_for_context(
        draw_selectable: bool,
        target_locally_controlled: bool,
        target_is_mine: bool,
        source_is_local_structure: bool,
    ) -> MouseCursor {
        if !draw_selectable && source_is_local_structure {
            MouseCursor::GenericInvalid
        } else if draw_selectable && target_locally_controlled && !target_is_mine {
            MouseCursor::Selecting
        } else {
            MouseCursor::MoveTo
        }
    }

    fn mouseover_cursor_update_allowed(
        recorder_playback_active: bool,
        look_at_mouse_moved_recently: bool,
    ) -> bool {
        !recorder_playback_active || look_at_mouse_moved_recently
    }

    fn is_left_hud_window_name(name: &str) -> bool {
        name.eq_ignore_ascii_case("ControlBar.wnd:LeftHUD")
            || name.eq_ignore_ascii_case("LeftHUD")
            || name.ends_with(":LeftHUD")
    }

    fn window_chain_blocks_world_input(mut window: Option<Rc<RefCell<GameWindow>>>) -> bool {
        while let Some(current) = window {
            let guard = current.borrow();
            if Self::is_left_hud_window_name(guard.get_name()) {
                return false;
            }

            if !guard.get_status().contains(WindowStatus::SEE_THRU) {
                return true;
            }

            window = guard.get_parent();
        }

        false
    }

    fn cursor_is_under_opaque_window() -> bool {
        let (x, y) = with_mouse(|mouse| mouse.state().position());
        with_window_manager_ref(|manager| {
            Self::window_chain_blocks_world_input(
                manager.get_window_under_cursor(x as i32, y as i32, false),
            )
        })
    }

    fn command_hint_update_allowed(
        is_scrolling: bool,
        is_selecting: bool,
        recorder_playback_active: bool,
    ) -> bool {
        !(is_scrolling || is_selecting || recorder_playback_active)
    }

    fn selected_source_id_for_command_hint(&self) -> Option<u32> {
        let selected = self.get_selection();
        (selected.len() == 1).then(|| selected[0])
    }

    fn command_hint_source_context(&self, object_id: u32) -> Option<(bool, bool)> {
        // Wave 968: host empty dual-world → presentation catalog residual.
        if dual_world_registry_unavailable() {
            let entry = self
                .presentation_unit_catalog
                .iter()
                .find(|u| u.object_id == object_id)?;
            // Wave 1088/1091: command-hint source residual fail-closed on unusable
            // sources (dead/sold/masked/disabled/unselectable must not drive cursors).
            if entry.destroyed || entry.sold || entry.masked || entry.disabled || entry.unselectable
            {
                return None;
            }
            let local = !self.presentation_local_team_name.is_empty()
                && entry.team_name == self.presentation_local_team_name;
            let is_structure = entry
                .kind_names
                .iter()
                .any(|k| k == "Structure" || k.eq_ignore_ascii_case("structure"));
            return Some((local, is_structure));
        }

        OBJECT_REGISTRY.get_object(object_id).and_then(|obj| {
            obj.read().ok().map(|guard| {
                (
                    guard.is_locally_controlled(),
                    guard.is_kind_of(KindOf::Structure),
                )
            })
        })
    }

    fn cursor_for_name(name: &str) -> Option<MouseCursor> {
        match name.trim() {
            "ARROW" => Some(MouseCursor::Arrow),
            "SELECTING" => Some(MouseCursor::Selecting),
            "MOVETO" => Some(MouseCursor::MoveTo),
            "ATTACKMOVETO" => Some(MouseCursor::AttackMoveTo),
            "WAYPOINT" => Some(MouseCursor::Waypoint),
            "ATTACK_OBJECT" => Some(MouseCursor::AttackObject),
            "OUTRANGE" => Some(MouseCursor::OutOfRange),
            "FORCE_ATTACK_OBJECT" => Some(MouseCursor::ForceAttackObject),
            "FORCE_ATTACK_GROUND" => Some(MouseCursor::ForceAttackGround),
            "GET_REPAIRED" => Some(MouseCursor::GetRepaired),
            "DOCK" => Some(MouseCursor::Dock),
            "GET_HEALED" => Some(MouseCursor::GetHealed),
            "DO_REPAIR" => Some(MouseCursor::DoRepair),
            "RESUME_CONSTRUCTION" => Some(MouseCursor::ResumeConstruction),
            "ENTER_FRIENDLY" => Some(MouseCursor::EnterFriendly),
            "ENTER_AGGRESSIVELY" => Some(MouseCursor::EnterAggressively),
            "DEFECTOR" => Some(MouseCursor::Defector),
            "CAPTUREBUILDING" => Some(MouseCursor::CaptureBuilding),
            "HACK" => Some(MouseCursor::Hack),
            "GENERIC_INVALID" => Some(MouseCursor::GenericInvalid),
            "SET_RALLY_POINT" => Some(MouseCursor::SetRallyPoint),
            "BUILD_PLACEMENT" => Some(MouseCursor::BuildPlacement),
            "INVALID_BUILD_PLACEMENT" => Some(MouseCursor::InvalidBuildPlacement),
            "PARTICLE_UPLINK_CANNON" => Some(MouseCursor::ParticleUplinkCannon),
            "CROSS" => Some(MouseCursor::Cross),
            _ => None,
        }
    }

    fn pending_command_uses_context_cursor_behavior(pending: &PendingCommand) -> bool {
        (pending.options & CMD_CONTEXTMODE_COMMAND) != 0
            || matches!(
                pending.command_type,
                CommandType::SpecialPower
                    | CommandType::DoSpecialPowerAtLocation
                    | CommandType::DoSpecialPowerAtObject
            )
    }

    fn pending_gui_command_cursor(
        pending: &PendingCommand,
        hint_type: CommandHintType,
    ) -> MouseCursor {
        let cursor_name = if Self::pending_command_uses_context_cursor_behavior(pending)
            && hint_type != CommandHintType::ValidGuiCommand
        {
            &pending.invalid_cursor_name
        } else {
            &pending.cursor_name
        };

        Self::cursor_for_name(cursor_name).unwrap_or(MouseCursor::Cross)
    }

    /// Port of C++ InGameUI::createCommandHint() (InGameUI.cpp:2500-2772).
    ///
    /// Handles 25+ message types across 3 mouse modes to set the appropriate
    /// mouse cursor and radius cursor as a preview of what command would be
    /// issued if the player clicked.

    /// Wave 969: hover target residual (selectable, local-controlled, mine).
    fn hover_target_command_context(&self) -> (bool, bool, bool) {
        let id = self.moused_over_drawable_id;
        if id == Self::INVALID_DRAWABLE_ID {
            return (false, false, false);
        }
        if dual_world_registry_unavailable() {
            let Some(entry) = self
                .presentation_unit_catalog
                .iter()
                .find(|u| u.object_id == id)
            else {
                return (false, false, false);
            };
            // Wave 1087: command-hint hover residual fail-closed on unusable /
            // non-local FOW/stealth (matches create_mouseover_hint peels).
            if entry.destroyed || entry.sold || entry.unselectable || entry.masked {
                return (false, false, false);
            }
            let local = !self.presentation_local_team_name.is_empty()
                && entry.team_name == self.presentation_local_team_name;
            if entry.effectively_stealthed && !local {
                return (false, false, false);
            }
            let fogged = matches!(
                entry.shroud_status,
                ObjectShroudStatus::PartialClear
                    | ObjectShroudStatus::Fogged
                    | ObjectShroudStatus::Shrouded
            );
            if fogged && !local {
                return (false, false, false);
            }
            let is_mine = entry
                .kind_names
                .iter()
                .any(|k| k == "Mine" || k.eq_ignore_ascii_case("mine"));
            return (entry.selectable, local, is_mine);
        }
        if let Some(obj) = OBJECT_REGISTRY.get_object(id) {
            if let Ok(guard) = obj.read() {
                return (
                    guard.is_selectable(),
                    guard.is_locally_controlled(),
                    guard.is_kind_of(KindOf::Mine),
                );
            }
        }
        (false, false, false)
    }

    /// Wave 969: attack-hint shroud residual (None when host dual-world empty).
    fn hover_target_shroud_for_command_hint(&self) -> Option<ObjectShroudStatus> {
        let id = self.moused_over_drawable_id;
        if id == Self::INVALID_DRAWABLE_ID {
            return None;
        }
        // Wave 981: host empty dual-world → presentation catalog FOW residual.
        if dual_world_registry_unavailable() {
            let entry = self
                .presentation_unit_catalog
                .iter()
                .find(|e| e.object_id == id)?;
            // Wave 1090: command-hint shroud residual fail-closed on unusable
            // hover targets (matches hover_target_command_context peels).
            if entry.destroyed || entry.sold || entry.masked || entry.unselectable {
                return None;
            }
            return Some(entry.shroud_status);
        }
        OBJECT_REGISTRY.get_object(id).and_then(|obj| {
            obj.read()
                .ok()
                .map(|guard| guard.get_shrouded_status(self.player_id as i32))
        })
    }

    pub fn create_command_hint(&mut self, hint_type: CommandHintType) {
        // Wave 969: host empty dual-world uses presentation catalog hover residual
        // (see hover_target_command_context / hover_target_shroud_for_command_hint).

        // Early exit: no cursor hints while scrolling, selecting, or in playback
        if !Self::command_hint_update_allowed(
            self.is_scrolling,
            self.is_selecting,
            self.recorder_playback_active,
        ) {
            return;
        }

        // C++: setRadiusCursorNone() at the top of createCommandHint
        self.clear_radius_cursor();

        // C++: doubleClickAttackMove guard timer — suppresses hints for a few frames
        // after a double-click attack-move to prevent spurious cursor flicker.
        if Self::consume_double_click_attack_move_guard_hint(
            &mut self.double_click_attack_move_guard_timer,
        ) {
            self.set_mouse_cursor(MouseCursor::ForceAttackGround);
            self.set_radius_cursor(
                RadiusCursorType::GuardArea,
                Coord3D::new(0.0, 0.0, 0.0),
                1.0,
            );
            return;
        }

        let target_shroud = match hint_type {
            CommandHintType::AttackObject | CommandHintType::AttackObjectAfterMoving => {
                self.hover_target_shroud_for_command_hint()
            }
            _ => None,
        };
        let hint_type = Self::command_hint_after_shroud_projection(hint_type, target_shroud);

        let under_window = Self::cursor_is_under_opaque_window();

        match self.mouse_mode {
            MouseMode::Default => {
                // C++: InGameUI.cpp:2585-2688
                // This section only applies when there is no specific cursor mode happening.
                // C++: if (underWindow || (srcObj && !srcObj->isLocallyControlled()))
                let source_context = self
                    .selected_source_id_for_command_hint()
                    .and_then(|id| self.command_hint_source_context(id));
                if under_window
                    || Self::default_command_hint_blocked_by_source(
                        source_context.map(|(locally_controlled, _)| locally_controlled),
                    )
                {
                    self.set_mouse_cursor(MouseCursor::Arrow);
                    return;
                }

                match hint_type {
                    CommandHintType::MoveTo => {
                        // C++: MSG_DO_MOVETO_HINT (InGameUI.cpp:2595-2608)
                        // Wave 969: hover residual via presentation catalog when dual-world empty.
                        let source_is_local_structure = source_context == Some((true, true));
                        let (selectable, local, is_mine) = self.hover_target_command_context();
                        self.set_mouse_cursor(Self::move_to_cursor_for_context(
                            selectable,
                            local,
                            is_mine,
                            source_is_local_structure,
                        ));
                    }
                    CommandHintType::AttackMoveTo => {
                        // C++: MSG_DO_ATTACKMOVETO_HINT (InGameUI.cpp:2610-2615)
                        // Wave 969: hover residual via presentation catalog when dual-world empty.
                        let (selectable, local, _) = self.hover_target_command_context();
                        if selectable && local {
                            self.set_mouse_cursor(MouseCursor::Selecting);
                        } else {
                            self.set_mouse_cursor(MouseCursor::AttackMoveTo);
                        }
                    }
                    CommandHintType::AddWaypoint => {
                        // C++: MSG_ADD_WAYPOINT_HINT (InGameUI.cpp:2616-2618)
                        self.set_mouse_cursor(MouseCursor::Waypoint);
                    }
                    CommandHintType::AttackObject => {
                        // C++: MSG_DO_ATTACK_OBJECT_HINT (InGameUI.cpp:2619-2621)
                        self.set_mouse_cursor(MouseCursor::AttackObject);
                    }
                    CommandHintType::AttackObjectAfterMoving => {
                        // C++: MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT (InGameUI.cpp:2622-2624)
                        self.set_mouse_cursor(MouseCursor::OutOfRange);
                    }
                    CommandHintType::ForceAttackObject => {
                        // C++: MSG_DO_FORCE_ATTACK_OBJECT_HINT (InGameUI.cpp:2625-2627)
                        self.set_mouse_cursor(MouseCursor::ForceAttackObject);
                    }
                    CommandHintType::ForceAttackGround => {
                        // C++: MSG_DO_FORCE_ATTACK_GROUND_HINT (InGameUI.cpp:2628-2630)
                        self.set_mouse_cursor(MouseCursor::ForceAttackGround);
                    }
                    CommandHintType::GetRepaired => {
                        // C++: MSG_GET_REPAIRED_HINT (InGameUI.cpp:2631-2633)
                        self.set_mouse_cursor(MouseCursor::GetRepaired);
                    }
                    CommandHintType::Dock => {
                        // C++: MSG_DOCK_HINT (InGameUI.cpp:2634-2636)
                        self.set_mouse_cursor(MouseCursor::Dock);
                    }
                    CommandHintType::GetHealed => {
                        // C++: MSG_GET_HEALED_HINT (InGameUI.cpp:2637-2639)
                        self.set_mouse_cursor(MouseCursor::GetHealed);
                    }
                    CommandHintType::DoRepair => {
                        // C++: MSG_DO_REPAIR_HINT (InGameUI.cpp:2640-2642)
                        self.set_mouse_cursor(MouseCursor::DoRepair);
                    }
                    CommandHintType::ResumeConstruction => {
                        // C++: MSG_RESUME_CONSTRUCTION_HINT (InGameUI.cpp:2643-2645)
                        self.set_mouse_cursor(MouseCursor::ResumeConstruction);
                    }
                    CommandHintType::Enter => {
                        // C++: MSG_ENTER_HINT (InGameUI.cpp:2646-2648)
                        self.set_mouse_cursor(MouseCursor::EnterFriendly);
                    }
                    CommandHintType::ConvertToCarbomb
                    | CommandHintType::Hijack
                    | CommandHintType::Sabotage => {
                        // C++: MSG_CONVERT_TO_CARBOMB_HINT, MSG_HIJACK_HINT,
                        //       MSG_SABOTAGE_HINT (InGameUI.cpp:2649-2653)
                        self.set_mouse_cursor(MouseCursor::EnterAggressively);
                    }
                    CommandHintType::Defector => {
                        // C++: MSG_DEFECTOR_HINT (InGameUI.cpp:2654-2656)
                        self.set_mouse_cursor(MouseCursor::Defector);
                    }
                    CommandHintType::PickUpPrisoner => {
                        // C++: MSG_PICK_UP_PRISONER_HINT (InGameUI.cpp:2658-2661)
                        // ALLOW_SURRENDER conditional — not in retail Zero Hour
                        // Keep for parity if the build supports it
                        self.set_mouse_cursor(MouseCursor::Defector); // Closest available cursor
                    }
                    CommandHintType::CaptureBuilding => {
                        // C++: MSG_CAPTUREBUILDING_HINT (InGameUI.cpp:2662-2664)
                        self.set_mouse_cursor(MouseCursor::CaptureBuilding);
                    }
                    CommandHintType::Hack => {
                        // C++: MSG_HACK_HINT (InGameUI.cpp:2665-2667)
                        self.set_mouse_cursor(MouseCursor::Hack);
                    }
                    CommandHintType::ImpossibleAttack => {
                        // C++: MSG_IMPOSSIBLE_ATTACK_HINT (InGameUI.cpp:2668-2670)
                        self.set_mouse_cursor(MouseCursor::GenericInvalid);
                    }
                    CommandHintType::SetRallyPoint => {
                        // C++: MSG_SET_RALLY_POINT_HINT (InGameUI.cpp:2671-2676)
                        // Wave 969: hover residual via presentation catalog when dual-world empty.
                        let (selectable, local, _) = self.hover_target_command_context();
                        if selectable && local {
                            self.set_mouse_cursor(MouseCursor::Selecting);
                        } else {
                            self.set_mouse_cursor(MouseCursor::SetRallyPoint);
                        }
                    }
                    CommandHintType::SpecialPowerOverrideDestination => {
                        // C++: MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT (InGameUI.cpp:2677-2679)
                        self.set_mouse_cursor(MouseCursor::ParticleUplinkCannon);
                    }
                    CommandHintType::DoSalvage => {
                        // C++: MSG_DO_SALVAGE_HINT (InGameUI.cpp:2680-2682)
                        self.set_mouse_cursor(MouseCursor::MoveTo);
                    }
                    CommandHintType::Invalid => {
                        // C++: MSG_DO_INVALID_HINT (InGameUI.cpp:2683-2685)
                        self.set_mouse_cursor(MouseCursor::GenericInvalid);
                    }
                    CommandHintType::ValidGuiCommand | CommandHintType::InvalidGuiCommand => {
                        // These are handled in MOUSEMODE_GUI_COMMAND, not here.
                        // Fall through to no-op in Default mode.
                    }
                }
            }
            MouseMode::BuildPlace => {
                // C++: InGameUI.cpp:2689-2708
                if under_window {
                    self.set_mouse_cursor(MouseCursor::Arrow);
                    return;
                }

                match hint_type {
                    CommandHintType::MoveTo
                    | CommandHintType::AttackMoveTo
                    | CommandHintType::AddWaypoint => {
                        // C++: MSG_DO_MOVETO_HINT, MSG_DO_ATTACKMOVETO_HINT, MSG_ADD_WAYPOINT
                        // C++: setMouseCursor(Mouse::BUILD_PLACEMENT) (InGameUI.cpp:2701)
                        self.set_mouse_cursor(MouseCursor::BuildPlacement);
                    }
                    CommandHintType::AttackObject | CommandHintType::AttackObjectAfterMoving => {
                        // C++: MSG_DO_ATTACK_OBJECT_HINT, MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT
                        // C++: setMouseCursor(Mouse::INVALID_BUILD_PLACEMENT) (InGameUI.cpp:2705)
                        self.set_mouse_cursor(MouseCursor::InvalidBuildPlacement);
                    }
                    _ => {
                        // Other hint types in build-place mode default to build cursor
                        self.set_mouse_cursor(MouseCursor::BuildPlacement);
                    }
                }
            }
            MouseMode::GuiCommand => {
                // C++: InGameUI.cpp:2710-2769
                if under_window {
                    self.set_mouse_cursor(MouseCursor::Arrow);
                    self.clear_radius_cursor();
                    return;
                }

                if let Some(pending) = TheInGameUI::get_pending_command() {
                    self.set_mouse_cursor(Self::pending_gui_command_cursor(&pending, hint_type));
                    if let Some(cursor_type) =
                        RadiusCursorType::from_name(&pending.radius_cursor_type)
                    {
                        if cursor_type != RadiusCursorType::None {
                            TheInGameUI::set_radius_cursor_active_with_type(
                                &pending.radius_cursor_type,
                            );
                            self.set_radius_cursor(
                                cursor_type,
                                Coord3D::new(0.0, 0.0, 0.0),
                                1.0,
                            );
                        }
                    }
                } else {
                    self.set_mouse_cursor(self.mouse_mode_cursor);
                }
            }
        }
    }

    /// Port of C++ InGameUI::createMouseoverHint() (InGameUI.cpp:2217-2494).
    ///
    /// Handles mouse-over drawable/location hints. Updates the moused-over
    /// drawable ID and sets the cursor to SELECTING for selectable+controlled
    /// drawables, or ARROW otherwise.
    /// C++ `InGameUI::createMouseoverHint` — player-name suffix, disguise,
    /// warehouse value, prop suppression, and garrison color.

    /// Wave 968: host mouseover cursor/tooltip residual without OBJECT_REGISTRY.
    fn create_mouseover_hint_from_presentation(
        &mut self,
        drawable_id: Option<u32>,
        is_location_hint: bool,
    ) {
        if self.is_scrolling || self.is_selecting {
            return;
        }
        if Self::cursor_is_under_opaque_window() {
            self.set_mouse_cursor(MouseCursor::Arrow);
            return;
        }

        let old_id = self.moused_over_drawable_id;
        if is_location_hint {
            self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
        } else if let Some(draw_id) = drawable_id {
            with_mouse(|m| m.set_cursor_tooltip(String::new(), None, None, None));
            if draw_id == Self::INVALID_DRAWABLE_ID {
                self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
            } else if let Some(entry) = self
                .presentation_unit_catalog
                .iter()
                .find(|u| u.object_id == draw_id)
                .cloned()
            {
                // Wave 1039: dead/sold/unselectable/masked/stealthed hover residual.
                if entry.destroyed || entry.sold || entry.unselectable || entry.masked {
                    self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
                    self.set_mouse_cursor(MouseCursor::Arrow);
                    return;
                }
                if entry.effectively_stealthed {
                    let local =
                        crate::presentation_translator_residual::translator_local_team_name();
                    if local.is_empty() || entry.team_name != local {
                        self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
                        self.set_mouse_cursor(MouseCursor::Arrow);
                        return;
                    }
                }
                // Wave 1065: FOW fogged/black non-local hover residual fail-closed.
                {
                    let local =
                        crate::presentation_translator_residual::translator_local_team_name();
                    let fogged = matches!(
                        entry.shroud_status,
                        ObjectShroudStatus::PartialClear
                            | ObjectShroudStatus::Fogged
                            | ObjectShroudStatus::Shrouded
                    );
                    if fogged && (local.is_empty() || entry.team_name != local) {
                        self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
                        self.set_mouse_cursor(MouseCursor::Arrow);
                        return;
                    }
                }
                // Wave 982: IgnoredInGui → slaver mouseover residual (C++ parity).
                let ignored = entry
                    .kind_names
                    .iter()
                    .any(|k| k == "IgnoredInGui" || k.eq_ignore_ascii_case("ignoredingui"));
                self.moused_over_drawable_id = if ignored {
                    entry.slaver_object_id.unwrap_or(draw_id)
                } else {
                    draw_id
                };
                // Tooltip still from the hovered entry (drone) unless remapped to slaver catalog.
                let tip_entry = if ignored {
                    self.presentation_unit_catalog
                        .iter()
                        .find(|u| Some(u.object_id) == entry.slaver_object_id)
                        .cloned()
                        .unwrap_or_else(|| entry.clone())
                } else {
                    entry.clone()
                };
                // Wave 1085: slaver/tip residual fail-closed on unusable/FOW/stealth non-local.
                {
                    let local =
                        crate::presentation_translator_residual::translator_local_team_name();
                    let fogged = matches!(
                        tip_entry.shroud_status,
                        ObjectShroudStatus::PartialClear
                            | ObjectShroudStatus::Fogged
                            | ObjectShroudStatus::Shrouded
                    );
                    let tip_local = !local.is_empty() && tip_entry.team_name == local;
                    if tip_entry.destroyed
                        || tip_entry.sold
                        || tip_entry.unselectable
                        || tip_entry.masked
                        || (tip_entry.effectively_stealthed && !tip_local)
                        || (fogged && !tip_local)
                    {
                        self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
                        self.set_mouse_cursor(MouseCursor::Arrow);
                        return;
                    }
                }
                // Wave 1042: C++ InGameUI disguise tooltip residual — non-allied
                // viewers see disguise template name.
                let local = crate::presentation_translator_residual::translator_local_team_name();
                let tip_name =
                    if tip_entry.disguised && (local.is_empty() || tip_entry.team_name != local) {
                        tip_entry
                            .disguise_as_template
                            .clone()
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| tip_entry.template_name.clone())
                    } else {
                        tip_entry.template_name.clone()
                    };
                if let Some(mut tooltip) =
                    Self::mouseover_tooltip_for_templates(&tip_name, &tip_entry.template_name)
                {
                    if let Some(player) = player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.find_player_by_name(&tip_entry.team_name))
                    {
                        if let Ok(player_guard) = player.read() {
                            tooltip = Self::mouseover_tooltip_with_player_suffix(
                                &tooltip,
                                &player_guard,
                                Self::mouseover_tooltip_is_multiplayer(),
                            );
                            let color = [
                                player_guard.get_player_color().r,
                                player_guard.get_player_color().g,
                                player_guard.get_player_color().b,
                                player_guard.get_player_color().a,
                            ];
                            with_mouse(|m| {
                                m.set_cursor_tooltip(tooltip, Some(-1), Some(color), None);
                            });
                        }
                    } else {
                        with_mouse(|m| {
                            m.set_cursor_tooltip(tooltip, Some(-1), None, None);
                        });
                    }
                }
            } else {
                self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
            }
        } else {
            self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
        }

        if old_id != self.moused_over_drawable_id {
            with_mouse(|m| m.reset_tooltip_delay());
        }

        if self.mouse_mode == MouseMode::Default
            && !self.is_scrolling
            && !self.is_selecting
            && self.get_select_count() == 0
            && Self::mouseover_cursor_update_allowed(
                self.recorder_playback_active,
                self.look_at_mouse_moved_recently,
            )
        {
            if self.moused_over_drawable_id != Self::INVALID_DRAWABLE_ID {
                let can_select = self
                    .presentation_unit_catalog
                    .iter()
                    .find(|u| u.object_id == self.moused_over_drawable_id)
                    .map(|u| {
                        u.selectable
                            && !self.presentation_local_team_name.is_empty()
                            && u.team_name == self.presentation_local_team_name
                    })
                    .unwrap_or(false);
                if can_select {
                    self.set_mouse_cursor(MouseCursor::Selecting);
                } else {
                    self.set_mouse_cursor(MouseCursor::Arrow);
                }
            } else {
                self.set_mouse_cursor(MouseCursor::Arrow);
            }
        } else if self.mouse_mode != MouseMode::Default
            && self.mouse_mode != MouseMode::BuildPlace
        {
            self.set_mouse_cursor(self.mouse_mode_cursor);
        }
    }

    pub fn create_mouseover_hint(&mut self, drawable_id: Option<u32>, is_location_hint: bool) {
        // Wave 968: host empty dual-world → presentation catalog residual path.
        if dual_world_registry_unavailable() {
            self.create_mouseover_hint_from_presentation(drawable_id, is_location_hint);
            return;
        }

        // Phase 1: Early exit guards
        // C++: if (m_isScrolling || m_isSelecting) return;
        if self.is_scrolling || self.is_selecting {
            return;
        }

        if Self::cursor_is_under_opaque_window() {
            self.set_mouse_cursor(MouseCursor::Arrow);
            return;
        }

        // Phase 2: Update moused_over_drawable_id
        // C++: InGameUI.cpp:2254-2454 — extensive tooltip/drawable logic
        let old_id = self.moused_over_drawable_id;
        if is_location_hint {
            // C++: else branch (MSG_MOUSEOVER_LOCATION_HINT) — line 2451-2454
            self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
        } else if let Some(draw_id) = drawable_id {
            with_mouse(|m| m.set_cursor_tooltip(String::new(), None, None, None));
            if draw_id == Self::INVALID_DRAWABLE_ID {
                self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
            } else {
                self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
                if let Some(obj) = OBJECT_REGISTRY.get_object(draw_id) {
                    if let Ok(guard) = obj.read() {
                        self.moused_over_drawable_id =
                            self.mouseover_drawable_id_for_lookup(draw_id, Some(&guard));

                        // C++: TheMouse->setCursorTooltip(displayName, -1, playerColor, widthMult)
                        // Deferred C++ behavior: multiplayer player suffix.
                        let visible = Self::mouseover_tooltip_visible_for_shroud(
                            guard.get_shrouded_status(self.player_id as i32),
                        );
                        if visible {
                            let template_name = Self::mouseover_tooltip_template_for_object(&guard);
                            let real_template = guard.get_template_name().to_string();
                            if let Some(player) = Self::mouseover_tooltip_player_for_object(&guard)
                            {
                                if let Some(mut display_name) =
                                    Self::mouseover_tooltip_for_templates(
                                        &template_name,
                                        &real_template,
                                    )
                                {
                                    if let Some(boxes) =
                                        Self::supply_warehouse_boxes_for_object(&guard)
                                    {
                                        let base_value = global_data::read_safe()
                                            .map(|data| data.base_value_per_supply_box)
                                            .unwrap_or(100);
                                        display_name.push_str(
                                            &Self::supply_warehouse_tooltip_feedback(
                                                boxes, base_value,
                                            ),
                                        );
                                    }
                                    if let Ok(player_guard) = player.read() {
                                        display_name = Self::mouseover_tooltip_with_player_suffix(
                                            &display_name,
                                            &player_guard,
                                            Self::mouseover_tooltip_is_multiplayer(),
                                        );
                                        let indicator =
                                            Self::mouseover_tooltip_color_for_object(&guard);
                                        with_mouse(|m| {
                                            m.set_cursor_tooltip(
                                                display_name,
                                                Some(-1),
                                                Some(indicator),
                                                None,
                                            );
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
        }

        // C++: TheMouse->resetTooltipDelay() when ID changes
        if old_id != self.moused_over_drawable_id {
            with_mouse(|m| m.reset_tooltip_delay());
        }

        // Phase 3: Cursor assignment
        // C++: InGameUI.cpp:2462-2493
        if self.mouse_mode == MouseMode::Default
            && !self.is_scrolling
            && !self.is_selecting
            && self.get_select_count() == 0
            && Self::mouseover_cursor_update_allowed(
                self.recorder_playback_active,
                self.look_at_mouse_moved_recently,
            )
        {
            if self.moused_over_drawable_id != Self::INVALID_DRAWABLE_ID {
                // C++: CanSelectDrawable(draw, FALSE) and obj->isLocallyControlled()
                let can_select = match OBJECT_REGISTRY.get_object(self.moused_over_drawable_id) {
                    Some(obj_ref) => obj_ref
                        .read()
                        .map(|g| g.is_selectable() && g.is_locally_controlled())
                        .unwrap_or(false),
                    None => false,
                };
                if can_select {
                    self.set_mouse_cursor(MouseCursor::Selecting);
                } else {
                    self.set_mouse_cursor(MouseCursor::Arrow);
                }
            } else {
                self.set_mouse_cursor(MouseCursor::Arrow);
            }
        } else if self.mouse_mode != MouseMode::Default
            && self.mouse_mode != MouseMode::BuildPlace
        {
            self.set_mouse_cursor(self.mouse_mode_cursor);
        }
    }
}
