//! Skirmish and trigger-area comparison condition evaluators
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;

impl ScriptConditionEvaluator {
    // ============================================================================
    // SKIRMISH CONDITION HANDLERS
    // ============================================================================

    pub(crate) fn eval_skirmish_special_power_ready(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ ScriptConditions::evaluateSkirmishSpecialPowerIsReady (line 1878-1922)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if skirmish special power '{}' ready for '{}'",
            power_name,
            player_name
        );

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(ready) =
                crate::scripting::host_eval_skirmish_special_power_ready(&player_name, &power_name)
            {
                return Ok(if ready {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
        }

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let Some(store) = get_special_power_store() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(template) = store.find_special_power_template(&power_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let template_name = template.get_name().to_string();

        for object_id in player.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if obj.is_destroyed()
                || obj.is_effectively_dead()
                || obj.is_disabled()
                || obj
                    .get_status_bits()
                    .contains(crate::common::ObjectStatusMaskType::UNDER_CONSTRUCTION)
            {
                continue;
            }
            let ready = obj
                .with_special_power_module_interface_by_name(&template_name, |module| {
                    module.is_ready()
                })
                .unwrap_or(false);
            if ready {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_skirmish_value_in_area(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: evaluateSkirmishValueInArea(SIDE, COMPARISON, INT, TRIGGER)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let compare_value = self.get_condition_int_param(condition, 2)?;
        let area_name = self.get_condition_string_param(condition, 3)?;
        log::debug!(
            "Evaluating skirmish value in area '{}' for '{}' {:?} {}",
            area_name,
            player_name,
            comparison,
            compare_value
        );

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(ok) = crate::scripting::host_eval_skirmish_value_in_area(
                &player_name,
                comparison as i32,
                compare_value,
                &area_name,
            ) {
                return Ok(if ok {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
        }

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index() as i32;

        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .map_err(|e| ScriptError::EvaluationFailed(e.to_string()))?;

        let mut total_cost: i32 = 0;
        for object_id in objects_in_area {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };

            // C++ !KINDOF_INERT && isInside && !isEffectivelyDead
            if obj.is_effectively_dead() || obj.is_destroyed() {
                continue;
            }
            if obj.is_kind_of(crate::common::KindOf::Inert) {
                continue;
            }
            let owner = obj
                .get_controlling_player_id()
                .map(|id| id as i32)
                .unwrap_or(-1);
            if owner != player_index {
                continue;
            }

            total_cost = total_cost.saturating_add(obj.get_template().get_build_cost());
        }

        let result = match comparison {
            ComparisonType::LessThan => total_cost < compare_value,
            ComparisonType::LessEqual => total_cost <= compare_value,
            ComparisonType::Equal => total_cost == compare_value,
            ComparisonType::GreaterEqual => total_cost >= compare_value,
            ComparisonType::Greater => total_cost > compare_value,
            ComparisonType::NotEqual => total_cost != compare_value,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_player_faction(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: evaluateSkirmishPlayerIsFaction(SIDE, FACTION)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let faction = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating skirmish player '{}' faction == '{}'",
            player_name,
            faction
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(if player.get_side() == &faction {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_supplies_value_within_distance(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let distance = self.get_condition_real_param(condition, 1)?;
        let area_name = self.get_condition_string_param(condition, 2)?;
        let compare_value = self.get_condition_real_param(condition, 3)?;

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(ok) = crate::scripting::host_eval_skirmish_supplies_value_within_distance(
                &player_name,
                distance,
                &area_name,
                compare_value,
            ) {
                return Ok(if ok {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
            // Live host: leftover PlayerList/partition are empty. C++
            // playerFromParam / missing trigger / no warehouses is false.
            return Ok(ScriptConditionResult::False);
        }

        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .ok_or_else(|| ScriptError::PlayerNotFound(player_name.clone()))?;
        let player_guard = player_arc
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read player".to_string()))?;

        let trigger = self.get_trigger_area(&area_name)?;
        let center = trigger.get_center_point();
        let radius = trigger.get_radius() + distance;

        let supply_box_value = player_guard.get_supply_box_value() as f32;

        let mut max_value = 0.0;
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            for obj_id in partition.get_objects_in_range(&center, radius) {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if obj_guard.is_destroyed() || obj_guard.is_off_map() {
                    continue;
                }
                if !obj_guard.is_kind_of(crate::common::KindOf::Structure) {
                    continue;
                }

                let allow_affiliation =
                    if let Some(owner_id) = obj_guard.get_controlling_player_id() {
                        if owner_id == player_guard.get_player_index() as u32 {
                            true
                        } else if let Some(owner_arc) = player_list()
                            .read()
                            .ok()
                            .and_then(|list| list.get_player(owner_id as i32).cloned())
                        {
                            if let Ok(owner_guard) = owner_arc.read() {
                                player_guard.get_relationship(&owner_guard) == Relationship::Neutral
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                if !allow_affiliation {
                    continue;
                }

                let Some(module) = obj_guard.find_update_module("SupplyWarehouseDockUpdate") else {
                    continue;
                };
                let mut boxes = None;
                module.with_module(|module| {
                    if let Some(warehouse) = module.get_supply_warehouse_dock_interface() {
                        boxes = Some(warehouse.boxes_stored());
                    }
                });
                let Some(boxes) = boxes else {
                    continue;
                };

                let value = supply_box_value * boxes as f32;
                if value > max_value {
                    max_value = value;
                }
            }
        }

        Ok(if max_value > compare_value {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_tech_building_within_distance(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let leftover_player = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name));

        // C++ playerFromParam: missing player is false without latch.
        // Live leftover PlayerList may be empty — host census still runs.
        if leftover_player.is_none() && !crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(ScriptConditionResult::False);
        }

        // C++ latches the first evaluation in customData forever, after player exists.
        if condition.custom_data == 1 {
            return Ok(ScriptConditionResult::True);
        }
        if condition.custom_data == -1 {
            return Ok(ScriptConditionResult::False);
        }

        let distance = self.get_condition_real_param(condition, 1)?;
        let area_name = self.get_condition_string_param(condition, 2)?;

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(found) = crate::scripting::host_eval_skirmish_tech_building_within_distance(
                &player_name,
                distance,
                &area_name,
            ) {
                condition.custom_data = if found { 1 } else { -1 };
                return Ok(if found {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
            // No host census / missing trigger: C++ false without latch.
            return Ok(ScriptConditionResult::False);
        }

        let player_arc =
            leftover_player.ok_or_else(|| ScriptError::PlayerNotFound(player_name.clone()))?;
        let player_guard = player_arc
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read player".to_string()))?;

        let trigger = self.get_trigger_area(&area_name)?;
        let center = trigger.get_center_point();
        let radius = trigger.get_radius() + distance;

        let mut found = false;
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            for obj_id in partition.get_objects_in_range(&center, radius) {
                let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if obj_guard.is_destroyed() || obj_guard.is_off_map() {
                    continue;
                }
                if !obj_guard.is_kind_of(crate::common::KindOf::TechBuilding) {
                    continue;
                }

                let Some(owner_id) = obj_guard.get_controlling_player_id() else {
                    continue;
                };
                if owner_id == player_guard.get_player_index() as u32 {
                    continue;
                }
                if let Some(owner_arc) = player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(owner_id as i32).cloned())
                {
                    if let Ok(owner_guard) = owner_arc.read() {
                        let rel = player_guard.get_relationship(&owner_guard);
                        if matches!(rel, Relationship::Allies) {
                            continue;
                        }
                    }
                }

                found = true;
                break;
            }
        }

        condition.custom_data = if found { 1 } else { -1 };
        Ok(if found {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_command_button_ready_all(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        self.eval_skirmish_command_button_ready(condition, true)
    }

    pub(crate) fn eval_skirmish_command_button_ready_partial(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        self.eval_skirmish_command_button_ready(condition, false)
    }

    pub(crate) fn eval_skirmish_command_button_ready(
        &self,
        condition: &Condition,
        all_ready: bool,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 1)?;
        let command_button = self.get_condition_string_param(condition, 2)?;
        let ready = self.eval_skirmish_command_button_ready_by_name(
            &team_name,
            &command_button,
            all_ready,
        )?;
        Ok(if ready {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_command_button_ready_by_name(
        &self,
        team_name: &str,
        command_button_name: &str,
        all_ready: bool,
    ) -> Result<bool, ScriptError> {
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(ready) = crate::scripting::host_eval_skirmish_command_button_ready(
                team_name,
                command_button_name,
                all_ready,
            ) {
                return Ok(ready);
            }
        }
        let team_arc = self.get_team_by_name(team_name)?;
        let control_bar = get_control_bar_bridge().ok_or_else(|| {
            ScriptError::ExecutionFailed("Control bar not initialized".to_string())
        })?;
        let Some(command_button) = control_bar.find_command_button_by_name(command_button_name)
        else {
            return Ok(false);
        };

        let members = team_arc
            .read()
            .map(|team| team.get_members().to_vec())
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read team".to_string()))?;

        for obj_id in members {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            let Some(is_ready) = self.command_button_ready_for_object(&obj_guard, command_button)
            else {
                continue;
            };

            if is_ready {
                if !all_ready {
                    return Ok(true);
                }
            } else if all_ready {
                return Ok(false);
            }
        }

        Ok(all_ready)
    }

    pub(crate) fn command_button_ready_for_object(
        &self,
        obj: &crate::object::Object,
        command_button: &crate::command_button::CommandButton,
    ) -> Option<bool> {
        leftover_command_button_ready_for_object(obj, command_button)
    }

    pub(crate) fn eval_skirmish_unowned_faction_unit_exists(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(count) = crate::scripting::host_eval_skirmish_unowned_faction_unit_count() {
                let result = Self::compare_i32(comparison, count, target_count);
                return Ok(if result {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
        }

        let neutral_player = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_neutral_player())
            .ok_or_else(|| ScriptError::ExecutionFailed("Neutral player not found".to_string()))?;
        let neutral_guard = neutral_player.read().map_err(|_| {
            ScriptError::ExecutionFailed("Failed to read neutral player".to_string())
        })?;
        let neutral_id = neutral_guard.get_player_index() as u32;

        let mut count = 0;
        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.get_all_teams() {
                let Ok(team_guard) = team_arc.read() else {
                    continue;
                };
                if team_guard.get_controlling_player_id().unwrap_or(u32::MAX) != neutral_id {
                    continue;
                }
                for obj_id in team_guard.get_members() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(*obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned)
                    {
                        count += 1;
                    }
                }
            }
        }

        let result = match comparison {
            ComparisonType::LessThan => count < target_count,
            ComparisonType::LessEqual => count <= target_count,
            ComparisonType::Equal => count == target_count,
            ComparisonType::GreaterEqual => count >= target_count,
            ComparisonType::Greater => count > target_count,
            ComparisonType::NotEqual => count != target_count,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_player_has_prerequisite_to_build(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let object_type = condition
            .get_parameter(1)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 1 not found".to_string()))?;

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(ok) = crate::scripting::host_eval_skirmish_player_has_prerequisite_to_build(
                &player_name,
                object_type.get_string(),
            ) {
                return Ok(if ok {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
        }
        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .ok_or_else(|| ScriptError::PlayerNotFound(player_name.clone()))?;
        let player_guard = player_arc
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read player".to_string()))?;

        let mut types = crate::object::object_types::ObjectTypes::new();
        let type_name = object_type.get_string();
        if !type_name.is_empty() {
            if let Some(found) =
                with_script_engine_ref(|engine| engine.get_object_types(type_name)).flatten()
            {
                types = found;
            } else {
                types.add_object_type(AsciiString::from(type_name));
            }
        }

        let can_build = types.can_build_any(&player_guard);
        Ok(if can_build {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_player_has_comparison_garrisoned(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(count) = crate::scripting::host_eval_skirmish_garrisoned_count(&player_name)
            {
                let result = Self::compare_i32(comparison, count, target_count);
                return Ok(if result {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
        }

        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .ok_or_else(|| ScriptError::PlayerNotFound(player_name.clone()))?;
        let player_guard = player_arc
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read player".to_string()))?;
        let player_id = player_guard.get_player_index() as u32;

        let mut count = 0;
        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.get_all_teams() {
                let Ok(team_guard) = team_arc.read() else {
                    continue;
                };
                if team_guard.get_controlling_player_id().unwrap_or(u32::MAX) != player_id {
                    continue;
                }
                for obj_id in team_guard.get_members() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(*obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let Some(contain) = obj_guard.get_contain() else {
                        continue;
                    };
                    let Ok(contain_guard) = contain.lock() else {
                        continue;
                    };
                    if contain_guard.is_garrisonable() && contain_guard.get_contained_count() > 0 {
                        count += 1;
                    }
                }
            }
        }

        let result = match comparison {
            ComparisonType::LessThan => count < target_count,
            ComparisonType::LessEqual => count <= target_count,
            ComparisonType::Equal => count == target_count,
            ComparisonType::GreaterEqual => count >= target_count,
            ComparisonType::Greater => count > target_count,
            ComparisonType::NotEqual => count != target_count,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_player_has_comparison_captured_units(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(count) = crate::scripting::host_eval_skirmish_captured_count(&player_name) {
                let result = Self::compare_i32(comparison, count, target_count);
                return Ok(if result {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
        }

        let player_arc = player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(&player_name))
            .ok_or_else(|| ScriptError::PlayerNotFound(player_name.clone()))?;
        let player_guard = player_arc
            .read()
            .map_err(|_| ScriptError::ExecutionFailed("Failed to read player".to_string()))?;
        let player_id = player_guard.get_player_index() as u32;

        let mut count = 0;
        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.get_all_teams() {
                let Ok(team_guard) = team_arc.read() else {
                    continue;
                };
                if team_guard.get_controlling_player_id().unwrap_or(u32::MAX) != player_id {
                    continue;
                }
                for obj_id in team_guard.get_members() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(*obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_captured() {
                        count += 1;
                    }
                }
            }
        }

        let result = match comparison {
            ComparisonType::LessThan => count < target_count,
            ComparisonType::LessEqual => count <= target_count,
            ComparisonType::Equal => count == target_count,
            ComparisonType::GreaterEqual => count >= target_count,
            ComparisonType::Greater => count > target_count,
            ComparisonType::NotEqual => count != target_count,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_named_area_exist(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ ignores parameter 0 here and uses the trigger-name parameter.
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!("Evaluating if skirmish named area '{}' exists", area_name);

        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let exists = terrain.get_trigger_area_by_name(&area_name).is_some();

        Ok(if exists {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_player_has_units_in_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if skirmish player '{}' has units in area '{}'",
            player_name,
            area_name
        );

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(ok) = crate::scripting::host_eval_skirmish_player_has_units_in_area(
                &player_name,
                &area_name,
            ) {
                condition.custom_data = if ok { 1 } else { -1 };
                return Ok(if ok {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
        }

        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(trigger) = terrain.get_trigger_area_by_name(&area_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index();

        let mut any_changes = condition.custom_data == 0;
        if !any_changes {
            if let Ok(factory) = get_team_factory().lock() {
                for team_arc in factory.get_all_teams() {
                    let Ok(team_guard) = team_arc.read() else {
                        continue;
                    };
                    if team_guard.get_controlling_player_id().map(|id| id as i32)
                        != Some(player_index)
                    {
                        continue;
                    }
                    if team_guard.did_enter_or_exit() {
                        any_changes = true;
                        break;
                    }
                }
            }
        }

        if !any_changes
            && with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
                .is_some_and(|frame| frame > condition.custom_frame)
        {
            any_changes = true;
        }

        if !any_changes {
            return Ok(if condition.custom_data == 1 {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        let mut count = 0;
        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.get_all_teams() {
                let Ok(team_guard) = team_arc.read() else {
                    continue;
                };
                if team_guard.get_controlling_player_id().map(|id| id as i32) != Some(player_index)
                {
                    continue;
                }
                for obj_id in team_guard.get_members() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(*obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let pos = obj_guard.get_position();
                    let point =
                        crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
                    if trigger.point_in_trigger_int(&point) {
                        if !(obj_guard.is_effectively_dead()
                            || obj_guard.is_kind_of(crate::common::KindOf::Inert)
                            || obj_guard.is_kind_of(crate::common::KindOf::Projectile))
                        {
                            count += 1;
                        }
                    }
                }
            }
        }

        let comparison = count > 0;
        condition.custom_data = if comparison { 1 } else { -1 };
        if let Some(frame) =
            with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
        {
            condition.custom_frame = frame;
        }

        Ok(if comparison {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_player_has_been_attacked_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: evaluateSkirmishPlayerHasBeenAttackedByPlayer(SIDE, SIDE)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let attacked_by_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if skirmish player '{}' has been attacked by '{}'",
            player_name,
            attacked_by_name
        );

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(src_arc) = players.find_player_by_name(&attacked_by_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(src) = src_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let attacked = player.get_attacked_by(src.get_player_index() as i32);
        Ok(if attacked {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_skirmish_player_is_outside_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: return !evaluateSkirmishPlayerHasUnitsInArea(...)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        if players.find_player_by_name(&player_name).is_none() {
            return Ok(ScriptConditionResult::False);
        }
        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        if terrain.get_trigger_area_by_name(&area_name).is_none() {
            return Ok(ScriptConditionResult::False);
        }

        match self.eval_skirmish_player_has_units_in_area(condition)? {
            ScriptConditionResult::True => Ok(ScriptConditionResult::False),
            ScriptConditionResult::False => Ok(ScriptConditionResult::True),
            ScriptConditionResult::Error(e) => Ok(ScriptConditionResult::Error(e)),
        }
    }

    pub(crate) fn eval_skirmish_player_has_discovered_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateSkirmishPlayerHasDiscoveredPlayer(SIDE, SIDE)
        let player_name = self.get_condition_string_param(condition, 0)?;
        let discovered_by_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if skirmish player '{}' has been discovered by '{}'",
            player_name,
            discovered_by_name
        );

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(ok) = crate::scripting::host_eval_skirmish_player_has_discovered_player(
                &player_name,
                &discovered_by_name,
            ) {
                return Ok(if ok {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
        }

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(discovered_by_arc) = players.find_player_by_name(&discovered_by_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(discovered_by) = discovered_by_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let player_index = player.get_player_index();
        let discovered_by_index = discovered_by.get_player_index();

        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.get_all_teams() {
                let Ok(team_guard) = team_arc.read() else {
                    continue;
                };
                if team_guard.get_controlling_player_id().map(|id| id as i32) != Some(player_index)
                {
                    continue;
                }

                for obj_id in team_guard.get_members() {
                    let Some(obj_arc) = TheGameLogic::find_object_by_id(*obj_id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let shroud_status = obj_guard.get_shrouded_status(discovered_by_index);
                    if matches!(
                        shroud_status,
                        crate::common::ObjectShroudStatus::Clear
                            | crate::common::ObjectShroudStatus::PartialClear
                    ) {
                        return Ok(ScriptConditionResult::True);
                    }
                }
            }
        }

        Ok(ScriptConditionResult::False)
    }

    // ============================================================================
    // AREA CONDITION HANDLERS
    // ============================================================================

    pub(crate) fn eval_player_has_comparison_unit_type_in_trigger_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;
        let type_name = self.get_condition_string_param(condition, 3)?;
        let trigger_name = self.get_condition_string_param(condition, 4)?;
        log::debug!(
            "Evaluating player '{}' has unit type '{}' in area '{}' {:?} {}",
            player_name,
            type_name,
            trigger_name,
            comparison,
            target_count
        );

        // Live host never registers crate objects, so Player::get_all_objects is empty.
        // C++ evaluatePlayerHasUnitTypeInArea walks live Team member lists.
        let types = self.resolve_object_types_param(&type_name);
        let type_names: Vec<String> = {
            let names: Vec<String> = types.iter().map(|s| s.as_str().to_string()).collect();
            if names.is_empty() {
                vec![type_name.clone()]
            } else {
                names
            }
        };
        if let Some(count) = crate::scripting::host_count_player_type_in_area(
            &player_name,
            &trigger_name,
            &type_names,
        ) {
            let comparison_result = Self::compare_i32(comparison, count, target_count);
            condition.custom_data = if comparison_result { 1 } else { -1 };
            if let Some(frame) =
                with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
            {
                condition.custom_frame = frame;
            }
            return Ok(if comparison_result {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(trigger) = terrain.get_trigger_area_by_name(&trigger_name).cloned() else {
            return Ok(ScriptConditionResult::False);
        };
        drop(terrain);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index();
        let player_object_ids = player.get_all_objects();
        drop(player);
        drop(players);

        let mut any_changes = condition.custom_data == 0;
        if !any_changes {
            if let Ok(factory) = get_team_factory().lock() {
                for team_arc in factory.get_all_teams() {
                    let Ok(team_guard) = team_arc.read() else {
                        continue;
                    };
                    if team_guard.get_controlling_player_id().map(|id| id as i32)
                        != Some(player_index)
                    {
                        continue;
                    }
                    if team_guard.did_enter_or_exit() {
                        any_changes = true;
                        break;
                    }
                }
            }
        }
        if !any_changes
            && with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
                .is_some_and(|frame| frame > condition.custom_frame)
        {
            any_changes = true;
        }
        if !any_changes {
            return Ok(if condition.custom_data == 1 {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        let mut count = 0i32;
        for object_id in player_object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !types.contains_template(Some(obj_guard.get_template().as_ref())) {
                continue;
            }
            let pos = obj_guard.get_position();
            let point = crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
            if !trigger.point_in_trigger_int(&point) {
                continue;
            }

            // C++ includes crates even though they can be effectively dead/inert.
            let include = !(obj_guard.is_effectively_dead()
                || obj_guard.is_kind_of(crate::common::KindOf::Inert))
                || obj_guard.is_kind_of(crate::common::KindOf::Crate);
            if include {
                count += 1;
            }
        }

        let comparison_result = Self::compare_i32(comparison, count, target_count);
        condition.custom_data = if comparison_result { 1 } else { -1 };
        if let Some(frame) =
            with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
        {
            condition.custom_frame = frame;
        }

        Ok(if comparison_result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_player_has_comparison_unit_kind_in_trigger_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;
        let kind_param = condition
            .get_parameter(3)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 3 not found".to_string()))?;
        let trigger_name = self.get_condition_string_param(condition, 4)?;
        log::debug!(
            "Evaluating player '{}' has kind '{}' in area '{}' {:?} {}",
            player_name,
            kind_param.get_int(),
            trigger_name,
            comparison,
            target_count
        );

        let kind = if kind_param.get_int() >= 0 {
            crate::common::ALL_KIND_OF
                .get(kind_param.get_int() as usize)
                .copied()
        } else {
            None
        }
        .or_else(|| parse_kind_of(kind_param.get_string()));
        let Some(kind) = kind else {
            return Ok(ScriptConditionResult::False);
        };

        // Live host never registers crate objects, so Player::get_all_objects is empty.
        // C++ evaluatePlayerHasUnitKindInArea walks live Team member lists.
        if let Some(count) =
            crate::scripting::host_count_player_kind_in_area(&player_name, &trigger_name, kind)
        {
            let comparison_result = Self::compare_i32(comparison, count, target_count);
            // Match C++: this writes frame object count into custom_data (legacy quirk).
            if let Some(frame) =
                with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
            {
                condition.custom_data = frame as i32;
            }
            return Ok(if comparison_result {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(trigger) = terrain.get_trigger_area_by_name(&trigger_name).cloned() else {
            return Ok(ScriptConditionResult::False);
        };
        drop(terrain);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = player.get_player_index();
        let player_object_ids = player.get_all_objects();
        drop(player);
        drop(players);

        let mut any_changes = condition.custom_data == 0;
        if !any_changes {
            if let Ok(factory) = get_team_factory().lock() {
                for team_arc in factory.get_all_teams() {
                    let Ok(team_guard) = team_arc.read() else {
                        continue;
                    };
                    if team_guard.get_controlling_player_id().map(|id| id as i32)
                        != Some(player_index)
                    {
                        continue;
                    }
                    if team_guard.did_enter_or_exit() {
                        any_changes = true;
                        break;
                    }
                }
            }
        }
        if !any_changes
            && with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
                .is_some_and(|frame| frame > condition.custom_frame)
        {
            any_changes = true;
        }
        if !any_changes {
            return Ok(if condition.custom_data == 1 {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        let mut count = 0i32;
        for object_id in player_object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if !obj_guard.is_kind_of(kind) {
                continue;
            }
            let pos = obj_guard.get_position();
            let point = crate::common::ICoord3D::new(pos.x as i32, pos.y as i32, pos.z as i32);
            if !trigger.point_in_trigger_int(&point) {
                continue;
            }
            if !(obj_guard.is_effectively_dead()
                || obj_guard.is_kind_of(crate::common::KindOf::Inert))
            {
                count += 1;
            }
        }

        let comparison_result = Self::compare_i32(comparison, count, target_count);

        // Match C++ behavior: this writes frame object count into custom_data (legacy quirk).
        if let Some(frame) =
            with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
        {
            condition.custom_data = frame as i32;
        }

        Ok(if comparison_result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }
}

pub(crate) fn leftover_command_button_ready_for_object(
    obj: &crate::object::Object,
    command_button: &crate::command_button::CommandButton,
) -> Option<bool> {
    if let Some(template) = command_button.get_special_power_template() {
        if !obj.has_special_power(template.get_special_power_type()) {
            return None;
        }
        return obj
            .with_special_power_module_interface_by_name(template.get_name(), |sp_module| {
                sp_module.is_ready()
            })
            .or(Some(false));
    }

    let Some(upgrade) = command_button.get_upgrade_template() else {
        return None;
    };

    if upgrade.get_upgrade_type() == crate::upgrade::UpgradeType::Object {
        if obj.has_upgrade(upgrade) || !obj.affected_by_upgrade(upgrade) {
            return Some(false);
        }
    }

    if !obj.can_produce_upgrade(upgrade) {
        return Some(false);
    }

    let player_id = obj.get_controlling_player_id()?;
    let player_arc = {
        let list = player_list().read().ok()?;
        list.get_player(player_id as i32).cloned()?
    };
    let player_guard = player_arc.read().ok()?;

    if player_guard.has_upgrade_complete(upgrade) || player_guard.has_upgrade_in_production(upgrade)
    {
        return Some(false);
    }

    Some(true)
}

#[cfg(test)]
mod tech_building_latch_tests {
    use super::*;
    use crate::scripting::{
        Condition, ConditionType, HostScriptQuerySnapshot, HostTechBuildingCensus, Parameter,
        ParameterType, ScriptConditionResult, clear_host_script_query_snapshot,
        set_host_script_query_snapshot,
    };
    use std::sync::{Arc, RwLock};
    fn tech_condition() -> Condition {
        let mut condition = Condition::new(ConditionType::SkirmishTechBuildingWithinDistance);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "PlyrAmerica".into(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_real(ParameterType::Real, 200.0))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::TriggerArea,
                "HomeBase".into(),
            ))
            .unwrap();
        condition
    }

    #[test]
    fn empty_leftover_without_snapshot_does_not_latch() {
        crate::object::registry::OBJECT_REGISTRY.clear();
        clear_host_script_query_snapshot();
        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
        let mut condition = tech_condition();
        assert_eq!(
            evaluator.evaluate_condition(&mut condition).unwrap(),
            ScriptConditionResult::False
        );
        assert_eq!(condition.custom_data, 0);
    }

    #[test]
    fn empty_leftover_host_census_latches_true() {
        crate::object::registry::OBJECT_REGISTRY.clear();
        clear_host_script_query_snapshot();
        let mut snap = HostScriptQuerySnapshot::default();
        snap.areas.insert("HomeBase".into(), (0.0, 0.0, 20.0, 20.0));
        snap.tech_buildings.push(HostTechBuildingCensus {
            x: 10.0,
            z: 10.0,
            owner_player: String::new(),
            team: 3,
            off_map: false,
        });
        set_host_script_query_snapshot(snap);
        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
        let mut condition = tech_condition();
        assert_eq!(
            evaluator.evaluate_condition(&mut condition).unwrap(),
            ScriptConditionResult::True
        );
        assert_eq!(condition.custom_data, 1);
        clear_host_script_query_snapshot();
        assert_eq!(
            evaluator.evaluate_condition(&mut condition).unwrap(),
            ScriptConditionResult::True
        );
    }
}
