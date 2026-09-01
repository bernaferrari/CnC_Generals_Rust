// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

impl ControlBar {
    /// Send a real Control Bar command to Main's authoritative world when that
    /// world owns this UI.  The legacy queue remains untouched in this mode;
    /// it is intentionally still used when the bridge is disabled.
    fn publish_host_command_if_enabled(
        &self,
        button: &CommandButton,
        source: CommandSourceType,
        context: &ControlBarContext,
    ) -> bool {
        if !super::host_control_bar_bridge_enabled() {
            return false;
        }

        let special_power_id = if button.special_power.is_empty() {
            None
        } else {
            self.resolve_logic_button(button).and_then(|logic_button| {
                logic_button
                    .get_special_power_template()
                    .map(|template| template.get_id())
            })
        };
        // Both C++ FIRE_WEAPON and SWITCH_WEAPON carry a concrete
        // WeaponSlotType in the CommandButton INI.  Do not fall back to a
        // default slot: Main must reject a missing/invalid payload rather
        // than changing a different weapon set.
        let weapon_slot = if matches!(
            button.command_type,
            CommandType::FireWeapon | CommandType::SwitchWeapons
        ) {
            Some(button.weapon_slot_number())
        } else {
            None
        };
        let request = super::host_request_from_button_with_weapon_slot(
            button,
            context,
            source,
            special_power_id,
            weapon_slot,
            Self::command_needs_target(button.options),
        );
        super::publish_host_control_bar_request(request)
    }

    fn command_needs_target(options: u32) -> bool {
        let mut mask = CommandOption::NeedTargetEnemyObject as u32
            | CommandOption::NeedTargetNeutralObject as u32
            | CommandOption::NeedTargetAllyObject as u32
            | CommandOption::NeedTargetPos as u32
            | CommandOption::ContextmodeCommand as u32;
        #[cfg(feature = "allow_surrender")]
        {
            mask |= CommandOption::NeedTargetPrisoner as u32;
        }
        options & mask != 0
    }

    fn enter_targeting_mode(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source_id = context.selected_objects.first().copied().unwrap_or(0);
        TheInGameUI::place_build_available(None, None);
        TheInGameUI::clear_pending_special_power();
        TheInGameUI::set_force_attack_mode(false);
        TheInGameUI::set_force_move_to_mode(false);
        TheInGameUI::set_prefer_selection_mode(false);

        if (button.options & CommandOption::UsesMineClearingWeaponSet as u32) != 0 {
            if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
                stream.append_message(GameMessageType::SetMineClearingDetail(0));
            }
        }

        if button.command_type == CommandType::DozerConstruct && !button.object.is_empty() {
            TheInGameUI::place_build_available(Some(button.object.clone()), Some(source_id));
        }

        if !button.special_power.is_empty() {
            if let Some(logic_button) = self.resolve_logic_button(button) {
                if let Some(sp_template) = logic_button.get_special_power_template() {
                    TheInGameUI::set_pending_special_power(
                        sp_template.get_id(),
                        button.options,
                        source_id,
                    );
                }
            }
        }

        let pending_payload = if button.command_type == CommandType::FireWeapon {
            button.weapon_slot_number()
        } else {
            source_id
        };
        TheInGameUI::set_pending_command_with_visual(
            button.command_type,
            button.options,
            pending_payload,
            button.cursor_name.clone(),
            button.invalid_cursor_name.clone(),
            button.radius_cursor_type.clone(),
        );

        if (button.options & CommandOption::NeedTargetEnemyObject as u32) != 0
            || (button.options & CommandOption::AttackObjectsPosition as u32) != 0
        {
            TheInGameUI::set_force_attack_mode(true);
        }
        if (button.options & CommandOption::NeedTargetPos as u32) != 0 {
            TheInGameUI::set_force_move_to_mode(true);
        }
        if (button.options
            & (CommandOption::NeedTargetAllyObject as u32
                | CommandOption::NeedTargetNeutralObject as u32))
            != 0
        {
            TheInGameUI::set_prefer_selection_mode(true);
        }

