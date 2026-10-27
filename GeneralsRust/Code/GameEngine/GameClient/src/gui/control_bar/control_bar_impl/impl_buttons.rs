// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

impl ControlBar {
    /// C++ CB_CONTEXT_NONE (ControlBar.cpp:2123-2174) hides CP_COMMAND and
    /// never populates command buttons — the 14 ButtonCommand windows keep
    /// whatever bind they had and stay out of sight with the context parent.
    /// The old port bound the first 12 INI CommandButtons here, which leaked
    /// arbitrary unit cameos over the command grid at match start.
    fn add_default_commands(
        &self,
        _context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn add_object_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if context.selected_objects.is_empty() {
            return Ok(());
        }

        if let Some(actions) = self.receiving_upgrade_actions.as_deref() {
            let name = self.presentation_primary_command_set.trim();
            if let Some(set) = actions.sets.find_command_set_resolved(name).or_else(|| {
                actions
                    .sets
                    .find_command_set_resolved(&name.to_ascii_uppercase())
            }) {
                context.contain_data.clear();
                context.contain_data.resize(14, None);
                context.available_commands = (0..14)
                    .map(|slot| {
                        self.command_from_receiving_slot(
                            actions,
                            self.receiving_command_slot(set, slot),
                        )
                    })
                    .collect();
                super::control_bar_structure_inventory::do_transport_inventory_ui(
                    context,
                    self.presentation_max_garrison,
                    self.presentation_garrisoned_count,
                    &self.presentation_occupants,
                    Some(self.presentation_availability.unmanned),
                )?;
            }
            return Ok(());
        }
        if self.presentation_selected_context.is_some() {
            return Ok(());
        }

        let Some(control_bar) = get_control_bar_bridge() else {
            return Ok(());
        };
        let Some(common_bar) = get_ini_control_bar() else {
            return Ok(());
        };

        let Some(first_id) = context.selected_objects.first().copied() else {
            return Ok(());
        };
        // Prefer dual-world registry command set when bound; otherwise presentation freeze.
        let command_set_name = if let Some(obj_arc) = OBJECT_REGISTRY.get_object(first_id) {
            let Ok(obj_guard) = obj_arc.read() else {
                return Ok(());
            };
            let name = obj_guard.get_command_set_string().to_string();
            if name.is_empty() {
                self.presentation_primary_command_set.clone()
            } else {
                name
            }
        } else {
            // Host/presentation residual — no OBJECT_REGISTRY modules.
            // C++ ControlBar.cpp:3594 binds slots from Object::getCommandSetString
            // (Object.cpp:6084 → template friend_getCommandSetString). Host map
            // objects have no OBJECT_REGISTRY module, so resolve the selected
            // object's authored command set from its presentation catalog entry.
            let catalog_cs =
                crate::presentation_translator_residual::translator_catalog_entry(first_id)
                    .map(|entry| entry.command_set_name)
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| self.presentation_primary_command_set.clone());
            catalog_cs
        };
        if command_set_name.is_empty() {
            return Ok(());
        }
        let command_set = control_bar
            .find_command_set_by_name(&command_set_name)
            .or_else(|| {
                control_bar.find_command_set_by_name(&command_set_name.to_ascii_uppercase())
            });
        let Some(command_set) = command_set else {
            return Ok(());
        };
        const WND_SLOTS: usize = 14;
        context.available_commands.clear();
        context.contain_data.clear();
        context.contain_data.resize(WND_SLOTS, None);
        for slot in 0..WND_SLOTS {
            let button_opt = command_set.buttons.get(slot).and_then(|b| b.as_ref());
            let Some(button) = button_opt else {
                context.available_commands.push(Self::hidden_slot_button());
                continue;
            };
            if (button.get_options_bits() & CommandOption::ScriptOnly as u32) != 0 {
                context.available_commands.push(Self::hidden_slot_button());
                continue;
            }
            let mut cmd = if let Some(common_button) =
                common_bar.find_command_button_resolved(button.get_name())
            {
                Self::command_from_definition(common_button)
            } else {
                Self::command_from_logic_button(button)
            };
            self.apply_need_special_power_science(&mut cmd, button);
            context.available_commands.push(cmd);
        }

