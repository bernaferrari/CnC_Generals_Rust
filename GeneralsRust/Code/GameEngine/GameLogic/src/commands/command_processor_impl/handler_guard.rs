impl DefaultCommandHandler {
    fn extract_command_location(&self, command: &QueuedCommand) -> Option<Coord3D> {
        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                if let crate::commands::command::CommandArgumentType::Location(pos) = arg {
                    return Some(*pos);
                }
            }
        }
        None
    }

    fn extract_command_text(&self, command: &QueuedCommand) -> Option<AsciiString> {
        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                if let crate::commands::command::CommandArgumentType::AsciiString(text) = arg {
                    return Some(text.clone());
                }
            }
        }
        None
    }

    fn extract_object_ids(&self, command: &QueuedCommand) -> Vec<ObjectID> {
        let mut ids = Vec::new();
        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                if let crate::commands::command::CommandArgumentType::ObjectID(id) = arg {
                    ids.push(*id);
                }
            }
        }
        ids
    }

    fn extract_target_and_sources(
        &self,
        command: &QueuedCommand,
    ) -> (Option<ObjectID>, Vec<ObjectID>) {
        use crate::common::INVALID_OBJECT_ID;

        let ids = self.extract_object_ids(command);
        if ids.is_empty() {
            return (None, Vec::new());
        }

        let is_selection_marker = |id: ObjectID| id == 0 || id == INVALID_OBJECT_ID;

        if ids.len() >= 2 && is_selection_marker(ids[0]) {
            let target = Some(ids[1]);
            let sources: Vec<ObjectID> = ids.iter().skip(2).copied().collect();
            return (target, sources);
        }

        let target = Some(ids[0]);
        let sources: Vec<ObjectID> = ids.iter().skip(1).copied().collect();
        (target, sources)
    }

    /// Guard a position: move units to the position and hold.
    fn execute_guard_position(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let position = match self.extract_command_location(command) {
            Some(pos) => pos,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No guard position specified",
                ))
            }
        };
        let mut guard_mode = crate::ai::GuardMode::Normal;
        for i in 0..command.command.get_argument_count() {
            if let Some(crate::commands::command::CommandArgumentType::Integer(mode)) =
                command.command.get_argument(i as Int)
            {
                guard_mode = crate::ai::GuardMode::from_i32(*mode);
                break;
            }
        }

        let mut object_ids = self.extract_object_ids(command);
        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }
        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No objects to guard position",
            ));
        }

        // Validate objects are controllable and alive.
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for id in &object_ids {
                    if let Some(obj) = om.get_object(*id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Guard command includes dead object",
                            ));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Player cannot control object for guard command",
                            ));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Guard command object not found",
                        ));
                    }
                }
            }
        }

        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_guard_position_order(&object_ids, position, guard_mode) {
                    return CommandExecutionResult::Success;
                }
            }
        }

        CommandExecutionResult::Failed(AsciiString::from(
            "AI manager unavailable for guard position",
        ))
    }

    /// Guard an object: move units to the target object's position.
    fn execute_guard_object(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut target_id = None;
        let mut guard_mode = crate::ai::GuardMode::Normal;
        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                match arg {
                    crate::commands::command::CommandArgumentType::ObjectID(id) => {
                        if target_id.is_none() {
                            target_id = Some(*id);
                        }
                    }
                    crate::commands::command::CommandArgumentType::Integer(mode) => {
                        guard_mode = crate::ai::GuardMode::from_i32(*mode);
                    }
                    _ => {}
                }
            }
        }
        let target_id = target_id.ok_or_else(|| {
            CommandExecutionResult::Failed(AsciiString::from("No guard target object specified"))
        });
        let target_id = match target_id {
            Ok(id) => id,
            Err(res) => return res,
        };

        let _position = if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                if let Some(obj) = om.get_object(target_id) {
                    obj.get_position()
                } else {
                    return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
                }
            } else {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Cannot access object manager",
                ));
            }
        } else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No object manager available",
            ));
        };

        let mut object_ids = self.extract_object_ids(command);
        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }
        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from("No objects to guard target"));
        }

        // Validate objects the player can control and are alive.
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for id in &object_ids {
                    if let Some(obj) = om.get_object(*id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Guard command includes dead object",
                            ));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Player cannot control object for guard target",
                            ));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Guard command object not found",
                        ));
                    }
                }
            }
        }

        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_guard_object_order(&object_ids, target_id, guard_mode) {
                    return CommandExecutionResult::Success;
                }
            }
        }

        CommandExecutionResult::Failed(AsciiString::from("AI manager unavailable for guard object"))
    }

    /// Capture a structure: issue capture orders to selected units.
    fn execute_capture_building(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let (target, mut object_ids) = self.extract_target_and_sources(command);
        let target = match target {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No target specified for capture",
                ))
            }
        };

        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }

        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No objects specified for capture",
            ));
        }

        let Some(target_arc) = TheGameLogic::find_object_by_id(target) else {
            return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
        };
        let Ok(target_guard) = target_arc.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Target lock failed"));
        };
        if target_guard.is_effectively_dead() {
            return CommandExecutionResult::Failed(AsciiString::from("Target is not alive"));
        }

        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for object_id in &object_ids {
                    if let Some(obj) = om.get_object(*object_id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Object {} is not alive",
                                object_id
                            )));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Player {} cannot control object {}",
                                context.player_id, object_id
                            )));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(&format!(
                            "Object {} not found",
                            object_id
                        )));
                    }
                }
            }
        }

        let mut issued = 0;
        if let Ok(mut factory) = get_object_factory().write() {
            for object_id in &object_ids {
                let Some(GameObjectInstance::Unit(unit)) = factory.get_object_mut(*object_id)
                else {
                    continue;
                };

                let Some(unit_base) = unit.base_object() else {
                    continue;
                };
                let Ok(unit_guard) = unit_base.read() else {
                    continue;
                };

                if !TheActionManager::can_capture_building(
                    &unit_guard,
                    &*target_guard,
                    CommandSourceType::FromPlayer,
                ) {
                    continue;
                }

                {
                    let _ = unit.give_capture_order(target, false);
                    issued += 1;
                }
            }
        }

        if issued > 0 {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("No capture-capable units available"))
        }
    }

    fn execute_hack_special_power_at_object(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
        power_type: crate::common::types::SpecialPowerType,
        can_execute: fn(&crate::object::Object, &crate::object::Object, CommandSourceType) -> bool,
        failure_label: &'static str,
    ) -> CommandExecutionResult {
        use crate::common::INVALID_OBJECT_ID;
        use crate::modules::SpecialPowerCommandOptions;

        let (target_id, mut source_ids) = self.extract_target_and_sources(command);
        let target_id = target_id.filter(|id| *id != INVALID_OBJECT_ID);

        let target_id = match target_id {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(&format!(
                    "No target specified for {}",
                    failure_label
                )));
            }
        };

        if source_ids.is_empty() {
            let selection_manager = get_selection_manager();
            let mut selected_ids = Vec::new();
            if let Ok(manager) = selection_manager.read() {
                if let Some(selection) = manager.get_player_selection_ref(context.player_id) {
                    selected_ids = selection.get_selected_objects();
                }
            }
            source_ids = selected_ids;
        }

        if source_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                "No objects specified for {}",
                failure_label
            )));
        }

        let Some(target_arc) = TheGameLogic::find_object_by_id(target_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
        };
        let Ok(target_guard) = target_arc.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Target lock failed"));
        };
        if target_guard.is_effectively_dead() {
            return CommandExecutionResult::Failed(AsciiString::from("Target is not alive"));
        }

        let mut any_executed = false;
        for source_id in &source_ids {
            let Some(source_arc) = TheGameLogic::find_object_by_id(*source_id) else {
                continue;
            };
            let Ok(source_guard) = source_arc.read() else {
                continue;
            };
            if source_guard.is_effectively_dead() {
                continue;
            }
            let source_owner = source_guard
                .get_controlling_player_id()
                .map(|id| id as Int)
                .unwrap_or(-1);
            if source_owner != -1 && source_owner != context.player_id {
                continue;
            }

            if !can_execute(&source_guard, &target_guard, CommandSourceType::FromPlayer) {
                continue;
            }

            let mut executed_here = false;
            for module_handle in source_guard.behavior_modules() {
                module_handle.with_module(|module| {
                    let Some(sp_module) = module_special_power_interface(module) else {
                        return;
                    };
                    if sp_module.get_power_type() != power_type as u32 {
                        return;
                    }
                    sp_module
                        .do_special_power_at_object(target_id, SpecialPowerCommandOptions::NONE);
                    executed_here = true;
                });
                if executed_here {
                    break;
                }
            }

            if !executed_here {
                for behavior_arc in source_guard.get_behavior_modules() {
                    let Ok(mut behavior_guard) = behavior_arc.lock() else {
                        continue;
                    };
                    let Some(sp_module) = behavior_guard.get_special_power() else {
                        continue;
                    };
                    if sp_module.get_power_type() != power_type as u32 {
                        continue;
                    }
                    sp_module
                        .do_special_power_at_object(target_id, SpecialPowerCommandOptions::NONE);
                    executed_here = true;
                    break;
                }
            }

            if executed_here {
                any_executed = true;
            }
        }

        if any_executed {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from(&format!(
                "No eligible units available for {}",
                failure_label
            )))
        }
    }

    fn execute_snipe_vehicle(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        use crate::common::INVALID_OBJECT_ID;

        let (target_id, mut attacker_ids) = self.extract_target_and_sources(command);
        let target_id = target_id.filter(|id| *id != INVALID_OBJECT_ID);

        let target_id = match target_id {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No target specified for snipe vehicle",
                ));
            }
        };

        if attacker_ids.is_empty() {
            let selection_manager = get_selection_manager();
            let mut selected_ids = Vec::new();
            if let Ok(manager) = selection_manager.read() {
                if let Some(selection) = manager.get_player_selection_ref(context.player_id) {
                    selected_ids = selection.get_selected_objects();
                }
            }
            attacker_ids = selected_ids;
        }

        if attacker_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No attackers specified for snipe vehicle",
            ));
        }

        let Some(target_arc) = TheGameLogic::find_object_by_id(target_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
        };
        let Ok(target_guard) = target_arc.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Target lock failed"));
        };
        if target_guard.is_effectively_dead() {
            return CommandExecutionResult::Failed(AsciiString::from("Target is not alive"));
        }

        let mut eligible_attackers = Vec::new();
        for attacker_id in attacker_ids {
            let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                continue;
            };
            let Ok(attacker_guard) = attacker_arc.read() else {
                continue;
            };
            if attacker_guard.is_effectively_dead() {
                continue;
            }
            let attacker_owner = attacker_guard
                .get_controlling_player_id()
                .map(|id| id as Int)
                .unwrap_or(-1);
            if attacker_owner != -1 && attacker_owner != context.player_id {
                continue;
            }
            if !TheActionManager::can_snipe_vehicle(
                &attacker_guard,
                &target_guard,
                CommandSourceType::FromPlayer,
            ) {
                continue;
            }
            eligible_attackers.push(attacker_id);
        }

        if eligible_attackers.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No attackers can snipe vehicle",
            ));
        }

        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_attack_order(&eligible_attackers, target_id) {
                    return CommandExecutionResult::Success;
                }
            }
        }

        CommandExecutionResult::Failed(AsciiString::from(
            "AI system failed to process snipe vehicle order",
        ))
    }

}