        Ok(())
    }

    fn resolve_logic_button(
        &self,
        button: &CommandButton,
    ) -> Option<gamelogic::command_button::CommandButton> {
        let control_bar = get_control_bar_bridge()?;
        control_bar
            .find_command_button_by_name(&button.command_name)
            .cloned()
    }

    fn execute_purchase_science(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(store) = get_science_store() else {
            return Ok(());
        };
        let player_index: PlayerIndex = context.player_id as PlayerIndex;
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_index).cloned());
        let Some(player_arc) = player_arc else {
            return Ok(());
        };
        let Ok(player) = player_arc.read() else {
            return Ok(());
        };

        let mut selected_science = SCIENCE_INVALID;
        for &science in &button.sciences_ids {
            if science == SCIENCE_INVALID {
                continue;
            }
            if !player.has_science(science)
                && store.player_has_prereqs_for_science(&*player, science)
                && store.get_science_purchase_cost(science) <= player.get_science_purchase_points()
            {
                selected_science = science;
                break;
            }
        }

        if selected_science == SCIENCE_INVALID {
            return Ok(());
        }

        let mut command = Command::new(CommandType::PurchaseScience);
        command.set_player_index(context.player_id as i32);
        command.append_integer_argument(selected_science);
        self.queue_command(context.player_id as i32, command)?;
        Ok(())
    }

    fn select_all_units_of_type(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let template_id = if !button.object.is_empty() {
            gamelogic::helpers::TheThingFactory::find_template(button.object.as_str())
                .map(|t| t.get_id())
        } else {
            self.resolve_logic_button(button)
                .and_then(|logic_button| logic_button.get_thing_template().map(|t| t.get_id()))
        };

        let Some(template_id) = template_id else {
            return Ok(());
        };

        let player_index: PlayerIndex = context.player_id as PlayerIndex;
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_index).cloned());
        let Some(player_arc) = player_arc else {
            return Ok(());
        };
        let Ok(player) = player_arc.read() else {
            return Ok(());
        };

        let mut matches: Vec<u32> = Vec::new();
        let _ = player.iterate_objects(|obj| {
            let guard = obj.read().map_err(|_| GameError::LockError)?;
            if guard.get_template().get_id() == template_id {
                matches.push(guard.get_id());
            }
            Ok(())
        });

        if matches.is_empty() {
            return Ok(());
        }

        if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
            stream.append_message(GameMessageType::CreateSelectedGroup(true, matches));
        }
        Ok(())
    }

    fn execute_upgrade_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        _source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let upgrade_name = button.upgrade.as_str();
        let upgrade_template = with_upgrade_center(|center| center.find_upgrade(upgrade_name));
        let Some(template) = upgrade_template else {
            return Ok(());
        };

        let source_obj_id = context.selected_objects.first().copied();
        let mut command = Command::new(CommandType::QueueUpgrade);
        command.set_player_index(context.player_id as i32);

        if let Some(obj_id) = source_obj_id {
            command.append_object_id_argument(obj_id);
        }

        command.append_integer_argument(template.get_name_key() as i32);
        self.queue_command(context.player_id as i32, command)?;
        Ok(())
    }

    fn execute_production_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Dual-world residual: live modules via OBJECT_REGISTRY when bound.
        let mut applied = 0usize;
        if let Ok(button_id) = self.resolve_command_button_id(button) {
            let cmd_source = Self::map_command_source(source);
            for object_id in &context.selected_objects {
                let Some(obj_arc) = OBJECT_REGISTRY.get_object(*object_id) else {
                    continue;
                };
                let Ok(mut obj_guard) = obj_arc.write() else {
                    continue;
                };
                let _ = obj_guard.do_command_button(button_id, cmd_source);
                applied += 1;
            }
        }
        if applied > 0 {
            return Ok(());
        }
        // Host/presentation residual: MSG_QUEUE_UNIT_CREATE (no OBJECT_REGISTRY).
        let Some(logic_button) = self.resolve_logic_button(button) else {
            return Ok(());
        };
        let Some(thing_template) = logic_button.get_thing_template() else {
            return Ok(());
        };
        let template_id = thing_template.get_id();
        let production_id = 0u32;
        if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
            stream.append_message(GameMessageType::QueueUnitCreate(template_id, production_id));
        }
        Ok(())
    }

    fn execute_special_power_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut applied = 0usize;
        if let Ok(button_id) = self.resolve_command_button_id(button) {
            let cmd_source = Self::map_command_source(source);
            for object_id in &context.selected_objects {
                let Some(obj_arc) = OBJECT_REGISTRY.get_object(*object_id) else {
                    continue;
                };
                let Ok(mut obj_guard) = obj_arc.write() else {
                    continue;
                };
                let _ = obj_guard.do_command_button(button_id, cmd_source);
                applied += 1;
            }
        }
        if applied > 0 {
            return Ok(());
        }
        // Host/presentation residual: MSG_DO_SPECIAL_POWER without dual-world modules.
        let Some(logic_button) = self.resolve_logic_button(button) else {
            return Ok(());
        };
        let Some(sp_template) = logic_button.get_special_power_template() else {
            return Ok(());
        };
        let sp_id = sp_template.get_id();
        let options = logic_button.get_options_bits();
        let source_obj_id = context.selected_objects.first().copied().unwrap_or(0);
        if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
            stream.append_message(GameMessageType::DoSpecialPower(
                sp_id,
                options,
                source_obj_id,
            ));
        }
        Ok(())
    }

    fn execute_direct_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if button.command_type == CommandType::Exit {
            let occupant = button
                .exit_object_id
                .or_else(|| context.contain_data.iter().copied().flatten().next());
            let Some(occupant) = occupant else {
                return Ok(());
            };
            if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
                stream.append_message(GameMessageType::Exit(occupant));
            }
            return Ok(());
        }
        if context.selected_objects.is_empty() {
            return Ok(());
        }
        if button.command_type == CommandType::Invalid {
            return Ok(());
        }

        // Dual-world residual only when registry objects actually accept the button.
        let mut applied = 0usize;
        if let Ok(button_id) = self.resolve_command_button_id(button) {
            let cmd_source = Self::map_command_source(source);
            for object_id in &context.selected_objects {
                let Some(obj_arc) = OBJECT_REGISTRY.get_object(*object_id) else {
                    continue;
                };
                let Ok(mut obj_guard) = obj_arc.write() else {
                    continue;
                };
                let _ = obj_guard.do_command_button(button_id, cmd_source);
                applied += 1;
            }
        }
        if applied > 0 {
            return Ok(());
        }

        // Host/presentation residual: queue typed Command with selected IDs.
        let mut command = Command::new(button.command_type);
        command.set_player_index(context.player_id as i32);
        if let Some(occupant) = button.exit_object_id {
            command.append_object_id_argument(occupant);
        } else {
            for object_id in &context.selected_objects {
                command.append_object_id_argument(*object_id);
            }
        }
        self.queue_command(context.player_id as i32, command)?;
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Build queue cancel
    // ---------------------------------------------------------------------------

    /// Cancel a build queue item by index. Mirrors C++ CancelUnitCreate/CancelUpgradeCreate.
    pub fn cancel_build_queue_item(
        &self,
        queue_index: usize,
        context: &ControlBarContext,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if queue_index >= self.build_queue_data.len() {
            return Ok(false);
        }

        let entry = &self.build_queue_data[queue_index];
        let Some(&producer_id) = context.selected_objects.first() else {
            return Ok(false);
        };

        // The host must cancel its own queue, not a legacy producer module.
        if super::publish_host_queue_cancel(
            context,
            producer_id,
            entry.production_id,
            entry.production_type,
            entry.upgrade_name.clone(),
            queue_index,
        ) {
            return Ok(true);
        }

        // Dual-world residual when producer modules are bound.
        if OBJECT_REGISTRY.get_object(producer_id).is_some() {
            Self::cancel_production_by_id(producer_id, entry.production_id);
            return Ok(true);
        }
        // Host/presentation residual: message-stream cancel (no OBJECT_REGISTRY modules).
        if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
            match entry.production_type {
                QueueProductionType::Upgrade => {
                    stream.append_message(GameMessageType::CancelUpgrade(entry.production_id));
                }
                _ => {
                    stream.append_message(GameMessageType::CancelUnitCreate(entry.production_id));
                }
            }
        }
        Ok(true)
    }

    /// Pause/resume the build queue for the selected producer.
    pub fn set_build_queue_paused(
        &mut self,
        paused: bool,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(&producer_id) = context.selected_objects.first() else {
            return Ok(());
        };

        // A host bridge is a single-authority boundary.  Do not update the
        // legacy module or its compatibility pause queue after publishing.
        if super::publish_host_production_pause(context, producer_id, paused) {
            self.portrait_state.production_paused = paused;
            return Ok(());
        }

        // Dual-world residual: live production modules when registry is bound.
        if OBJECT_REGISTRY.get_object(producer_id).is_some() {
            Self::set_object_production_paused(producer_id, paused);
        } else {
            // Wave 985: host empty dual-world → queue residual for Main BuildingData.
            queue_host_production_pause(producer_id, paused);
            // Wave 986: portrait residual reflects pause immediately.
            self.portrait_state.production_paused = paused;
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Command button rebuild helpers
    // ---------------------------------------------------------------------------

    fn rebuild_command_buttons(
        &mut self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        context.available_commands.clear();

        match context.current_state {
            ControlBarState::None => {
                self.add_default_commands(context)?;
            }
            ControlBarState::Command => {
                self.add_object_commands(context)?;
            }
            ControlBarState::MultiSelect => {
                self.add_multi_select_commands(context)?;
            }
            ControlBarState::Observer => {
                self.add_observer_commands(context)?;
            }
            ControlBarState::UnderConstruction => {
                self.add_construction_commands(context)?;
            }
            ControlBarState::StructureInventory => {
                self.add_structure_inventory_commands(context)?;
            }
            ControlBarState::Beacon => {
                self.add_beacon_commands(context)?;
            }
            ControlBarState::OclTimer => {
                // C++ ControlBarOCLTimer.cpp:55 populateOCLTimer: adds sell/rally-point
                // button depending on creator object kind, then updates timer display
                self.add_ocl_timer_commands(context)?;
            }
        }

        self.bind_command_windows(context);
        Ok(())
    }

    fn leftover_command_button_by_name(name: &str) -> Option<CommandButton> {
        if name.is_empty() {
            return None;
        }
        if let Some(ini) = get_ini_control_bar() {
            if let Some(definition) = ini.find_command_button_resolved(name) {
                return Some(Self::command_from_definition(definition));
            }
        }
        if let Some(bridge) = get_control_bar_bridge() {
            if let Some(logic) = bridge.find_command_button_by_name(name) {
                return Some(Self::command_from_logic_button(logic));
            }
        }
        None
    }

    fn leftover_apply_command_bar_border(&self, win: &mut GameWindow, cmd: &CommandButton) {
        let border = leftover_mapped_border_type(&cmd.button_border_type);
        let color = self.get_border_color_for_type(border);
        if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) = win.widget_mut() {
            match color {
                Some(packed) => button.set_border(true, leftover_color_from_packed(packed)),
                None => button.set_border(false, crate::gui::gadgets::Color::rgba(0, 0, 0, 0)),
            }
        }
    }

    /// C++ ControlBar::setControlCommand (ControlBar.cpp:2403-2480).
    fn set_control_command(&self, win: &Rc<RefCell<GameWindow>>, cmd: &CommandButton) {
        let mapped_image = if cmd.button_image.is_empty() {
            None
        } else {
            leftover_mapped_image(&cmd.button_image)
        };

        {
            let mut window = win.borrow_mut();
            if (cmd.options & CommandOption::CheckLike as u32) != 0 {
                if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) =
                    window.widget_mut()
                {
                    button.set_checkbox(true, false);
                }
            } else if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) =
                window.widget_mut()
            {
                button.set_checkbox(false, false);
            }
            if let Some(image) = mapped_image {
                let _ = window.set_enabled_image(0, image);
                window.set_status(crate::gui::game_window::WindowStatus::IMAGE);
            }
            if !cmd.text_label.is_empty()
                || !cmd.sciences_ids.is_empty()
                || !cmd.sciences.is_empty()
            {
                window.set_tooltip_callback(leftover_command_button_tooltip);
            }
            if let Some(overlay) = cmd.overlay_image.as_deref() {
                if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) =
                    window.widget_mut()
                {
                    button.set_overlay_image(Some(overlay));
                }
            } else if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) =
                window.widget_mut()
            {
                button.set_overlay_image(None::<String>);
            }
            window.set_user_data(cmd.clone());
            self.leftover_apply_command_bar_border(&mut window, cmd);
            if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) =
                window.widget_mut()
            {
                button.set_alt_sound(Some("GUICommandBarClick"));
            }
        }
        if !cmd.text_label.is_empty() {
            let hot_key =
                with_hot_key_manager(|manager| manager.search_hot_key(&cmd.text_label));
            if !hot_key.is_empty() {
                with_hot_key_manager(|manager| manager.add_hot_key(win.clone(), &hot_key));
            }
        }
    }



    /// C++ ControlBar.cpp:1086 / setControlCommand :2403-2480 — bind
    /// ButtonCommand01..14, tooltip, border, command identity, and hotkeys.
    fn bind_command_windows(&self, context: &ControlBarContext) {
        // C++ switchToContext resets TheHotKeyManager before rebinding.
        with_hot_key_manager(|manager| manager.reset());
        with_window_manager(|wm| {
            for i in 0..14 {
                let name = format!("ControlBar.wnd:ButtonCommand{:02}", i + 1);
                let Some(win) = wm.find_window_by_name(&name) else {
                    continue;
                };
                if let Some(cmd) = context.available_commands.get(i) {
                    if cmd.button_hidden || cmd.command_name.is_empty() {
                        let _ = win.borrow_mut().hide(true);
                        continue;
                    }
                    self.set_control_command(&win, cmd);
                    if context.current_state == ControlBarState::StructureInventory {
                        // C++ ControlBarCommand.cpp:813-821 — ALWAYS_COLOR on
                        // EXIT_CONTAINER was commented out so subdued
                        // containers can gray exit cameos.
                        let _ = win
                            .borrow_mut()
                            .clear_status(crate::gui::game_window::WindowStatus::NOT_READY);
                        let _ = win
                            .borrow_mut()
                            .clear_status(crate::gui::game_window::WindowStatus::ALWAYS_COLOR);
                    }
                    let _ = win.borrow_mut().enable(cmd.button_enabled);
                    let _ = win.borrow_mut().hide(false);
                } else if context.current_state == ControlBarState::Command
                    || context.current_state == ControlBarState::StructureInventory
                {
                    let _ = win.borrow_mut().hide(true);
                }
            }
        });
    }
}

