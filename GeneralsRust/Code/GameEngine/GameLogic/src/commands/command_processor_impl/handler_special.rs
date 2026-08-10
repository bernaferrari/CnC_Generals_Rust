impl DefaultCommandHandler {
    /// Execute a special power command with basic validation against power registry and targets.
    fn execute_special_power(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        use crate::commands::command::CommandArgumentType;
        use crate::common::INVALID_OBJECT_ID;
        use crate::modules::SpecialPowerCommandOptions;
        use crate::object_creation_list::nuggets::INVALID_ANGLE;

        let cmd_type = command.command.get_type();
        let mut power_id: Option<u32> = None;
        let mut command_options = SpecialPowerCommandOptions::NONE;
        let mut target_location: Option<Coord3D> = None;
        let mut target_object: Option<ObjectID> = None;
        let mut source_object: Option<ObjectID> = None;
        let mut angle: f32 = INVALID_ANGLE;
        let mut object_in_way: Option<ObjectID> = None;
        let mut override_power_type: Option<u32> = None;

        let arg_count = command.command.get_argument_count();
        let arg_at = |idx: Int| command.command.get_argument(idx);

        match cmd_type {
            CommandType::DoSpecialPower => {
                if let Some(CommandArgumentType::Integer(id)) = arg_at(0) {
                    power_id = Some(*id as u32);
                }
                if let Some(CommandArgumentType::Integer(options)) = arg_at(1) {
                    command_options =
                        SpecialPowerCommandOptions::from_bits_truncate(*options as u32);
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(2) {
                    if *id != INVALID_OBJECT_ID {
                        source_object = Some(*id);
                    }
                }
            }
            CommandType::DoSpecialPowerAtLocation => {
                if let Some(CommandArgumentType::Integer(id)) = arg_at(0) {
                    power_id = Some(*id as u32);
                }
                if let Some(CommandArgumentType::Location(pos)) = arg_at(1) {
                    target_location = Some(*pos);
                }
                if let Some(CommandArgumentType::Real(value)) = arg_at(2) {
                    angle = *value;
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(3) {
                    if *id != INVALID_OBJECT_ID {
                        object_in_way = Some(*id);
                    }
                }
                if let Some(CommandArgumentType::Integer(options)) = arg_at(4) {
                    command_options =
                        SpecialPowerCommandOptions::from_bits_truncate(*options as u32);
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(5) {
                    if *id != INVALID_OBJECT_ID {
                        source_object = Some(*id);
                    }
                }
            }
            CommandType::DoSpecialPowerAtObject => {
                if let Some(CommandArgumentType::Integer(id)) = arg_at(0) {
                    power_id = Some(*id as u32);
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(1) {
                    if *id != INVALID_OBJECT_ID {
                        target_object = Some(*id);
                    }
                }
                if let Some(CommandArgumentType::Integer(options)) = arg_at(2) {
                    command_options =
                        SpecialPowerCommandOptions::from_bits_truncate(*options as u32);
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(3) {
                    if *id != INVALID_OBJECT_ID {
                        source_object = Some(*id);
                    }
                }
            }
            CommandType::DoSpecialPowerOverrideDestination => {
                if let Some(CommandArgumentType::Location(pos)) = arg_at(0) {
                    target_location = Some(*pos);
                }
                if let Some(CommandArgumentType::Integer(value)) = arg_at(1) {
                    override_power_type = Some(*value as u32);
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(2) {
                    if *id != INVALID_OBJECT_ID {
                        source_object = Some(*id);
                    }
                }
            }
            _ => {}
        }

        if cmd_type != CommandType::DoSpecialPowerOverrideDestination {
            if power_id.is_none() {
                for i in 0..arg_count {
                    if let Some(CommandArgumentType::Integer(id)) = arg_at(i as Int) {
                        power_id = Some(*id as u32);
                        break;
                    }
                }
            }
            if target_location.is_none() {
                for i in 0..arg_count {
                    if let Some(CommandArgumentType::Location(pos)) = arg_at(i as Int) {
                        target_location = Some(*pos);
                        break;
                    }
                }
            }
            if target_object.is_none() {
                for i in 0..arg_count {
                    if let Some(CommandArgumentType::ObjectID(id)) = arg_at(i as Int) {
                        if *id != INVALID_OBJECT_ID {
                            target_object = Some(*id);
                            break;
                        }
                    }
                }
            }
        }

        let object_exists = |object_id: ObjectID| -> bool {
            if let Some(object_manager) = &context.object_manager {
                if let Ok(om) = object_manager.read() {
                    if om.get_object(object_id).is_some() {
                        return true;
                    }
                }
            }
            TheGameLogic::find_object_by_id(object_id).is_some()
        };

        let object_position = |object_id: ObjectID| -> Option<Coord3D> {
            if let Some(object_manager) = &context.object_manager {
                if let Ok(om) = object_manager.read() {
                    if let Some(obj) = om.get_object(object_id) {
                        return Some(obj.get_position());
                    }
                }
            }
            TheGameLogic::find_object_by_id(object_id)
                .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()))
        };

        let object_is_alive = |object_id: ObjectID| -> bool {
            if let Some(object_manager) = &context.object_manager {
                if let Ok(om) = object_manager.read() {
                    if let Some(obj) = om.get_object(object_id) {
                        return obj.is_alive();
                    }
                }
            }
            TheGameLogic::find_object_by_id(object_id)
                .and_then(|obj| obj.read().ok().map(|guard| !guard.is_destroyed()))
                .unwrap_or(false)
        };

        let object_can_be_controlled_by = |object_id: ObjectID, player_id: Int| -> bool {
            if let Some(object_manager) = &context.object_manager {
                if let Ok(om) = object_manager.read() {
                    if let Some(obj) = om.get_object(object_id) {
                        return obj.can_be_controlled_by(player_id);
                    }
                }
            }
            let owner = TheGameLogic::find_object_by_id(object_id)
                .and_then(|obj| {
                    obj.read()
                        .ok()
                        .and_then(|guard| guard.get_controlling_player_id())
                        .map(|id| id as Int)
                })
                .unwrap_or(-1);
            owner == -1 || owner == player_id
        };

        if cmd_type == CommandType::DoSpecialPowerOverrideDestination {
            let location = match target_location {
                Some(pos) => pos,
                None => {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Special power override requires a target location",
                    ))
                }
            };

            let mut source_ids = Vec::new();
            if let Some(source_id) = source_object {
                source_ids.push(source_id);
            } else {
                let selection_manager = get_selection_manager();
                let mut selected_ids = Vec::new();
                if let Ok(manager) = selection_manager.read() {
                    if let Some(selection) = manager.get_player_selection_ref(context.player_id) {
                        selected_ids = selection.get_selected_objects();
                    }
                }
                source_ids = selected_ids;
            }

            let mut any_overridden = false;
            if !source_ids.is_empty() {
                for id in &source_ids {
                    if !object_is_alive(*id) || !object_can_be_controlled_by(*id, context.player_id)
                    {
                        continue;
                    }
                    let Some(obj) = TheGameLogic::find_object_by_id(*id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj.read() else {
                        continue;
                    };
                    if let Some(power_type) = override_power_type {
                        let mut matches_power = false;
                        for module_handle in obj_guard.behavior_modules() {
                            module_handle.with_module(|module| {
                                let Some(sp_module) = module_special_power_interface(module) else {
                                    return;
                                };
                                let Some(template) = sp_module.get_special_power_template_full()
                                else {
                                    return;
                                };
                                if template.get_special_power_type() as u32 == power_type {
                                    matches_power = true;
                                }
                            });
                            if matches_power {
                                break;
                            }
                        }
                        if !matches_power {
                            for behavior_arc in obj_guard.get_behavior_modules() {
                                let Ok(mut behavior_guard) = behavior_arc.lock() else {
                                    continue;
                                };
                                let Some(sp_module) = behavior_guard.get_special_power() else {
                                    continue;
                                };
                                let Some(template) = sp_module.get_special_power_template_full()
                                else {
                                    continue;
                                };
                                if template.get_special_power_type() as u32 == power_type {
                                    matches_power = true;
                                    break;
                                }
                            }
                        }
                        if !matches_power {
                            continue;
                        }
                    }
                    let mut overridden_here = false;
                    for module_handle in obj_guard.behavior_modules() {
                        module_handle.with_module(|module| {
                            let Some(update) = module_special_power_update_interface(module) else {
                                return;
                            };
                            if update.does_special_power_have_overridable_destination_active()
                                || update.does_special_power_have_overridable_destination()
                            {
                                update.set_special_power_overridable_destination(&location);
                                overridden_here = true;
                            }
                        });
                    }
                    if !overridden_here {
                        for behavior_arc in obj_guard.get_behavior_modules() {
                            let Ok(mut behavior_guard) = behavior_arc.lock() else {
                                continue;
                            };
                            if let Some(update) =
                                behavior_guard.get_special_power_update_interface()
                            {
                                if update.does_special_power_have_overridable_destination_active()
                                    || update.does_special_power_have_overridable_destination()
                                {
                                    update.set_special_power_overridable_destination(&location);
                                    overridden_here = true;
                                }
                            }
                        }
                    }
                    if overridden_here {
                        any_overridden = true;
                    }
                }
            }

            // C++ GameLogicDispatch falls through to MSG_DO_ATTACK_OBJECT here.
            self.execute_override_destination_fallthrough_attack(command, context);

            return if any_overridden {
                CommandExecutionResult::Success
            } else {
                CommandExecutionResult::Failed(AsciiString::from(
                    "No overridable special power destination available",
                ))
            };
        }

        let pid = match power_id {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Special power ID not specified",
                ))
            }
        };

        if cmd_type == CommandType::DoSpecialPowerAtObject && target_object.is_none() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Special power requires a target object",
            ));
        }

        if cmd_type == CommandType::DoSpecialPowerAtLocation && target_location.is_none() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Special power requires a target location",
            ));
        }

        // Validate target object existence (ownership is not enforced here because many powers are offensive).
        if let Some(target_id) = target_object {
            if !object_exists(target_id) {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Special power target object not found",
                ));
            }
        }

        // Resolve the target position if not explicitly provided.
        if target_location.is_none() {
            if let Some(target_id) = target_object {
                target_location = object_position(target_id);
            }
        }

        let mut source_ids = Vec::new();
        if let Some(source_id) = source_object {
            source_ids.push(source_id);
        } else {
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
            return CommandExecutionResult::Success;
        }

        // Validate executor objects and attempt to execute special powers.
        let mut any_executed = false;
        for id in &source_ids {
            if !object_is_alive(*id) || !object_can_be_controlled_by(*id, context.player_id) {
                continue;
            }

            let Some(obj) = TheGameLogic::find_object_by_id(*id) else {
                continue;
            };
            let Ok(obj_guard) = obj.read() else {
                continue;
            };

            for module_handle in obj_guard.behavior_modules() {
                let mut executed_here = false;
                module_handle.with_module(|module| {
                    let Some(sp_module) = module_special_power_interface(module) else {
                        return;
                    };
                    let Some(template) = sp_module.get_special_power_template_full() else {
                        return;
                    };
                    if template.get_id() != pid {
                        return;
                    }

                    let allowed = match cmd_type {
                        CommandType::DoSpecialPower => TheActionManager::can_do_special_power(
                            &obj_guard,
                            template.as_ref(),
                            CommandSourceType::FromPlayer,
                            command_options.bits(),
                            true,
                        ),
                        CommandType::DoSpecialPowerAtObject => {
                            if let Some(target_id) = target_object {
                                if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id)
                                {
                                    if let Ok(target_guard) = target_obj.read() {
                                        TheActionManager::can_do_special_power_at_object(
                                            &obj_guard,
                                            &target_guard,
                                            CommandSourceType::FromPlayer,
                                            template.as_ref(),
                                            command_options.bits(),
                                            true,
                                        )
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        }
                        CommandType::DoSpecialPowerAtLocation => {
                            if let Some(pos) = target_location {
                                let object_in_way_arc = object_in_way
                                    .and_then(|id| TheGameLogic::find_object_by_id(id));
                                let object_in_way_ref = match object_in_way_arc.as_ref() {
                                    Some(obj_arc) => obj_arc.read().ok(),
                                    None => None,
                                };
                                TheActionManager::can_do_special_power_at_location(
                                    &obj_guard,
                                    &pos,
                                    CommandSourceType::FromPlayer,
                                    template.as_ref(),
                                    object_in_way_ref.as_deref(),
                                    command_options.bits(),
                                    true,
                                )
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };

                    if !allowed {
                        return;
                    }

                    match cmd_type {
                        CommandType::DoSpecialPower => {
                            sp_module.do_special_power(command_options);
                            executed_here = true;
                        }
                        CommandType::DoSpecialPowerAtObject => {
                            if let Some(target_id) = target_object {
                                sp_module.do_special_power_at_object(target_id, command_options);
                                executed_here = true;
                            }
                        }
                        CommandType::DoSpecialPowerAtLocation => {
                            if let Some(pos) = target_location {
                                sp_module.do_special_power_at_location(
                                    &pos,
                                    angle,
                                    command_options,
                                );
                                let _ = object_in_way;
                                executed_here = true;
                            }
                        }
                        _ => {}
                    }
                });

                if executed_here {
                    any_executed = true;
                    if let Ok(mut write_guard) = obj.write() {
                        write_guard.friend_set_undetected_defector(false);
                    }
                    break;
                }
            }
            if any_executed {
                continue;
            }
            for behavior_arc in obj_guard.get_behavior_modules() {
                let Ok(mut behavior_guard) = behavior_arc.lock() else {
                    continue;
                };
                let Some(sp_module) = behavior_guard.get_special_power() else {
                    continue;
                };
                let Some(template) = sp_module.get_special_power_template_full() else {
                    continue;
                };
                if template.get_id() != pid {
                    continue;
                }

                let allowed = match cmd_type {
                    CommandType::DoSpecialPower => TheActionManager::can_do_special_power(
                        &obj_guard,
                        template.as_ref(),
                        CommandSourceType::FromPlayer,
                        command_options.bits(),
                        true,
                    ),
                    CommandType::DoSpecialPowerAtObject => {
                        if let Some(target_id) = target_object {
                            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                                if let Ok(target_guard) = target_obj.read() {
                                    TheActionManager::can_do_special_power_at_object(
                                        &obj_guard,
                                        &target_guard,
                                        CommandSourceType::FromPlayer,
                                        template.as_ref(),
                                        command_options.bits(),
                                        true,
                                    )
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    CommandType::DoSpecialPowerAtLocation => {
                        if let Some(pos) = target_location {
                            let object_in_way_arc =
                                object_in_way.and_then(|id| TheGameLogic::find_object_by_id(id));
                            let object_in_way_ref = match object_in_way_arc.as_ref() {
                                Some(obj_arc) => obj_arc.read().ok(),
                                None => None,
                            };
                            TheActionManager::can_do_special_power_at_location(
                                &obj_guard,
                                &pos,
                                CommandSourceType::FromPlayer,
                                template.as_ref(),
                                object_in_way_ref.as_deref(),
                                command_options.bits(),
                                true,
                            )
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if !allowed {
                    continue;
                }

                match cmd_type {
                    CommandType::DoSpecialPower => {
                        sp_module.do_special_power(command_options);
                        any_executed = true;
                    }
                    CommandType::DoSpecialPowerAtObject => {
                        if let Some(target_id) = target_object {
                            sp_module.do_special_power_at_object(target_id, command_options);
                            any_executed = true;
                        }
                    }
                    CommandType::DoSpecialPowerAtLocation => {
                        if let Some(pos) = target_location {
                            sp_module.do_special_power_at_location(&pos, angle, command_options);
                            let _ = object_in_way;
                            any_executed = true;
                        }
                    }
                    _ => {}
                }

                if any_executed {
                    if let Ok(mut write_guard) = obj.write() {
                        write_guard.friend_set_undetected_defector(false);
                    }
                    break;
                }
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_override_destination_fallthrough_attack(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) {
        let Some(target_id) = override_destination_fallthrough_target_id(command) else {
            return;
        };

        if TheGameLogic::find_object_by_id(target_id).is_none() {
            return;
        }

        let mut attack_command = Command::new(CommandType::DoAttackObject);
        attack_command.set_player_index(context.player_id);
        attack_command.append_object_id_argument(target_id);

        let queued_attack =
            QueuedCommand::new(attack_command, CommandPriority::High, context.current_frame);
        let _ = self.execute_attack_command(&queued_attack, context);
    }

    fn execute_place_beacon(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut position = match self.extract_command_location(command) {
            Some(position) => position,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No beacon position supplied",
                ))
            }
        };

        if let Some(terrain) = TheTerrainLogic::get() {
            let extent = terrain.get_maximum_pathfind_extent();
            if !Self::is_in_region_no_z(&extent, &position) {
                position = terrain.find_closest_edge_point(&position);
            }
        }

        let (player_arc, local_player_arc) = match player_list().read() {
            Ok(list) => {
                let player = match list.get_player(context.player_id) {
                    Some(player) => Arc::clone(player),
                    None => {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Player not found for beacon placement",
                        ))
                    }
                };
                (player, list.get_local_player().cloned())
            }
            Err(_) => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Player list lock poisoned",
                ))
            }
        };

        let (template_name, player_display_name, player_defeated, player_team) = {
            let guard = match player_arc.read() {
                Ok(guard) => guard,
                Err(_) => {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Player lock poisoned",
                    ))
                }
            };
            let template_name = guard
                .get_player_template()
                .map(|template| template.beacon_name.clone())
                .unwrap_or_default();
            let defeated = guard.is_defeated()
                || (!guard.has_any_units()
                    && !guard.has_any_buildings_counts_for_victory()
                    && !guard.has_any_objects());
            (
                template_name,
                guard.get_player_display_name().clone(),
                defeated,
                guard.get_default_team(),
            )
        };

        if player_defeated {
            self.notify_beacon_failed(context.player_id, &position, local_player_arc.as_ref());
            return CommandExecutionResult::Failed(AsciiString::from("Player is defeated"));
        }

        if template_name.is_empty() {
            self.notify_beacon_failed(context.player_id, &position, local_player_arc.as_ref());
            return CommandExecutionResult::Failed(AsciiString::from("Beacon template missing"));
        }

        let template = match TheThingFactory::find_template(&template_name) {
            Some(template) => template,
            None => {
                self.notify_beacon_failed(context.player_id, &position, local_player_arc.as_ref());
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Beacon template not found",
                ));
            }
        };

        let max_beacons = with_multiplayer_settings(|settings| settings.max_beacons_per_player);
        let current_count = self.count_player_beacons(context.player_id, &template);
        if current_count >= max_beacons {
            self.notify_beacon_limit_reached(
                context.player_id,
                &position,
                local_player_arc.as_ref(),
            );
            return CommandExecutionResult::Failed(AsciiString::from("Too many beacons"));
        }

        let new_object = match TheThingFactory::get() {
            Ok(factory) => {
                let team_ref = player_team.as_ref().and_then(|team| team.read().ok());
                if let Some(team_guard) = team_ref.as_ref() {
                    factory.new_object(template.clone(), team_guard).ok()
                } else {
                    factory
                        .new_object_optional_team(template.clone(), None)
                        .ok()
                }
            }
            Err(_) => None,
        };

        let Some(beacon_object) = new_object else {
            self.notify_beacon_failed(context.player_id, &position, local_player_arc.as_ref());
            return CommandExecutionResult::Failed(AsciiString::from("Beacon creation failed"));
        };

        if let Ok(mut obj_guard) = beacon_object.write() {
            let _ = obj_guard.set_position(&position);
            obj_guard.set_producer(None);
        }

        let (local_visibility, local_allies) =
            self.beacon_visibility_and_allies(&player_arc, local_player_arc.as_ref());
        if local_visibility {
            let mut manager = match get_beacon_manager().lock() {
                Ok(lock) => lock,
                Err(_) => {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Beacon manager lock poisoned",
                    ))
                }
            };
            manager.place_beacon(context.player_id, position, context.current_frame);

            self.notify_beacon_placed(
                context.player_id,
                &position,
                &player_display_name,
                local_player_arc.as_ref(),
            );
            if let Ok(mut radar) = get_radar_system().write() {
                let radar_pos = RadarCoord3D::new(position.x, position.y, position.z);
                radar.create_event(&radar_pos, RadarEventType::Information, 1.0);
            }
            if local_allies {
                let _ = TheEva::set_should_play(EvaEvent::BeaconDetected);
            }
            control_bar::mark_ui_dirty();
        } else {
            let beacon_id = beacon_object
                .read()
                .map(|guard| guard.get_id())
                .unwrap_or_default();
            self.hide_beacon_for_local(beacon_id);
        }

        CommandExecutionResult::Success
    }

    fn execute_remove_beacon(
        &mut self,
        _command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        let (player_arc, _local_player_arc, is_local_player) = match player_list().read() {
            Ok(list) => {
                let player = match list.get_player(context.player_id) {
                    Some(player) => Arc::clone(player),
                    None => {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Player not found for beacon removal",
                        ))
                    }
                };
                let local = list.get_local_player().cloned();
                let is_local = local
                    .as_ref()
                    .and_then(|player| {
                        player
                            .read()
                            .ok()
                            .map(|guard| guard.get_player_index() as Int)
                    })
                    .map(|index| index == context.player_id)
                    .unwrap_or(false);
                (player, local, is_local)
            }
            Err(_) => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Player list lock poisoned",
                ))
            }
        };

        let selected_ids = {
            let Ok(guard) = player_arc.write() else {
                return CommandExecutionResult::Failed(AsciiString::from("Player lock poisoned"));
            };
            guard.get_current_selection_ids()
        };

        let mut removed_entries: Vec<(Int, Coord3D)> = Vec::new();
        let mut _removed_any = false;

        for object_id in selected_ids {
            let Some((owner_id, entry_position)) = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |obj_guard| {
                    let owner_id = obj_guard
                        .get_controlling_player_id()
                        .map(|id| id as Int)
                        .unwrap_or(-1);
                    if owner_id < 0 {
                        return None;
                    }
                    let Some(owner_template) = self.resolve_beacon_template_for_player(owner_id)
                    else {
                        return None;
                    };
                    if !owner_template.is_equivalent_to(obj_guard.get_template().as_ref()) {
                        return None;
                    }
                    Some((owner_id, *obj_guard.get_position()))
                })
                .flatten()
            else {
                continue;
            };

            if owner_id == context.player_id {
                let _ = TheGameLogic::destroy_object_by_id(object_id);
                _removed_any = true;
                removed_entries.push((owner_id, entry_position));
                control_bar::mark_ui_dirty();
            } else if is_local_player {
                self.hide_beacon_for_local(object_id);
                removed_entries.push((owner_id, entry_position));
            }
        }

        if !removed_entries.is_empty() {
            if let Ok(mut manager) = get_beacon_manager().lock() {
                for (owner_id, pos) in removed_entries {
                    let _ = manager.remove_beacon(owner_id, &pos);
                }
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_set_beacon_text(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let text = match self.extract_command_text(command) {
            Some(text) => text,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from("No beacon text supplied"))
            }
        };

        let selected_beacons = self.collect_selected_beacon_positions(context.player_id);
        let mut manager = match get_beacon_manager().lock() {
            Ok(lock) => lock,
            Err(_) => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Beacon manager lock poisoned",
                ))
            }
        };

        let fallback = self
            .extract_command_location(command)
            .map(|position| (context.player_id, position));
        if Self::apply_beacon_text_updates(&mut manager, &selected_beacons, fallback, text) {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Beacon not found"))
        }
    }

    fn apply_beacon_text_updates(
        manager: &mut BeaconManager,
        selected_beacons: &[(Int, Coord3D)],
        fallback: Option<(Int, Coord3D)>,
        text: AsciiString,
    ) -> bool {
        let mut updated = false;
        for (owner_id, position) in selected_beacons {
            updated |= manager.set_beacon_text(*owner_id, position, text.clone());
        }
        if updated {
            true
        } else if let Some((owner_id, position)) = fallback {
            manager.set_beacon_text(owner_id, &position, text)
        } else {
            false
        }
    }

    fn collect_selected_beacon_positions(&self, player_id: Int) -> Vec<(Int, Coord3D)> {
        // Wave 275: empty dual-world → no factory objects.
        if dual_world_registry_unavailable() {
            return Vec::new();
        }

        let selected_ids = match player_list().read() {
            Ok(list) => list
                .get_player(player_id)
                .and_then(|player| {
                    player
                        .read()
                        .ok()
                        .map(|guard| guard.get_current_selection_ids())
                })
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        let mut beacons = Vec::new();
        for object_id in selected_ids {
            let Some(beacon) = OBJECT_REGISTRY.with_object(object_id, |obj_guard| {
                let owner_id = obj_guard
                    .get_controlling_player_id()
                    .map(|id| id as Int)
                    .unwrap_or(player_id);
                let Some(owner_template) = self.resolve_beacon_template_for_player(owner_id) else {
                    return None;
                };
                if owner_template.is_equivalent_to(obj_guard.get_template().as_ref()) {
                    Some((owner_id, *obj_guard.get_position()))
                } else {
                    None
                }
            }) else {
                continue;
            };
            if let Some(entry) = beacon {
                beacons.push(entry);
            }
        }
        beacons
    }

    fn is_in_region_no_z(region: &crate::common::Region3D, position: &Coord3D) -> bool {
        position.x >= region.lo.x
            && position.x <= region.hi.x
            && position.y >= region.lo.y
            && position.y <= region.hi.y
    }

    fn resolve_beacon_template_for_player(
        &self,
        player_id: Int,
    ) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        let list = player_list().read().ok()?;
        let player_arc = list.get_player(player_id)?.clone();
        let player_guard = player_arc.read().ok()?;
        let template_name = player_guard
            .get_player_template()
            .map(|template| template.beacon_name.clone())?;
        if template_name.is_empty() {
            return None;
        }
        TheThingFactory::find_template(&template_name)
    }

    fn count_player_beacons(
        &self,
        player_id: Int,
        template: &Arc<dyn crate::common::ThingTemplate>,
    ) -> Int {
        let manager = get_object_manager();
        let Ok(manager_guard) = manager.read() else {
            return 0;
        };

        let mut count = 0;
        for object_id in manager_guard.get_objects_owned_by_player(player_id as UnsignedInt) {
            let Some(instance) = manager_guard.get_object(object_id) else {
                continue;
            };
            let Ok(instance_guard) = instance.read() else {
                continue;
            };
            let obj_arc = instance_guard.base();
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if template.is_equivalent_to(obj_guard.get_template().as_ref()) {
                count += 1;
            }
        }
        count
    }

    fn is_beacon_visible_to_local(
        &self,
        player_arc: &Arc<RwLock<crate::player::Player>>,
        local_player: Option<&Arc<RwLock<crate::player::Player>>>,
    ) -> bool {
        let Some(local_player) = local_player else {
            return false;
        };
        let Ok(local_guard) = local_player.read() else {
            return false;
        };
        if local_guard.is_player_observer() {
            return true;
        }
        let Some(local_team) = local_guard.get_default_team() else {
            return false;
        };
        let Ok(local_team_guard) = local_team.read() else {
            return false;
        };
        let Ok(player_guard) = player_arc.read() else {
            return false;
        };
        matches!(
            player_guard.get_relationship_with_team(&local_team_guard),
            Relationship::Allies
        )
    }

    fn beacon_visibility_and_allies(
        &self,
        player_arc: &Arc<RwLock<crate::player::Player>>,
        local_player: Option<&Arc<RwLock<crate::player::Player>>>,
    ) -> (bool, bool) {
        let Some(local_player) = local_player else {
            return (false, false);
        };
        let Ok(local_guard) = local_player.read() else {
            return (false, false);
        };
        if local_guard.is_player_observer() {
            return (true, false);
        }
        let Some(local_team) = local_guard.get_default_team() else {
            return (false, false);
        };
        let Ok(local_team_guard) = local_team.read() else {
            return (false, false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return (false, false);
        };
        let relation = player_guard.get_relationship_with_team(&local_team_guard);
        let visible = matches!(relation, Relationship::Allies);
        let allies = matches!(relation, Relationship::Allies);
        (visible, allies)
    }

    fn notify_beacon_placed(
        &self,
        player_id: Int,
        position: &Coord3D,
        player_name: &str,
        _local_player: Option<&Arc<RwLock<crate::player::Player>>>,
    ) {
        let template = TheGameText::fetch("GUI:BeaconPlaced");
        let message = template.replace("%s", player_name);
        TheInGameUI::display_message(&message);

        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new("BeaconPlaced");
            event.set_position(&(position.x, position.y, position.z));
            event.set_player_index(player_id as u32);
            audio.add_audio_event(&event);
        }
    }

    fn notify_beacon_failed(
        &self,
        player_id: Int,
        position: &Coord3D,
        _local_player: Option<&Arc<RwLock<crate::player::Player>>>,
    ) {
        TheInGameUI::display_message(&TheGameText::fetch("GUI:BeaconPlacementFailed"));
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new("BeaconPlacementFailed");
            event.set_position(&(position.x, position.y, position.z));
            event.set_player_index(player_id as u32);
            audio.add_audio_event(&event);
        }
    }

    fn notify_beacon_limit_reached(
        &self,
        player_id: Int,
        position: &Coord3D,
        local_player: Option<&Arc<RwLock<crate::player::Player>>>,
    ) {
        let local_matches = local_player
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|guard| guard.get_player_index() as Int)
            })
            .map(|index| index == player_id)
            .unwrap_or(false);
        if !local_matches {
            return;
        }

        TheInGameUI::display_message(&TheGameText::fetch("GUI:TooManyBeacons"));
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new("BeaconPlacementFailed");
            event.set_position(&(position.x, position.y, position.z));
            event.set_player_index(player_id as u32);
            audio.add_audio_event(&event);
        }
    }

    fn hide_beacon_for_local(&self, beacon_id: crate::common::ObjectID) {
        // Wave 275: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(modules) = crate::object::registry::OBJECT_REGISTRY
            .with_object(beacon_id, |guard| guard.client_update_modules())
        else {
            return;
        };
        for module in modules {
            module.with_module(|module| {
                if let Some(client_update) = module.get_client_update_interface() {
                    let _ = client_update.hide_beacon();
                }
            });
        }
    }

    fn hide_non_owned_beacon_for_local(
        &self,
        position: &Coord3D,
        exclude_owner: Option<Int>,
    ) -> Vec<(Int, Coord3D)> {
        let manager = get_object_manager();
        let Ok(manager_guard) = manager.read() else {
            return Vec::new();
        };
        let mut hidden = Vec::new();
        let object_ids = manager_guard.find_objects_in_radius(*position, 3.0);
        for object_id in object_ids {
            let Some(instance) = manager_guard.get_object(object_id) else {
                continue;
            };
            let Ok(instance_guard) = instance.read() else {
                continue;
            };
            let obj_arc = instance_guard.base();
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.get_position().distance(*position) > 3.0 {
                continue;
            }
            let owner_id = obj_guard
                .get_controlling_player_id()
                .map(|id| id as Int)
                .unwrap_or(-1);
            if owner_id < 0 {
                continue;
            }
            if exclude_owner.map(|id| id == owner_id).unwrap_or(false) {
                continue;
            }
            let Some(owner_template) = self.resolve_beacon_template_for_player(owner_id) else {
                continue;
            };
            if !owner_template.is_equivalent_to(obj_guard.get_template().as_ref()) {
                continue;
            }
            let entry_position = *obj_guard.get_position();
            let beacon_id = obj_guard.get_id();
            drop(obj_guard);
            self.hide_beacon_for_local(beacon_id);
            hidden.push((owner_id, entry_position));
        }
        hidden
    }

}
