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

        return OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                let Some(body) = obj_guard.get_body_module() else {
                    return Ok(false);
                };
                let Some(last) = body.get_last_damage_info() else {
                    return Ok(false);
                };

                let types = self.resolve_object_types(type_param);
                if let Some(template) = last.input.source_template.as_deref() {
                    return Ok(types.contains_template(Some(template)));
                }

                let attacker_id = last.input.source_id;
                OBJECT_REGISTRY
                    .with_object(attacker_id, |attacker_guard| {
                        Ok(types.contains_template(Some(attacker_guard.get_template())))
                    })
                    .unwrap_or(Ok(false))
            })
            .unwrap_or(Ok(false));
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

        for team_id in self.resolve_team_instances(&team_name) {
            let Some(members) =
                crate::team::with_team(team_id, |team_guard| team_guard.get_members().to_vec())
            else {
                continue;
            };
            for member_id in members {
                let hit = OBJECT_REGISTRY.with_object(member_id, |member_guard| {
                    let Some(body) = member_guard.get_body_module() else {
                        return false;
                    };
                    let Some(last) = body.get_last_damage_info() else {
                        return false;
                    };

                    if let Some(template) = last.input.source_template.as_deref() {
                        return types.contains_template(Some(template));
                    }

                    let attacker_id = last.input.source_id;
                    OBJECT_REGISTRY
                        .with_object(attacker_id, |attacker_guard| {
                            types.contains_template(Some(attacker_guard.get_template()))
                        })
                        .unwrap_or(false)
                });
                if hit == Some(true) {
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

        return OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                let Some(body) = obj_guard.get_body_module() else {
                    return Ok(false);
                };
                let Some(last) = body.get_last_damage_info() else {
                    return Ok(false);
                };

                let target_player = self.resolve_player_from_param(player_param);
                if target_player.is_none() {
                    return Ok(false);
                }

                if last.input.source_player_mask != PlayerMaskType::none() {
                    if let Some(target_index) = target_player {
                        if crate::player::with_player(target_index, |target_guard| {
                            last.input
                                .source_player_mask
                                .intersects(target_guard.get_player_mask())
                        })
                        .unwrap_or(false)
                        {
                            return Ok(true);
                        }
                    }
                }

                let attacker_id = last.input.source_id;
                OBJECT_REGISTRY
                    .with_object(attacker_id, |attacker_guard| {
                        let Some(attacker_player) = attacker_guard.get_controlling_player() else {
                            return Ok(false);
                        };
                        let Some(target_player) = target_player else {
                            return Ok(false);
                        };
                        Ok(attacker_player == target_player)
                    })
                    .unwrap_or(Ok(false))
            })
            .unwrap_or(Ok(false));
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

        for team_id in self.resolve_team_instances(&team_name) {
            let Some(members) =
                crate::team::with_team(team_id, |team_guard| team_guard.get_members().to_vec())
            else {
                continue;
            };
            for member_id in members {
                let hit = OBJECT_REGISTRY.with_object(member_id, |member_guard| {
                    let Some(body) = member_guard.get_body_module() else {
                        return false;
                    };
                    let Some(last) = body.get_last_damage_info() else {
                        return false;
                    };
                    let attacker_id = last.input.source_id;
                    OBJECT_REGISTRY
                        .with_object(attacker_id, |attacker_guard| {
                            attacker_guard.get_controlling_player() == target_player
                        })
                        .unwrap_or(false)
                });
                if hit == Some(true) {
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
            let dead = crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |obj_guard| obj_guard.is_effectively_dead());
            return Ok(dead.unwrap_or(false));
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

        // C++ ScriptConditions::evaluateNamedUnitTotallyDead (ScriptConditions.cpp:323-335):
        // if (getUnitNamed) return false; if (didUnitExist) return true; else false.
        if dual_world_registry_unavailable() {
            if crate::scripting::host_script_named_unit_alive(unit_name).is_some() {
                return Ok(false);
            }
            let tracker = get_named_object_tracker();
            return Ok(tracker.did_object_exist(unit_name).unwrap_or(false));
        }


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