fn leftover_mapped_border_type(raw: &str) -> CommandButtonMappedBorderType {
    match raw.trim().to_ascii_uppercase().as_str() {
        "BUILD" => CommandButtonMappedBorderType::Build,
        "UPGRADE" => CommandButtonMappedBorderType::Upgrade,
        "ACTION" => CommandButtonMappedBorderType::Action,
        "SYSTEM" => CommandButtonMappedBorderType::System,
        _ => CommandButtonMappedBorderType::None,
    }
}

fn leftover_color_from_packed(color: u32) -> crate::gui::gadgets::Color {
    crate::gui::gadgets::Color::rgba(
        ((color >> 16) & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        (color & 0xFF) as u8,
        ((color >> 24) & 0xFF) as u8,
    )
}

fn leftover_command_button_tooltip(
    window: &GameWindow,
    _inst: &crate::gui::game_window::WindowInstanceData,
    _mouse: u32,
) {
    let id = window.get_id();
    let Some(rc) = with_window_manager(|wm| wm.get_window_by_id(id)) else {
        return;
    };
    let _ = crate::gui::gui_callbacks::control_bar_popup_description::show_build_tooltip_layout(rc);
}


#[cfg(test)]
mod host_bridge_execution_tests {
    use super::*;

    #[test]
    fn enabled_host_bridge_intercepts_real_control_bar_execution() {
        let _guard = crate::gui::control_bar::acquire_host_control_bar_bridge_test_guard();
        crate::gui::control_bar::set_host_control_bar_bridge_enabled(true);

        let mut button = CommandButton::default();
        button.command_name = "Command_ConstructAmericaTank".to_string();
        button.command_type = CommandType::QueueUnitCreate;
        button.object = "AmericaTankCrusader".to_string();
        let context = ControlBarContext {
            player_id: 3,
            selected_objects: vec![17],
            ..ControlBarContext::default()
        };

        ControlBar::new()
            .execute_command(&button, CommandSourceType::FromUser, &context)
            .expect("host bridge request");

        assert!(matches!(
            crate::gui::control_bar::take_host_control_bar_requests().as_slice(),
            [crate::gui::control_bar::HostControlBarRequest::Production {
                template_name,
                producer_ids,
                player_id: 3,
                ..
            }] if template_name == "AmericaTankCrusader" && producer_ids == &[17]
        ));
    }

    #[test]
    fn parsed_retail_mig_fire_weapon_keeps_max_shots_and_options_without_logic_button() {
        let _guard = crate::gui::control_bar::acquire_host_control_bar_bridge_test_guard();
        crate::gui::control_bar::set_host_control_bar_bridge_enabled(true);

        const RETAIL_MIG_BUTTON: &str = "Command_ChinaJetMIGFireNapalmMissile";
        // Parse the actual retail CommandButton.ini block, rather than hand
        // constructing equivalent fields.  The host path must be driven by
        // the loaded button data, not by a MiG-specific bridge rule.
        const RETAIL_COMMAND_BUTTONS: &str = include_str!(
            "../../../../../../../../windows_game/extracted_big_files_v2/INI/CommandButton.ini"
        );
        let header = format!("CommandButton {RETAIL_MIG_BUTTON}");
        let tail = &RETAIL_COMMAND_BUTTONS[RETAIL_COMMAND_BUTTONS
            .find(&header)
            .expect("retail MiG command button must be present")..];
        let end = tail
            .find("\nEnd")
            .expect("retail MiG command button must end");
        let retail_block = &tail[..end + "\nEnd".len()];
        let mut ini = game_engine::common::ini::INI::new();
        let definition = ini
            .with_inline_source(retail_block, |ini| {
                ini.read_line()?;
                IniCommandButton::parse_from_ini(ini, RETAIL_MIG_BUTTON.to_string())
            })
            .expect("parse retail MiG command button");
        let button = ControlBar::command_from_definition(&definition);
        assert_eq!(button.command_name, RETAIL_MIG_BUTTON);
        assert_eq!(button.weapon_slot, WeaponSlotType::Primary);
        assert_eq!(button.weapon_slot_number(), 0);
        assert_eq!(button.max_shots_to_fire, 1);
        assert_eq!(
            button.options,
            CommandOption::OkForMultiSelect as u32
                | CommandOption::NeedTargetEnemyObject as u32
                | CommandOption::NeedTargetNeutralObject as u32
        );

        let context = ControlBarContext {
            player_id: 3,
            selected_objects: vec![17],
            ..ControlBarContext::default()
        };
        ControlBar::new()
            .execute_command(&button, CommandSourceType::FromUser, &context)
            .expect("host bridge request from parsed command button");

        assert!(matches!(
            crate::gui::control_bar::take_host_control_bar_requests().as_slice(),
            [crate::gui::control_bar::HostControlBarRequest::ArmTarget {
                command_name,
                command_type: CommandType::FireWeapon,
                options,
                weapon_slot: Some(0),
                max_shots_to_fire: Some(1),
                ..
            }] if command_name == RETAIL_MIG_BUTTON
                && *options == button.options
        ));
    }

    #[test]
    fn parsed_retail_switch_weapon_slot_reaches_host_direct_command_without_logic_button() {
        let _guard = crate::gui::control_bar::acquire_host_control_bar_bridge_test_guard();
        crate::gui::control_bar::set_host_control_bar_bridge_enabled(true);

        // Retail CommandButton.ini: the Demo Trap manual fuse is a tertiary
        // SWITCH_WEAPON command.  Keep the slot sourced from the parsed live
        // definition, not the legacy GameLogic command-button mirror.
        let mut definition = IniCommandButton::default();
        definition.name = "Command_SetDemoTrapManualDetonation".to_string();
        definition.command = "SWITCH_WEAPON".to_string();
        definition.weapon_slot = WeaponSlotType::Tertiary;
        let button = ControlBar::command_from_definition(&definition);
        assert_eq!(button.command_type, CommandType::SwitchWeapons);
        assert_eq!(button.weapon_slot, WeaponSlotType::Tertiary);
        assert_eq!(button.weapon_slot_number(), 2);

        let context = ControlBarContext {
            player_id: 3,
            selected_objects: vec![17],
            ..ControlBarContext::default()
        };
        ControlBar::new()
            .execute_command(&button, CommandSourceType::FromUser, &context)
            .expect("host bridge request from parsed retail switch button");

        let requests = crate::gui::control_bar::take_host_control_bar_requests();
        assert!(
            matches!(
                requests.as_slice(),
                [crate::gui::control_bar::HostControlBarRequest::DirectCommand {
                    command_name,
                    command_type: CommandType::SwitchWeapons,
                    weapon_slot: Some(2),
                    ..
                }] if command_name == "Command_SetDemoTrapManualDetonation"
            ),
            "unexpected host bridge requests: {requests:#?}"
        );
    }

    #[test]
    fn parsed_fire_weapon_buttons_keep_distinct_retail_keys() {
        // These are two distinct retail CommandButton.ini entries with the
        // same FIRE_WEAPON operation.  Collapsing them to the operation text
        // makes exact host mappings impossible and aliases their UI state.
        let mut rocket_pods = IniCommandButton::default();
        rocket_pods.name = "Command_AmericaVehicleComancheFireRocketPods".to_string();
        rocket_pods.command = "FIRE_WEAPON".to_string();
        rocket_pods.weapon_slot = WeaponSlotType::Tertiary;

        let mut demo_suicide = IniCommandButton::default();
        demo_suicide.name = "Demo_Command_TertiarySuicide".to_string();
        demo_suicide.command = "FIRE_WEAPON".to_string();
        demo_suicide.weapon_slot = WeaponSlotType::Tertiary;

        let rocket_pods = ControlBar::command_from_definition(&rocket_pods);
        let demo_suicide = ControlBar::command_from_definition(&demo_suicide);

        assert_eq!(
            rocket_pods.command_name,
            "Command_AmericaVehicleComancheFireRocketPods"
        );
        assert_eq!(demo_suicide.command_name, "Demo_Command_TertiarySuicide");
        assert_ne!(rocket_pods.command_name, demo_suicide.command_name);
        assert_eq!(rocket_pods.command_type, CommandType::FireWeapon);
        assert_eq!(demo_suicide.command_type, CommandType::FireWeapon);
    }
}
