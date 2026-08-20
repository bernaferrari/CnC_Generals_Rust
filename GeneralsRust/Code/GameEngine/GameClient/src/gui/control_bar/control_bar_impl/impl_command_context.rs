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
                if let Some(win) = wm.find_window_by_name("ControlBar.wnd:BuildQueue") {
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
                                    a: 255,
                                },
                            );

                        }
                    }
                }
            });
        } else {
            with_window_manager(|wm| {
                if let Some(win) = wm.find_window_by_name("ControlBar.wnd:BuildQueue") {
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
                if command.command_type == CommandType::Sell && entry.script_unsellable {
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
            && (command.options & CommandOption::IgnoresUnderpowered as u32) != 0
            && obj.is_disabled_by_type(gamelogic::common::types::DisabledType::DisabledUnderpowered)
            && !obj.is_disabled_by_type(gamelogic::common::types::DisabledType::DisabledUnmanned)
            && !obj.is_disabled_by_type(gamelogic::common::types::DisabledType::DisabledSubdued)
        {
            disabled = false;
        }
        if disabled && !self.force_disabled_evaluation(command) {
            let cmd_type = command.command_type;
            if cmd_type != CommandType::Sell
                && cmd_type != CommandType::Evacuate
                && cmd_type != CommandType::Exit
                && cmd_type != CommandType::DoStop
                && cmd_type != CommandType::SwitchWeapons
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

        let queue_count = self.build_queue_data.len();
        let queue_maxed = queue_count >= MAX_BUILD_QUEUE_BUTTONS;

        match command.command_type {
            CommandType::DozerConstruct => {
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
                                || obj.has_upgrade(&template)
                            {
                                return Ok(CommandAvailability::CantAfford);
                            }
                            if !with_upgrade_center(|c| {
                                c.can_afford_upgrade(&player, &template, false)
                            }) {
                                return Ok(CommandAvailability::Restricted);
                            }
                            for science in &command.sciences_ids {
                                if !player.has_science(*science) {
                                    return Ok(CommandAvailability::Restricted);
                                }
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
                if obj.is_moving() {
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

    fn force_disabled_evaluation(&self, command: &CommandButton) -> bool {
        matches!(
            command.command_type,
            CommandType::Sell
                | CommandType::Evacuate
                | CommandType::Exit
                | CommandType::DoStop
                | CommandType::SwitchWeapons
        )
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
        match command.command_type {
            CommandType::DoSpecialPower => {
                if command.special_power.is_empty() {
                    return CommandAvailability::Restricted;
                }
                let ready = entry
                    .map(|e| e.special_power_ready)
                    .unwrap_or(false)
                    || self.portrait_state.special_power_ready;
                if ready {
                    CommandAvailability::Available
                } else {
                    CommandAvailability::NotReady
                }
            }
            CommandType::ToggleOvercharge => CommandAvailability::Available,
            CommandType::FireWeapon | CommandType::SwitchWeapons => {
                CommandAvailability::Restricted
            }
            _ => CommandAvailability::Restricted,
        }
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
        let obj_arc = OBJECT_REGISTRY.get_object(obj_id)?;
        let obj = obj_arc.read().ok()?;
        match command.command_type {
            CommandType::FireWeapon => self.fire_weapon_availability(&obj, command).1,
            CommandType::DoSpecialPower => self.special_power_availability(&obj, command).1,
            _ => None,
        }
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
}

