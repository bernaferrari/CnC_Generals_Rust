//! Named-object, unit, camera, building, special-power, media, and miscellaneous condition evaluators
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;

impl ScriptConditionEvaluator {
    // ============================================================================
    // NAMED OBJECT CONDITION HANDLERS
    // ============================================================================

    /// C++ Reference: ScriptConditions::evaluateNamedInsideArea() line 395-415
    /// C++ pattern: Gets object position and calls pTrig->pointInTrigger(iCoord)
    pub(crate) fn eval_named_inside_area(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' is inside area '{}'",
            object_name,
            area_name
        );

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            // Match named.rs: existence is not inside-area. Missing host AABB
            // is False (do not fall through to NamedObjectTracker).
            return Ok(
                if crate::scripting::host_script_named_unit_in_named_area(&object_name, &area_name)
                    .unwrap_or(false)
                {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                },
            );
        }

        // Look up the named object using the tracker
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            // Check if object is in the area using the area tracker
            let area_tracker = get_area_tracker();
            if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                return Ok(if objects_in_area.contains(&object_id) {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
        }
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluateNamedOutsideArea() line 625
    /// C++ simply returns !evaluateNamedInsideArea(...)
    pub(crate) fn eval_named_outside_area(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating named outside area (inverting inside check per C++)");

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            // Do not invert unresolved host area geometry (fail-closed).
            let object_name = self.get_condition_string_param(condition, 0)?;
            let area_name = self.get_condition_string_param(condition, 1)?;
            return Ok(
                match crate::scripting::host_script_named_unit_in_named_area(
                    &object_name,
                    &area_name,
                ) {
                    Some(inside) => {
                        if inside {
                            ScriptConditionResult::False
                        } else {
                            ScriptConditionResult::True
                        }
                    }
                    None => ScriptConditionResult::False,
                },
            );
        }

        // C++ pattern: return !evaluateNamedInsideArea(pUnitParm, pTriggerParm);
        match self.eval_named_inside_area(condition)? {
            ScriptConditionResult::True => Ok(ScriptConditionResult::False),
            ScriptConditionResult::False => Ok(ScriptConditionResult::True),
            ScriptConditionResult::Error(e) => Ok(ScriptConditionResult::Error(e)),
        }
    }

    pub(crate) fn eval_named_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is destroyed", object_name);

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(alive) = crate::scripting::host_script_named_unit_alive(&object_name) {
                return Ok(if alive {
                    ScriptConditionResult::False
                } else {
                    ScriptConditionResult::True
                });
            }
            if crate::scripting::host_script_query_has_any() {
                return Ok(ScriptConditionResult::True);
            }
        }

        // Look up the named object using the tracker
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            // Check if the object exists and is destroyed
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    return Ok(if obj.is_destroyed() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
            // Object ID exists but object not found - considered destroyed
            return Ok(ScriptConditionResult::True);
        }
        // Object not in tracker - considered destroyed
        Ok(ScriptConditionResult::True)
    }

    pub(crate) fn eval_named_not_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is not destroyed", object_name);

        // Invert the destroyed check
        match self.eval_named_destroyed(condition)? {
            ScriptConditionResult::True => Ok(ScriptConditionResult::False),
            ScriptConditionResult::False => Ok(ScriptConditionResult::True),
            ScriptConditionResult::Error(e) => Ok(ScriptConditionResult::Error(e)),
        }
    }

    pub(crate) fn eval_named_attacked_by_object_type(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateNamedAttackedByType
        let object_name = self.get_condition_string_param(condition, 0)?;
        let types_param = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' attacked by object type '{}'",
            object_name,
            types_param
        );

        let wanted_types: Vec<&str> = types_param
            .split(|c| c == ',' || c == '|' || c == ';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if wanted_types.is_empty() {
            return Ok(ScriptConditionResult::False);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(body) = obj.get_body_module() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(last) = body_guard.get_last_damage_info() else {
            return Ok(ScriptConditionResult::False);
        };

        if let Some(template) = &last.input.source_template {
            return Ok(
                if wanted_types
                    .iter()
                    .any(|wanted| template.get_name().as_str() == *wanted)
                {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                },
            );
        }

        // Old system: consult attacker object if source template wasn't set.
        let attacker_id = last.input.source_id;
        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(attacker) = attacker_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let attacker_template = attacker.get_template();

        Ok(
            if wanted_types
                .iter()
                .any(|wanted| attacker_template.get_name().as_str() == *wanted)
            {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    pub(crate) fn eval_named_attacked_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateNamedAttackedByPlayer
        let object_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' attacked by player '{}'",
            object_name,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(body) = obj.get_body_module() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(last) = body_guard.get_last_damage_info() else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(victim_player) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(victim_guard) = victim_player.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let victim_index = victim_guard.get_player_index();

        // Prefer the source player mask if present (C++ does this first).
        let mask_bits = last.input.source_player_mask.bits();
        if mask_bits != 0 {
            let masked_index = mask_bits.trailing_zeros() as i32;
            if masked_index == victim_index {
                return Ok(ScriptConditionResult::True);
            }
        }

        // Fallback to attacker object controlling player.
        let attacker_id = last.input.source_id;
        let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(attacker) = attacker_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(attacker_owner) = attacker.get_controlling_player_id() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(if attacker_owner as i32 == victim_index {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptConditions::evaluateNamedCreated() line 900-907
    /// Note: the original implementation checks whether the named unit exists, not whether it was
    /// created this frame.
    pub(crate) fn eval_named_created(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!(
            "Evaluating if '{}' was created (checking existence per C++)",
            object_name
        );

        // C++ pattern: return (TheScriptEngine->getUnitNamed(pUnitParm->getString()) != NULL);
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            // Verify object actually exists
            if TheGameLogic::find_object_by_id(object_id).is_some() {
                return Ok(ScriptConditionResult::True);
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_named_discovered(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateNamedDiscovered
        let object_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' was discovered by player '{}'",
            object_name,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
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
        let player_id: u32 = match player.get_player_index().try_into() {
            Ok(value) => value,
            Err(_) => return Ok(ScriptConditionResult::False),
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        // We are held, so we are not visible.
        if obj.is_disabled_by_type(crate::common::DisabledType::Held) {
            return Ok(ScriptConditionResult::False);
        }

        // If we are stealthed we are not visible (unless DETECTED or DISGUISED).
        let status = obj.get_status_bits();
        if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
            && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
            && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
        {
            return Ok(ScriptConditionResult::False);
        }

        let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
        let Ok(shroud_mgr) = shroud_mgr.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let shroud_state = shroud_mgr.get_shroud_state(player_id, obj.get_position());

        Ok(
            if matches!(
                shroud_state,
                crate::system::shroud_manager::ShroudState::Visible
                    | crate::system::shroud_manager::ShroudState::Explored
            ) {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    pub(crate) fn eval_named_owned_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' owned by player '{}'",
            object_name,
            player_name
        );

        // Look up the named object
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    // Get controlling player and compare display name
                    if let Some(controlling_player) = obj.get_controlling_player() {
                        if let Ok(player) = controlling_player.read() {
                            return Ok(
                                if player.get_player_name_key()
                                    == NameKeyGenerator::name_to_key(&player_name)
                                {
                                    ScriptConditionResult::True
                                } else {
                                    ScriptConditionResult::False
                                },
                            );
                        }
                    }
                }
            }
        }
        // Object not found or has no owner - condition is false
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_named_reached_waypoints_end(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        let waypoint_path = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' reached waypoints end for path '{}'",
            object_name,
            waypoint_path
        );

        // C++ parity: ScriptConditions::evaluateNamedReachedWaypointsEnd
        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(ai_arc) = obj.get_ai_update_interface() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(ai) = ai_arc.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(completed_waypoint_id) = ai.get_completed_waypoint_id() else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(terrain) = crate::terrain::get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(target_waypoint) = terrain.get_waypoint_by_id(completed_waypoint_id) else {
            return Ok(ScriptConditionResult::False);
        };

        let reached = target_waypoint.get_path_label1().as_str() == waypoint_path
            || target_waypoint.get_path_label2().as_str() == waypoint_path
            || target_waypoint.get_path_label3().as_str() == waypoint_path;
        Ok(if reached {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_named_selected(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // Wave 284: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(ScriptConditionResult::False);
        }

        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is selected", object_name);

        let game_logic = crate::system::game_logic::get_game_logic();
        if game_logic
            .lock()
            .ok()
            .is_some_and(|logic| logic.is_in_multiplayer_game())
        {
            return Ok(ScriptConditionResult::False);
        }

        let Ok(list) = crate::player::player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let local_player_id = list.get_local_player_index();
        if local_player_id < 0 {
            return Ok(ScriptConditionResult::False);
        }

        let selection_manager = crate::commands::get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let selection_changed_frame = manager.get_frame_selection_changed();
        let mut any_changes = condition.custom_data == 0;
        if selection_changed_frame != condition.custom_frame {
            any_changes = true;
        }

        if !any_changes {
            return Ok(if condition.custom_data == 1 {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        let Some(selection) = manager.get_player_selection_ref(local_player_id) else {
            condition.custom_data = -1;
            condition.custom_frame = selection_changed_frame;
            return Ok(ScriptConditionResult::False);
        };

        let wanted = crate::common::AsciiString::from(object_name.as_str());
        let mut is_selected = false;
        for object_id in selection.get_selected_objects() {
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(object_id, |guard| guard.get_name() == &wanted)
                .unwrap_or(false)
            {
                is_selected = true;
                break;
            }
        }

        condition.custom_data = if is_selected { 1 } else { -1 };
        condition.custom_frame = selection_changed_frame;
        Ok(if is_selected {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_named_entered_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' entered area '{}'",
            object_name,
            area_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let area_tracker = crate::scripting::engine::get_area_tracker();
        let last_enter = area_tracker.get_last_enter_frame(&area_name, object_id);
        let last_seen = condition.custom_frame;

        let entered = last_enter.is_some_and(|frame| frame > last_seen);
        if entered {
            condition.custom_frame = last_enter.unwrap_or(last_seen);
        }

        Ok(if entered {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_named_exited_area(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if '{}' exited area '{}'",
            object_name,
            area_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let area_tracker = crate::scripting::engine::get_area_tracker();
        let last_exit = area_tracker.get_last_exit_frame(&area_name, object_id);
        let last_seen = condition.custom_frame;

        let exited = last_exit.is_some_and(|frame| frame > last_seen);
        if exited {
            condition.custom_frame = last_exit.unwrap_or(last_seen);
        }

        Ok(if exited {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_named_dying(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is dying", object_name);

        // Look up the named object
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    // Object is dying if destroyed but not yet effectively dead
                    let is_dying = obj.is_destroyed() && !obj.is_effectively_dead();
                    return Ok(if is_dying {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // Object not found - not dying
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_named_totally_dead(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is totally dead", object_name);

        // Look up the named object
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    return Ok(if obj.is_effectively_dead() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
            // Object ID exists but object not found - considered totally dead
            return Ok(ScriptConditionResult::True);
        }
        // Object not in tracker - considered totally dead
        Ok(ScriptConditionResult::True)
    }

    /// C++ Reference: ScriptConditions::evaluateIsBuildingEmpty() line 1008-1024
    pub(crate) fn eval_named_building_is_empty(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' building is empty", object_name);

        // Look up the building object
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    // C++ pattern: get contain module, check if count > 0
                    if let Some(contain_arc) = obj.get_contain() {
                        if let Ok(contain_guard) = contain_arc.lock() {
                            let count = contain_guard.get_contained_count();
                            return Ok(if count == 0 {
                                ScriptConditionResult::True
                            } else {
                                ScriptConditionResult::False
                            });
                        }
                    }
                    // No contain module = false per C++
                    return Ok(ScriptConditionResult::False);
                }
            }
        }
        // Building not found = false per C++
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_named_has_free_container_slots(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' has free container slots", object_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(contain_arc) = obj.get_contain() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(contain) = contain_arc.lock() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(
            if contain.get_contained_count() < contain.get_max_capacity() {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
    }

    // ============================================================================
    // UNIT CONDITION HANDLERS
    // ============================================================================

    pub(crate) fn eval_unit_health(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_health = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating unit '{}' health {:?} {}",
            unit_name,
            comparison,
            target_health
        );

        // Look up the named object
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) {
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    // Get health percentage (0-100 scale)
                    let health_percent = (obj.get_health_percentage() * 100.0) as i32;

                    let result = match comparison {
                        ComparisonType::LessThan => health_percent < target_health,
                        ComparisonType::LessEqual => health_percent <= target_health,
                        ComparisonType::Equal => health_percent == target_health,
                        ComparisonType::GreaterEqual => health_percent >= target_health,
                        ComparisonType::Greater => health_percent > target_health,
                        ComparisonType::NotEqual => health_percent != target_health,
                    };

                    return Ok(if result {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // Object not found - consider health check as false
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_unit_completed_sequential_execution(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        log::debug!(
            "Evaluating if unit '{}' completed sequential execution",
            unit_name
        );
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_unit_emptied(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if unit '{}' emptied", unit_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let num_peeps = obj
            .get_contain()
            .and_then(|contain| contain.lock().ok().map(|c| c.get_contained_count()))
            .unwrap_or(0);
        let frame = TheGameLogic::get_frame();

        let Ok(mut statuses) = TRANSPORT_STATUSES.write() else {
            return Ok(ScriptConditionResult::False);
        };
        let entry = statuses.entry(object_id).or_insert((frame, num_peeps));

        if entry.0 == frame.saturating_sub(1) && entry.1 > 0 && num_peeps == 0 {
            // Match C++: do not update this frame so repeated checks remain true.
            return Ok(ScriptConditionResult::True);
        }

        *entry = (frame, num_peeps);
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_unit_has_object_status(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        let status_mask = condition
            .get_parameter(1)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 1 not found".to_string()))?
            .get_object_status();
        log::debug!("Evaluating if unit '{}' has object status", unit_name);

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(if obj.get_status_bits().intersects(status_mask) {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // CAMERA CONDITION HANDLERS
    // ============================================================================

    pub(crate) fn eval_camera_movement_finished(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating if camera movement finished");
        if let Some(Some(finished)) = with_script_engine_ref(|script_engine| {
            script_engine
                .action_handler()
                .map(|handler| handler.is_camera_movement_finished())
        }) {
            return Ok(if finished {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }
        Ok(ScriptConditionResult::True)
    }

    // ============================================================================
    // BUILDING CONDITION HANDLERS
    // ============================================================================

    pub(crate) fn eval_built_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // Wave 284: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(ScriptConditionResult::False);
        }

        let type_or_list_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' has built object type/list '{}'",
            player_name,
            type_or_list_name
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

        let types = self.resolve_object_types_param(&type_or_list_name);
        if types.list_size() == 0 {
            return Ok(ScriptConditionResult::False);
        }

        for obj_id in player.get_object_ids() {
            let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
            else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if types.contains_template(Some(obj.get_template())) {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_building_entered_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let building_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if building '{}' entered by player '{}'",
            building_name,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&building_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(contain_arc) = obj.get_contain() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(contain) = contain_arc.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let entered_mask = contain.get_player_who_entered();
        if entered_mask == crate::common::PlayerMaskType::none() {
            return Ok(ScriptConditionResult::False);
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

        Ok(if entered_mask == player.get_player_mask() {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_bridge_repaired(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let bridge_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if bridge '{}' repaired", bridge_name);
        let tracker = get_named_object_tracker();
        let Ok(Some(bridge_id)) = tracker.get_object_id(&bridge_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        Ok(if terrain.is_bridge_repaired(bridge_id) {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_bridge_broken(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let bridge_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if bridge '{}' broken", bridge_name);
        let tracker = get_named_object_tracker();
        let Ok(Some(bridge_id)) = tracker.get_object_id(&bridge_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(terrain) = get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };
        Ok(if terrain.is_bridge_broken(bridge_id) {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // SPECIAL POWER CONDITION HANDLERS
    // ============================================================================

    pub(crate) fn eval_player_triggered_special_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' triggered special power '{}'",
            player_name,
            power_name
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
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let event_hit = with_script_engine_mut(|engine| {
            engine.is_special_power_triggered(player_index, &power_name, true, INVALID_ID)
        })
        .unwrap_or(false);

        Ok(if event_hit {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_player_completed_special_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' completed special power '{}'",
            player_name,
            power_name
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
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let event_hit = with_script_engine_mut(|engine| {
            engine.is_special_power_complete(player_index, &power_name, true, INVALID_ID)
        })
        .unwrap_or(false);

        Ok(if event_hit {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_player_midway_special_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' midway special power '{}'",
            player_name,
            power_name
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
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let event_hit = with_script_engine_mut(|engine| {
            engine.is_special_power_midway(player_index, &power_name, true, INVALID_ID)
        })
        .unwrap_or(false);

        Ok(if event_hit {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_player_triggered_special_power_from_named(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        let unit_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' triggered special power '{}' from '{}'",
            player_name,
            power_name,
            unit_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(_) = TheGameLogic::find_object_by_id(source_id) else {
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
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let event_hit = with_script_engine_mut(|engine| {
            engine.is_special_power_triggered(player_index, &power_name, true, source_id)
        })
        .unwrap_or(false);

        Ok(if event_hit {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_player_completed_special_power_from_named(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        let unit_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' completed special power '{}' from '{}'",
            player_name,
            power_name,
            unit_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(_) = TheGameLogic::find_object_by_id(source_id) else {
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
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let event_hit = with_script_engine_mut(|engine| {
            engine.is_special_power_complete(player_index, &power_name, true, source_id)
        })
        .unwrap_or(false);

        Ok(if event_hit {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_player_midway_special_power_from_named(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let power_name = self.get_condition_string_param(condition, 1)?;
        let unit_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' midway special power '{}' from '{}'",
            player_name,
            power_name,
            unit_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(_) = TheGameLogic::find_object_by_id(source_id) else {
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
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let event_hit = with_script_engine_mut(|engine| {
            engine.is_special_power_midway(player_index, &power_name, true, source_id)
        })
        .unwrap_or(false);

        Ok(if event_hit {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // UPGRADE CONDITION HANDLERS
    // ============================================================================

    pub(crate) fn eval_player_built_upgrade(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let upgrade_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' built upgrade '{}'",
            player_name,
            upgrade_name
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
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        // C++ `evaluateUpgradeFromUnitComplete` consumes only the matching
        // ScriptEngine completion event.  A completed player upgrade by
        // itself must not make this edge-triggered condition true forever.
        let event_hit = with_script_engine_mut(|engine| {
            engine.is_upgrade_complete(player_index, &upgrade_name, true, INVALID_ID)
        })
        .unwrap_or(false);

        Ok(if event_hit {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_player_built_upgrade_from_named(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let upgrade_name = self.get_condition_string_param(condition, 1)?;
        let unit_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' built upgrade '{}' from '{}'",
            player_name,
            upgrade_name,
            unit_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(source_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(_) = TheGameLogic::find_object_by_id(source_id) else {
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
        let player_index = player.get_player_index() as usize;
        drop(player);
        drop(players);

        let event_hit = with_script_engine_mut(|engine| {
            engine.is_upgrade_complete(player_index, &upgrade_name, true, source_id)
        })
        .unwrap_or(false);

        Ok(if event_hit {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // MULTIPLAYER CONDITION HANDLERS
    // ============================================================================

    /// C++ Reference: ScriptConditions::checkMultiplayerAlliedVictory()
    /// Checks if all allied players have won
    pub(crate) fn eval_multiplayer_allied_victory(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating multiplayer allied victory");
        // C++ uses TheVictoryConditions for local allied victory checks.
        Ok(if TheVictoryConditions::is_local_allied_victory() {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptConditions::checkMultiplayerAlliedDefeat()
    /// Checks if all allied players have been defeated
    pub(crate) fn eval_multiplayer_allied_defeat(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating multiplayer allied defeat");
        let players = player_list();
        let Ok(players_lock) = players.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let Some(local_player_arc) = players_lock.get_local_player() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(local_player) = local_player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let mut allied_count = 0usize;
        for player_arc in players_lock.iter() {
            if Arc::ptr_eq(player_arc, &local_player_arc) {
                allied_count += 1;
                if !local_player.is_defeated() {
                    return Ok(ScriptConditionResult::False);
                }
                continue;
            }

            let Ok(player) = player_arc.read() else {
                continue;
            };
            if local_player.is_allied_with_player(&player) {
                allied_count += 1;
                if !player.is_defeated() {
                    return Ok(ScriptConditionResult::False);
                }
            }
        }

        Ok(if allied_count > 0 {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptConditions::checkMultiplayerPlayerDefeat()
    /// Checks if a specific player has been defeated
    pub(crate) fn eval_multiplayer_player_defeat(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating multiplayer player '{}' defeat", player_name);

        // Look up player by name and check their defeat status
        let players = player_list();
        if let Ok(players_lock) = players.read() {
            for player_arc in players_lock.iter() {
                if let Ok(player) = player_arc.read() {
                    if player.get_player_name_key() == NameKeyGenerator::name_to_key(&player_name) {
                        // Check if player has been defeated
                        if player.is_player_dead() {
                            return Ok(ScriptConditionResult::True);
                        }
                        break;
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    // ============================================================================
    // MEDIA CONDITION HANDLERS
    // ============================================================================

    pub(crate) fn eval_has_finished_video(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let name = self.get_condition_string_param(_condition, 0)?;
        log::debug!("Evaluating if video '{}' finished", name);
        if let Some(Some(finished)) = with_script_engine_ref(|script_engine| {
            script_engine
                .action_handler()
                .map(|handler| handler.is_video_complete(&name, true))
        }) {
            return Ok(if finished {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }
        Ok(ScriptConditionResult::True)
    }

    pub(crate) fn eval_has_finished_speech(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let name = self.get_condition_string_param(_condition, 0)?;
        log::debug!("Evaluating if speech '{}' finished", name);
        if let Some(Some(finished)) = with_script_engine_ref(|script_engine| {
            script_engine
                .action_handler()
                .map(|handler| handler.is_speech_complete(&name, true))
        }) {
            return Ok(if finished {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }
        Ok(ScriptConditionResult::True)
    }

    pub(crate) fn eval_has_finished_audio(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let name = self.get_condition_string_param(_condition, 0)?;
        log::debug!("Evaluating if audio '{}' finished", name);
        if let Some(Some(finished)) = with_script_engine_ref(|script_engine| {
            script_engine
                .action_handler()
                .map(|handler| handler.is_audio_complete(&name, true))
        }) {
            return Ok(if finished {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }
        Ok(ScriptConditionResult::True)
    }

    pub(crate) fn eval_music_track_has_completed(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let track = self.get_condition_string_param(_condition, 0)?;
        let param = self.get_condition_int_param(_condition, 1).unwrap_or(0);
        log::debug!(
            "Evaluating if music track '{}' completed (param: {})",
            track,
            param
        );
        if let Some(Some(finished)) = with_script_engine_ref(|script_engine| {
            script_engine
                .action_handler()
                .map(|handler| handler.has_music_track_completed(&track, param))
        }) {
            return Ok(if finished {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }
        Ok(ScriptConditionResult::True)
    }

    // ============================================================================
    // MISCELLANEOUS CONDITION HANDLERS
    // ============================================================================

    pub(crate) fn eval_enemy_sighted(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        let alliance = self.get_condition_int_param(condition, 1)?;
        let player_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if unit '{}' has sighted alliance {} unit from player '{}'",
            unit_name,
            alliance,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(unit) = unit_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(target_player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let src_pos = *unit.get_position();
        let vision_range = unit.get_vision_range();
        let src_off_map = unit.is_off_map();
        let Some(partition) = ThePartitionManager::get() else {
            return Ok(ScriptConditionResult::False);
        };

        for obj_id in partition.get_objects_in_range(&src_pos, vision_range) {
            if obj_id == unit_id {
                continue;
            }
            let Some(candidate_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(candidate) = candidate_arc.read() else {
                continue;
            };

            if candidate.is_effectively_dead() {
                continue;
            }
            if candidate.is_off_map() != src_off_map {
                continue;
            }

            let status = candidate.get_status_bits();
            if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
                && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
                && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
            {
                continue;
            }

            let relationship = unit.relationship_to(&candidate);
            let relation_ok = match alliance {
                0 => relationship == Relationship::Enemies, // REL_ENEMY
                1 => relationship == Relationship::Neutral, // REL_NEUTRAL
                2 => matches!(relationship, Relationship::Allies), // REL_FRIEND
                _ => false,
            };
            if !relation_ok {
                continue;
            }

            if let Some(owner) = candidate.get_controlling_player() {
                if Arc::ptr_eq(&owner, &target_player_arc) {
                    return Ok(ScriptConditionResult::True);
                }
            }
        }

        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_type_sighted(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let unit_name = self.get_condition_string_param(condition, 0)?;
        let type_or_list_name = self.get_condition_string_param(condition, 1)?;
        let player_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if unit '{}' has sighted type/list '{}' from player '{}'",
            unit_name,
            type_or_list_name,
            player_name
        );

        let tracker = get_named_object_tracker();
        let Ok(Some(unit_id)) = tracker.get_object_id(&unit_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(unit_arc) = TheGameLogic::find_object_by_id(unit_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(unit) = unit_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(target_player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };

        let wanted_types = self.resolve_object_types_param(&type_or_list_name);
        if wanted_types.list_size() == 0 {
            return Ok(ScriptConditionResult::False);
        }

        let src_pos = *unit.get_position();
        let vision_range = unit.get_vision_range();
        let src_off_map = unit.is_off_map();
        let Some(partition) = ThePartitionManager::get() else {
            return Ok(ScriptConditionResult::False);
        };

        for obj_id in partition.get_objects_in_range(&src_pos, vision_range) {
            if obj_id == unit_id {
                continue;
            }
            let Some(candidate_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(candidate) = candidate_arc.read() else {
                continue;
            };

            if candidate.is_effectively_dead() {
                continue;
            }
            if candidate.is_off_map() != src_off_map {
                continue;
            }

            let status = candidate.get_status_bits();
            if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
                && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
                && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
            {
                continue;
            }

            let Some(owner) = candidate.get_controlling_player() else {
                continue;
            };
            if !Arc::ptr_eq(&owner, &target_player_arc) {
                continue;
            }
            if !wanted_types.contains_template(Some(candidate.get_template())) {
                continue;
            }
            return Ok(ScriptConditionResult::True);
        }

        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_mission_attempts(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ evaluateMissionAttempts has the [SIDE, COMPARISON, INT] template, but the
        // implementation does not read any parameters and always returns false.
        log::debug!("Evaluating unimplemented C++ mission-attempts condition");
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_supply_source_safe(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let min_supply_amount = self.get_condition_int_param(condition, 1)?;
        log::debug!("Evaluating if supply source safe for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_id = player.get_player_index() as u32;
        drop(player);
        drop(players);

        let is_safe = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| match ai_player {
                crate::ai::integration::IntegratedAiPlayer::Standard(ai) => {
                    ai.is_supply_source_safe(min_supply_amount)
                }
                crate::ai::integration::IntegratedAiPlayer::Skirmish(ai) => {
                    ai.is_supply_source_safe(min_supply_amount)
                }
            })
        })
        .flatten()
        .unwrap_or(false);

        Ok(if is_safe {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_supply_source_attacked(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if supply source attacked for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };
        let player_id = player.get_player_index() as u32;
        drop(player);
        drop(players);

        let attacked = with_ai_integration_mut(|manager| {
            manager.with_ai_player_mut(player_id, |ai_player| match ai_player {
                crate::ai::integration::IntegratedAiPlayer::Standard(ai) => {
                    ai.is_supply_source_attacked()
                }
                crate::ai::integration::IntegratedAiPlayer::Skirmish(ai) => {
                    ai.is_supply_source_attacked()
                }
            })
        })
        .flatten()
        .unwrap_or(false);

        Ok(if attacked {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_start_position_is(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let start_position = self.get_condition_int_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' start position is {}",
            player_name,
            start_position
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

        // C++ expects external start positions as 1-based indices.
        let expected_index = start_position - 1;
        Ok(if player.get_mp_start_index() == expected_index {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }
}
