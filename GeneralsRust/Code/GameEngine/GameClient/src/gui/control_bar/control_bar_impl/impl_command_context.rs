// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

impl ControlBar {
    fn update_context_command(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let obj_id = {
            let context = self
                .context
                .read()
                .map_err(|_| "Failed to acquire context read lock")?;
            context.selected_objects.first().copied()
        };

        let Some(obj_id) = obj_id else {
            return Ok(());
        };

        let has_production = self.get_object_has_production(obj_id);
        let registry_producer = OBJECT_REGISTRY.get_object(obj_id).is_some();

        if has_production {
            // Wave 1026: dual-world peels populate_build_queue from presentation/catalog
            // even when OBJECT_REGISTRY has no producer Arc.
            // Dual-world residual: live production modules own queue when registry is bound.
            let _ = registry_producer;
            let mut context = {
                let mut guard = self
                    .context
                    .write()
                    .map_err(|_| "Failed to acquire context write lock")?;
                std::mem::take(&mut *guard)
            };
            self.populate_build_queue(&mut context, obj_id)?;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
        } else if !has_production && !self.portrait_state.is_visible {
            // Only clear when neither registry nor presentation claims production.
            if !self.build_queue_data.is_empty() {
                self.build_queue_data.clear();
                self.displayed_queue_count = 0;
                if let Ok(mut context) = self.context.write() {
                    context.construction_queue.clear();
                }
            }
        }
        // else: presentation-fed queue residual stays (host path).

        let first_progress = self.get_first_production_progress(obj_id);

        if let Some(percent) = first_progress {
            if let Ok(mut context) = self.context.write() {
                if let Some(first_item) = context.construction_queue.first_mut() {
                    first_item.progress = percent;
                }
            }
        }

        let context = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?;
        let player_id = context.player_id;
        let buttons_snapshot: Vec<CommandButton> = context.available_commands.clone();
        drop(context);

        for button in &buttons_snapshot {
            let availability = self.get_command_availability(button, obj_id, player_id)?;
            let name = button.command_name.clone();
            if let Ok(mut context) = self.context.write() {
                if let Some(state) = context
                    .available_commands
                    .iter_mut()
                    .find(|b| b.command_name == name)
                {
                    match availability {
                        CommandAvailability::Hidden => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.visible = false;
                            }
                        }
                        CommandAvailability::Restricted => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.enabled = false;
                                bs.availability = CommandAvailability::Restricted;
                            }
                        }
                        CommandAvailability::NotReady => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.enabled = false;
                                bs.availability = CommandAvailability::NotReady;
                            }
                        }
                        CommandAvailability::CantAfford => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.enabled = false;
                                bs.availability = CommandAvailability::CantAfford;
                            }
                        }
                        CommandAvailability::Active => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.enabled = true;
                                bs.availability = CommandAvailability::Active;
                                if (button.options & CommandOption::CheckLike as u32) != 0 {
                                    bs.check_like_active = true;
                                }
                            }
                        }
                        CommandAvailability::Available => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.enabled = true;
                                bs.availability = CommandAvailability::Available;
                                if (button.options & CommandOption::CheckLike as u32) != 0 {
                                    bs.check_like_active = false;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // getCommandAvailability - per-button availability check
    // C++ ControlBarCommand.cpp:993-1516
    // ---------------------------------------------------------------------------

    fn get_command_availability(
        &self,
        command: &CommandButton,
        obj_id: u32,
        player_id: u32,
    ) -> Result<CommandAvailability, Box<dyn std::error::Error>> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            // Host/presentation path: Main already filtered unit_command_buttons.
            // Wave 1025: dual-world peels catalog/command-set residual when portrait
            // freeze is not yet visible for this selection frame.
            // Wave 1025/1026/1052: catalog/command-set residual; disabled => Restricted.
            // Wave 1052: destroyed/sold/unselectable fail-closed (no command UI).
            if let Some(entry) =
                crate::presentation_translator_residual::translator_catalog_entry(obj_id)
            {
                if entry.destroyed || entry.sold || entry.unselectable {
                    return Ok(CommandAvailability::Hidden);
                }
                if entry.disabled && !self.force_disabled_evaluation(command) {
                    let cmd_type = command.command_type;
                    if cmd_type != CommandType::Sell
                        && cmd_type != CommandType::Evacuate
                        && cmd_type != CommandType::DoStop
                    {
                        return Ok(CommandAvailability::Restricted);
                    }
                }
                if !entry.command_set_name.is_empty() || entry.selectable {
                    return Ok(CommandAvailability::Available);
                }
            }
            if self.portrait_state.is_visible
                || !self.presentation_primary_command_set.is_empty()
                || !self.presentation_command_set_names.is_empty()
            {
                return Ok(CommandAvailability::Available);
            }
            return Ok(CommandAvailability::Hidden);
        };
        let Ok(obj) = obj_arc.read() else {
            if self.portrait_state.is_visible {
                return Ok(CommandAvailability::Available);
            }
            return Ok(CommandAvailability::Hidden);
        };

        if obj.is_disabled() && !self.force_disabled_evaluation(command) {
            let cmd_type = command.command_type;
            if cmd_type != CommandType::Sell
                && cmd_type != CommandType::Evacuate
                && cmd_type != CommandType::DoStop
            {
                return Ok(CommandAvailability::Restricted);
            }
        }

        if (command.options & CommandOption::NeedUpgrade as u32) != 0 && !command.upgrade.is_empty()
        {
            let player_arc = logic_player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(player_id as PlayerIndex).cloned());
            if let Some(player_arc) = player_arc {
                if let Ok(player) = player_arc.read() {
                    let upgrade = with_upgrade_center(|c| c.find_upgrade(command.upgrade.as_str()));
                    if let Some(template) = upgrade {
                        if !player.has_upgrade_complete(&template) {
                            return Ok(CommandAvailability::Restricted);
                        }
                    }
                }
            }
        }

        let queue_count = self.build_queue_data.len();
        let queue_maxed = queue_count >= MAX_BUILD_QUEUE_BUTTONS;

        if queue_maxed && (command.options & CommandOption::NotQueueable as u32) != 0 {
            return Ok(CommandAvailability::Restricted);
        }

        match command.command_type {
            CommandType::DozerConstruct => {
                if queue_maxed {
                    return Ok(CommandAvailability::Restricted);
                }
                let player_arc = logic_player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as PlayerIndex).cloned());
                if let Some(player_arc) = player_arc {
                    if let Ok(player) = player_arc.read() {
                        if !command.purchase_cost.is_empty() {
                            for (resource, cost) in &command.purchase_cost {
                                if *cost > 0
                                    && (resource.eq_ignore_ascii_case("cash")
                                        || resource.eq_ignore_ascii_case("money"))
                                    && !player.get_money().can_afford(*cost)
                                {
                                    return Ok(CommandAvailability::Restricted);
                                }
                            }
                        }
                    }
                }
                Ok(CommandAvailability::Available)
            }
            CommandType::QueueUpgrade => {
                if queue_maxed {
                    return Ok(CommandAvailability::Restricted);
                }
                let player_arc = logic_player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as PlayerIndex).cloned());
                if let Some(player_arc) = player_arc {
                    if let Ok(player) = player_arc.read() {
                        let upgrade =
                            with_upgrade_center(|c| c.find_upgrade(command.upgrade.as_str()));
                        if let Some(template) = upgrade {
                            if player.has_upgrade_complete(&template)
                                || player.has_upgrade_in_production(&template)
                            {
                                return Ok(CommandAvailability::CantAfford);
                            }
                            if !with_upgrade_center(|c| {
                                c.can_afford_upgrade(&player, &template, false)
                            }) {
                                return Ok(CommandAvailability::Restricted);
                            }
                        } else {
                            return Ok(CommandAvailability::Restricted);
                        }
                    }
                }
                Ok(CommandAvailability::Available)
            }
            CommandType::DoStop => Ok(CommandAvailability::Available),
            CommandType::DoGuardPosition | CommandType::DoGuardObject => {
                Ok(CommandAvailability::Available)
            }
            CommandType::Sell => Ok(CommandAvailability::Available),
            CommandType::Evacuate => Ok(CommandAvailability::Available),
            CommandType::SpecialPower => Ok(CommandAvailability::Available),
            CommandType::MetaSelectMatchingUnits => Ok(CommandAvailability::Available),
            CommandType::PurchaseScience => Ok(CommandAvailability::Available),
            _ => Ok(CommandAvailability::Available),
        }
    }

    fn force_disabled_evaluation(&self, _command: &CommandButton) -> bool {
        false
    }

    // ---------------------------------------------------------------------------
    // populateBuildQueue - fill build queue from producer object
    // C++ ControlBarCommand.cpp:531-674
    // ---------------------------------------------------------------------------

    fn populate_build_queue(
        &mut self,
        context: &mut ControlBarContext,
        producer_id: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Wave 981/1010/1014/1046: host empty dual-world keeps presentation residual queue/portrait.
        if Self::dual_world_registry_unavailable() {
            // Wave 1046: destroyed/sold/disabled producer clears residual production UI.
            if let Some(entry) =
                crate::presentation_translator_residual::translator_catalog_entry(producer_id)
            {
                // Wave 1070: masked/UC producer residual clears production UI.
                if entry.destroyed
                    || entry.sold
                    || entry.disabled
                    || entry.unselectable
                    || entry.masked
                    || entry.under_construction
                {
                    self.portrait_state.production_progress = None;
                    self.portrait_state.production_template = None;
                    self.portrait_state.production_paused = false;
                    self.build_queue_data.clear();
                    self.displayed_queue_count = 0;
                    return Ok(());
                }
            }
            // Wave 1014: seed portrait production head from translator catalog when empty.
            if self.portrait_state.production_template.is_none()
                && self.portrait_state.production_progress.is_none()
            {
                if let Some(entry) =
                    crate::presentation_translator_residual::translator_catalog_entry(producer_id)
                {
                    if entry.production_template.is_some() || entry.production_progress.is_some() {
                        self.portrait_state.production_progress = entry.production_progress;
                        self.portrait_state.production_template = entry.production_template.clone();
                        self.portrait_state.production_paused = entry.production_paused;
                    }
                }
            }
            if self.portrait_state.production_progress.is_some()
                || self.portrait_state.production_template.is_some()
                || !self.build_queue_data.is_empty()
            {
                // Wave 1010: peel portrait production_template into residual queue entry.
                if self.build_queue_data.is_empty() {
                    if let Some(tmpl) = self.portrait_state.production_template.clone() {
                        let is_upgrade = self.portrait_state.production_paused
                            && tmpl.to_ascii_lowercase().contains("upgrade");
                        self.build_queue_data.push(BuildQueueEntry {
                            production_type: if is_upgrade {
                                QueueProductionType::Upgrade
                            } else {
                                QueueProductionType::Unit
                            },
                            production_id: producer_id,
                            upgrade_name: tmpl,
                        });
                    }
                }
                self.displayed_queue_count = self
                    .displayed_queue_count
                    .max(self.build_queue_data.len())
                    .max(
                        if self.portrait_state.production_progress.is_some()
                            || self.portrait_state.production_template.is_some()
                        {
                            1
                        } else {
                            0
                        },
                    );
            }
            let _ = producer_id;
            return Ok(());
        }

        self.build_queue_data.clear();
        context.construction_queue.clear();

        let Some(obj_arc) = OBJECT_REGISTRY.get_object(producer_id) else {
            self.displayed_queue_count = 0;
            return Ok(());
        };
        let Ok(obj) = obj_arc.read() else {
            self.displayed_queue_count = 0;
            return Ok(());
        };

        let mut found_pu = false;
        for module in obj.get_behavior_modules() {
            if let Ok(mut guard) = module.lock() {
                if let Some(pu) = guard.get_production_update_interface() {
                    found_pu = true;
                    for entry in pu.get_queue_entries() {
                        let mut cost = HashMap::new();
                        cost.insert("Supplies".to_string(), entry.cost);
                        let progress = entry.progress().clamp(0.0, 1.0);
                        context.construction_queue.push(ProductionItem {
                            template_name: entry.template_name.clone(),
                            production_type: Self::map_logic_production_type(entry.production_type),
                            progress,
                            cost,
                            build_time: entry.build_time as f32,
                        });
                        self.build_queue_data.push(BuildQueueEntry {
                            production_type: Self::map_logic_queue_type(entry.production_type),
                            production_id: entry.queue_index as u32,
                            upgrade_name: if entry.production_type
                                == gamelogic::object::production::queue::ProductionType::Upgrade
                            {
                                entry.template_name
                            } else {
                                String::new()
                            },
                        });
                    }
                    break;
                }
            }
        }

        if !found_pu {
            self.displayed_queue_count = 0;
        } else {
            self.displayed_queue_count = context.construction_queue.len();
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Command processing (click dispatch)
    // C++ ControlBar.cpp:2071-2090
    // ---------------------------------------------------------------------------

    /// C++ ControlBar::processContextSensitiveButtonClick → processCommandUI.
    pub fn process_context_sensitive_button_click(&mut self, control_id: u32, _msg: u32) {
        let command_name = with_window_manager(|wm| {
            wm.get_window_by_id(control_id as crate::gui::WindowId)
                .and_then(|win| win.borrow().get_user_data::<String>().cloned())
        });
        if let Some(command_name) = command_name {
            if command_name.is_empty() {
                return;
            }
            if command_name.eq_ignore_ascii_case("Command_StructureExit") {
                let slot = with_window_manager(|wm| {
                    for i in 0..14 {
                        let name = format!("ControlBar.wnd:ButtonCommand{:02}", i + 1);
                        if let Some(win) = wm.find_window_by_name(&name) {
                            if win.borrow().get_id() as u32 == control_id {
                                return Some(i);
                            }
                        }
                    }
                    None
                });
                if let Some(slot) = slot {
                    let occupant = self
                        .context
                        .read()
                        .ok()
                        .and_then(|ctx| ctx.contain_data.get(slot).copied().flatten());
                    if occupant.is_none() {
                        return;
                    }
                }
            }
            let _ = self.process_command(&command_name, CommandSourceType::FromUser);
        }
    }

    pub fn process_command(
        &mut self,
        command_name: &str,
        source: CommandSourceType,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let context = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?;

        if let Some(button) = context
            .available_commands
            .iter()
            .find(|b| b.command_name == command_name)
        {
            let enabled = self
                .button_states
                .get(&button.command_name)
                .map(|s| s.enabled)
                .unwrap_or(false);

            if !enabled {
                return Ok(false);
            }

            self.execute_command(button, source, &context)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn execute_command(
        &self,
        button: &CommandButton,
        source: CommandSourceType,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Main owns the authoritative Rust simulation.  When it has explicitly
        // enabled the bridge, do not also route this click through the legacy
        // GameLogic globals: that would make one HUD action mutate two worlds.
        if self.publish_host_command_if_enabled(button, source, context) {
            return Ok(());
        }

        if Self::command_needs_target(button.options) {
            self.enter_targeting_mode(button, context)?;
            return Ok(());
        }

        if button.command_type == CommandType::PurchaseScience {
            self.execute_purchase_science(button, context)?;
            return Ok(());
        }

        if button.command_type == CommandType::MetaSelectMatchingUnits {
            self.select_all_units_of_type(button, context)?;
            return Ok(());
        }

        if !button.upgrade.is_empty() {
            self.execute_upgrade_command(button, context, source)?;
            return Ok(());
        }

        if !button.object.is_empty() {
            self.execute_production_command(button, context, source)?;
        } else if !button.special_power.is_empty() {
            self.execute_special_power_command(button, context, source)?;
        } else {
            self.execute_direct_command(button, context, source)?;
        }

        Ok(())
    }
}
