//! Remaining skirmish/AI, EVA, options, scoring, and train-held script actions
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;

impl ScriptActionDispatcher {
    pub(crate) fn do_skirmish_build_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let building_type = self.get_string_param(action, 0)?;
        log::debug!("Skirmish building '{}'", building_type);
        let building = building_type.clone();
        self.with_current_player_ai(|ai_player| {
            let _ = ai_player.build_specific_building(&building);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_skirmish_follow_approach_path(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_path_label = self.get_string_param(action, 1)?;
        let as_team = self.get_int_param(action, 2)? != 0;
        log::debug!(
            "Skirmish team '{}' following approach path '{}' as_team={}",
            team_name,
            waypoint_path_label,
            as_team
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let Some((center, first_unit)) = self.compute_team_center_and_first(&team_arc) else {
            return Ok(ScriptActionResult::Success);
        };

        let enemy_player = self.get_skirmish_enemy_player();
        let Some(enemy_player) = enemy_player else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(enemy_guard) = enemy_player.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mp_index = enemy_guard.get_mp_start_index() + 1;

        let path_label = format!("{}{}", waypoint_path_label, mp_index);
        let (waypoint_id, waypoint_pos) =
            match get_terrain_logic().read().ok().and_then(|terrain| {
                terrain
                    .get_closest_waypoint_on_path(&center, &path_label)
                    .map(|way| (way.get_id(), *way.get_location()))
            }) {
                Some(result) => result,
                None => return Ok(ScriptActionResult::Success),
            };

        let current_player_name = get_script_engine().read().ok().and_then(|g| {
            g.as_ref()
                .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
        });
        if let Some(current_player_name) = current_player_name {
            if let Ok(list) = player_list().read() {
                if let Some(player_arc) = list.find_player_by_name(&current_player_name) {
                    if let Ok(player_guard) = player_arc.read() {
                        let player_id = player_guard.get_player_index() as u32;
                        self.check_bridges_for_waypoint(player_id, &first_unit, waypoint_id);
                    }
                }
            }
        }

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        if let Ok(mut group) = group_arc.write() {
            let command_type = if as_team {
                AiCommandType::FollowWaypointPathAsTeam
            } else {
                AiCommandType::FollowWaypointPath
            };
            let mut params = AiCommandParams::new(command_type, CommandSourceType::FromScript);
            params.waypoint = Some(waypoint_id);
            let _ = group.ai_do_command(&params);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_skirmish_move_to_approach_path(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 0)?;
        let waypoint_path_label = self.get_string_param(action, 1)?;
        log::debug!(
            "Skirmish team '{}' moving to approach path '{}'",
            team_name,
            waypoint_path_label
        );

        let team_arc = self.get_team_by_name(&team_name)?;
        let Some((center, _first_unit)) = self.compute_team_center_and_first(&team_arc) else {
            return Ok(ScriptActionResult::Success);
        };

        let enemy_player = self.get_skirmish_enemy_player();
        let Some(enemy_player) = enemy_player else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(enemy_guard) = enemy_player.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let mp_index = enemy_guard.get_mp_start_index() + 1;

        let path_label = format!("{}{}", waypoint_path_label, mp_index);
        let waypoint_pos = match get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_closest_waypoint_on_path(&center, &path_label)
                .map(|way| *way.get_location())
        }) {
            Some(pos) => pos,
            None => return Ok(ScriptActionResult::Success),
        };

        let group_arc = self.create_ai_group_from_team(&team_name)?;
        if let Ok(group) = group_arc.read() {
            group.group_move_to_position(&waypoint_pos, false, CommandSourceType::FromScript);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_skirmish_build_base_defense_front(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        if action.num_parms > 0 {
            let _ = self.get_string_param(action, 0);
        }
        log::debug!("Skirmish building base defense front");
        self.with_current_player_ai(|ai_player| {
            let _ = ai_player.build_base_defense(false);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_skirmish_build_base_defense_flank(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        if action.num_parms > 0 {
            let _ = self.get_string_param(action, 0);
        }
        log::debug!("Skirmish building base defense flank");
        self.with_current_player_ai(|ai_player| {
            let _ = ai_player.build_base_defense(true);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_skirmish_build_structure_front(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let structure_type = self.get_string_param(action, 0)?;
        log::debug!("Skirmish building structure front '{}'", structure_type);
        let structure = structure_type.clone();
        self.with_current_player_ai(|ai_player| {
            let _ = ai_player.build_base_defense_structure(&structure, false);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_skirmish_build_structure_flank(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let structure_type = self.get_string_param(action, 0)?;
        log::debug!("Skirmish building structure flank '{}'", structure_type);
        let structure = structure_type.clone();
        self.with_current_player_ai(|ai_player| {
            let _ = ai_player.build_base_defense_structure(&structure, true);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_skirmish_fire_special_power_at_most_cost(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        // Wave 284: empty dual-world → no-op success.
        if dual_world_registry_unavailable() {
            return Ok(ScriptActionResult::Success);
        }

        let player_name = self.resolve_player_name_token(&self.get_string_param(action, 0)?);
        let power_name = self.get_string_param(action, 1)?;
        log::debug!(
            "Skirmish player '{}' firing special power '{}' at most cost",
            player_name,
            power_name
        );

        let Some(enemy_player) = self.get_skirmish_enemy_player() else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(enemy_guard) = enemy_player.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let enemy_player_index = enemy_guard.get_player_index();

        let (power_template, template_name, radius) = {
            let Some(store) = get_special_power_store() else {
                return Ok(ScriptActionResult::Success);
            };
            let Some(template) = store.find_special_power_template(&power_name) else {
                return Ok(ScriptActionResult::Success);
            };

            (
                template.clone(),
                template.get_name().to_string(),
                template.get_radius_cursor_radius().max(50.0),
            )
        };

        let Some(player_arc) = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
        else {
            log::warn!("Skirmish action: player '{}' not found", player_name);
            return Ok(ScriptActionResult::Success);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let player_id = player_guard.get_player_index() as u32;

        let mut target_location: Option<Coord3D> = None;
        let _ = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| match ai_player {
                IntegratedAiPlayer::Skirmish(skirmish_ai) => {
                    let mut location = Coord3D::ZERO;
                    if skirmish_ai.compute_superweapon_target(
                        &power_template,
                        &mut location,
                        enemy_player_index,
                        radius,
                    ) {
                        target_location = Some(location);
                    }
                }
                IntegratedAiPlayer::Standard(standard_ai) => {
                    if let Ok(Some(location)) = standard_ai.compute_superweapon_target(
                        power_template.get_name(),
                        radius,
                        enemy_player_index,
                    ) {
                        target_location = Some(location);
                    }
                }
            })
        });

        let Some(target_location) = target_location else {
            return Ok(ScriptActionResult::Success);
        };
        if target_location.x == 0.0 && target_location.y == 0.0 && target_location.z == 0.0 {
            return Ok(ScriptActionResult::Success);
        }

        // Host path: empty dual-world registry → no SP fire residual.
        if OBJECT_REGISTRY.is_empty() {
            return Ok(ScriptActionResult::Success);
        }
        let mut fired = false;
        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
            let object_arc = match OBJECT_REGISTRY.get_object(obj_id) {
                Some(v) => v,
                None => continue,
            };
            let Ok(object_guard) = object_arc.read() else {
                continue;
            };
            if object_guard.is_destroyed() {
                continue;
            }
            let Some(owner_id) = object_guard.get_controlling_player_id() else {
                continue;
            };
            if owner_id as u32 != player_id {
                continue;
            }

            let is_ready = object_guard
                .with_special_power_module_interface_by_name(&template_name, |sp_module| {
                    sp_module.is_ready()
                })
                .unwrap_or(false);
            if !is_ready {
                continue;
            }

            let fired_here =
                object_guard.with_special_power_module_mut_by_name(&template_name, |sp_module| {
                    sp_module.do_special_power_at_location(
                        &target_location,
                        INVALID_ANGLE,
                        SpecialPowerCommandOption::COMMAND_FIRED_BY_SCRIPT,
                    );
                    true
                });
            if fired_here.unwrap_or(false) {
                fired = true;
                break;
            }
        }

        if !fired {
            log::debug!(
                "Skirmish special power '{}' not fired: no ready module found for '{}'",
                power_name,
                player_name
            );
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_skirmish_attack_nearest_group_with_value(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let comparison = self.get_int_param(action, 1)?;
        let value = self.get_int_param(action, 2)?;
        log::debug!(
            "Skirmish team '{}' attacking nearest group with comparison {} value {}",
            team_name,
            comparison,
            value
        );

        let group_arc = self.create_ai_group_from_team(&team_name)?;

        let team_arc = self.get_team_by_name(&team_name)?;
        let controlling_player_id = team_arc
            .read()
            .ok()
            .and_then(|team| team.get_controlling_player_id())
            .ok_or_else(|| {
                ScriptError::ExecutionFailed("Skirmish team has no controlling player".to_string())
            })?;

        let player_list_guard = player_list()
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to lock player list".to_string()))?;
        let controlling_player = player_list_guard
            .get_player(controlling_player_id as i32)
            .cloned()
            .ok_or_else(|| {
                ScriptError::ExecutionFailed("Skirmish team player not found".to_string())
            })?;
        let controlling_player_guard = controlling_player.read().map_err(|_| {
            ScriptError::ExecutionFailed("Failed to read skirmish player".to_string())
        })?;

        let group_center = group_arc
            .read()
            .ok()
            .and_then(|group| group.get_center())
            .ok_or_else(|| {
                ScriptError::ExecutionFailed("Failed to get group center".to_string())
            })?;

        let comparison_type = match comparison {
            0 => ComparisonType::LessThan,
            1 => ComparisonType::LessEqual,
            2 => ComparisonType::Equal,
            3 => ComparisonType::GreaterEqual,
            4 => ComparisonType::Greater,
            5 => ComparisonType::NotEqual,
            _ => ComparisonType::Equal,
        };

        let mut target_loc = group_center;
        {
            if let Ok(manager) = get_object_manager().read() {
                let mut best_dist = f32::MAX;
                let mut best_pos = None;

                for obj_id in manager.all_object_ids() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_destroyed() {
                        continue;
                    }
                    if obj_guard
                        .get_status_bits()
                        .test(crate::common::ObjectStatusTypes::UnderConstruction)
                    {
                        continue;
                    }
                    let Some(obj_player_id) = obj_guard.get_controlling_player_id() else {
                        continue;
                    };
                    if obj_player_id == controlling_player_id {
                        continue;
                    }

                    let Some(target_player_arc) =
                        player_list_guard.get_player(obj_player_id as i32).cloned()
                    else {
                        continue;
                    };
                    let Ok(target_player_guard) = target_player_arc.read() else {
                        continue;
                    };
                    if controlling_player_guard.get_relationship(&target_player_guard)
                        != Relationship::Enemies
                    {
                        continue;
                    }

                    let build_cost = obj_guard.get_build_cost();
                    let meets_value = match comparison_type {
                        ComparisonType::LessThan => build_cost < value,
                        ComparisonType::LessEqual => build_cost <= value,
                        ComparisonType::Equal => build_cost == value,
                        ComparisonType::GreaterEqual => build_cost >= value,
                        ComparisonType::Greater => build_cost > value,
                        ComparisonType::NotEqual => build_cost != value,
                    };
                    if !meets_value {
                        continue;
                    }

                    let pos = obj_guard.get_position();
                    let dx = pos.x - group_center.x;
                    let dy = pos.y - group_center.y;
                    let dist = dx * dx + dy * dy;
                    if dist < best_dist {
                        best_dist = dist;
                        best_pos = Some(*pos);
                    }
                }

                if let Some(pos) = best_pos {
                    target_loc = pos;
                }
            }
        }

        if let Ok(group) = group_arc.read() {
            group.group_attack_move_to_position(&target_loc, CommandSourceType::FromScript);
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_skirmish_perform_command_button_on_most_valuable_object(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.resolve_team_name_token(&self.get_string_param(action, 0)?);
        let ability = self.get_string_param(action, 1)?;
        let range = self.get_real_param(action, 2)?;
        let _all_team_members = self.get_bool_param_optional(action, 3).unwrap_or(false);

        log::debug!(
            "Skirmish team '{}' performing command '{}' on most valuable object (range {})",
            team_name,
            ability,
            range
        );

        let group_arc = self.create_ai_group_from_team(&team_name)?;

        let control_bar = get_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        let Some(command_button) = control_bar.find_command_button_by_name(&ability) else {
            return Ok(ScriptActionResult::Success);
        };

        let source_obj = if let Some(template) = command_button.get_special_power_template() {
            group_arc
                .read()
                .ok()
                .and_then(|group| group.get_special_power_source_object(template.get_id()))
        } else {
            group_arc
                .read()
                .ok()
                .and_then(|group| group.get_command_button_source_object(command_button.get_id()))
        };

        let Some(source_obj) = source_obj else {
            return Ok(ScriptActionResult::Success);
        };

        let source_guard = match source_obj.read() {
            Ok(guard) => guard,
            Err(_) => return Ok(ScriptActionResult::Success),
        };

        let group_center = group_arc
            .read()
            .ok()
            .and_then(|group| group.get_center())
            .ok_or_else(|| {
                ScriptError::ExecutionFailed("Failed to get group center".to_string())
            })?;

        let target_ids = crate::helpers::ThePartitionManager::get()
            .map(|mgr| mgr.get_objects_in_range(&group_center, range))
            .unwrap_or_default();

        let options =
            SpecialPowerCommandOption::from_bits_truncate(command_button.get_options_bits());
        let requires_object_target = options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        );

        let mut best_target: Option<Arc<RwLock<crate::object::Object>>> = None;
        let mut best_cost = i32::MIN;

        for obj_id in target_ids {
            let Some(target_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(target_guard) = target_arc.read() else {
                continue;
            };
            if target_guard.is_destroyed() {
                continue;
            }
            if target_guard
                .get_status_bits()
                .test(crate::common::ObjectStatusTypes::UnderConstruction)
            {
                continue;
            }
            if target_guard.is_off_map() != source_guard.is_off_map() {
                continue;
            }

            let relationship = source_guard.relationship_to(&target_guard);
            let relationship_ok = if requires_object_target {
                (options.contains(SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT)
                    && relationship == Relationship::Enemies)
                    || (options.contains(SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT)
                        && relationship == Relationship::Neutral)
                    || (options.contains(SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT)
                        && matches!(relationship, Relationship::Allies))
                    || (!options.intersects(
                        SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT,
                    ) && relationship == Relationship::Enemies)
            } else {
                relationship == Relationship::Enemies
            };
            if !relationship_ok {
                continue;
            }

            if options.contains(SpecialPowerCommandOption::NEED_TARGET_PRISONER)
                && !target_guard.is_captured()
            {
                continue;
            }

            let cost = target_guard.get_build_cost();
            if cost > best_cost {
                best_cost = cost;
                best_target = Some(target_arc.clone());
            }
        }

        if let Some(target_arc) = best_target {
            if let Ok(target_guard) = target_arc.read() {
                let _ = source_guard.do_command_button_at_object(
                    command_button.get_id(),
                    &target_guard,
                    CommandSourceType::FromScript,
                );
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_skirmish_wait_for_command_button_available_all(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 1)?;
        let command_button = self.get_string_param(action, 2)?;
        log::debug!(
            "Skirmish waiting for command '{}' available (all) on team '{}'",
            command_button,
            team_name
        );

        let ready =
            self.eval_skirmish_command_button_ready_by_name(&team_name, &command_button, true)?;
        if ready {
            Ok(ScriptActionResult::Success)
        } else {
            Ok(ScriptActionResult::Pending(1.0))
        }
    }

    pub(crate) fn do_skirmish_wait_for_command_button_available_partial(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let team_name = self.get_string_param(action, 1)?;
        let command_button = self.get_string_param(action, 2)?;
        log::debug!(
            "Skirmish waiting for command '{}' available (partial) on team '{}'",
            command_button,
            team_name
        );

        let ready =
            self.eval_skirmish_command_button_ready_by_name(&team_name, &command_button, false)?;
        if ready {
            Ok(ScriptActionResult::Success)
        } else {
            Ok(ScriptActionResult::Pending(1.0))
        }
    }

    pub(crate) fn do_ai_player_build_supply_center(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let building_type = self.get_string_param(action, 1)?;
        let cash = self.get_int_param(action, 2)?;
        log::debug!(
            "AI player '{}' building supply center '{}' with cash {}",
            player_name,
            building_type,
            cash
        );
        let building = building_type.clone();
        self.with_named_player_ai(&player_name, |ai_player| {
            let _ = ai_player.build_by_supplies(cash, &building);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_ai_player_build_upgrade(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let upgrade_name = self.get_string_param(action, 1)?;
        log::debug!(
            "AI player '{}' building upgrade '{}'",
            player_name,
            upgrade_name
        );
        let upgrade = upgrade_name.clone();
        self.with_named_player_ai(&player_name, |ai_player| {
            let _ = ai_player.build_upgrade(&upgrade);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_ai_player_build_type_nearest_team(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        let build_type = self.get_string_param(action, 1)?;
        let team_name = self.get_string_param(action, 2)?;
        log::debug!(
            "AI player '{}' building '{}' nearest team '{}'",
            player_name,
            build_type,
            team_name
        );

        let team_factory = get_team_factory();
        let team_arc = team_factory
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name));
        let Some(team_arc) = team_arc else {
            return Ok(ScriptActionResult::Success);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(location) = team_guard.get_estimate_team_position() else {
            return Ok(ScriptActionResult::Success);
        };

        let building = build_type.clone();
        self.with_named_player_ai(&player_name, |ai_player| {
            let _ = ai_player.build_specific_building_near_location(&building, location);
        });
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_idle_all_units(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Idling all units for '{}'", player_name);
        if let Ok(list) = player_list().read() {
            if !player_name.is_empty() {
                if let Some(player_arc) = list.find_player_by_name(&player_name) {
                    if let Ok(mut player_guard) = player_arc.write() {
                        player_guard
                            .set_units_should_idle_or_resume(true, CommandSourceType::FromScript);
                    }
                }
            } else {
                for player_arc in list.iter() {
                    if let Ok(mut player_guard) = player_arc.write() {
                        if player_guard.get_player_type() == PlayerType::Human {
                            player_guard.set_units_should_idle_or_resume(
                                true,
                                CommandSourceType::FromScript,
                            );
                        }
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_resume_supply_trucking(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let player_name = self.get_string_param(action, 0)?;
        log::debug!("Resuming supply trucking for '{}'", player_name);
        if let Ok(list) = player_list().read() {
            if !player_name.is_empty() {
                if let Some(player_arc) = list.find_player_by_name(&player_name) {
                    if let Ok(mut player_guard) = player_arc.write() {
                        player_guard
                            .set_units_should_idle_or_resume(false, CommandSourceType::FromScript);
                    }
                }
            } else {
                for player_arc in list.iter() {
                    if let Ok(mut player_guard) = player_arc.write() {
                        if player_guard.get_player_type() == PlayerType::Human {
                            player_guard.set_units_should_idle_or_resume(
                                false,
                                CommandSourceType::FromScript,
                            );
                        }
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }

    // ============================================================================
    // EVA/MISC ACTION IMPLEMENTATIONS
    // ============================================================================

    pub(crate) fn do_eva_set_enabled_disabled(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let enabled = self.get_int_param(action, 0)? != 0;
        log::debug!("EVA enabled: {}", enabled);
        if let Err(err) = crate::helpers::TheEva::set_enabled(enabled) {
            log::warn!("Failed to update EVA enabled state: {}", err);
        }
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_options_set_occlusion_mode(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let mode = self.get_int_param(action, 0)?;
        TheGameLogic::set_show_behind_building_markers(mode != 0);
        log::debug!("Setting occlusion mode to {}", mode);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_options_set_draw_icon_ui_mode(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let mode = self.get_int_param(action, 0)?;
        TheGameLogic::set_draw_icon_ui(mode != 0);
        log::debug!("Setting draw icon UI mode to {}", mode);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_options_set_particle_cap_mode(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let mode = self.get_int_param(action, 0)?;
        TheGameLogic::set_show_dynamic_lod(mode != 0);
        log::debug!("Setting particle cap mode to {}", mode);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_exit_specific_building(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let building_name = self.get_string_param(action, 0)?;
        log::debug!("Exiting specific building '{}'", building_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(building_id)) = tracker.get_object_id(&building_name) else {
            return Ok(ScriptActionResult::Success);
        };
        let Some(building_obj) = TheGameLogic::find_object_by_id(building_id) else {
            return Ok(ScriptActionResult::Success);
        };

        if let Ok(mut building_guard) = building_obj.write() {
            if !building_guard.is_kind_of(crate::common::KindOf::Structure) {
                return Ok(ScriptActionResult::Success);
            }

            if let Some(ai_arc) = building_guard.get_ai_update_interface() {
                let _ = building_guard.leave_group();
                if let Ok(mut ai_guard) = ai_arc.lock() {
                    let _ = ai_guard.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                    let params = AiCommandParams::new(
                        AiCommandType::Evacuate,
                        CommandSourceType::FromScript,
                    );
                    let _ = ai_guard.execute_command(&params);
                }
                return Ok(ScriptActionResult::Success);
            }

            if let Some(contain) = building_guard.get_contain() {
                if let Ok(mut contain_guard) = contain.lock() {
                    let _ = contain_guard.remove_all_contained(false);
                }
            }
        }

        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_enable_scoring(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Enabling scoring");
        TheGameLogic::set_scoring_enabled(true);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_disable_scoring(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::debug!("Disabling scoring");
        TheGameLogic::set_scoring_enabled(false);
        Ok(ScriptActionResult::Success)
    }

    pub(crate) fn do_set_train_held(
        &mut self,
        action: &ScriptAction,
    ) -> Result<ScriptActionResult, ScriptError> {
        let loco_name = self.get_string_param(action, 0)?;
        let held = self.get_int_param(action, 1)? != 0;
        log::debug!("Setting train '{}' held: {}", loco_name, held);

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&loco_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    if let Some(module) = obj_guard.find_update_module("RailroadBehavior") {
                        module.with_module(|module| {
                            if let Some(train_control) = module.get_train_control_interface() {
                                train_control.set_held(held);
                            }
                        });
                    }
                }
            }
        }
        Ok(ScriptActionResult::Success)
    }
}