        super::control_bar_structure_inventory::do_transport_inventory_ui(
            context,
            self.presentation_max_garrison,
            self.presentation_garrisoned_count,
            &self.presentation_occupants,
            None,
        )?;
        super::control_bar_beacon::append_beacon_commands_with_presentation(
            context,
            &self.presentation_primary_command_set,
        )?;
        Ok(())
    }

    fn add_multi_select_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(commands) =
            self.receiving_multi_selection_commands(&self.presentation_command_set_names)
        {
            context.available_commands = commands;
            return Ok(());
        }
        if self.presentation_selected_context.is_some() {
            return Ok(());
        }
        // Presentation residual first (host path has no dual-world registry).
        let mut presentation_names = self.presentation_command_set_names.clone();
        // Wave 1017: dual-world multi-select seeds command-set names from translator catalog.
        if Self::dual_world_registry_unavailable()
            && presentation_names.len() < 2
            && context.selected_objects.len() >= 2
        {
            for obj_id in &context.selected_objects {
                if let Some(entry) =
                    crate::presentation_translator_residual::translator_catalog_entry(*obj_id)
                {
                    // Wave 1076: unusable multi-select residual fail-closed.
                    if entry.destroyed
                        || entry.sold
                        || entry.disabled
                        || entry.unselectable
                        || entry.masked
                    {
                        continue;
                    }
                    if entry.command_set_name.is_empty() {
                        continue;
                    }
                    if !presentation_names
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&entry.command_set_name))
                    {
                        presentation_names.push(entry.command_set_name.clone());
                    }
                }
            }
        }
        if presentation_names.len() >= 2 {
            super::control_bar_multi_select::populate_multi_select_commands_from_sets(
                self,
                context,
                &presentation_names,
            )?;
        }
        if context.available_commands.is_empty() {
            // Dual-world residual: OBJECT_REGISTRY intersection.
            super::control_bar_multi_select::populate_multi_select_commands(self, context)?;
        }
        // C++ populateMultiSelect never falls back to a single unit's full set.
        Ok(())
    }

    /// Intersect the selected objects' sets using this bar's definition owner.
    fn receiving_multi_selection_commands(&self, names: &[String]) -> Option<Vec<CommandButton>> {
        let actions = self.receiving_upgrade_actions.as_deref()?;
        let mut common = vec![None::<String>; gamelogic::command_button::MAX_COMMANDS_PER_SET];
        let mut saw_first = false;
        for name in names {
            let name = name.trim();
            let set = (!name.is_empty())
                .then(|| {
                    actions.sets.find_command_set_resolved(name).or_else(|| {
                        actions
                            .sets
                            .find_command_set_resolved(&name.to_ascii_uppercase())
                    })
                })
                .flatten();
            let Some(set) = set else {
                common.fill(None);
                saw_first = true;
                break;
            };
            for (slot, shared) in common.iter_mut().enumerate() {
                let candidate = self.receiving_command_slot(set, slot);
                let definition =
                    candidate.and_then(|name| actions.buttons.find_command_button_resolved(name));
                if !saw_first {
                    if definition.is_some_and(|button| {
                        (button.options_bits & CommandOption::OkForMultiSelect as u32) != 0
                    }) {
                        *shared = candidate.map(str::to_owned);
                    }
                    continue;
                }
                let is_attack_move = |name: Option<&str>| {
                    name.and_then(|name| actions.buttons.find_command_button_resolved(name))
                        .is_some_and(|button| {
                            map_gui_command_to_command_type(&button.command)
                                == CommandType::DoAttackMoveTo
                        })
                };
                if is_attack_move(candidate) || is_attack_move(shared.as_deref()) {
                    if shared.is_none() {
                        *shared = candidate.map(str::to_owned);
                    }
                } else if candidate != shared.as_deref() {
                    *shared = None;
                }
            }
            saw_first = true;
        }
        Some(if saw_first {
            common
                .iter()
                .take(14)
                .map(|name| self.command_from_receiving_slot(actions, name.as_deref()))
                .collect()
        } else {
            Vec::new()
        })
    }

    fn receiving_command_slot<'a>(
        &'a self,
        set: &'a game_engine::common::ini::ini_command_set::CommandSet,
        slot: usize,
    ) -> Option<&'a str> {
        let key = format!(
            "{}{}",
            char::from_u32('0' as u32 + u32::try_from(slot).ok()?)?,
            set.name
        );
        match self.presentation_command_overrides.get(&key) {
            Some(override_button) => override_button.as_deref(),
            None => set.get_button_at_position(slot).map(String::as_str),
        }
    }

    fn add_observer_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        super::control_bar_observer::populate_observer_commands(context)?;
        Ok(())
    }

    fn add_construction_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.presentation_selected_context.is_some() {
            if let Some(definition) =
                self.receiving_upgrade_actions
                    .as_deref()
                    .and_then(|actions| {
                        actions
                            .buttons
                            .find_command_button_resolved("Command_CancelConstruction")
                    })
            {
                Self::push_command_if_missing(context, Self::command_from_definition(definition));
            }
        } else {
            super::control_bar_under_construction::populate_under_construction_commands(context)?;
        }
        Ok(())
    }

    fn add_structure_inventory_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        super::control_bar_structure_inventory::append_structure_inventory_commands_with_presentation(
            context,
            self.presentation_max_garrison,
            self.presentation_garrisoned_count,
            &self.presentation_occupants,
            self.receiving_upgrade_actions.as_deref(),
            self.presentation_selected_context.is_some(),
        )?;
        Ok(())
    }

    fn add_beacon_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(facts) = self.presentation_selected_context {
            if facts.controllable && facts.beacon {
                if let Some(definition) =
                    self.receiving_upgrade_actions
                        .as_deref()
                        .and_then(|actions| {
                            actions
                                .buttons
                                .find_command_button_resolved("Command_BeaconDelete")
                        })
                {
                    Self::push_command_if_missing(
                        context,
                        Self::command_from_definition(definition),
                    );
                }
            }
            return Ok(());
        }
        super::control_bar_beacon::append_beacon_commands_with_presentation(
            context,
            &self.presentation_primary_command_set,
        )?;
        Ok(())
    }

    fn add_ocl_timer_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(facts) = self.presentation_selected_context {
            let name = match facts.ocl_timer_kind {
                Some(super::control_bar_ocl_timer::OclTimerButtonKind::Sell) => "Command_Sell",
                Some(super::control_bar_ocl_timer::OclTimerButtonKind::RallyPoint) => {
                    "Command_SetRallyPoint"
                }
                _ => return Ok(()),
            };
            if let Some(definition) = self
                .receiving_upgrade_actions
                .as_deref()
                .and_then(|actions| actions.buttons.find_command_button_resolved(name))
            {
                Self::push_command_if_missing(context, Self::command_from_definition(definition));
            }
            return Ok(());
        }
        // C++ ControlBarOCLTimer.cpp:55 populateOCLTimer:
        // Adds Command_Sell for non-tech buildings, Command_SetRallyPoint for
        // tech buildings with AUTO_RALLYPOINT, or hides the button.
        // Delegates to the OCL timer module for command population.
        super::control_bar_ocl_timer::populate_ocl_timer_commands(context)
    }

    // ---------------------------------------------------------------------------
    // Utility / conversion helpers
    // ---------------------------------------------------------------------------

    pub(super) fn command_from_definition(definition: &IniCommandButton) -> CommandButton {
        let mut button = CommandButton::default();

        // `name` is the retail CommandButton identity (for example,
        // `Command_SetDemoTrapManualDetonation`); `command` is only the C++
        // operation shared by many distinct buttons (`SWITCH_WEAPON`).  Keep
        // both facts: callers need the identity for exact host translation,
        // while command_type must be parsed from the operation.
        let command_identity = if definition.name.trim().is_empty() {
            &definition.command
        } else {
            &definition.name
        };
        let command_operation = if definition.command.trim().is_empty() {
            command_identity
        } else {
            &definition.command
        };
        button.command_name = command_identity.clone();
        button.command_type = map_gui_command_to_command_type(command_operation);
        button.gui_command = command_operation.to_string();

        button.button_image = definition.button_image.clone();
        button.button_border_type = definition.button_border_type.clone();
        button.text_label = definition.text_label.clone();
        button.text_label_placehold = definition.text_label.clone();
        button.descriptive_text = definition.descriptive_text.clone();
        button.conflicting_element = definition.conflicting_element.clone();
        button.cursor_name = definition.cursor_name.clone();
        button.invalid_cursor_name = definition.invalid_cursor_name.clone();
        button.unit_specific_sound = definition.unit_specific_sound.clone();
        button.sciences = definition.science_required.clone();
        button.sciences_ids = definition.parsed_science_required.clone();
        button.options = definition.options_bits;
        button.object = definition.object.clone();
        button.upgrade = definition.upgrade.clone();
        button.special_power = definition
            .special_power_template
            .clone()
            .unwrap_or_default();
        button.radius_cursor_type = definition.radius_cursor_type.clone();
        button.max_shots_to_fire = definition.max_shots_to_fire;
        button.weapon_slot = definition.weapon_slot;

        if definition.purchase_cost != 0 {
            button
                .purchase_cost
                .insert("Cash".to_string(), definition.purchase_cost);
        }

        button
    }

    pub(super) fn command_from_logic_button(
        logic_button: &gamelogic::command_button::CommandButton,
    ) -> CommandButton {
        let mut button = CommandButton::default();
        button.command_name = logic_button.get_name().to_string();
        button.command_type = logic_button.get_command_type();
        button.gui_command = logic_button.get_gui_command().to_string();
        button.text_label = logic_button.get_name().to_string();
        button.descriptive_text = logic_button.tooltip.clone();
        button.options = logic_button.get_options_bits();
        button.sciences_ids = logic_button.science_vec().to_vec();
        button.max_shots_to_fire = logic_button.get_max_shots_to_fire();
        button.weapon_slot = match logic_button.get_weapon_slot() {
            gamelogic::weapon::WeaponSlotType::Primary => WeaponSlotType::Primary,
            gamelogic::weapon::WeaponSlotType::Secondary => WeaponSlotType::Secondary,
            gamelogic::weapon::WeaponSlotType::Tertiary => WeaponSlotType::Tertiary,
        };
        if let Some(template) = logic_button.get_thing_template() {
            button.object = template.get_name().as_str().to_string();
        }
        if let Some(upgrade) = logic_button.get_upgrade_template() {
            button.upgrade = upgrade.get_name().as_str().to_string();
        }
        if let Some(sp) = logic_button.get_special_power_template() {
            button.special_power = sp.get_name().to_string();
        }
        button
    }

    pub(super) fn hidden_slot_button() -> CommandButton {
        CommandButton {
            button_hidden: true,
            button_enabled: false,
            ..CommandButton::default()
        }
    }

    pub(super) fn command_from_set_slot(
        &self,
        common_bar: &game_engine::common::ini::ini_command_button::ControlBar,
        button: Option<&gamelogic::command_button::CommandButton>,
    ) -> CommandButton {
        let Some(button) = button else {
            return Self::hidden_slot_button();
        };
        if (button.get_options_bits() & CommandOption::ScriptOnly as u32) != 0 {
            return Self::hidden_slot_button();
        }
        let mut cmd = if let Some(common_button) =
            common_bar.find_command_button_resolved(button.get_name())
        {
            Self::command_from_definition(common_button)
        } else {
            Self::command_from_logic_button(button)
        };
        self.apply_need_special_power_science(&mut cmd, button);
        cmd
    }

    /// Populate a native command directly from its receiving action catalog.
    pub(super) fn command_from_receiving_slot(
        &self,
        actions: &game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions,
        name: Option<&str>,
    ) -> CommandButton {
        let Some(definition) =
            name.and_then(|name| actions.buttons.find_command_button_resolved(name))
        else {
            return Self::hidden_slot_button();
        };
        if (definition.options_bits & CommandOption::ScriptOnly as u32) != 0 {
            return Self::hidden_slot_button();
        }
        let mut command = Self::command_from_definition(definition);
        self.apply_receiving_special_power_science(&mut command, actions);
        command
    }

    /// C++ `ControlBar::populateCommand` (`ControlBarCommand.cpp:329-343`):
    /// hide NEED_SPECIAL_POWER_SCIENCE only when
    /// `power->getRequiredScience() != SCIENCE_INVALID` and the controlling player
    /// does not own that science. ScienceVec is never a hide source.
    fn hide_need_special_power_science(
        required_from_power: i32,
        player_has_required: bool,
    ) -> bool {
        required_from_power != SCIENCE_INVALID && !player_has_required
    }

    /// Live host leftover PlayerList is empty; consult stamped player sciences.
    fn presentation_player_has_required_science(&self, required: ScienceType) -> bool {
        if required == SCIENCE_INVALID {
            return true;
        }
        if let Some(ids) = &self.presentation_science_ids {
            return ids.contains(&required);
        }
        let names = &self.presentation_unlocked_sciences;
        if let Some(store) = get_science_store() {
            let name = store.get_internal_name_for_science(required);
            let name = name.as_str();
            if !name.is_empty() {
                return names.iter().any(|owned| owned.eq_ignore_ascii_case(name));
            }
        }
        false
    }

    fn selected_player_has_required_science(&self, required: ScienceType) -> bool {
        self.presentation_selected_player.as_ref().map_or_else(
            || self.presentation_player_has_required_science(required),
            |owner| required == SCIENCE_INVALID || owner.science_ids.contains(&required),
        )
    }

    /// PopulateCommand uses the selected object's controller for both the
    /// RequiredScience gate and its best contiguous purchased-rank art.
    fn apply_receiving_special_power_science(
        &self,
        command: &mut CommandButton,
        actions: &game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions,
    ) {
        if command.options & CommandOption::NeedSpecialPowerScience as u32 == 0
            || matches!(
                command.command_type,
                CommandType::PurchaseScience | CommandType::QueueUpgrade
            )
        {
            return;
        }
        let required = actions
            .powers
            .find_template(&command.special_power)
            .map(|power| power.required_science)
            .unwrap_or(SCIENCE_INVALID);
        if required == SCIENCE_INVALID {
            return;
        }
        if Self::hide_need_special_power_science(
            required,
            self.selected_player_has_required_science(required),
        ) {
            command.button_hidden = true;
            return;
        }
        let Some(owner) = &self.presentation_selected_player else {
            return;
        };
        let Some(science) = command
            .sciences_ids
            .iter()
            .copied()
            .take_while(|science| owner.science_ids.contains(science))
            .last()
        else {
            return;
        };
        let names = &owner.purchase_command_sets;
        if names.iter().any(String::is_empty) {
            return;
        }
        let find_set = |name: &str| {
            actions.sets.find_command_set_resolved(name).or_else(|| {
                actions
                    .sets
                    .find_command_set_resolved(&name.to_ascii_uppercase())
            })
        };
        let (Some(rank1), Some(rank3), Some(rank8)) = (
            find_set(&names[0]),
            find_set(&names[1]),
            find_set(&names[2]),
        ) else {
            return;
        };
        for (set, limit) in [rank1, rank3, rank8].into_iter().zip([
            MAX_PURCHASE_SCIENCE_RANK_1,
            MAX_PURCHASE_SCIENCE_RANK_3,
            MAX_PURCHASE_SCIENCE_RANK_8,
        ]) {
            for name in (0..limit).filter_map(|slot| self.receiving_command_slot(set, slot)) {
                let Some(purchase) = actions.buttons.find_command_button_resolved(name) else {
                    continue;
                };
                if map_gui_command_to_command_type(&purchase.command)
                    != CommandType::PurchaseScience
                    || purchase.parsed_science_required.first().copied() != Some(science)
                {
                    continue;
                }
                command.button_image.clone_from(&purchase.button_image);
                if !purchase.text_label.is_empty() {
                    command.text_label.clone_from(&purchase.text_label);
                }
                if !purchase.descriptive_text.is_empty() {
                    command
                        .descriptive_text
                        .clone_from(&purchase.descriptive_text);
                }
                return;
            }
        }
    }

    /// C++ populateCommand NEED_SPECIAL_POWER_SCIENCE hide + copyImagesFrom rank 1/3/8.
    fn apply_need_special_power_science(
        &self,
        cmd: &mut CommandButton,
        logic_button: &gamelogic::command_button::CommandButton,
    ) {
        if let Some(actions) = &self.receiving_upgrade_actions {
            self.apply_receiving_special_power_science(cmd, actions);
            return;
        }
        if (cmd.options & CommandOption::NeedSpecialPowerScience as u32) == 0 {
            return;
        }
        if matches!(
            cmd.command_type,
            CommandType::PurchaseScience | CommandType::QueueUpgrade
        ) {
            return;
        }
        let required = logic_button
            .get_special_power_template()
            .map(|sp| sp.get_required_science())
            .unwrap_or(SCIENCE_INVALID);
        let leftover_player = if self.presentation_selected_player.is_some()
            || self.presentation_science_ids.is_some()
        {
            None
        } else {
            logic_player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
        };
        let leftover_has = leftover_player
            .as_ref()
            .and_then(|arc| arc.read().ok().map(|player| player.has_science(required)));
        let player_has =
            leftover_has.unwrap_or_else(|| self.selected_player_has_required_science(required));
        if Self::hide_need_special_power_science(required, player_has) {
            cmd.button_hidden = true;
            return;
        }
        let Some(player_arc) = leftover_player else {
            return;
        };
        let Ok(player) = player_arc.read() else {
            return;
        };
        let mut best_index: Option<usize> = None;
        for (i, science) in cmd.sciences_ids.iter().copied().enumerate() {
            if player.has_science(science) {
                best_index = Some(i);
            } else {
                break;
            }
        }
        let Some(best_index) = best_index else {
            return;
        };
        let science = cmd.sciences_ids[best_index];
        let Some(template) = player.get_player_template() else {
            return;
        };
        let Some(control_bar) = get_control_bar_bridge() else {
            return;
        };
        let Some(common_bar) = get_ini_control_bar() else {
            return;
        };
        let names = [
            template
                .get_purchase_science_command_set_rank1()
                .to_string(),
            template
                .get_purchase_science_command_set_rank3()
                .to_string(),
            template
                .get_purchase_science_command_set_rank8()
                .to_string(),
        ];
        if names.iter().any(|n| n.is_empty()) {
            return;
        }
        let limits = [
            MAX_PURCHASE_SCIENCE_RANK_1,
            MAX_PURCHASE_SCIENCE_RANK_3,
            MAX_PURCHASE_SCIENCE_RANK_8,
        ];
        for (name, limit) in names.iter().zip(limits) {
            let Some(set) = control_bar
                .find_command_set_by_name(name)
                .or_else(|| control_bar.find_command_set_by_name(&name.to_ascii_uppercase()))
            else {
                return;
            };
            for slot in 0..limit {
                let Some(purchase) = set.buttons.get(slot).and_then(|b| b.as_ref()) else {
                    continue;
                };
                if purchase.get_command_type() != CommandType::PurchaseScience {
                    continue;
                }
                let purchase_science = purchase.science_vec().first().copied();
                if purchase_science != Some(science) {
                    continue;
                }
                if let Some(def) = common_bar.find_command_button_resolved(purchase.get_name()) {
                    if !def.button_image.is_empty() {
                        cmd.button_image = def.button_image.clone();
                    }
                    if !def.text_label.is_empty() {
                        cmd.text_label = def.text_label.clone();
                    }
                    if !def.descriptive_text.is_empty() {
                        cmd.descriptive_text = def.descriptive_text.clone();
                    }
                }
                return;
            }
        }
    }

    pub(super) fn push_command_if_missing(context: &mut ControlBarContext, button: CommandButton) {
        if context.available_commands.iter().any(|existing| {
            existing
                .command_name
                .eq_ignore_ascii_case(&button.command_name)
        }) {
            return;
        }
        context.available_commands.push(button);
    }

    fn resolve_command_button_id(
        &self,
        button: &CommandButton,
    ) -> Result<gamelogic::command_button::CommandButtonId, Box<dyn std::error::Error>> {
        let Some(control_bar) = get_control_bar_bridge() else {
            return Err("Control bar bridge not initialized".into());
        };
        let Some(logic_button) = control_bar.find_command_button_by_name(&button.command_name)
        else {
            return Err(format!(
                "Command button '{}' not found in GameLogic bridge",
                button.command_name
            )
            .into());
        };
        Ok(logic_button.get_id())
    }

    fn map_command_source(source: CommandSourceType) -> gamelogic::common::CommandSourceType {
        match source {
            CommandSourceType::FromUser => gamelogic::common::CommandSourceType::FromPlayer,
            CommandSourceType::FromScript => gamelogic::common::CommandSourceType::FromScript,
            CommandSourceType::FromAI => gamelogic::common::CommandSourceType::FromAi,
            CommandSourceType::None => gamelogic::common::CommandSourceType::FromPlayer,
        }
    }

    fn queue_command(
        &self,
        player_id: i32,
        command: Command,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let current_frame = TheGameLogic::get_frame();
        let queued = QueuedCommand::new(command, CommandPriority::Normal, current_frame);

        let queue_manager = get_command_queue_manager();
        let mut manager = queue_manager
            .lock()
            .map_err(|_| "Failed to lock command queue manager")?;

        if let Err(err) = manager.queue_player_command(player_id, queued.clone()) {
            if let Err(init_err) = manager.initialize_player(player_id) {
                return Err(format!(
                    "Failed to initialize player {} for command queue: {}",
                    player_id, init_err
                )
                .into());
            }
            if let Err(queue_err) = manager.queue_player_command(player_id, queued) {
                return Err(format!(
                    "Failed to queue command for player {}: {}",
                    player_id, queue_err
                )
                .into());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod need_special_power_science_hide_tests {
    use super::*;

    #[test]
    fn need_special_power_science_hides_only_on_power_required_science() {
        // C++ ControlBarCommand.cpp:333-343 — ScienceVec[0] is never a hide gate.
        assert!(
            !ControlBar::hide_need_special_power_science(SCIENCE_INVALID, false),
            "SCIENCE_INVALID RequiredScience must not hide even if ScienceVec is unowned"
        );
        assert!(
            ControlBar::hide_need_special_power_science(42, false),
            "unowned RequiredScience must hide"
        );
        assert!(
            !ControlBar::hide_need_special_power_science(42, true),
            "owned RequiredScience must not hide"
        );
    }
}
