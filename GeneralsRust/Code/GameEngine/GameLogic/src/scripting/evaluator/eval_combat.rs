// Attacked, dying, totally-dead, and selected condition evaluators
//
// Split from `scripting/evaluator.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEvaluator {
    fn evaluate_named_attacked_by_object_type_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByObjecttype condition missing unit parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByObjecttype condition missing type parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(body) = obj_guard.get_body_module() else {
            return Ok(false);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(false);
        };
        let Some(last) = body_guard.get_last_damage_info() else {
            return Ok(false);
        };

        let types = self.resolve_object_types(type_param);
        if let Some(template) = last.input.source_template.as_deref() {
            return Ok(types.contains_template(Some(template)));
        }

        let attacker_id = last.input.source_id;
        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            return Ok(false);
        };
        let Ok(attacker_guard) = attacker_arc.read() else {
            return Ok(false);
        };
        Ok(types.contains_template(Some(attacker_guard.get_template())))
    }

    fn evaluate_team_attacked_by_object_type_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByObjecttype condition missing team parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByObjecttype condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let types = self.resolve_object_types(type_param);

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            for member_id in team_guard.get_members() {
                let Some(member_arc) = TheGameLogic::find_object_by_id(*member_id) else {
                    continue;
                };
                let Ok(member_guard) = member_arc.read() else {
                    continue;
                };
                let Some(body) = member_guard.get_body_module() else {
                    continue;
                };
                let Ok(body_guard) = body.lock() else {
                    continue;
                };
                let Some(last) = body_guard.get_last_damage_info() else {
                    continue;
                };

                if let Some(template) = last.input.source_template.as_deref() {
                    if types.contains_template(Some(template)) {
                        return Ok(true);
                    }
                    continue;
                }

                let attacker_id = last.input.source_id;
                let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                    continue;
                };
                let Ok(attacker_guard) = attacker_arc.read() else {
                    continue;
                };
                if types.contains_template(Some(attacker_guard.get_template())) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn evaluate_named_attacked_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByPlayer condition missing unit parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedAttackedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(body) = obj_guard.get_body_module() else {
            return Ok(false);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(false);
        };
        let Some(last) = body_guard.get_last_damage_info() else {
            return Ok(false);
        };

        let target_player = self.resolve_player_from_param(player_param);
        if target_player.is_none() {
            return Ok(false);
        }

        if last.input.source_player_mask != PlayerMaskType::none() {
            if let Some(target_player) = target_player.as_ref() {
                if let Ok(target_guard) = target_player.read() {
                    if last
                        .input
                        .source_player_mask
                        .intersects(target_guard.get_player_mask())
                    {
                        return Ok(true);
                    }
                }
            }
        }

        let attacker_id = last.input.source_id;
        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            return Ok(false);
        };
        let Ok(attacker_guard) = attacker_arc.read() else {
            return Ok(false);
        };
        let Some(attacker_player) = attacker_guard.get_controlling_player() else {
            return Ok(false);
        };
        let Some(target_player) = target_player else {
            return Ok(false);
        };
        Ok(Arc::ptr_eq(&attacker_player, &target_player))
    }

    fn evaluate_team_attacked_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByPlayer condition missing team parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamAttackedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let target_player = self.resolve_player_from_param(player_param);
        if target_player.is_none() {
            return Ok(false);
        }

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };
            for member_id in team_guard.get_members() {
                let Some(member_arc) = TheGameLogic::find_object_by_id(*member_id) else {
                    continue;
                };
                let Ok(member_guard) = member_arc.read() else {
                    continue;
                };
                let Some(body) = member_guard.get_body_module() else {
                    continue;
                };
                let Ok(body_guard) = body.lock() else {
                    continue;
                };
                let Some(last) = body_guard.get_last_damage_info() else {
                    continue;
                };
                let attacker_id = last.input.source_id;
                let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                    continue;
                };
                let Ok(attacker_guard) = attacker_arc.read() else {
                    continue;
                };
                let Some(attacker_player) = attacker_guard.get_controlling_player() else {
                    continue;
                };
                if Arc::ptr_eq(
                    &attacker_player,
                    target_player.as_ref().expect("checked above"),
                ) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn evaluate_named_dying_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("NamedDying condition missing unit parameter".to_string())
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        if let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                return Ok(false);
            };
            let Ok(obj_guard) = obj_arc.read() else {
                return Ok(false);
            };
            return Ok(obj_guard.is_effectively_dead());
        }

        Ok(false)
    }

    fn evaluate_named_totally_dead_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedTotallyDead condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        if tracker.get_object_id(unit_name).ok().flatten().is_some() {
            return Ok(false);
        }
        Ok(tracker.did_object_exist(unit_name).unwrap_or(false))
    }

    fn evaluate_named_selected_condition(
        &self,
        condition: &mut Condition,
    ) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedSelected condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };

        let selection_manager = get_selection_manager();
        let Ok(manager_guard) = selection_manager.read() else {
            return Ok(false);
        };

        let frame_changed = manager_guard.get_frame_selection_changed();
        if condition.custom_data != 0 && condition.custom_frame == frame_changed {
            return Ok(condition.custom_data == 1);
        }

        let mut is_selected = false;
        if let Ok(list) = player_list().read() {
            let local_index = list.get_local_player_index();
            if local_index >= 0 {
                if let Some(selection) = manager_guard.get_player_selection_ref(local_index) {
                    is_selected = selection.is_object_selected(object_id);
                }
            }
        }

        if !is_selected {
            is_selected = manager_guard.is_object_selected_by_any_player(object_id);
        }

        condition.custom_data = if is_selected { 1 } else { -1 };
        condition.custom_frame = frame_changed;
        Ok(is_selected)
    }
}
