impl DefaultCommandHandler {
    /// Cheer: triggers cheering model condition on selected group.
    /// Matches C++ MSG_DO_CHEER / AIGroup::groupCheer.
    fn execute_cheer(
        &self,
        _command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|m| {
                m.get_player_selection_ref(context.player_id)
                    .map(|s| s.get_selected_objects())
            })
            .unwrap_or_default();

        if selected.is_empty() {
            return CommandExecutionResult::Success;
        }

        let mut group = crate::ai::group::AIGroup::new(0);
        for object_id in &selected {
            let _ = group.add_by_id(*object_id);
        }

        group.group_cheer(CommandSourceType::FromPlayer);
        CommandExecutionResult::Success
    }

    fn execute_overcharge_toggle(
        &self,
        _command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let selection_manager = get_selection_manager();
        let object_ids = match selection_manager.read() {
            Ok(manager) => manager
                .get_player_selection_ref(context.player_id)
                .map(|selection| selection.get_selected_objects())
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        if object_ids.is_empty() {
            return CommandExecutionResult::Success;
        }

        let mut any_toggled = false;
        for object_id in object_ids {
            let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.write() else {
                continue;
            };
            let mut toggled = false;
            for module_handle in obj_guard.behavior_modules() {
                let matched = module_handle.with_module(|module| {
                    module
                        .get_overcharge_control_interface()
                        .map(|overcharge| {
                            let _ = overcharge.toggle();
                        })
                        .is_some()
                });
                if matched {
                    toggled = true;
                    break;
                }
            }

            if !toggled {
                for behavior in obj_guard.get_behavior_modules() {
                    if let Ok(mut behavior_guard) = behavior.lock() {
                        if let Some(overcharge) = behavior_guard.get_overcharge_behavior_interface()
                        {
                            let _ = overcharge.toggle();
                            toggled = true;
                            break;
                        }
                    }
                }
            }

            if toggled {
                any_toggled = true;
            }
        }

        let _ = any_toggled;
        CommandExecutionResult::Success
    }

    fn execute_switch_weapons(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        // Matches C++ MSG_SWITCH_WEAPONS: lock chosen weapon slot for the current selection.
        let weapon_slot = command.command.get_argument(0).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::Integer(value) => match *value {
                0 => Some(WeaponSlotType::Primary),
                1 => Some(WeaponSlotType::Secondary),
                2 => Some(WeaponSlotType::Tertiary),
                _ => None,
            },
            _ => None,
        });
        let Some(weapon_slot) = weapon_slot else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "SwitchWeapons missing weapon slot",
            ));
        };

        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Selection manager unavailable for SwitchWeapons",
            ));
        };
        let Some(selection) = manager.get_player_selection_ref(context.player_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("No player selection"));
        };
        let selected = selection.get_selected_objects();
        if selected.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from("No selected objects"));
        }

        for object_id in selected {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |guard| {
                if guard.is_destroyed() {
                    return;
                }
                if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id)
                {
                    return;
                }
                guard.set_weapon_lock(weapon_slot, WeaponLockType::LockedPermanently);
            });
        }

        CommandExecutionResult::Success
    }

    fn execute_evacuate_command(
        &self,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        // Mirrors MSG_EVACUATE / AIGroup::groupEvacuate for the current selection.
        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();

        // C++ dispatch unlocks the entire selected group first, then issues
        // evacuation commands.
        for object_id in &selected {
            let _ = OBJECT_REGISTRY.with_object_mut(*object_id, |guard| {
                if guard.is_destroyed() {
                    return;
                }
                guard.release_weapon_lock(WeaponLockType::LockedTemporarily);
            });
        }

        for object_id in selected {
            let Some((ai, is_aircraft, is_airborne_target, position, contain)) = OBJECT_REGISTRY
                .with_object(object_id, |guard| {
                    if guard.is_destroyed() {
                        return None;
                    }

                    let ai = guard.get_ai_update_interface();
                    let is_aircraft = guard.is_kind_of(KindOf::Aircraft);
                    let is_airborne_target = guard.is_airborne_target();
                    let position = *guard.get_position();
                    let contain = if ai.is_none() && guard.is_kind_of(KindOf::Structure) {
                        guard.get_contain()
                    } else {
                        None
                    };
                    Some((ai, is_aircraft, is_airborne_target, position, contain))
                })
                .flatten()
            else {
                continue;
            };

            if let Some(ai) = ai {
                if is_aircraft && is_airborne_target {
                    let mut drop_position = position;
                    if let Some(terrain) = TheTerrainLogic::get() {
                        let layer = terrain.get_highest_layer_for_destination(&drop_position);
                        drop_position.z =
                            terrain.get_layer_height(drop_position.x, drop_position.y, layer);
                    }
                    ai.ai_move_to_and_evacuate(&drop_position, CommandSourceType::FromPlayer);
                } else if let Ok(mut ai_guard) = ai.lock() {
                    let params = crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::Evacuate,
                        CommandSourceType::FromPlayer,
                    );
                    let _ = ai_guard.execute_command(&params);
                }
            } else if let Some(contain) = contain {
                let _ = contain.order_all_passengers_to_exit(CommandSourceType::FromPlayer, false);
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_internet_hack_command(
        &self,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        // Mirrors MSG_INTERNET_HACK / AIGroup::groupHackInternet for the current selection.
        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();

        for object_id in &selected {
            let _ = OBJECT_REGISTRY.with_object_mut(*object_id, |guard| {
                if guard.is_destroyed() {
                    return;
                }
                if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id)
                {
                    return;
                }
                guard.release_weapon_lock(WeaponLockType::LockedTemporarily);
            });
        }

        for object_id in selected {
            let Some(ai) = OBJECT_REGISTRY
                .with_object(object_id, |guard| {
                    if guard.is_destroyed() {
                        return None;
                    }
                    if guard.get_controlling_player_id().map(|id| id as Int)
                        != Some(context.player_id)
                    {
                        return None;
                    }
                    guard.get_ai_update_interface()
                })
                .flatten()
            else {
                continue;
            };

            let ai_lock = ai.lock();
            if let Ok(mut ai_guard) = ai_lock {
                let params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::HackInternet,
                    CommandSourceType::FromPlayer,
                );
                let _ = ai_guard.execute_command(&params);
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_combat_drop_command(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        let mut target_object = None;
        let mut target_position = self.extract_command_location(command);

        if command.command.get_type() == CommandType::CombatDropAtObject {
            target_object = self.extract_object_ids(command).first().copied();
            if let Some(target_id) = target_object {
                target_position = TheGameLogic::find_object_by_id(target_id)
                    .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()));
            }
        }

        let Some(position) = target_position else {
            return CommandExecutionResult::Failed(AsciiString::from("CombatDrop missing target"));
        };

        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();

        for object_id in selected {
            let Some(ai) = OBJECT_REGISTRY
                .with_object(object_id, |guard| {
                    if guard.is_destroyed() {
                        return None;
                    }
                    if guard.get_controlling_player_id().map(|id| id as Int)
                        != Some(context.player_id)
                    {
                        return None;
                    }
                    guard.get_ai_update_interface()
                })
                .flatten()
            else {
                continue;
            };

            let ai_lock = ai.lock();
            if let Ok(mut ai_guard) = ai_lock {
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::CombatDrop,
                    CommandSourceType::FromPlayer,
                );
                params.obj = target_object;
                params.pos = position;
                let _ = ai_guard.execute_command(&params);
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_selected_ai_command(
        &self,
        context: &mut CommandExecutionContext,
        ai_command: crate::ai::AiCommandType,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();

        for object_id in selected {
            let Some(ai) = OBJECT_REGISTRY
                .with_object(object_id, |guard| {
                    if guard.is_destroyed() {
                        return None;
                    }
                    if guard.get_controlling_player_id().map(|id| id as Int)
                        != Some(context.player_id)
                    {
                        return None;
                    }
                    guard.get_ai_update_interface()
                })
                .flatten()
            else {
                continue;
            };

            let ai_lock = ai.lock();
            if let Ok(mut ai_guard) = ai_lock {
                let params =
                    crate::ai::AiCommandParams::new(ai_command, CommandSourceType::FromPlayer);
                let _ = ai_guard.execute_command(&params);
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_exit_command(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        let Some(object_wanting_to_exit) = self.extract_object_ids(command).first().copied() else {
            return CommandExecutionResult::Failed(AsciiString::from("Exit missing object"));
        };

        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();
        let Some(object_containing_exiter) = selected.first().copied() else {
            return CommandExecutionResult::Success;
        };

        if OBJECT_REGISTRY
            .with_object(object_containing_exiter, |_| ())
            .is_none()
        {
            return CommandExecutionResult::Success;
        }

        let Some(ai) = OBJECT_REGISTRY
            .with_object_mut(object_wanting_to_exit, |guard| {
                if guard.is_destroyed() {
                    return None;
                }
                if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id)
                {
                    return None;
                }

                guard.release_weapon_lock(WeaponLockType::LockedTemporarily);
                guard.get_ai_update_interface()
            })
            .flatten()
        else {
            return CommandExecutionResult::Success;
        };

        if let Ok(mut ai_guard) = ai.lock() {
            let mut params = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::Exit,
                CommandSourceType::FromPlayer,
            );
            params.obj = Some(object_containing_exiter);
            let _ = ai_guard.execute_command(&params);
        }

        CommandExecutionResult::Success
    }

    fn execute_queue_upgrade_command(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        use crate::commands::command::CommandArgumentType;

        let upgrade_key = match (
            command.command.get_argument(1),
            command.command.get_argument(0),
        ) {
            (Some(CommandArgumentType::Integer(value)), _) => *value as u32,
            (_, Some(CommandArgumentType::Integer(value))) => *value as u32,
            _ => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "QueueUpgrade missing upgrade key",
                ))
            }
        };

        let upgrade = match get_upgrade_center()
            .read()
            .ok()
            .and_then(|center| center.find_upgrade_by_key(upgrade_key))
        {
            Some(upgrade) => upgrade,
            None => return CommandExecutionResult::Success,
        };

        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();

        for object_id in selected {
            let _ = OBJECT_REGISTRY.with_object(object_id, |guard| {
                if guard.is_destroyed() {
                    return;
                }
                if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id)
                {
                    return;
                }
                let _ = guard.queue_upgrade(&upgrade);
            });
        }

        CommandExecutionResult::Success
    }

    fn execute_cancel_upgrade_command(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        use crate::commands::command::CommandArgumentType;

        let upgrade_key = match command.command.get_argument(0) {
            Some(CommandArgumentType::Integer(value)) => *value as u32,
            _ => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "CancelUpgrade missing upgrade key",
                ))
            }
        };

        let upgrade = match get_upgrade_center()
            .read()
            .ok()
            .and_then(|center| center.find_upgrade_by_key(upgrade_key))
        {
            Some(upgrade) => upgrade,
            None => return CommandExecutionResult::Success,
        };

        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();
        let Some(producer_id) = selected.first().copied() else {
            return CommandExecutionResult::Success;
        };

        let _ = OBJECT_REGISTRY.with_object(producer_id, |guard| {
            if guard.is_destroyed() {
                return;
            }
            if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id) {
                return;
            }
            let _ = guard.cancel_upgrade(&upgrade);
        });

        CommandExecutionResult::Success
    }

    fn execute_queue_unit_create_command(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        use crate::commands::command::CommandArgumentType;

        let template_id = match command.command.get_argument(0) {
            Some(CommandArgumentType::Integer(value)) => *value as u32,
            _ => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "QueueUnitCreate missing template id",
                ))
            }
        };
        let production_id = match command.command.get_argument(1) {
            Some(CommandArgumentType::Integer(value)) => *value as u32,
            _ => 0,
        };

        let Some(template) = TheThingFactory::find_template_by_id(template_id) else {
            return CommandExecutionResult::Success;
        };

        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();
        let Some(producer_id) = selected.first().copied() else {
            return CommandExecutionResult::Success;
        };

        let queued = OBJECT_REGISTRY
            .with_object(producer_id, |guard| {
                if guard.is_destroyed() {
                    return false;
                }
                if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id)
                {
                    return false;
                }
                guard.queue_unit_with_production_id(&template, production_id)
            })
            .unwrap_or(false);
        if queued {
            return CommandExecutionResult::Success;
        }

        CommandExecutionResult::Success
    }

    fn execute_cancel_unit_create_command(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        use crate::commands::command::CommandArgumentType;

        let production_or_template_id = match command.command.get_argument(0) {
            Some(CommandArgumentType::Integer(value)) => *value as u32,
            _ => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "CancelUnitCreate missing production id",
                ))
            }
        };

        let template = TheThingFactory::find_template_by_id(production_or_template_id);

        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();
        let Some(producer_id) = selected.first().copied() else {
            return CommandExecutionResult::Success;
        };

        let _ = OBJECT_REGISTRY.with_object(producer_id, |guard| {
            if guard.is_destroyed() {
                return;
            }
            if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id) {
                return;
            }

            let canceled = template
                .as_ref()
                .is_some_and(|template| guard.cancel_unit_by_template(template));
            if !canceled {
                let _ = guard.cancel_unit_by_production_id(production_or_template_id);
            }
        });

        CommandExecutionResult::Success
    }

    fn execute_dozer_cancel_construct_command(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        use crate::commands::command::CommandArgumentType;

        let target_from_message = match command.command.get_argument(0) {
            Some(CommandArgumentType::ObjectID(object_id)) => Some(*object_id),
            Some(CommandArgumentType::Integer(value)) => Some(*value as ObjectID),
            _ => None,
        };

        let building_id = if let Some(object_id) = target_from_message {
            object_id
        } else {
            let selection_manager = get_selection_manager();
            let selected = selection_manager
                .read()
                .ok()
                .and_then(|manager| {
                    manager
                        .get_player_selection_ref(context.player_id)
                        .map(|selection| selection.get_selected_objects())
                })
                .unwrap_or_default();
            let Some(object_id) = selected.first().copied() else {
                return CommandExecutionResult::Success;
            };
            object_id
        };

        let _ = OBJECT_REGISTRY.with_object_mut(building_id, |guard| {
            if guard.is_destroyed() {
                return;
            }
            if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id) {
                return;
            }
            if !guard.test_status(ObjectStatusTypes::UnderConstruction) {
                return;
            }

            if !guard.test_status(ObjectStatusTypes::Reconstructing) {
                let refund = if let Some(player_arc) = guard.get_controlling_player() {
                    if let Ok(player_guard) = player_arc.read() {
                        guard
                            .get_template()
                            .calc_cost_to_build(Some(&*player_guard))
                    } else {
                        guard.get_template().calc_cost_to_build(None)
                    }
                } else {
                    guard.get_template().calc_cost_to_build(None)
                };

                if refund > 0 {
                    if let Some(player_arc) = guard.get_controlling_player() {
                        if let Ok(mut player) = player_arc.write() {
                            player.get_money_mut().add_money(refund);
                        }
                    }
                }
            }

            guard.kill(None, None);
        });

        CommandExecutionResult::Success
    }

}
