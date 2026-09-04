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

        if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
            if let Ok(obj) = obj_arc.read() {
                if let Some(contain) = obj.get_contain() {
                    if let Ok(contain_guard) = contain.lock() {
                        if contain_guard.get_max_capacity() > 0 {
                            let count = contain_guard.get_contain_count();
                            let last = self
                                .context
                                .read()
                                .ok()
                                .map(|ctx| ctx.last_recorded_inventory_count)
                                .unwrap_or(0);
                            if last != count {
                                if let Ok(mut ctx) = self.context.write() {
                                    ctx.last_recorded_inventory_count = count;
                                }
                                self.evaluate_context_ui()?;
                            }
                        }
                    }
                }
            }
        }

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

        // C++ updateContextCommand: CP_BUILD_QUEUE vs setPortraitByObject.
        let queue_visible = has_production || !self.build_queue_data.is_empty();
        if queue_visible {
            self.set_portrait_by_object_id(None);
            with_window_manager(|wm| {
                // C++ ControlBar.cpp:1076-1077 — CP_BUILD_QUEUE parent is
                // "ControlBar.wnd:ProductionQueueWindow"; ControlBarCommand.cpp
                // :713-743 shows/hides it around the producer's queue. The
                // previous "ControlBar.wnd:BuildQueue" name never matched, so
                // the queue grid stayed painted while empty.
                if let Some(win) = wm.find_window_by_name("ControlBar.wnd:ProductionQueueWindow")
                {
                    let _ = win.borrow_mut().hide(false);
                }
                if let Some(percent) = first_progress {
                    if let Some(win) = wm.find_window_by_name("ControlBar.wnd:ButtonQueue01") {
                        if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) =
                            win.borrow_mut().widget_mut()
                        {
                            button.set_inverse_clock(
                                (percent * 100.0).clamp(0.0, 100.0) as u8,
                                crate::gui::gadgets::Color {
                                    r: 0,
                                    g: 0,
                                    b: 0,
                                    a: 100,
                                },
                            );

                        }
                    }
                }
            });
        } else {
            with_window_manager(|wm| {
                if let Some(win) = wm.find_window_by_name("ControlBar.wnd:ProductionQueueWindow")
                {
                    let _ = win.borrow_mut().hide(true);
                }
            });
            self.set_portrait_by_object_id(Some(obj_id));
        }


        let context = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?;
        let player_id = context.player_id;
        let buttons_snapshot: Vec<CommandButton> = context.available_commands.clone();
        drop(context);

        // C++ ControlBarCommand.cpp:788-881: evaluate each shown ButtonCommand
        // window, then winHide / winEnable / WIN_STATUS_NOT_READY / ALWAYS_COLOR.
        let mut availability_by_name: HashMap<String, (CommandAvailability, Option<u8>)> =
            HashMap::new();
        for button in &buttons_snapshot {
            let availability = self.get_command_availability(button, obj_id, player_id)?;
            let clock = if availability == CommandAvailability::NotReady {
                self.command_not_ready_clock(button, obj_id)
            } else {
                None
            };
            let name = button.command_name.clone();
            let bs = self
                .button_states
                .entry(name.clone())
                .or_insert_with(ButtonState::default);
            match availability {
                CommandAvailability::Hidden => {
                    bs.visible = false;
                    bs.enabled = false;
                    bs.availability = CommandAvailability::Hidden;
                    bs.progress = 0.0;
                }
                CommandAvailability::Restricted => {
                    bs.visible = true;
                    bs.enabled = false;
                    bs.availability = CommandAvailability::Restricted;
                    bs.progress = 0.0;
                }
                CommandAvailability::NotReady => {
                    bs.visible = true;
                    bs.enabled = false;
                    bs.availability = CommandAvailability::NotReady;
                    bs.progress = clock.map(|p| p as f32).unwrap_or(0.0);
                }
                CommandAvailability::CantAfford => {
                    bs.visible = true;
                    bs.enabled = false;
                    bs.availability = CommandAvailability::CantAfford;
                    bs.progress = 0.0;
                }
                CommandAvailability::Active => {
                    bs.visible = true;
                    bs.enabled = true;
                    bs.availability = CommandAvailability::Active;
                    bs.progress = 0.0;
                    if (button.options & CommandOption::CheckLike as u32) != 0 {
                        bs.check_like_active = true;
                    }
                }
                CommandAvailability::Available => {
                    bs.visible = true;
                    bs.enabled = true;
                    bs.availability = CommandAvailability::Available;
                    bs.progress = 0.0;
                    if (button.options & CommandOption::CheckLike as u32) != 0 {
                        bs.check_like_active = false;
                    }
                }
            }
            availability_by_name.insert(name, (availability, clock));
        }

        self.apply_command_availability_to_windows(&buttons_snapshot, &availability_by_name);

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
                if leftover_presentation_command_set_hidden(self) {
                    return Ok(CommandAvailability::Hidden);
                }
                if command.command_type == CommandType::Sell && entry.script_unsellable {
                    return Ok(CommandAvailability::Hidden);
                }
                if let Some(avail) = leftover_presentation_sell_or_subdued(self, command) {
                    return Ok(avail);
                }
                if entry.disabled && !self.force_disabled_evaluation(command) {
                    if !command_evaluable_when_disabled(command.command_type) {
                        return Ok(CommandAvailability::Restricted);
                    }
                }
                if let Some(hidden) = leftover_buildable_hidden(command, player_id) {
                    return Ok(hidden);
                }
                if leftover_presentation_common_restricted(self, command, Some(&entry)) {
                    return Ok(CommandAvailability::Restricted);
                }
                if leftover_presentation_queue_or_drop_restricted(self, command, Some(&entry)) {
                    return Ok(CommandAvailability::Restricted);
                }
                if let Some(avail) = leftover_presentation_queue_upgrade_availability(self, command)
                {
                    return Ok(avail);
                }
                if leftover_presentation_can_make_restricted(self, command) {
                    return Ok(CommandAvailability::Restricted);
                }
                if let Some(avail) = leftover_presentation_stop_or_rail(self, command) {
                    return Ok(avail);
                }
                if Self::command_uses_ready_clock(command) {
                    return Ok(self.presentation_typed_availability(command, Some(&entry)));
                }
                if !entry.command_set_name.is_empty() || entry.selectable {
                    return Ok(CommandAvailability::Available);
                }
            }
            if self.portrait_state.is_visible
                || !self.presentation_primary_command_set.is_empty()
                || !self.presentation_command_set_names.is_empty()
            {
                if leftover_presentation_command_set_hidden(self) {
                    return Ok(CommandAvailability::Hidden);
                }
                if let Some(avail) = leftover_presentation_sell_or_subdued(self, command) {
                    return Ok(avail);
                }
                if let Some(hidden) = leftover_buildable_hidden(command, player_id) {
                    return Ok(hidden);
                }
                if leftover_presentation_common_restricted(self, command, None) {
                    return Ok(CommandAvailability::Restricted);
                }
                if leftover_presentation_queue_or_drop_restricted(self, command, None) {
                    return Ok(CommandAvailability::Restricted);
                }
                if let Some(avail) = leftover_presentation_queue_upgrade_availability(self, command)
                {
                    return Ok(avail);
                }
                if leftover_presentation_can_make_restricted(self, command) {
                    return Ok(CommandAvailability::Restricted);
                }
                if let Some(avail) = leftover_presentation_stop_or_rail(self, command) {
                    return Ok(avail);
                }
                if Self::command_uses_ready_clock(command) {
                    return Ok(self.presentation_typed_availability(command, None));
                }
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
        if obj.test_script_status_bit(gamelogic::object::ObjectScriptStatusBit::ScriptDisabled)
            || obj.test_script_status_bit(gamelogic::object::ObjectScriptStatusBit::ScriptUnderpowered)
        {
            return Ok(CommandAvailability::Hidden);
        }
        if obj.is_disabled_by_type(gamelogic::common::types::DisabledType::DisabledUnmanned) {
            return Ok(CommandAvailability::Hidden);
        }
        if obj.has_single_use_command_been_used() {
            return Ok(CommandAvailability::Restricted);
        }
        if (command.options & CommandOption::MustBeStopped as u32) != 0 && obj.is_moving() {
            return Ok(CommandAvailability::Restricted);
        }

        let mut disabled = obj.is_disabled();
        if disabled
            && leftover_ignores_underpowered_clears_disabled(
                command.options,
                obj.get_disabled_flags(),
            )
        {
            disabled = false;
        }
        if disabled && !self.force_disabled_evaluation(command) {
            if !command_evaluable_when_disabled(command.command_type) {
                if self
                    .get_command_availability_forced(command, obj_id, player_id)
                    == CommandAvailability::Hidden
                {
                    return Ok(CommandAvailability::Hidden);
                }
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
                        if !player.has_upgrade_complete(&template) && !obj.has_upgrade(&template) {
                            return Ok(CommandAvailability::Restricted);
                        }
                    }
                }
            }
        }

        let has_production = obj.has_production_in_queue();
        if has_production && (command.options & CommandOption::NotQueueable as u32) != 0 {
            return Ok(CommandAvailability::Restricted);
        }

        let queue_count = leftover_production_count(&obj).unwrap_or(self.build_queue_data.len());
        let queue_maxed = queue_count == MAX_BUILD_QUEUE_BUTTONS;

        match command.command_type {
            CommandType::DozerConstruct => {
                if leftover_buildable_hidden(command, player_id)
                    == Some(CommandAvailability::Hidden)
                {
                    return Ok(CommandAvailability::Hidden);
                }
                if !obj.is_kind_of(KindOf::Dozer) {
                    return Ok(CommandAvailability::Restricted);
                }
                if obj.is_dozer_task_pending() {
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
            CommandType::QueueUnitCreate => {
                if leftover_buildable_hidden(command, player_id)
                    == Some(CommandAvailability::Hidden)
                {
                    return Ok(CommandAvailability::Hidden);
                }
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
                leftover_queue_upgrade_availability(command, &obj, player_id, queue_maxed)
            }
            CommandType::DoStop => Ok(CommandAvailability::Available),
            CommandType::DoGuardPosition | CommandType::DoGuardObject => {
                Ok(CommandAvailability::Available)
            }
            CommandType::Sell => {
                if obj.is_script_unsellable() {
                    return Ok(CommandAvailability::Hidden);
                }
                if obj.is_disabled_by_type(gamelogic::common::types::DisabledType::DisabledSubdued)
                {
                    return Ok(CommandAvailability::Restricted);
                }
                Ok(CommandAvailability::Available)
            }
            CommandType::Evacuate => {
                if !obj.has_contained_objects() {
                    return Ok(CommandAvailability::Restricted);
                }
                if obj.is_disabled_by_type(gamelogic::common::types::DisabledType::DisabledSubdued)
                {
                    return Ok(CommandAvailability::Restricted);
                }
                Ok(CommandAvailability::Available)
            }
            CommandType::Exit => {
                if obj.is_disabled_by_type(gamelogic::common::types::DisabledType::DisabledSubdued)
                {
                    return Ok(CommandAvailability::Restricted);
                }
                Ok(CommandAvailability::Available)
            }
            CommandType::FireWeapon => Ok(self.fire_weapon_availability(&obj, command).0),
            CommandType::DoSpecialPower => Ok(self.special_power_availability(&obj, command).0),
            CommandType::ToggleOvercharge => Ok(self.toggle_overcharge_availability(&obj)),
            CommandType::SwitchWeapons => Ok(self.switch_weapon_availability(&obj, command)),
            CommandType::InternetHack => {
                if leftover_is_hacking_packing_or_unpacking(&obj) {
                    return Ok(CommandAvailability::Restricted);
                }
                Ok(CommandAvailability::Available)
            }
            CommandType::CombatDropAtLocation | CommandType::CombatDropAtObject => {
                if leftover_rappeller_count(&obj) == 0 {
                    return Ok(CommandAvailability::Restricted);
                }
                Ok(CommandAvailability::Available)
            }
            CommandType::MetaSelectMatchingUnits => Ok(CommandAvailability::Available),
            CommandType::PurchaseScience => Ok(CommandAvailability::Available),
            CommandType::ExecuteRailedTransport => Ok(CommandAvailability::Available),
            _ => Ok(CommandAvailability::Available),
        }
    }

    /// Live GameHUD strip: leftover `getCommandAvailability` without WND.
    pub fn leftover_evaluate_named_command(
        &self,
        command_name: &str,
        obj_id: u32,
        player_id: u32,
    ) -> CommandAvailability {
        if command_name.is_empty() {
            return CommandAvailability::Hidden;
        }
        let button = get_ini_control_bar()
            .and_then(|bar| bar.find_command_button_resolved(command_name).cloned())
            .map(|def| Self::command_from_definition(&def))
            .unwrap_or_else(|| CommandButton {
                command_name: command_name.to_string(),
                ..CommandButton::default()
            });
        self.get_command_availability(&button, obj_id, player_id)
            .unwrap_or(CommandAvailability::Hidden)
    }

    fn force_disabled_evaluation(&self, command: &CommandButton) -> bool {
        command_evaluable_when_disabled(command.command_type)
    }

    fn get_command_availability_forced(
        &self,
        command: &CommandButton,
        _obj_id: u32,
        player_id: u32,
    ) -> CommandAvailability {
        leftover_buildable_hidden(command, player_id)
            .unwrap_or(CommandAvailability::Restricted)
    }

    fn command_uses_ready_clock(command: &CommandButton) -> bool {
        matches!(
            command.command_type,
            CommandType::FireWeapon
                | CommandType::DoSpecialPower
                | CommandType::ToggleOvercharge
                | CommandType::SwitchWeapons
        )
    }

    fn presentation_typed_availability(
        &self,
        command: &CommandButton,
        entry: Option<&crate::presentation_translator_residual::TranslatorCatalogEntry>,
    ) -> CommandAvailability {
        leftover_presentation_clock_availability(self, command, entry)
    }

    fn logic_weapon_slot(slot: WeaponSlotType) -> gamelogic::weapon::WeaponSlotType {
        match slot {
            WeaponSlotType::Primary => gamelogic::weapon::WeaponSlotType::Primary,
            WeaponSlotType::Secondary => gamelogic::weapon::WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary => gamelogic::weapon::WeaponSlotType::Tertiary,
        }
    }

    fn fire_weapon_availability(
        &self,
        obj: &gamelogic::object::Object,
        command: &CommandButton,
    ) -> (CommandAvailability, Option<u8>) {
        // C++ ControlBarCommand.cpp:1266-1328 GUI_COMMAND_FIRE_WEAPON.
        if obj.get_ai_update_interface().is_none() {
            return (CommandAvailability::Restricted, None);
        }
        let slot = Self::logic_weapon_slot(command.weapon_slot);
        let weapon = obj.get_weapon_in_weapon_slot(slot);
        if let Some(weapon) = weapon {
            if weapon.get_clip_reload_time(obj.get_id()) == 0 {
                return (CommandAvailability::Available, None);
            }
            let now = TheGameLogic::get_frame();
            let next = weapon.get_possible_next_shot_frame();
            let status = weapon.get_status();
            if status != gamelogic::weapon::WeaponStatus::ReadyToFire
                || next == now
                || next == now.saturating_sub(1)
            {
                let clock = if status == gamelogic::weapon::WeaponStatus::ReloadingClip {
                    Some((weapon.get_percent_ready_to_fire() * 100.0).clamp(0.0, 100.0) as u8)
                } else {
                    None
                };
                return (CommandAvailability::NotReady, clock);
            }
            (CommandAvailability::Available, None)
        } else if (command.options & CommandOption::UsesMineClearingWeaponSet as u32) != 0
            && !obj.test_weapon_set_flag(gamelogic::weapon::WeaponSetType::MineClearingDetail)
        {
            (CommandAvailability::Available, None)
        } else {
            (CommandAvailability::Restricted, None)
        }
    }

    fn special_power_availability(
        &self,
        obj: &gamelogic::object::Object,
        command: &CommandButton,
    ) -> (CommandAvailability, Option<u8>) {
        // C++ ControlBarCommand.cpp:1385-1428 GUI_COMMAND_SPECIAL_POWER*.
        if command.special_power.is_empty() {
            return (CommandAvailability::Restricted, None);
        }
        let Some((ready, percent)) =
            obj.with_special_power_module_interface_by_name(&command.special_power, |sp| {
                (sp.is_ready(), sp.get_percent_ready())
            })
        else {
            // C++ DEBUG_CRASH then falls through to AVAILABLE when the module is missing.
            return (CommandAvailability::Available, None);
        };
        if !ready {
            let clock = Some((percent * 100.0).clamp(0.0, 100.0) as u8);
            return (CommandAvailability::NotReady, clock);
        }
        (CommandAvailability::Available, None)
    }

    fn toggle_overcharge_availability(
        &self,
        obj: &gamelogic::object::Object,
    ) -> CommandAvailability {
        // C++ ControlBarCommand.cpp:1430-1444: ACTIVE while overcharge is on.
        if obj
            .with_overcharge_behavior_interface(|overcharge| overcharge.is_overcharge_active())
            .unwrap_or(false)
        {
            CommandAvailability::Active
        } else {
            CommandAvailability::Available
        }
    }

    fn switch_weapon_availability(
        &self,
        obj: &gamelogic::object::Object,
        command: &CommandButton,
    ) -> CommandAvailability {
        // C++ ControlBarCommand.cpp:1448-1471: missing slot is restricted;
        // ACTIVE when every locally-controlled selected unit already uses the slot.
        let slot = Self::logic_weapon_slot(command.weapon_slot);
        if obj.get_weapon_in_weapon_slot(slot).is_none() {
            return CommandAvailability::Restricted;
        }
        let selected = {
            let Ok(context) = self.context.read() else {
                return CommandAvailability::Active;
            };
            context.selected_objects.clone()
        };
        for selected_id in selected {
            let Some(selected_arc) = OBJECT_REGISTRY.get_object(selected_id) else {
                continue;
            };
            let Ok(selected_obj) = selected_arc.read() else {
                continue;
            };
            if !selected_obj.is_locally_controlled() {
                continue;
            }
            if let Some((_, current_slot)) = selected_obj.get_current_weapon() {
                if current_slot != slot {
                    return CommandAvailability::Available;
                }
            }
        }
        CommandAvailability::Active
    }

    fn command_not_ready_clock(&self, command: &CommandButton, obj_id: u32) -> Option<u8> {
        if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
            if let Ok(obj) = obj_arc.read() {
                return match command.command_type {
                    CommandType::FireWeapon => self.fire_weapon_availability(&obj, command).1,
                    CommandType::DoSpecialPower => {
                        self.special_power_availability(&obj, command).1
                    }
                    _ => None,
                };
            }
        }
        // Live host: OBJECT_REGISTRY is empty. C++ ControlBarCommand.cpp:1404-1407
        // GadgetButtonDrawInverseClock(applyToWin, getPercentReady()*100, color).
        // SpecialPowerModule::getPercentReady is 1.0 when ready, 0.5 half charged —
        // 1.0 - remaining/total, not remaining/total.
        if matches!(command.command_type, CommandType::DoSpecialPower)
            && !self.portrait_state.special_power_ready
        {
            let rem = self.portrait_state.special_power_cooldown_remaining;
            let total = self.portrait_state.special_power_cooldown_total.max(rem);
            if rem > 0.0 && total > 0.0 {
                return Some(((1.0 - rem / total) * 100.0).clamp(0.0, 100.0) as u8);
            }
        }
        None
    }

    fn apply_command_availability_to_windows(
        &self,
        buttons: &[CommandButton],
        availability_by_name: &HashMap<String, (CommandAvailability, Option<u8>)>,
    ) {
        const CLOCK_COLOR: crate::gui::gadgets::Color = crate::gui::gadgets::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 100,
        };
        with_window_manager(|wm| {
            for i in 0..14 {
                let win_name = format!("ControlBar.wnd:ButtonCommand{:02}", i + 1);
                let Some(win) = wm.find_window_by_name(&win_name) else {
                    continue;
                };
                if win.borrow().is_hidden() {
                    continue;
                }
                let command_name = win
                    .borrow()
                    .get_user_data::<String>()
                    .cloned()
                    .or_else(|| buttons.get(i).map(|button| button.command_name.clone()));
                let Some(command_name) = command_name else {
                    continue;
                };
                let Some((availability, clock)) = availability_by_name.get(&command_name) else {
                    continue;
                };
                let options = buttons
                    .iter()
                    .find(|button| button.command_name == command_name)
                    .map(|button| button.options)
                    .unwrap_or(0);
                let mut window = win.borrow_mut();
                let _ = window.clear_status(crate::gui::game_window::WindowStatus::NOT_READY);
                let _ = window.clear_status(crate::gui::game_window::WindowStatus::ALWAYS_COLOR);
                match availability {
                    CommandAvailability::Hidden => {
                        let _ = window.hide(true);
                    }
                    CommandAvailability::Restricted => {
                        let _ = window.enable(false);
                    }
                    CommandAvailability::NotReady => {
                        let _ = window.enable(false);
                        window.set_status(crate::gui::game_window::WindowStatus::NOT_READY);
                        if let Some(percent) = clock {
                            if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) =
                                window.widget_mut()
                            {
                                button.set_inverse_clock(*percent, CLOCK_COLOR);
                            }
                        }
                    }
                    CommandAvailability::CantAfford => {
                        let _ = window.enable(false);
                        window.set_status(crate::gui::game_window::WindowStatus::ALWAYS_COLOR);
                    }
                    CommandAvailability::Available | CommandAvailability::Active => {
                        let _ = window.enable(true);
                    }
                }
                if (options & CommandOption::CheckLike as u32) != 0 {
                    if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) =
                        window.widget_mut()
                    {
                        button.set_checkbox(true, *availability == CommandAvailability::Active);
                    }
                }
            }
        });
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
                    self.leftover_bind_build_queue_windows(context);
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
            self.leftover_bind_build_queue_windows(context);
            return Ok(());
        }

        self.build_queue_data.clear();
        context.construction_queue.clear();

        let Some(obj_arc) = OBJECT_REGISTRY.get_object(producer_id) else {
            self.displayed_queue_count = 0;
            self.leftover_bind_build_queue_windows(context);
            return Ok(());
        };
        let Ok(obj) = obj_arc.read() else {
            self.displayed_queue_count = 0;
            self.leftover_bind_build_queue_windows(context);
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
                            upgrade_name: entry.template_name,
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
        self.leftover_bind_build_queue_windows(context);
        Ok(())
    }

    /// C++ ControlBarCommand.cpp:561-665 — bind ButtonQueueNN cancel cameos.
    fn leftover_bind_build_queue_windows(&self, context: &ControlBarContext) {
        for i in 0..MAX_BUILD_QUEUE_BUTTONS {
            leftover_ensure_named_window(&format!("ControlBar.wnd:ButtonQueue{:02}", i + 1));
        }
        with_window_manager(|wm| {
            for i in 0..MAX_BUILD_QUEUE_BUTTONS {
                let name = format!("ControlBar.wnd:ButtonQueue{:02}", i + 1);
                let Some(win) = wm.find_window_by_name(&name) else {
                    continue;
                };
                {
                    let mut window = win.borrow_mut();
                    let _ = window.enable(false);
                    let _ = window.clear_status(
                        crate::gui::game_window::WindowStatus::USE_OVERLAY_STATES,
                    );
                    if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) =
                        window.widget_mut()
                    {
                        let _ = button.set_text("");
                        button.set_overlay_image(None::<String>);
                    }
                }
                let Some(entry) = self.build_queue_data.get(i) else {
                    continue;
                };
                let cancel_name = match entry.production_type {
                    QueueProductionType::Upgrade => "Command_CancelUpgradeCreate",
                    _ => "Command_CancelUnitCreate",
                };
                let mut cmd = Self::leftover_command_button_by_name(cancel_name)
                    .or_else(|| Self::leftover_lookup_command_button(cancel_name))
                    .unwrap_or_else(|| {
                        let mut fallback = CommandButton::default();
                        fallback.command_name = cancel_name.to_string();
                        fallback.command_type = match entry.production_type {
                            QueueProductionType::Upgrade => CommandType::CancelUpgrade,
                            _ => CommandType::CancelUnitCreate,
                        };
                        fallback
                    });
                cmd.purchase_cost
                    .insert("production_id".to_string(), entry.production_id as i32);
                let image_name = leftover_queue_slot_image(self, context, i);
                if !image_name.is_empty() {
                    cmd.button_image = image_name;
                }
                if entry.production_type != QueueProductionType::Upgrade {
                    cmd.overlay_image = leftover_calculate_veterancy_overlay_for_thing(
                        &leftover_queue_slot_template(self, context, i),
                    );
                }
                let mapped_image = if cmd.button_image.is_empty() {
                    None
                } else {
                    leftover_mapped_image(&cmd.button_image)
                        .or_else(|| wm.win_find_image(&cmd.button_image))
                };
                {
                    let mut window = win.borrow_mut();
                    if let Some(image) = mapped_image {
                        let _ = window.set_enabled_image(0, image);
                        window.set_status(crate::gui::game_window::WindowStatus::IMAGE);
                    }
                    if let Some(crate::gui::game_window::WindowWidget::PushButton(button)) =
                        window.widget_mut()
                    {
                        button.set_overlay_image(cmd.overlay_image.clone());
                    }
                    window.set_user_data(cmd);
                    let _ = window.enable(true);
                    window.set_status(crate::gui::game_window::WindowStatus::USE_OVERLAY_STATES);
                }
            }
        });
    }


    // ---------------------------------------------------------------------------
    // Command processing (click dispatch)
    // C++ ControlBar.cpp:2071-2090
    // ---------------------------------------------------------------------------

    /// C++ ControlBar::processContextSensitiveButtonClick → processCommandUI.
    pub fn process_context_sensitive_button_click(&mut self, control_id: u32, _msg: u32) {
        let command_name = with_window_manager(|wm| {
            wm.get_window_by_id(control_id as crate::gui::WindowId)
                .and_then(|win| {
                    let win = win.borrow();
                    if let Some(cmd) = win.get_user_data::<CommandButton>() {
                        return Some(cmd.command_name.clone());
                    }
                    if let Some(name) = win.get_user_data::<String>() {
                        return Some(name.clone());
                    }
                    win.get_user_data::<IniCommandButton>()
                        .map(|button| button.name.clone())
                })
        });
        let mut queue_slot = None;
        with_window_manager(|wm| {
            for i in 0..MAX_BUILD_QUEUE_BUTTONS {
                let name = format!("ControlBar.wnd:ButtonQueue{:02}", i + 1);
                if let Some(win) = wm.find_window_by_name(&name) {
                    if win.borrow().get_id() as u32 == control_id {
                        queue_slot = Some(i);
                        return;
                    }
                }
            }
        });
        if let Some(i) = queue_slot {
            // C++ processCommand: only cancel a filled m_queueData slot.
            if i < self.build_queue_data.len() {
                if let Ok(ctx) = self.context.read() {
                    let _ = self.cancel_build_queue_item(i, &ctx);
                }
            }
            return;
        }
        if let Some(command_name) = command_name {
            if command_name.is_empty() {
                return;
            }
            let mut slot = None;
            with_window_manager(|wm| {
                for i in 0..14 {
                    let name = format!("ControlBar.wnd:ButtonCommand{:02}", i + 1);
                    if let Some(win) = wm.find_window_by_name(&name) {
                        if win.borrow().get_id() as u32 == control_id {
                            slot = Some(i);
                            return;
                        }
                    }
                }
            });
            let slot_button = self
                .context
                .read()
                .ok()
                .and_then(|ctx| slot.and_then(|i| ctx.available_commands.get(i).cloned()));
            if command_name.eq_ignore_ascii_case("Command_StructureExit")
                || slot_button
                    .as_ref()
                    .is_some_and(|button| button.command_type == CommandType::Exit)
            {
                let occupant = slot.and_then(|i| {
                    self.context
                        .read()
                        .ok()
                        .and_then(|ctx| ctx.contain_data.get(i).copied().flatten())
                });
                if occupant.is_none() {
                    return;
                }
                if let Some(i) = slot {
                    let button = self.context.read().ok().and_then(|ctx| {
                        ctx.available_commands.get(i).cloned()
                    });
                    if let Some(mut button) = button {
                        button.exit_object_id = occupant;
                        let source = CommandSourceType::FromUser;
                        if let Ok(ctx) = self.context.read() {
                            let _ = self.execute_command(&button, source, &ctx);
                        }
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
        } else if let Some(button) = Self::leftover_lookup_command_button(command_name) {
            // C++ processCommandUI uses GadgetButtonGetData (setControlCommand).
            // Purchase-science cameos are not in the 14-slot available_commands.
            self.execute_command(&button, source, &context)?;
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
        // C++ processCommandUI plays UnitSpecificSound before dispatch.
        crate::gui::control_bar::commands::play_command_button_unit_specific_sound(button);

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

/// C++ ControlBarCommand.cpp:1064-1070 — these types stay evaluable while disabled.
fn command_evaluable_when_disabled(cmd_type: CommandType) -> bool {
    matches!(
        cmd_type,
        CommandType::Sell
            | CommandType::Evacuate
            | CommandType::Exit
            | CommandType::RemoveBeacon
            | CommandType::SetRallyPoint
            | CommandType::DoStop
            | CommandType::SwitchWeapons
    )
}

/// C++ ControlBarCommand.cpp:1051-1058 — sole DISABLED_UNDERPOWERED + IGNORES_UNDERPOWERED.
fn leftover_ignores_underpowered_clears_disabled(
    options: u32,
    flags: gamelogic::common::types::DisabledMaskType,
) -> bool {
    (options & CommandOption::IgnoresUnderpowered as u32) != 0
        && flags.test(gamelogic::common::types::DisabledType::DisabledUnderpowered)
        && flags.bits().count_ones() == 1
}

fn leftover_production_count(obj: &gamelogic::object::Object) -> Option<usize> {
    let arc = obj.get_production_update_interface()?;
    let mut guard = arc.lock().ok()?;
    let pu = guard.get_production_update_interface()?;
    Some(pu.get_queue_size())
}


/// C++ `GUI_COMMAND_OBJECT_UPGRADE` vs `PLAYER_UPGRADE`.
fn leftover_is_object_upgrade_command(command: &CommandButton) -> bool {
    if command.gui_command.eq_ignore_ascii_case("OBJECT_UPGRADE") {
        return true;
    }
    if command.gui_command.eq_ignore_ascii_case("PLAYER_UPGRADE") {
        return false;
    }
    with_upgrade_center(|c| {
        c.find_upgrade(command.upgrade.as_str())
            .is_some_and(|template| {
                template.get_upgrade_type() == gamelogic::upgrade::UpgradeType::Object
            })
    })
}

fn leftover_production_update_present(obj: &gamelogic::object::Object) -> bool {
    leftover_production_count(obj).is_some()
}

fn leftover_upgrade_in_queue(obj: &gamelogic::object::Object, upgrade_name: &str) -> bool {
    let Some(arc) = obj.get_production_update_interface() else {
        return false;
    };
    let Ok(mut guard) = arc.lock() else {
        return false;
    };
    let Some(pu) = guard.get_production_update_interface() else {
        return false;
    };
    pu.is_upgrade_in_queue(upgrade_name)
}

/// C++ ControlBarCommand.cpp:1204-1264.
fn leftover_queue_upgrade_availability(
    command: &CommandButton,
    obj: &gamelogic::object::Object,
    player_id: u32,
    queue_maxed: bool,
) -> Result<CommandAvailability, Box<dyn std::error::Error>> {
    if queue_maxed {
        return Ok(CommandAvailability::Restricted);
    }
    let is_object = leftover_is_object_upgrade_command(command);
    if is_object && !leftover_production_update_present(obj) {
        return Ok(CommandAvailability::Restricted);
    }
    let player_arc = logic_player_list()
        .read()
        .ok()
        .and_then(|list| list.get_player(player_id as PlayerIndex).cloned());
    let Some(player_arc) = player_arc else {
        return Ok(CommandAvailability::Available);
    };
    let Ok(player) = player_arc.read() else {
        return Ok(CommandAvailability::Available);
    };
    let Some(template) = with_upgrade_center(|c| c.find_upgrade(command.upgrade.as_str())) else {
        return Ok(CommandAvailability::Restricted);
    };
    if is_object {
        if obj.has_upgrade(&template)
            || leftover_upgrade_in_queue(obj, template.get_name().as_str())
            || !obj.affected_by_upgrade(&template)
        {
            return Ok(CommandAvailability::CantAfford);
        }
    } else if player.has_upgrade_complete(&template) || player.has_upgrade_in_production(&template)
    {
        return Ok(CommandAvailability::CantAfford);
    }
    if !with_upgrade_center(|c| c.can_afford_upgrade(&player, &template, false)) {
        return Ok(CommandAvailability::Restricted);
    }
    for science in &command.sciences_ids {
        if !player.has_science(*science) {
            return Ok(CommandAvailability::Restricted);
        }
    }
    Ok(CommandAvailability::Available)
}

fn leftover_queue_slot_image(
    bar: &ControlBar,
    context: &ControlBarContext,
    index: usize,
) -> String {
    let Some(entry) = bar.build_queue_data.get(index) else {
        return String::new();
    };
    if entry.production_type == QueueProductionType::Upgrade && !entry.upgrade_name.is_empty() {
        return resolve_upgrade_cameo_button_image(
            &entry.upgrade_name,
            Some(&context.available_commands),
        );
    }
    if !entry.upgrade_name.is_empty() {
        return leftover_thing_button_image(&entry.upgrade_name);
    }
    if let Some(item) = context.construction_queue.get(index) {
        if !item.template_name.is_empty() {
            return leftover_thing_button_image(&item.template_name);
        }
    }
    if index == 0 {
        if let Some(tmpl) = bar.portrait_state.production_template.as_deref() {
            return leftover_thing_button_image(tmpl);
        }
    }
    String::new()
}

fn leftover_thing_button_image(template_name: &str) -> String {
    if template_name.is_empty() {
        return String::new();
    }
    let Ok(factory) = game_engine::common::thing::thing_factory::get_thing_factory() else {
        return String::new();
    };
    let Some(factory) = factory.as_ref() else {
        return String::new();
    };
    factory
        .find_template(template_name, false)
        .and_then(|tmpl| tmpl.get_button_image().cloned())
        .map(|image| image.name)
        .filter(|name| !name.is_empty())
        .unwrap_or_default()
}

fn leftover_queue_slot_template(
    bar: &ControlBar,
    context: &ControlBarContext,
    index: usize,
) -> String {
    if let Some(entry) = bar.build_queue_data.get(index) {
        if !entry.upgrade_name.is_empty() {
            return entry.upgrade_name.clone();
        }
    }
    if let Some(item) = context.construction_queue.get(index) {
        if !item.template_name.is_empty() {
            return item.template_name.clone();
        }
    }
    if index == 0 {
        if let Some(tmpl) = bar.portrait_state.production_template.clone() {
            return tmpl;
        }
    }
    String::new()
}

/// C++ ControlBarCommand.cpp:893-947 calculateVeterancyOverlayForThing.
fn leftover_calculate_veterancy_overlay_for_thing(template_name: &str) -> Option<String> {
    if template_name.is_empty() {
        return None;
    }
    let Ok(factory) = game_engine::common::thing::thing_factory::get_thing_factory() else {
        return None;
    };
    let factory = factory.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    let mut level = gamelogic::common::types::VeterancyLevel::Regular;
    let player_has_science = |science: ScienceType| -> bool {
        if science == SCIENCE_INVALID {
            return true;
        }
        logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|arc| arc.read().ok().map(|player| player.has_science(science)))
            .unwrap_or(false)
    };
    for entry in tmpl.get_behavior_module_info().iter() {
        if entry.name.as_str() != "VeterancyGainCreate" {
            continue;
        }
        if let Some(data) = entry
            .data
            .as_any()
            .downcast_ref::<gamelogic::object::create::VeterancyGainCreateModuleData>()
        {
            if player_has_science(data.science_required)
                && (data.starting_level as i32) > (level as i32)
            {
                level = match data.starting_level as i32 {
                    1 => gamelogic::common::types::VeterancyLevel::Veteran,
                    2 => gamelogic::common::types::VeterancyLevel::Elite,
                    3 => gamelogic::common::types::VeterancyLevel::Heroic,
                    _ => level,
                };
            }
            continue;
        }
        let starting = entry
            .data
            .get_ini_field("StartingLevel")
            .unwrap_or("")
            .to_ascii_uppercase();
        let science_name = entry.data.get_ini_field("ScienceRequired").unwrap_or("");
        let science = if science_name.is_empty() {
            SCIENCE_INVALID
        } else {
            get_science_store()
                .map(|store| store.get_science_from_internal_name(science_name))
                .unwrap_or(SCIENCE_INVALID)
        };
        if !player_has_science(science) {
            continue;
        }
        let parsed = match starting.as_str() {
            "VETERAN" | "LEVEL_VETERAN" => gamelogic::common::types::VeterancyLevel::Veteran,
            "ELITE" | "LEVEL_ELITE" => gamelogic::common::types::VeterancyLevel::Elite,
            "HEROIC" | "LEVEL_HEROIC" => gamelogic::common::types::VeterancyLevel::Heroic,
            _ => continue,
        };
        if parsed > level {
            level = parsed;
        }
    }
    match level {
        gamelogic::common::types::VeterancyLevel::Veteran => Some("SSChevron1L".to_string()),
        gamelogic::common::types::VeterancyLevel::Elite => Some("SSChevron2L".to_string()),
        gamelogic::common::types::VeterancyLevel::Heroic => Some("SSChevron3L".to_string()),
        _ => None,
    }
}


fn leftover_rappeller_count(obj: &gamelogic::object::Object) -> usize {
    let Some(contain) = obj.get_contain() else {
        return 0;
    };
    let Ok(guard) = contain.lock() else {
        return 0;
    };
    let ids: Vec<_> = guard.get_contained_objects().to_vec();
    drop(guard);
    ids.into_iter()
        .filter(|&id| {
            let Some(arc) = OBJECT_REGISTRY.get_object(id) else {
                return false;
            };
            let Ok(inner) = arc.read() else {
                return false;
            };
            inner.is_kind_of(KindOf::CanRappel)
        })
        .count()
}

fn leftover_is_hacking_packing_or_unpacking(obj: &gamelogic::object::Object) -> bool {
    let Some(ai) = obj.get_ai() else {
        return false;
    };
    let Ok(mut guard) = ai.lock() else {
        return false;
    };
    let Some(hack) = guard.get_hack_internet_ai_update_interface() else {
        return false;
    };
    hack.is_hacking_packing_or_unpacking()
}

/// C++ ControlBarCommand.cpp:1112-1122 / 1170-1178 — BSTATUS_NO / ONLY_BY_AI hide.
fn leftover_buildable_hidden(command: &CommandButton, player_id: u32) -> Option<CommandAvailability> {
    if !matches!(
        command.command_type,
        CommandType::DozerConstruct | CommandType::QueueUnitCreate
    ) {
        return None;
    }
    if command.object.is_empty() {
        return None;
    }
    let template = TheThingFactory::find_template(command.object.as_str())?;
    let status = template.get_buildable_status()?;
    let hide = match status {
        game_engine::common::thing::BuildableStatus::No => true,
        game_engine::common::thing::BuildableStatus::OnlyByAi => {
            !leftover_player_is_computer(player_id)
        }
        _ => false,
    };
    hide.then_some(CommandAvailability::Hidden)
}

fn leftover_player_is_computer(player_id: u32) -> bool {
    let Some(list) = logic_player_list().read().ok() else {
        return false;
    };
    let Some(arc) = list.get_player(player_id as PlayerIndex).cloned() else {
        return false;
    };
    drop(list);
    let Ok(player) = arc.read() else {
        return false;
    };
    player.get_player_type() == gamelogic::player::PlayerType::Computer
}

fn leftover_presentation_command_set_hidden(bar: &ControlBar) -> bool {
    let residual = &bar.presentation_availability;
    residual.script_disabled || residual.script_unpowered || residual.unmanned
}

fn leftover_presentation_upgrade_matches(list: &[String], upgrade: &str) -> bool {
    if upgrade.is_empty() {
        return false;
    }
    let norm = |s: &str| {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect::<String>()
    };
    let u = norm(upgrade);
    list.iter().any(|owned| {
        let n = norm(owned);
        n == u || n.contains(&u) || u.contains(&n)
    })
}

fn leftover_presentation_upgrade_key(command: &CommandButton) -> String {
    if !command.upgrade.is_empty() {
        return command.upgrade.clone();
    }
    command
        .command_name
        .trim_start_matches("Command_")
        .trim_start_matches("command_")
        .to_string()
}

/// C++ ControlBarCommand.cpp:1204-1264 on the live-host presentation path.
fn leftover_presentation_queue_upgrade_availability(
    bar: &ControlBar,
    command: &CommandButton,
) -> Option<CommandAvailability> {
    if command.command_type != CommandType::QueueUpgrade {
        return None;
    }
    if bar.build_queue_data.len() == MAX_BUILD_QUEUE_BUTTONS {
        return Some(CommandAvailability::Restricted);
    }
    let key = leftover_presentation_upgrade_key(command);
    let residual = &bar.presentation_availability;
    let queued = leftover_presentation_upgrade_matches(&bar.presentation_queued_upgrades, &key);
    if leftover_is_object_upgrade_command(command) {
        let has = leftover_presentation_upgrade_matches(&residual.object_applied_upgrades, &key);
        let unaffected =
            leftover_presentation_upgrade_matches(&residual.object_unaffected_upgrades, &key);
        if has || queued || unaffected {
            return Some(CommandAvailability::CantAfford);
        }
    } else if leftover_presentation_upgrade_matches(&residual.player_completed_upgrades, &key)
        || queued
    {
        return Some(CommandAvailability::CantAfford);
    }
    for science in &command.sciences_ids {
        if *science == SCIENCE_INVALID {
            continue;
        }
        let leftover_has = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|arc| arc.read().ok().map(|player| player.has_science(*science)));
        let has = leftover_has.unwrap_or_else(|| {
            ControlBar::presentation_player_has_required_science(*science)
        });
        if !has {
            return Some(CommandAvailability::Restricted);
        }
    }
    None
}

fn leftover_presentation_queue_or_drop_restricted(
    bar: &ControlBar,
    command: &CommandButton,
    entry: Option<&crate::presentation_translator_residual::TranslatorCatalogEntry>,
) -> bool {
    let queue_maxed = bar.build_queue_data.len() == MAX_BUILD_QUEUE_BUTTONS;
    if queue_maxed
        && matches!(
            command.command_type,
            CommandType::QueueUnitCreate | CommandType::QueueUpgrade
        )
    {
        return true;
    }
    if matches!(
        command.command_type,
        CommandType::CombatDropAtLocation | CommandType::CombatDropAtObject
    ) && entry.is_some_and(|e| e.occupant_count == 0)
    {
        return true;
    }
    false
}

fn leftover_presentation_can_make_status(
    bar: &ControlBar,
    object_or_upgrade: &str,
    command_name: &str,
) -> Option<u32> {
    bar.presentation_can_make.iter().find_map(|(name, status)| {
        if name.eq_ignore_ascii_case(object_or_upgrade) {
            return Some(*status);
        }
        if !command_name.is_empty() {
            let construct = format!("Command_Construct{name}");
            if command_name.eq_ignore_ascii_case(&construct) {
                return Some(*status);
            }
        }
        None
    })
}

/// C++ CanMakeType ordinals (BuildAssistant.h).
const CANMAKE_OK: u32 = 0;

/// C++ ControlBarCommand.cpp:1185-1198 — canBuild / MAXED_OUT / PARKING / NO_MONEY
/// all return COMMAND_RESTRICTED (gray; CANT_AFFORD unused).
fn leftover_presentation_can_make_restricted(bar: &ControlBar, command: &CommandButton) -> bool {
    if !matches!(
        command.command_type,
        CommandType::DozerConstruct | CommandType::QueueUnitCreate | CommandType::QueueUpgrade
    ) {
        return false;
    }
    let key = if !command.object.is_empty() {
        command.object.as_str()
    } else {
        command.upgrade.as_str()
    };
    if let Some(status) = leftover_presentation_can_make_status(bar, key, &command.command_name) {
        if status != CANMAKE_OK {
            return true;
        }
    }
    leftover_presentation_money_restricted(bar, command)
}

fn leftover_presentation_money_restricted(bar: &ControlBar, command: &CommandButton) -> bool {
    let money = bar.last_displayed_money;
    if money < 0 {
        return false;
    }
    for (resource, cost) in &command.purchase_cost {
        if *cost > 0
            && (resource.eq_ignore_ascii_case("cash") || resource.eq_ignore_ascii_case("money"))
            && money < *cost
        {
            return true;
        }
    }
    if command.purchase_cost.is_empty() && !command.object.is_empty() {
        if let Some(template) = TheThingFactory::find_template(command.object.as_str()) {
            let cost = template.get_build_cost() as i32;
            if cost > 0 && money < cost {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod command_availability_window_tests {
    use super::*;

    fn button(command_type: CommandType) -> CommandButton {
        let mut button = CommandButton::default();
        button.command_type = command_type;
        button.command_name = "Command_Test".to_string();
        button
    }

    #[test]
    fn ready_clock_types_match_cpp_special_fire_overcharge_switch() {
        assert!(ControlBar::command_uses_ready_clock(&button(CommandType::FireWeapon)));
        assert!(ControlBar::command_uses_ready_clock(&button(CommandType::DoSpecialPower)));
        assert!(ControlBar::command_uses_ready_clock(&button(CommandType::ToggleOvercharge)));
        assert!(ControlBar::command_uses_ready_clock(&button(CommandType::SwitchWeapons)));
        assert!(!ControlBar::command_uses_ready_clock(&button(CommandType::Sell)));
        assert!(!ControlBar::command_uses_ready_clock(&button(CommandType::DozerConstruct)));
    }

    #[test]
    fn queue_upgrade_gui_command_splits_player_and_object() {
        let mut object = button(CommandType::QueueUpgrade);
        object.gui_command = "OBJECT_UPGRADE".to_string();
        assert!(leftover_is_object_upgrade_command(&object));
        let mut player = button(CommandType::QueueUpgrade);
        player.gui_command = "PLAYER_UPGRADE".to_string();
        assert!(!leftover_is_object_upgrade_command(&player));
    }

    #[test]
    fn disabled_exceptions_include_beacon_delete_and_rally() {
        assert!(command_evaluable_when_disabled(CommandType::Sell));
        assert!(command_evaluable_when_disabled(CommandType::Evacuate));
        assert!(command_evaluable_when_disabled(CommandType::Exit));
        assert!(command_evaluable_when_disabled(CommandType::RemoveBeacon));
        assert!(command_evaluable_when_disabled(CommandType::SetRallyPoint));
        assert!(command_evaluable_when_disabled(CommandType::DoStop));
        assert!(command_evaluable_when_disabled(CommandType::SwitchWeapons));
        assert!(!command_evaluable_when_disabled(CommandType::DoSpecialPower));
        assert!(!command_evaluable_when_disabled(CommandType::QueueUnitCreate));
    }

    #[test]
    fn ignores_underpowered_requires_sole_flag() {
        use gamelogic::common::types::{DisabledMaskType, DisabledType};
        let opts = CommandOption::IgnoresUnderpowered as u32;
        let mut sole = DisabledMaskType::none();
        sole.set_disabled(DisabledType::DisabledUnderpowered);
        assert!(leftover_ignores_underpowered_clears_disabled(opts, sole));
        let mut stacked = sole;
        stacked.set_disabled(DisabledType::DisabledEmp);
        assert!(!leftover_ignores_underpowered_clears_disabled(opts, stacked));
        assert!(!leftover_ignores_underpowered_clears_disabled(0, sole));
    }

    #[test]
    fn presentation_can_make_no_money_and_maxed_are_restricted() {
        let mut bar = ControlBar::new();
        bar.apply_presentation_can_make(&[
            ("AmericaInfantryRanger".to_string(), 2), // CANMAKE_NO_MONEY
            ("AmericaInfantryColonelBurton".to_string(), 6), // CANMAKE_MAXED_OUT
            ("AmericaVehicleDozer".to_string(), 0),
        ]);
        let mut ranger = button(CommandType::QueueUnitCreate);
        ranger.object = "AmericaInfantryRanger".to_string();
        assert!(leftover_presentation_can_make_restricted(&bar, &ranger));
        let mut burton = button(CommandType::QueueUnitCreate);
        burton.object = "AmericaInfantryColonelBurton".to_string();
        assert!(leftover_presentation_can_make_restricted(&bar, &burton));
        let mut dozer = button(CommandType::QueueUnitCreate);
        dozer.object = "AmericaVehicleDozer".to_string();
        assert!(!leftover_presentation_can_make_restricted(&bar, &dozer));
        let sell = button(CommandType::Sell);
        assert!(!leftover_presentation_can_make_restricted(&bar, &sell));
    }

    #[test]
    fn presentation_can_make_matches_construct_command_name() {
        let mut bar = ControlBar::new();
        bar.apply_presentation_can_make(&[("AmericaPowerPlant".to_string(), 2)]);
        let mut btn = button(CommandType::DozerConstruct);
        btn.command_name = "Command_ConstructAmericaPowerPlant".to_string();
        assert!(leftover_presentation_can_make_restricted(&bar, &btn));
        btn.object = "AmericaPowerPlant".to_string();
        assert!(leftover_presentation_can_make_restricted(&bar, &btn));
        bar.apply_presentation_can_make(&[("AmericaPowerPlant".to_string(), 0)]);
        assert!(!leftover_presentation_can_make_restricted(&bar, &btn));
    }


    #[test]
    fn presentation_money_fallback_grays_unaffordable_cameo() {
        let mut bar = ControlBar::new();
        bar.apply_presentation_money(100);
        let mut btn = button(CommandType::QueueUnitCreate);
        btn.object = "AmericaTankCrusader".to_string();
        btn.purchase_cost.insert("Cash".to_string(), 900);
        assert!(leftover_presentation_can_make_restricted(&bar, &btn));
        bar.apply_presentation_money(1000);
        assert!(!leftover_presentation_can_make_restricted(&bar, &btn));
    }

    #[test]
    fn presentation_object_upgrade_owned_or_unaffected_is_cant_afford() {
        let mut bar = ControlBar::new();
        bar.apply_presentation_availability(PresentationAvailabilityResidual {
            object_applied_upgrades: vec!["Upgrade_AmericaRangerFlashBangGrenade".into()],
            object_unaffected_upgrades: vec!["Upgrade_AmericaRangerCaptureBuilding".into()],
            ..Default::default()
        });
        let mut owned = button(CommandType::QueueUpgrade);
        owned.gui_command = "OBJECT_UPGRADE".to_string();
        owned.upgrade = "Upgrade_AmericaRangerFlashBangGrenade".to_string();
        assert_eq!(
            leftover_presentation_queue_upgrade_availability(&bar, &owned),
            Some(CommandAvailability::CantAfford)
        );
        let mut unaff = button(CommandType::QueueUpgrade);
        unaff.gui_command = "OBJECT_UPGRADE".to_string();
        unaff.upgrade = "Upgrade_AmericaRangerCaptureBuilding".to_string();
        assert_eq!(
            leftover_presentation_queue_upgrade_availability(&bar, &unaff),
            Some(CommandAvailability::CantAfford)
        );
        let mut fresh = button(CommandType::QueueUpgrade);
        fresh.gui_command = "OBJECT_UPGRADE".to_string();
        fresh.upgrade = "Upgrade_AmericaTOWMissile".to_string();
        assert_eq!(
            leftover_presentation_queue_upgrade_availability(&bar, &fresh),
            None
        );
    }

    #[test]
    fn presentation_player_upgrade_complete_is_cant_afford() {
        let mut bar = ControlBar::new();
        bar.apply_presentation_availability(PresentationAvailabilityResidual {
            player_completed_upgrades: vec!["Upgrade_AmericaSupplyLines".into()],
            ..Default::default()
        });
        let mut btn = button(CommandType::QueueUpgrade);
        btn.gui_command = "PLAYER_UPGRADE".to_string();
        btn.upgrade = "Upgrade_AmericaSupplyLines".to_string();
        assert_eq!(
            leftover_presentation_queue_upgrade_availability(&bar, &btn),
            Some(CommandAvailability::CantAfford)
        );
    }

    #[test]
    fn presentation_script_disabled_and_unmanned_hide_command_set() {
        let mut bar = ControlBar::new();
        assert!(!leftover_presentation_command_set_hidden(&bar));
        bar.apply_presentation_availability(PresentationAvailabilityResidual {
            script_disabled: true,
            ..Default::default()
        });
        assert!(leftover_presentation_command_set_hidden(&bar));
        bar.apply_presentation_availability(PresentationAvailabilityResidual {
            unmanned: true,
            ..Default::default()
        });
        assert!(leftover_presentation_command_set_hidden(&bar));
    }
}

