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
        // C++ ScriptConditions::evaluateNamedUnitDestroyed (ScriptConditions.cpp:274-286)
        // if (theUnit) return theUnit->isEffectivelyDead();
        // if (didUnitExist(name)) return true;  // name known, pointer now NULL
        // return false;                         // never existed is not destroyed
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is destroyed", object_name);

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&object_name) {
                return Ok(if obj.effectively_dead || !obj.alive {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
            if let Some(alive) = crate::scripting::host_script_named_unit_alive(&object_name) {
                return Ok(if alive {
                    ScriptConditionResult::False
                } else {
                    ScriptConditionResult::True
                });
            }
            // Host snapshot does not list this name (destroyed after processDestroyList,
            // or never existed). C++ getUnitNamed is NULL; didUnitExist decides.
            // Do not return False just because other host objects exist (hq-umqv8).
        }

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
            // Name still tracked, object gone — C++ didUnitExist (pointer == NULL).
            return Ok(ScriptConditionResult::True);
        }
        Ok(if tracker.did_object_exist(&object_name).unwrap_or(false) {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_named_not_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ NAMED_NOT_DESTROYED → evaluateNamedUnitExists (ScriptConditions.cpp:291-299)
        // if (theUnit) return !theUnit->isEffectivelyDead();
        // return false;
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is not destroyed", object_name);

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(alive) = crate::scripting::host_script_named_unit_alive(&object_name) {
                return Ok(if alive {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                });
            }
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
        Ok(if obj.is_effectively_dead() {
            ScriptConditionResult::False
        } else {
            ScriptConditionResult::True
        })
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

        let types = self.resolve_object_types_param(&types_param);
        if types.list_size() == 0 {
            return Ok(ScriptConditionResult::False);
        }

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&object_name) {
                if !obj.last_damage_template.is_empty()
                    && types.is_in_set(&crate::common::AsciiString::from(
                        obj.last_damage_template.as_str(),
                    ))
                {
                    return Ok(ScriptConditionResult::True);
                }
                if obj.last_damage_source_id != 0 {
                    if let Some(src) =
                        crate::scripting::host_script_query_object_by_id(obj.last_damage_source_id)
                    {
                        return Ok(Self::bool_result(types.is_in_set(
                            &crate::common::AsciiString::from(src.template_name.as_str()),
                        )));
                    }
                }
            }
            return Ok(ScriptConditionResult::False);
        }

        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };
        Ok(Self::bool_result(
            self.last_damage_matches_object_types(object_id, &types),
        ))
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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&object_name) {
                return Ok(Self::bool_result(
                    !obj.last_damage_player.is_empty()
                        && obj.last_damage_player.eq_ignore_ascii_case(&player_name),
                ));
            }
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

        // C++: return (TheScriptEngine->getUnitNamed(pUnitParm->getString()) != NULL);
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(Self::bool_result(
                crate::scripting::host_script_named_unit_present(&object_name),
            ));
        }

        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&object_name) {
                if obj.held || obj.stealthed_hidden {
                    return Ok(ScriptConditionResult::False);
                }
                return Ok(Self::bool_result(
                    obj.discovered_by
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(&player_name)),
                ));
            }
            return Ok(ScriptConditionResult::False);
        }

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
        let player_index = player.get_player_index();

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        Ok(Self::bool_result(
            self.object_is_discovered_by_player(&obj, player_index),
        ))
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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&object_name) {
                return Ok(Self::bool_result(
                    !obj.owner_player.is_empty()
                        && obj.owner_player.eq_ignore_ascii_case(&player_name),
                ));
            }
            return Ok(ScriptConditionResult::False);
        }

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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&object_name) {
                return Ok(Self::bool_result(
                    obj.waypoint_labels
                        .iter()
                        .any(|label| label == &waypoint_path),
                ));
            }
            return Ok(ScriptConditionResult::False);
        }

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

        // C++ ScriptConditions::evaluateNamedSelected walks TheInGameUI
        // selected drawables and compares Object::getName(). Live host never
        // mirrors objects into OBJECT_REGISTRY, so consult the host snapshot.
        if dual_world_registry_unavailable() {
            let is_selected =
                crate::scripting::host_script_named_unit_selected(&object_name).unwrap_or(false);
            condition.custom_data = if is_selected { 1 } else { -1 };
            return Ok(if is_selected {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
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

        // C++ ScriptConditions.cpp:1614-1631 evaluateNamedEnteredArea:
        // getUnitNamed, skip KINDOF_INERT, getQualifiedTriggerAreaByName, Object::didEnter.
        let Ok(trigger) = self.get_trigger_area(&area_name) else {
            return Ok(ScriptConditionResult::False);
        };
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if crate::scripting::host_script_query_object(&object_name)
                .is_some_and(|obj| obj.kind_inert)
            {
                return Ok(ScriptConditionResult::False);
            }
            let Some(object_id) = crate::scripting::host_script_named_unit_id(&object_name) else {
                return Ok(ScriptConditionResult::False);
            };
            return Ok(Self::bool_result(crate::scripting::host_object_did_enter(
                object_id, &trigger,
            )));
        }
        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let entered = crate::object::registry::OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                if obj.is_kind_of(crate::common::KindOf::Inert) {
                    return false;
                }
                obj.did_enter(&trigger)
            })
            .unwrap_or(false);
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

        // C++ ScriptConditions.cpp:1637-1651 evaluateNamedExitedArea:
        // getUnitNamed, getQualifiedTriggerAreaByName, Object::didExit.
        let Ok(trigger) = self.get_trigger_area(&area_name) else {
            return Ok(ScriptConditionResult::False);
        };
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            let Some(object_id) = crate::scripting::host_script_named_unit_id(&object_name) else {
                return Ok(ScriptConditionResult::False);
            };
            return Ok(Self::bool_result(crate::scripting::host_object_did_exit(
                object_id, &trigger,
            )));
        }
        let tracker = get_named_object_tracker();
        let Ok(Some(object_id)) = tracker.get_object_id(&object_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let exited = crate::object::registry::OBJECT_REGISTRY
            .with_object(object_id, |obj| obj.did_exit(&trigger))
            .unwrap_or(false);
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
        // C++ ScriptConditions::evaluateNamedUnitDying (ScriptConditions.cpp:305-318)
        // if (theUnit) return theUnit->isEffectivelyDead();
        // return false; // already totally gone, or never existed, is not dying
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is dying", object_name);

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&object_name) {
                return Ok(Self::bool_result(obj.effectively_dead || !obj.alive));
            }
            return Ok(ScriptConditionResult::False);
        }

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
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_named_totally_dead(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' is totally dead", object_name);

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            // C++ getUnitNamed: host snapshot lists this name (alive or dying) → not totally dead.
            if crate::scripting::host_script_named_unit_alive(&object_name).is_some() {
                return Ok(ScriptConditionResult::False);
            }
            if crate::scripting::host_script_query_has_any() {
                let tracker = get_named_object_tracker();
                let existed = tracker.did_object_exist(&object_name).unwrap_or(false)
                    || with_script_engine_ref(|engine| engine.did_unit_exist(&object_name))
                        .unwrap_or(false);
                return Ok(Self::bool_result(existed));
            }
        }

        // C++ ScriptConditions::evaluateNamedUnitTotallyDead (ScriptConditions.cpp:323-335):
        // false while getUnitNamed succeeds; true only after it existed and is gone.
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(&object_name) {
            if TheGameLogic::find_object_by_id(object_id).is_some() {
                return Ok(ScriptConditionResult::False);
            }
        }
        let existed = tracker.did_object_exist(&object_name).unwrap_or(false)
            || with_script_engine_ref(|engine| engine.did_unit_exist(&object_name))
                .unwrap_or(false);
        Ok(Self::bool_result(existed))
    }

    /// C++ Reference: ScriptConditions::evaluateIsBuildingEmpty() line 1008-1024
    pub(crate) fn eval_named_building_is_empty(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let object_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if '{}' building is empty", object_name);

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&object_name) {
                if !obj.has_contain {
                    return Ok(ScriptConditionResult::False);
                }
                return Ok(Self::bool_result(obj.contain_count == 0));
            }
            return Ok(ScriptConditionResult::False);
        }

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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&object_name) {
                if !obj.has_contain {
                    return Ok(ScriptConditionResult::False);
                }
                return Ok(Self::bool_result(obj.contain_count < obj.contain_max));
            }
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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&unit_name) {
                if obj.initial_health <= 0.0 {
                    return Ok(ScriptConditionResult::False);
                }
                let health_percent =
                    ((obj.health * 100.0 + obj.initial_health / 2.0) / obj.initial_health) as i32;
                return Ok(Self::bool_result(Self::compare_i32(
                    comparison,
                    health_percent,
                    target_health,
                )));
            }
            return Ok(ScriptConditionResult::False);
        }

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
        let Some(body) = obj.get_body_module() else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(body_guard) = body.lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let cur_health = body_guard.get_health();
        let initial_health = body_guard.get_initial_health();
        if initial_health <= 0.0 {
            return Ok(ScriptConditionResult::False);
        }
        // C++ ScriptConditions.cpp:934 (curHealth*100 + initialHealth/2)/initialHealth
        let health_percent = ((cur_health * 100.0 + initial_health / 2.0) / initial_health) as i32;
        Ok(Self::bool_result(Self::compare_i32(
            comparison,
            health_percent,
            target_health,
        )))
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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            if let Some(obj) = crate::scripting::host_script_query_object(&unit_name) {
                let object_id = obj.id;
                let num_peeps = obj.contain_count as usize;
                let frame = TheGameLogic::get_frame();
                let Ok(mut statuses) = TRANSPORT_STATUSES.write() else {
                    return Ok(ScriptConditionResult::False);
                };
                let entry = statuses.entry(object_id).or_insert((frame, num_peeps));
                if entry.0 == frame.saturating_sub(1) && entry.1 > 0 && num_peeps == 0 {
                    return Ok(ScriptConditionResult::True);
                }
                *entry = (frame, num_peeps);
                return Ok(ScriptConditionResult::False);
            }
            return Ok(ScriptConditionResult::False);
        }

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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(
                if crate::scripting::host_eval_unit_has_object_status(
                    &unit_name,
                    status_mask.bits(),
                )
                .unwrap_or(false)
                {
                    ScriptConditionResult::True
                } else {
                    ScriptConditionResult::False
                },
            );
        }

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
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let type_or_list_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' has built object type '{}'",
            player_name,
            type_or_list_name
        );

        // C++ ScriptConditions.cpp:859-865 — cached while object count is unchanged.
        if condition.custom_data != 0
            && with_script_engine_ref(|engine| {
                engine.get_frame_object_count_changed() == condition.custom_frame
            })
            .unwrap_or(false)
        {
            return Ok(if condition.custom_data == 1 {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            });
        }

        // C++ ScriptConditions.cpp:872-874 — false unless findTemplate(raw name) exists.
        // Live leftover factory may be empty; a host object of that exact type is
        // proof the ThingTemplate exists. ObjectTypes list names stay false.
        if crate::helpers::TheThingFactory::find_template(&type_or_list_name).is_none() {
            let key = type_or_list_name.to_ascii_lowercase();
            let host_has =
                crate::scripting::host_query_player_census(&player_name).is_some_and(|c| {
                    c.template_counts.contains_key(&key)
                        || c.template_counts_ignore_dead.contains_key(&key)
                });
            if !host_has {
                return Ok(ScriptConditionResult::False);
            }
        }

        let sum = if let Some(count) =
            self.host_player_object_type_count(&player_name, &type_or_list_name, false)
        {
            count
        } else {
            if dual_world_registry_unavailable() {
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
            let types = self.resolve_object_types_param(&type_or_list_name);
            let (templates, mut counts) = types.prep_for_player_counting();
            if templates.is_empty() {
                return Ok(ScriptConditionResult::False);
            }
            player.count_objects_by_thing_template(&templates, false, true, &mut counts);
            counts.iter().copied().sum()
        };

        condition.custom_data = if sum != 0 { 1 } else { -1 };
        if let Some(frame) =
            with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
        {
            condition.custom_frame = frame;
        }
        Ok(Self::bool_result(sum != 0))
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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(Self::bool_result(
                crate::scripting::host_building_entered_by_player(&building_name, &player_name)
                    .unwrap_or(false),
            ));
        }

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
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(Self::bool_result(crate::scripting::host_bridge_repaired(
                &bridge_name,
            )));
        }
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
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(Self::bool_result(crate::scripting::host_bridge_broken(
                &bridge_name,
            )));
        }
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

        let Some(source_id) = leftover_named_source_id(&unit_name) else {
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

        let Some(source_id) = leftover_named_source_id(&unit_name) else {
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

        let Some(source_id) = leftover_named_source_id(&unit_name) else {
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

        let Some(source_id) = leftover_named_source_id(&unit_name) else {
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

    /// C++ ScriptConditions::evaluateMultiplayerAlliedDefeat —
    /// TheVictoryConditions->isLocalAlliedDefeat() (last alliance standing).
    pub(crate) fn eval_multiplayer_allied_defeat(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating multiplayer allied defeat");
        Ok(if TheVictoryConditions::is_local_allied_defeat() {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ ScriptConditions::evaluateMultiplayerPlayerDefeat — no params.
    pub(crate) fn eval_multiplayer_player_defeat(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // isLocalDefeat() && !isLocalAlliedDefeat()
        Ok(Self::bool_result(
            TheVictoryConditions::is_local_defeat()
                && !TheVictoryConditions::is_local_allied_defeat(),
        ))
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
        // C++ evaluateVideoHasCompleted → TheScriptEngine->isVideoComplete(name, true).
        // Live handler waits leftover m_completedVideo; unknown names stay false.
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
        let finished =
            with_script_engine_ref(|engine| engine.is_video_complete(&name, true)).unwrap_or(false);
        Ok(if finished {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
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
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_has_finished_audio(
        &self,
        _condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let name = self.get_condition_string_param(_condition, 0)?;
        log::debug!("Evaluating if audio '{}' finished", name);
        // C++ evaluateAudioHasCompleted → TheScriptEngine->isAudioComplete(name, true).
        // Live handler waits leftover TheAudio length on the live frame clock.
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
        let finished =
            with_script_engine_ref(|engine| engine.is_audio_complete(&name, true)).unwrap_or(false);
        Ok(if finished {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
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
        // C++ evaluateMusicHasCompleted → TheAudio->hasMusicTrackCompleted(track, N).
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
        let finished = crate::helpers::TheAudio::get()
            .map(|audio| audio.has_music_track_completed(&track, param))
            .unwrap_or(false);
        Ok(if finished {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(Self::bool_result(
                crate::scripting::host_enemy_sighted(&unit_name, alliance, &player_name)
                    .unwrap_or(false),
            ));
        }

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

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            let wanted_types = self.resolve_object_types_param(&type_or_list_name);
            let type_names: Vec<String> = wanted_types
                .iter()
                .map(|t| t.as_str().to_string())
                .collect();
            return Ok(Self::bool_result(
                crate::scripting::host_type_sighted(&unit_name, &type_names, &player_name)
                    .unwrap_or(false),
            ));
        }

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
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ evaluateSkirmishSupplySourceSafe: cache for 2*LOGICFRAMES_PER_SECOND.
        let frame = TheGameLogic::get_frame();
        if frame <= condition.custom_frame {
            if condition.custom_data == -1 {
                return Ok(ScriptConditionResult::False);
            }
            if condition.custom_data == 1 {
                return Ok(ScriptConditionResult::True);
            }
        }
        condition.custom_frame = frame.saturating_add(2 * LOGICFRAMES_PER_SECOND as u32);

        let player_name = self.get_condition_string_param(condition, 0)?;
        let min_supply_amount = self.get_condition_int_param(condition, 1)?;
        log::debug!("Evaluating if supply source safe for '{}'", player_name);

        let Ok(players) = player_list().read() else {
            condition.custom_data = -1;
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            condition.custom_data = -1;
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            condition.custom_data = -1;
            return Ok(ScriptConditionResult::False);
        };
        let player_id = player.get_player_index() as u32;
        drop(player);
        drop(players);

        let is_safe = if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            crate::scripting::host_query_supply_source_safe(&player_name, min_supply_amount)
                .unwrap_or(false)
        } else {
            with_ai_integration_mut(|manager| {
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
            .unwrap_or(false)
        };

        condition.custom_data = if is_safe { 1 } else { -1 };
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

        let attacked = if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            crate::scripting::host_query_supply_source_attacked(&player_name).unwrap_or(false)
        } else {
            with_ai_integration_mut(|manager| {
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
            .unwrap_or(false)
        };

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

/// C++ TheScriptEngine->getUnitNamed then Object::getID. Host IDs are not leftover crate Objects.
fn leftover_named_source_id(unit_name: &str) -> Option<u32> {
    let tracker_id = get_named_object_tracker()
        .get_object_id(unit_name)
        .ok()
        .flatten();
    if crate::object::registry::OBJECT_REGISTRY.is_empty() {
        if let Some(obj) = crate::scripting::host_script_query_object(unit_name) {
            return Some(obj.id);
        }
        if let Some(id) = tracker_id {
            if crate::scripting::host_script_query_object_by_id(id).is_some() {
                return Some(id);
            }
        }
        return crate::scripting::host_script_named_unit_id(unit_name).or(tracker_id);
    }
    let id = tracker_id?;
    TheGameLogic::find_object_by_id(id).map(|_| id)
}
