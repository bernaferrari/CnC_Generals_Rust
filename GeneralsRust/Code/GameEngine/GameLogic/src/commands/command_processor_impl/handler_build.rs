impl DefaultCommandHandler {
    /// Execute construction command
    fn execute_build_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        use game_engine::common::system::build_assistant;

        let mut builder_id: Option<ObjectID> = None;
        let mut build_template_id: Option<u32> = None;
        let mut build_template_name: Option<AsciiString> = None;
        let mut build_positions: Vec<Coord3D> = Vec::new();
        let mut build_angle: Option<Real> = None;

        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                match arg {
                    crate::commands::command::CommandArgumentType::ObjectID(id) => {
                        if builder_id.is_none() {
                            builder_id = Some(*id);
                        }
                    }
                    crate::commands::command::CommandArgumentType::Integer(value) => {
                        if build_template_id.is_none() {
                            build_template_id = Some(*value as u32);
                        }
                    }
                    crate::commands::command::CommandArgumentType::AsciiString(template) => {
                        if build_template_name.is_none() {
                            build_template_name = Some(template.clone());
                        }
                    }
                    crate::commands::command::CommandArgumentType::Location(pos) => {
                        build_positions.push(*pos);
                    }
                    crate::commands::command::CommandArgumentType::Real(real) => {
                        if build_angle.is_none() {
                            build_angle = Some(*real);
                        }
                    }
                    _ => {}
                }
            }
        }

        let builder = match builder_id {
            Some(id) => id,
            None => {
                let selection_manager = get_selection_manager();
                let mut selected_ids = Vec::new();
                if let Ok(manager) = selection_manager.read() {
                    if let Some(selection) = manager.get_player_selection_ref(context.player_id) {
                        selected_ids = selection.get_selected_objects();
                    }
                }

                let mut selected_builder = None;
                for object_id in &selected_ids {
                    if let Some(object_arc) = TheGameLogic::find_object_by_id(*object_id) {
                        if let Ok(object_guard) = object_arc.read() {
                            if object_guard.is_kind_of(KindOf::Dozer) {
                                selected_builder = Some(*object_id);
                                break;
                            }
                        }
                    }
                }

                let builder_id = selected_builder.or_else(|| selected_ids.first().copied());
                let Some(builder_id) = builder_id else {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "No builder specified",
                    ));
                };
                builder_id
            }
        };

        let template = match (build_template_id, build_template_name.as_ref()) {
            (Some(id), _) => TheThingFactory::find_template_by_id(id),
            (_, Some(name)) => TheThingFactory::find_template(name.as_str()),
            _ => None,
        };
        let Some(template) = template else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No building template specified",
            ));
        };

        let angle = build_angle.unwrap_or(0.0);

        let (start_pos, end_pos) = match command.command.get_type() {
            CommandType::DozerConstructLine => {
                if build_positions.len() < 2 {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "No build line end specified",
                    ));
                }
                (build_positions[0], Some(build_positions[1]))
            }
            _ => {
                let Some(pos) = build_positions.first().copied() else {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "No build position specified",
                    ));
                };
                (pos, None)
            }
        };

        // Validate builder exists and is controllable
        let Some(builder_arc) = TheGameLogic::find_object_by_id(builder) else {
            return CommandExecutionResult::Failed(AsciiString::from("Builder not found"));
        };
        let Ok(builder_guard) = builder_arc.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Builder lock poisoned"));
        };
        if builder_guard.is_effectively_dead() {
            return CommandExecutionResult::Failed(AsciiString::from("Builder is not alive"));
        }
        let builder_owner = builder_guard
            .get_controlling_player_id()
            .map(|id| id as Int)
            .unwrap_or(-1);
        if builder_owner != -1 && builder_owner != context.player_id {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Player cannot control builder",
            ));
        }

        // Check resources (matches C++ ThingTemplate::getBuildCost behavior).
        let build_cost = ResourceCost {
            supplies: template.get_build_cost(),
            power: 0,
        };
        if let Some(player_manager) = &context.player_manager {
            if let Ok(pm) = player_manager.read() {
                if !pm.can_player_afford(context.player_id, &build_cost) {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Insufficient resources",
                    ));
                }
            }
        }

        let player_index = if let Some(player_arc) = builder_guard.get_controlling_player() {
            if let Ok(player_guard) = player_arc.read() {
                player_guard.get_player_index() as u32
            } else {
                context.player_id as u32
            }
        } else {
            context.player_id as u32
        };

        let builder_snapshot = build_assistant::Object {
            id: builder_guard.get_id(),
            position: build_assistant::Coord3D {
                x: builder_guard.get_position().x,
                y: builder_guard.get_position().y,
                z: builder_guard.get_position().z,
            },
            orientation: builder_guard.get_orientation(),
            command_set: None,
        };
        let owning_player = build_assistant::Player { player_index };

        let mut assistant_template =
            build_assistant::ThingTemplate::new(template.get_name().as_str());
        let template_geometry = template.get_template_geometry_info();
        assistant_template.geometry_info.major_radius =
            template_geometry.get_major_radius().max(1.0);
        assistant_template.geometry_info.minor_radius =
            template_geometry.get_minor_radius().max(1.0);
        assistant_template.geometry_info.height =
            template_geometry.get_max_height_above_position().max(1.0);

        let Some(assistant) = build_assistant::get_build_assistant() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Build assistant unavailable",
            ));
        };

        let mut _build_success = false;
        match end_pos {
            Some(end) => {
                assistant.build_object_line_now(
                    Some(&builder_snapshot),
                    &assistant_template,
                    &build_assistant::Coord3D {
                        x: start_pos.x,
                        y: start_pos.y,
                        z: start_pos.z,
                    },
                    &build_assistant::Coord3D {
                        x: end.x,
                        y: end.y,
                        z: end.z,
                    },
                    angle as f32,
                    &owning_player,
                );
                _build_success = true;
            }
            None => {
                let built = assistant.build_object_now(
                    Some(&builder_snapshot),
                    &assistant_template,
                    &build_assistant::Coord3D {
                        x: start_pos.x,
                        y: start_pos.y,
                        z: start_pos.z,
                    },
                    angle as f32,
                    &owning_player,
                );
                _build_success = built.is_some();
            }
        }

        if _build_success {
            let mut place_event = AudioEventRts::new("PlaceBuilding");
            place_event.set_object_id(builder);
            if let Some(audio) = TheAudio::get() {
                let _ = audio.add_audio_event(&place_event);
            }

            if let Some(player_manager) = &context.player_manager {
                if let Ok(mut pm) = player_manager.write() {
                    pm.modify_player_resources(
                        context.player_id,
                        -build_cost.supplies,
                        -build_cost.power,
                    );
                }
            }

            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Build failed"))
        }
    }

    fn execute_sell_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        use game_engine::common::system::build_assistant;

        let mut object_ids = Vec::new();
        for i in 0..command.command.get_argument_count() {
            if let Some(crate::commands::command::CommandArgumentType::ObjectID(id)) =
                command.command.get_argument(i as Int)
            {
                object_ids.push(*id);
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

        let Some(mut assistant) = build_assistant::get_build_assistant() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Build assistant unavailable",
            ));
        };
        let current_frame = TheGameLogic::get_frame();

        for object_id in object_ids {
            let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                return CommandExecutionResult::Failed(AsciiString::from("Object lock poisoned"));
            };

            let owner = object_guard
                .get_controlling_player_id()
                .map(|id| id as Int)
                .unwrap_or(-1);
            if owner != -1 && owner != context.player_id {
                continue;
            }

            let sell_object = build_assistant::Object {
                id: object_guard.get_id(),
                position: build_assistant::Coord3D {
                    x: object_guard.get_position().x,
                    y: object_guard.get_position().y,
                    z: object_guard.get_position().z,
                },
                orientation: object_guard.get_orientation(),
                command_set: None,
            };
            assistant.sell_object(&sell_object, current_frame);
        }

        CommandExecutionResult::Success
    }

    fn execute_set_rally_point(
        &mut self,
        command: &QueuedCommand,
        _context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let object_id = command.command.get_argument(0).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::ObjectID(id) => Some(*id),
            _ => None,
        });
        let destination = command.command.get_argument(1).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::Location(pos) => Some(*pos),
            _ => None,
        });

        let (Some(object_id), Some(destination)) = (object_id, destination) else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "SetRallyPoint missing object or destination",
            ));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return CommandExecutionResult::Success;
        };
        let Ok(object_guard) = object_arc.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Object lock poisoned"));
        };
        let from = *object_guard.get_position();
        let display_name = object_guard.get_template().get_name().as_str().to_string();
        drop(object_guard);

        // C++ doSetRallyPoint: BasicHumanLocomotor, not the building's own loco.
        let loco = basic_human_rally_locomotor_set();

        let ai_store = crate::ai::the_ai();let path_ok = ai_store
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .map(|pf| {
                pf.read()
                    .ok()
                    .map(|pf| pf.client_safe_quick_does_path_exist(&loco, &from, &destination))
                    .unwrap_or(true)
            })
            .unwrap_or(true);
        if !path_ok {
            crate::helpers::TheInGameUI::display_message("GUI:RallyPointNoPath");
            if let Some(audio) = crate::helpers::TheAudio::get() {
                let mut ev = crate::common::audio::AudioEventRts::new("UnableToSetRallyPoint");
                ev.set_position(&(destination.x, destination.y, destination.z));
                let _ = audio.add_audio_event(&ev);
            }
            return CommandExecutionResult::Failed(AsciiString::from("GUI:RallyPointNoPath"));
        }

        let Ok(mut object_guard) = object_arc.write() else {
            return CommandExecutionResult::Failed(AsciiString::from("Object lock poisoned"));
        };
        let _ = object_guard.set_rally_point(&destination);
        let message = format_rally_point_set_message(
            &crate::helpers::TheGameText::fetch("GUI:RallyPointSet"),
            &display_name,
        );
        crate::helpers::TheInGameUI::display_message(&message);
        if let Some(audio) = crate::helpers::TheAudio::get() {
            let mut ev = crate::common::audio::AudioEventRts::new("RallyPointSet");
            ev.set_position(&(destination.x, destination.y, destination.z));
            let _ = audio.add_audio_event(&ev);
        }
        CommandExecutionResult::Success
    }

    fn execute_set_mine_clearing_detail(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut object_ids = Vec::new();
        for i in 0..command.command.get_argument_count() {
            if let Some(crate::commands::command::CommandArgumentType::ObjectID(id)) =
                command.command.get_argument(i as Int)
            {
                object_ids.push(*id);
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

        for object_id in object_ids {
            let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(mut object_guard) = object_arc.write() else {
                return CommandExecutionResult::Failed(AsciiString::from("Object lock poisoned"));
            };

            object_guard.set_weapon_set_flag(WeaponSetType::MineClearingDetail);
        }

        CommandExecutionResult::Success
    }

}

/// C++ doSetRallyPoint: BasicHumanLocomotor set (GameLogicDispatch.cpp:130-132).
fn basic_human_rally_locomotor_set() -> crate::locomotor::LocomotorSet {
    let template = crate::locomotor::LOCOMOTOR_STORE
        .get_template("BasicHumanLocomotor")
        .unwrap_or_else(|| {
            std::sync::Arc::new(crate::locomotor::LocomotorTemplate::new_infantry(
                "BasicHumanLocomotor".to_string(),
            ))
        });
    let mut set = crate::locomotor::LocomotorSet::new();
    set.add_locomotor(
        "BasicHumanLocomotor".to_string(),
        std::sync::Arc::new(std::sync::Mutex::new(crate::locomotor::Locomotor::new(
            template,
        ))),
    );
    set
}

/// C++ `UnicodeString::format(TheGameText->fetch("GUI:RallyPointSet"), displayName)`.
fn format_rally_point_set_message(template: &str, display_name: &str) -> String {
    if template.contains("%s") {
        template.replace("%s", display_name)
    } else if template == "GUI:RallyPointSet" || template.starts_with("MISSING:") {
        format!("Rally point set for {display_name}")
    } else {
        format!("{template} {display_name}")
    }
}
