impl DefaultCommandHandler {
    pub fn new() -> Self {
        Self {
            validator: RtsCommandValidator::new(),
            stats: CommandExecutionStats::default(),
            build_plan_active: false,
            build_plan_subjects: Vec::with_capacity(MAX_PATH_SUBJECTS),
        }
    }

    /// Execute movement command
    fn execute_move_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Extract movement parameters
        let mut target_position = None;
        let mut object_ids = Vec::new();

        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                match arg {
                    crate::commands::command::CommandArgumentType::Location(pos) => {
                        if target_position.is_none() {
                            target_position = Some(*pos);
                        }
                    }
                    crate::commands::command::CommandArgumentType::ObjectID(id) => {
                        object_ids.push(*id);
                    }
                    _ => {}
                }
            }
        }

        let position = match target_position {
            Some(pos) => pos,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No target position specified",
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
                "No objects specified for movement",
            ));
        }

        // C++ parity: during build-plan (shift held), queue waypoints instead of executing moves
        if command.command.get_type() == CommandType::DoMoveTo && self.build_plan_active {
            if let Some(ai_manager) = &context.ai_manager {
                if let Ok(mut ai) = ai_manager.write() {
                    for &obj_id in &object_ids {
                        if !self.build_plan_subjects.contains(&obj_id)
                            && self.build_plan_subjects.len() < MAX_PATH_SUBJECTS
                        {
                            self.build_plan_subjects.push(obj_id);
                        }
                        ai.queue_waypoint_for_object(obj_id, position);
                    }
                }
            }
            return CommandExecutionResult::Success;
        }

        // Validate objects exist and are controllable
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

        // Issue move order to AI system
        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                let accepted = match command.command.get_type() {
                    CommandType::AddWaypoint => ai.issue_waypoint_order(&object_ids, position),
                    CommandType::DoAttackMoveTo => {
                        ai.issue_attack_move_order(&object_ids, position)
                    }
                    _ => ai.issue_move_order(&object_ids, position),
                };

                if accepted {
                    CommandExecutionResult::Success
                } else {
                    CommandExecutionResult::Failed(AsciiString::from(
                        "AI system failed to process move order",
                    ))
                }
            } else {
                CommandExecutionResult::Failed(AsciiString::from("Cannot access AI manager"))
            }
        } else {
            CommandExecutionResult::Failed(AsciiString::from("AI manager not available"))
        }
    }

    /// Execute attack command
    fn execute_attack_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut ids = Vec::new();
        for i in 0..command.command.get_argument_count() {
            if let Some(crate::commands::command::CommandArgumentType::ObjectID(id)) =
                command.command.get_argument(i as Int)
            {
                ids.push(*id);
            }
        }

        let target = match ids.first().copied() {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from("No target specified"));
            }
        };

        // C++ attack commands act on the currently selected group and carry only the target id.
        // Allow explicit attacker lists as an override for legacy/test paths.
        let mut attacker_ids: Vec<ObjectID> = ids.iter().skip(1).copied().collect();
        if attacker_ids.is_empty() {
            let selection_manager = get_selection_manager();
            let selected = {
                match selection_manager.read() {
                    Ok(manager) => manager
                        .get_player_selection_ref(context.player_id)
                        .map(|selection| selection.get_selected_objects())
                        .unwrap_or_default(),
                    Err(_) => Vec::new(),
                }
            };
            if !selected.is_empty() {
                attacker_ids = selected;
            }
        }

        if attacker_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from("No attackers specified"));
        }

        // Validate target exists
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                if let Some(target_obj) = om.get_object(target) {
                    if !target_obj.is_alive() {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Target is not alive",
                        ));
                    }
                } else {
                    return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
                }

                // Validate attackers
                for attacker_id in &attacker_ids {
                    if let Some(obj) = om.get_object(*attacker_id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Attacker {} is not alive",
                                attacker_id
                            )));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Player {} cannot control attacker {}",
                                context.player_id, attacker_id
                            )));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(&format!(
                            "Attacker {} not found",
                            attacker_id
                        )));
                    }
                }
            }
        }

        // Issue attack order to AI system
        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_attack_order(&attacker_ids, target) {
                    CommandExecutionResult::Success
                } else {
                    CommandExecutionResult::Failed(AsciiString::from(
                        "AI system failed to process attack order",
                    ))
                }
            } else {
                CommandExecutionResult::Failed(AsciiString::from("Cannot access AI manager"))
            }
        } else {
            CommandExecutionResult::Failed(AsciiString::from("AI manager not available"))
        }
    }

    /// Execute force-attack-ground command (matches C++ MSG_DO_FORCE_ATTACK_GROUND)
    fn execute_force_attack_ground(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        let mut target_position = None;
        for i in 0..command.command.get_argument_count() {
            if let Some(crate::commands::command::CommandArgumentType::Location(pos)) =
                command.command.get_argument(i as Int)
            {
                target_position = Some(*pos);
                break;
            }
        }
        let position = match target_position {
            Some(pos) => pos,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No target position for force-attack-ground",
                ))
            }
        };

        let selected = get_selection_manager()
            .read()
            .ok()
            .and_then(|m| {
                m.get_player_selection_ref(context.player_id)
                    .map(|s| s.get_selected_objects())
            })
            .unwrap_or_default();

        if selected.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No selected units for force-attack-ground",
            ));
        }

        let mut group = crate::ai::group::AIGroup::new(0);
        let mut valid_count = 0;
        for object_id in &selected {
            let controllable = OBJECT_REGISTRY
                .with_object(*object_id, |guard| {
                    !guard.is_destroyed()
                        && guard.get_controlling_player_id() == Some(context.player_id as u32)
                })
                .unwrap_or(false);
            if !controllable {
                continue;
            }
            if group.add_by_id(*object_id).is_ok() {
                valid_count += 1;
            }
        }

        if valid_count == 0 {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No controllable units for force-attack-ground",
            ));
        }

        let not_idle = !group.is_idle();
        if not_idle {
            group.set_weapon_lock_for_group(
                WeaponSlotType::Primary,
                WeaponLockType::LockedTemporarily,
            );
            group.group_attack_position(
                &position,
                NO_MAX_SHOTS_LIMIT,
                CommandSourceType::FromPlayer,
            );
            group.release_weapon_lock_for_group(WeaponLockType::LockedTemporarily);
        } else {
            group.release_weapon_lock_for_group(WeaponLockType::LockedTemporarily);
            group.group_attack_position(
                &position,
                NO_MAX_SHOTS_LIMIT,
                CommandSourceType::FromPlayer,
            );
        }

        CommandExecutionResult::Success
    }

    fn execute_targeted_group_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
        ai_command: crate::ai::AiCommandType,
        failure_label: &'static str,
    ) -> CommandExecutionResult {
        let (target, mut object_ids) = self.extract_target_and_sources(command);
        let target = match target {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(&format!(
                    "No target specified for {}",
                    failure_label
                )))
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
            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                "No objects specified for {}",
                failure_label
            )));
        }

        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                if let Some(target_obj) = om.get_object(target) {
                    if !target_obj.is_alive() {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Target is not alive",
                        ));
                    }
                } else {
                    return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
                }

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

        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_targeted_order(&object_ids, target, ai_command) {
                    CommandExecutionResult::Success
                } else {
                    CommandExecutionResult::Failed(AsciiString::from(&format!(
                        "AI system failed to process {} order",
                        failure_label
                    )))
                }
            } else {
                CommandExecutionResult::Failed(AsciiString::from("Cannot access AI manager"))
            }
        } else {
            CommandExecutionResult::Failed(AsciiString::from("AI manager not available"))
        }
    }

}
