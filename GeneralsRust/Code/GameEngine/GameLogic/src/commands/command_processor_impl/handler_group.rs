impl DefaultCommandHandler {
    /// Execute stop command
    fn execute_stop_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut object_ids = Vec::new();

        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                match arg {
                    crate::commands::command::CommandArgumentType::ObjectID(id) => {
                        object_ids.push(*id);
                    }
                    _ => {}
                }
            }
        }

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
            return CommandExecutionResult::Failed(AsciiString::from("No objects specified"));
        }

        // Issue stop order to AI system
        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_stop_order(&object_ids) {
                    CommandExecutionResult::Success
                } else {
                    CommandExecutionResult::Failed(AsciiString::from(
                        "AI system failed to process stop order",
                    ))
                }
            } else {
                CommandExecutionResult::Failed(AsciiString::from("Cannot access AI manager"))
            }
        } else {
            CommandExecutionResult::Failed(AsciiString::from("AI manager not available"))
        }
    }

    /// Execute scatter command (stop + jittered move targets)
    fn execute_scatter_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut object_ids = Vec::new();
        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                if let crate::commands::command::CommandArgumentType::ObjectID(id) = arg {
                    object_ids.push(*id);
                }
            }
        }

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
            return CommandExecutionResult::Failed(AsciiString::from("No objects to scatter"));
        }

        // Capture current positions for per-unit offsets
        let mut positions: Vec<(ObjectID, Coord3D)> = Vec::new();
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for id in &object_ids {
                    if let Some(obj) = om.get_object(*id) {
                        positions.push((*id, obj.get_position()));
                    }
                }
            }
        }

        if positions.len() != object_ids.len() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Unable to resolve objects for scatter",
            ));
        }

        // Stop current actions first
        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                let _ = ai.issue_stop_order(&object_ids);
            }
        }

        // Deterministic jitter seeded by frame/player
        let seed = (context.current_frame as u64) ^ ((context.player_id as u64) << 32);
        let mut rng = StdRng::seed_from_u64(seed);
        let mut all_ok = true;

        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                for (object_id, pos) in positions {
                    let angle = rng.r#gen::<f32>() * std::f32::consts::TAU;
                    let radius = rng.gen_range(8.0f32..22.0f32);
                    let dx = radius * angle.cos();
                    let dz = radius * angle.sin();
                    let dest = Coord3D::new(pos.x + dx, pos.y, pos.z + dz);

                    if !ai.issue_move_order(&[object_id], dest) {
                        all_ok = false;
                    }
                }
            } else {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Cannot access AI manager",
                ));
            }
        } else {
            return CommandExecutionResult::Failed(AsciiString::from("AI manager not available"));
        }

        if all_ok {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Failed to issue scatter move orders"))
        }
    }

    /// Execute self destruct on a set of objects
    fn execute_self_destruct(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let object_ids = self.extract_object_ids(command);
        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No objects to self destruct",
            ));
        }

        // Validate ownership/alive before issuing destruction.
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for id in &object_ids {
                    if let Some(obj) = om.get_object(*id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Cannot self-destruct a dead object",
                            ));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Player cannot self-destruct this object",
                            ));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Object not found",
                        ));
                    }
                }
            }
        }

        if let Some(object_manager) = &context.object_manager {
            if let Ok(mut om) = object_manager.write() {
                let mut all_ok = true;
                for id in &object_ids {
                    if !om.destroy_object(*id) {
                        all_ok = false;
                    }
                }
                if all_ok {
                    CommandExecutionResult::Success
                } else {
                    CommandExecutionResult::Failed(AsciiString::from(
                        "Failed to destroy one or more objects",
                    ))
                }
            } else {
                CommandExecutionResult::Failed(AsciiString::from("Cannot access object manager"))
            }
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Object manager not available"))
        }
    }

}
