impl DefaultCommandHandler {
    fn execute_weapon_target_command(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        use crate::commands::command::CommandArgumentType;

        let cmd_type = command.command.get_type();
        let weapon_slot = match command.command.get_argument(0) {
            Some(CommandArgumentType::Integer(0)) => WeaponSlotType::Primary,
            Some(CommandArgumentType::Integer(1)) => WeaponSlotType::Secondary,
            Some(CommandArgumentType::Integer(2)) => WeaponSlotType::Tertiary,
            _ => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Weapon command missing weapon slot",
                ))
            }
        };

        let max_shots_to_fire_arg = if cmd_type == CommandType::DoWeapon {
            1
        } else {
            2
        };
        let max_shots_to_fire = match command.command.get_argument(max_shots_to_fire_arg) {
            Some(CommandArgumentType::Integer(value)) => *value,
            _ => NO_MAX_SHOTS_LIMIT,
        };

        let target_object = if cmd_type == CommandType::DoWeaponAtObject {
            match command.command.get_argument(1) {
                Some(CommandArgumentType::ObjectID(id)) => Some(*id),
                _ => {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Weapon object command missing target",
                    ))
                }
            }
        } else {
            None
        };

        let target_position = if cmd_type == CommandType::DoWeaponAtLocation {
            match command.command.get_argument(1) {
                Some(CommandArgumentType::Location(pos)) => Some(*pos),
                _ => {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Weapon location command missing target",
                    ))
                }
            }
        } else {
            None
        };

        let target_arc = match target_object {
            Some(id) => match TheGameLogic::find_object_by_id(id) {
                Some(obj) => Some(obj),
                None => return CommandExecutionResult::Success,
            },
            None => None,
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
            let Some((ai, own_position)) = OBJECT_REGISTRY
                .with_object_mut(object_id, |guard| {
                    if guard.is_destroyed() {
                        return None;
                    }
                    if guard.get_controlling_player_id().map(|id| id as Int)
                        != Some(context.player_id)
                    {
                        return None;
                    }

                    guard.set_weapon_lock(weapon_slot, WeaponLockType::LockedTemporarily);
                    let Some(ai) = guard.get_ai_update_interface() else {
                        return None;
                    };
                    let own_position = if cmd_type == CommandType::DoWeapon {
                        Some(*guard.get_position())
                    } else {
                        None
                    };
                    Some((ai, own_position))
                })
                .flatten()
            else {
                continue;
            };

            if let Some(target) = &target_arc {
                ai.ai_attack_object(
                    target.read().ok().map(|g| g.get_id()).unwrap_or(0),
                    max_shots_to_fire,
                    CommandSourceType::FromPlayer,
                );
            } else if let Some(position) = target_position {
                ai.ai_attack_position(&position, max_shots_to_fire, CommandSourceType::FromPlayer);
            } else if let Some(position) = own_position {
                ai.ai_attack_position(&position, max_shots_to_fire, CommandSourceType::FromPlayer);
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_enable_retaliation(
        &self,
        command: &QueuedCommand,
        _context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Matches C++ MSG_ENABLE_RETALIATION_MODE: sets per-player logical retaliation mode.
        let player_index = command.command.get_argument(0).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::Integer(value) => Some(*value),
            _ => None,
        });
        let enable = command.command.get_argument(1).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::Boolean(value) => Some(*value),
            _ => None,
        });
        let (Some(player_index), Some(enable)) = (player_index, enable) else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "EnableRetaliationMode missing args",
            ));
        };

        let list_lock = crate::player::player_list();
        let Ok(list) = list_lock.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Player list unavailable"));
        };
        let Some(player) = list.get_player(player_index) else {
            return CommandExecutionResult::Failed(AsciiString::from("Player not found"));
        };
        let Ok(mut guard) = player.write() else {
            return CommandExecutionResult::Failed(AsciiString::from("Failed to lock player"));
        };
        guard.set_logical_retaliation_mode_enabled(enable);
        CommandExecutionResult::Success
    }

    fn execute_purchase_science(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let science = command.command.get_argument(0).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::Integer(value) => {
                Some(*value as ScienceType)
            }
            _ => None,
        });
        let Some(science) = science else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "PurchaseScience missing science",
            ));
        };

        if science == SCIENCE_INVALID {
            return CommandExecutionResult::Success;
        }

        let list_lock = crate::player::player_list();
        let Ok(list) = list_lock.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Player list unavailable"));
        };
        let Some(player) = list.get_player(context.player_id) else {
            return CommandExecutionResult::Success;
        };
        let Ok(mut guard) = player.write() else {
            return CommandExecutionResult::Failed(AsciiString::from("Failed to lock player"));
        };

        let _ = guard.attempt_to_purchase_science(science);
        CommandExecutionResult::Success
    }

    fn execute_create_formation(
        &self,
        _command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Wave 275: empty dual-world → invalid game state.
        if dual_world_registry_unavailable() {
            return CommandExecutionResult::InvalidGameState;
        }

        // Matches C++ MSG_CREATE_FORMATION: toggles a "preserve relative offsets" formation on the
        // currently selected controllable units by assigning a shared FormationID and per-unit offset.
        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Selection manager unavailable for CreateFormation",
            ));
        };
        let Some(selection) = manager.get_player_selection_ref(context.player_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("No player selection"));
        };
        let selected = selection.get_selected_objects();
        if selected.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from("No selected objects"));
        }

        let mut count = 0usize;
        let mut center = Coord3D::new(0.0, 0.0, 0.0);
        let mut formation_id: Option<FormationID> = None;

        for object_id in &selected {
            let Some(cur_id) = OBJECT_REGISTRY
                .with_object(*object_id, |guard| {
                    if guard.is_destroyed() {
                        return None;
                    }
                    if guard.is_disabled_by_type(crate::common::DisabledType::Held) {
                        return None;
                    }
                    if guard.get_ai_update_interface().is_none() {
                        return None;
                    }

                    let pos = guard.get_position();
                    center.x += pos.x;
                    center.y += pos.y;
                    center.z += pos.z;

                    Some(guard.get_formation_id())
                })
                .flatten()
            else {
                continue;
            };

            if count == 0 {
                formation_id = Some(cur_id);
            } else if formation_id.map_or(false, |id| id != cur_id) {
                formation_id = None;
            }
            count += 1;
        }

        if count == 0 {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No eligible objects for formation",
            ));
        }

        center.x /= count as f32;
        center.y /= count as f32;
        center.z /= count as f32;

        let is_formation = formation_id.map(|id| !id.is_none()).unwrap_or(false) && count >= 2;
        let is_formation =
            is_formation || (count == 1 && formation_id.map(|id| !id.is_none()).unwrap_or(false));

        let new_id = if is_formation {
            FormationID::NONE
        } else {
            FormationID::new(NEXT_FORMATION_ID.fetch_add(1, Ordering::Relaxed))
        };

        for object_id in selected {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |guard| {
                if guard.is_destroyed() {
                    return;
                }
                if guard.is_disabled_by_type(crate::common::DisabledType::Held) {
                    return;
                }
                if guard.get_ai_update_interface().is_none() {
                    return;
                }
                if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id)
                {
                    return;
                }

                let pos = *guard.get_position();
                let offset = crate::common::Coord2D::new(pos.x - center.x, pos.y - center.y);
                guard.set_formation_id(new_id);
                guard.set_formation_offset(offset);
            });
        }

        CommandExecutionResult::Success
    }

    fn execute_clear_game_data(&self) -> CommandExecutionResult {
        match TheGameLogic::clear_game_data() {
            Ok(()) => CommandExecutionResult::Success,
            Err(err) => CommandExecutionResult::Failed(AsciiString::from(&err)),
        }
    }

    fn execute_new_game(&self, command: &QueuedCommand) -> CommandExecutionResult {
        let read_int = |index: Int, fallback: Int| -> Int {
            match command.command.get_argument(index) {
                Some(crate::commands::command::CommandArgumentType::Integer(value)) => *value,
                _ => fallback,
            }
        };

        let game_mode = read_int(0, crate::system::game_logic::GAME_SINGLE_PLAYER);
        let difficulty = read_int(1, 1);
        let rank_points = read_int(2, 0);
        let max_fps_arg = read_int(3, -1);

        if max_fps_arg >= 0 {
            let default_fps = get_engine_global_data()
                .map(|data| data.read().frames_per_second_limit)
                .unwrap_or(30);
            let clamped_fps = if (1..=1000).contains(&max_fps_arg) {
                max_fps_arg
            } else {
                default_fps
            };

            if let Some(data) = get_engine_global_data() {
                let mut data = data.write();
                data.frames_per_second_limit = clamped_fps;
                data.use_fps_limit = true;
            }
            if let Some(engine) = get_game_engine() {
                let mut guard = engine.lock();
                guard.set_frames_per_second_limit(clamped_fps.max(0) as u32);
            }
        }

        TheGameLogic::prepare_new_game(game_mode, difficulty, rank_points);
        match TheGameLogic::start_new_game(false) {
            Ok(()) => CommandExecutionResult::Success,
            Err(err) => CommandExecutionResult::Failed(AsciiString::from(&err)),
        }
    }

    /// C++ parity: MSG_META_BEGIN_PATH_BUILD (GameLogicDispatch.cpp lines 445-457)
    fn execute_begin_path_build(&mut self) -> CommandExecutionResult {
        if !self.build_plan_active {
            self.build_plan_active = true;
            self.build_plan_subjects.clear();
        }
        CommandExecutionResult::Success
    }

    /// C++ parity: MSG_META_END_PATH_BUILD (GameLogicDispatch.cpp lines 460-477)
    fn execute_end_path_build(
        &mut self,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let subjects = std::mem::take(&mut self.build_plan_subjects);

        for object_id in &subjects {
            if let Some(ai_manager) = &context.ai_manager {
                if let Ok(mut ai) = ai_manager.write() {
                    ai.execute_waypoint_queue_for_object(*object_id);
                }
            }
        }

        self.build_plan_active = false;
        CommandExecutionResult::Success
    }
}
