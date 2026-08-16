// Script evaluate_script / OR / AND / evaluate_condition dispatch
//
// Split from `scripting/evaluator.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEvaluator {
    pub fn new(engine: ScriptEngineHandle) -> Self {
        Self { engine }
    }

    /// Access the engine that owns this evaluation.
    ///
    /// Normal game-script execution has a lexical active engine installed by
    /// `ScriptEngine::update`; using it first avoids re-locking the global
    /// handle during an immediate nested action.  Evaluators are also used by
    /// triggers and tests with a private `ScriptEngineHandle`, where no active
    /// scope exists, so retain that exact injected handle as the fallback.
    /// An active-but-currently-borrowed engine deliberately does not fall back:
    /// retrying the global handle there could deadlock the live update thread.
    fn with_evaluation_engine_ref<R>(&self, f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
        if is_script_engine_active() {
            return with_active_script_engine_ref(f);
        }

        let guard = self.engine.read().ok()?;
        guard.as_ref().map(f)
    }

    /// Mutating counterpart to `with_evaluation_engine_ref`.
    fn with_evaluation_engine_mut<R>(&self, f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
        if is_script_engine_active() {
            return with_active_script_engine_mut(f);
        }

        let guard = self.engine.write().ok()?;
        guard.as_ref().map(f)
    }

    /// Evaluate a complete script matching C++ EvaluateScripts
    pub fn evaluate_script(&self, script: &mut Script) -> GameLogicResult<bool> {
        log::debug!("Evaluating script: {}", script.get_name());

        // Check if script is active
        if !script.is_active() {
            return Ok(false);
        }

        // Evaluate conditions
        let condition_result = if let Some(or_condition) = script.condition.as_deref_mut() {
            self.evaluate_or_condition(or_condition)?
        } else {
            true // No conditions means always true
        };

        log::debug!(
            "Script '{}' condition result: {}",
            script.get_name(),
            condition_result
        );

        // Execute actions based on condition result
        if condition_result {
            if let Some(action) = script.get_action() {
                self.execute_action_sequence(action)?;
            }
        } else {
            if let Some(false_action) = script.get_false_action() {
                self.execute_action_sequence(false_action)?;
            }
        }

        Ok(condition_result)
    }

    /// Evaluate OR condition matching C++ EvaluateConditions
    pub fn evaluate_or_condition(&self, or_condition: &mut OrCondition) -> GameLogicResult<bool> {
        let mut current_or = Some(or_condition);

        while let Some(or_cond) = current_or {
            // Evaluate all AND conditions in this OR clause
            if let Some(and_condition) = or_cond.first_and.as_deref_mut() {
                if self.evaluate_and_condition(and_condition)? {
                    return Ok(true); // Short-circuit on first true OR
                }
            }

            current_or = or_cond.next_or.as_deref_mut();
        }

        Ok(false) // All OR conditions were false
    }

    /// Evaluate AND condition chain
    pub fn evaluate_and_condition(&self, and_condition: &mut Condition) -> GameLogicResult<bool> {
        let mut current_and = Some(and_condition);

        while let Some(and_cond) = current_and {
            if !self.evaluate_condition(and_cond)? {
                return Ok(false); // Short-circuit on first false AND
            }

            current_and = and_cond.next_and_condition.as_deref_mut();
        }

        Ok(true) // All AND conditions were true
    }

    /// Evaluate a single condition matching C++ EvaluateCondition
    pub fn evaluate_condition(&self, condition: &mut Condition) -> GameLogicResult<bool> {
        // C++ EvaluateCondition never fail-closes the whole evaluator because
        // OBJECT_REGISTRY is empty. TRUE / FALSE / Counter / Flag / TimerExpired
        // are engine-local and must evaluate on the live host.
        let condition_type = condition.get_condition_type();
        let object_world = !matches!(
            condition_type,
            ConditionType::ConditionFalse
                | ConditionType::ConditionTrue
                | ConditionType::Counter
                | ConditionType::Flag
                | ConditionType::TimerExpired
        );
        if object_world && dual_world_registry_unavailable() {
            return Ok(false);
        }

        const SLOW_SCRIPT_CONDITION_WARN_MS: u64 = 40;
        let condition_type = condition.get_condition_type();
        let eval_started = Instant::now();
        let result = match condition_type {
            ConditionType::ConditionFalse => Ok(false),
            ConditionType::ConditionTrue => Ok(true),
            ConditionType::Counter => self.evaluate_counter_condition(condition),
            ConditionType::Flag => self.evaluate_flag_condition(condition),
            ConditionType::TimerExpired => self.evaluate_timer_expired_condition(condition),
            ConditionType::PlayerAllDestroyed => {
                self.evaluate_player_all_destroyed_condition(condition)
            }
            ConditionType::PlayerAllBuildfacilitiesDestroyed => {
                self.evaluate_player_all_buildfacilities_destroyed_condition(condition)
            }
            ConditionType::TeamInsideAreaPartially => {
                self.evaluate_team_inside_area_partially_condition(condition)
            }
            ConditionType::TeamDestroyed => self.evaluate_team_destroyed_condition(condition),
            ConditionType::TeamHasUnits => self.evaluate_team_has_units_condition(condition),
            ConditionType::TeamStateIs => self.evaluate_team_state_is_condition(condition),
            ConditionType::TeamStateIsNot => self.evaluate_team_state_is_not_condition(condition),
            ConditionType::NamedInsideArea => self.evaluate_named_inside_area_condition(condition),
            ConditionType::NamedOutsideArea => {
                self.evaluate_named_outside_area_condition(condition)
            }
            ConditionType::NamedDestroyed => self.evaluate_named_destroyed_condition(condition),
            ConditionType::NamedNotDestroyed => {
                self.evaluate_named_not_destroyed_condition(condition)
            }
            ConditionType::NamedAttackedByObjecttype => {
                self.evaluate_named_attacked_by_object_type_condition(condition)
            }
            ConditionType::TeamAttackedByObjecttype => {
                self.evaluate_team_attacked_by_object_type_condition(condition)
            }
            ConditionType::NamedAttackedByPlayer => {
                self.evaluate_named_attacked_by_player_condition(condition)
            }
            ConditionType::TeamAttackedByPlayer => {
                self.evaluate_team_attacked_by_player_condition(condition)
            }
            ConditionType::NamedCreated => self.evaluate_named_created_condition(condition),
            ConditionType::TeamCreated => self.evaluate_team_created_condition(condition),
            ConditionType::NamedDiscovered => self.evaluate_named_discovered_condition(condition),
            ConditionType::TeamDiscovered => self.evaluate_team_discovered_condition(condition),
            ConditionType::TeamInsideAreaEntirely => {
                self.evaluate_team_inside_area_entirely_condition(condition)
            }
            ConditionType::TeamOutsideAreaEntirely => {
                self.evaluate_team_outside_area_entirely_condition(condition)
            }
            ConditionType::PlayerHasCredits => {
                self.evaluate_player_has_credits_condition(condition)
            }
            ConditionType::PlayerHasPower => self.evaluate_player_has_power_condition(condition),
            ConditionType::PlayerHasNoPower => {
                self.evaluate_player_has_no_power_condition(condition)
            }
            ConditionType::NamedOwnedByPlayer => {
                self.evaluate_named_owned_by_player_condition(condition)
            }
            ConditionType::TeamOwnedByPlayer => {
                self.evaluate_team_owned_by_player_condition(condition)
            }
            ConditionType::PlayerHasNOrFewerBuildings => {
                self.evaluate_player_has_n_or_fewer_buildings_condition(condition)
            }
            ConditionType::BuildingEnteredByPlayer => {
                self.evaluate_building_entered_by_player_condition(condition)
            }
            ConditionType::HasFinishedVideo => {
                self.evaluate_has_finished_video_condition(condition)
            }
            ConditionType::HasFinishedSpeech => {
                self.evaluate_has_finished_speech_condition(condition)
            }
            ConditionType::HasFinishedAudio => {
                self.evaluate_has_finished_audio_condition(condition)
            }
            ConditionType::UnitHealth => self.evaluate_unit_health_condition(condition),
            ConditionType::NamedEnteredArea => {
                self.evaluate_named_entered_area_condition(condition)
            }
            ConditionType::NamedExitedArea => self.evaluate_named_exited_area_condition(condition),
            ConditionType::NamedDying => self.evaluate_named_dying_condition(condition),
            ConditionType::NamedTotallyDead => {
                self.evaluate_named_totally_dead_condition(condition)
            }
            ConditionType::NamedSelected => self.evaluate_named_selected_condition(condition),
            ConditionType::TeamEnteredAreaEntirely => {
                self.evaluate_team_entered_area_entirely_condition(condition)
            }
            ConditionType::TeamEnteredAreaPartially => {
                self.evaluate_team_entered_area_partially_condition(condition)
            }
            ConditionType::TeamExitedAreaEntirely => {
                self.evaluate_team_exited_area_entirely_condition(condition)
            }
            ConditionType::TeamExitedAreaPartially => {
                self.evaluate_team_exited_area_partially_condition(condition)
            }
            ConditionType::PlayerHasNOrFewerFactionBuildings => {
                self.evaluate_player_has_n_or_fewer_faction_buildings_condition(condition)
            }
            ConditionType::BuiltByPlayer => self.evaluate_built_by_player_condition(condition),
            ConditionType::NamedBuildingIsEmpty => {
                self.evaluate_named_building_is_empty_condition(condition)
            }
            ConditionType::PlayerPowerComparePercent => {
                self.evaluate_player_power_compare_percent_condition(condition)
            }
            ConditionType::PlayerExcessPowerCompareValue => {
                self.evaluate_player_excess_power_compare_value_condition(condition)
            }
            ConditionType::UnitHasObjectStatus => {
                self.evaluate_unit_has_object_status_condition(condition)
            }
            ConditionType::TeamAllHasObjectStatus => {
                self.evaluate_team_has_object_status_condition(condition, true)
            }
            ConditionType::TeamSomeHaveObjectStatus => {
                self.evaluate_team_has_object_status_condition(condition, false)
            }
            ConditionType::PlayerAcquiredScience => {
                self.evaluate_player_acquired_science_condition(condition)
            }
            ConditionType::PlayerHasSciencepurchasepoints => {
                self.evaluate_player_has_science_purchase_points_condition(condition)
            }
            ConditionType::PlayerCanPurchaseScience => {
                self.evaluate_player_can_purchase_science_condition(condition)
            }
            ConditionType::NamedHasFreeContainerSlots => {
                self.evaluate_named_has_free_container_slots_condition(condition)
            }
            ConditionType::UnitEmptied => self.evaluate_unit_emptied_condition(condition),

            // Camera movement finished (C++: TheTacticalView->isCameraMovementFinished())
            ConditionType::CameraMovementFinished => {
                // C++ checks TheTacticalView->isCameraMovementFinished()
                // Query the action handler for camera state; default true (no camera = no movement = finished)
                let handler = self
                    .with_evaluation_engine_ref(|engine| engine.action_handler())
                    .flatten();
                Ok(handler
                    .map(|h| h.is_camera_movement_finished())
                    .unwrap_or(true))
            }

            // Mission attempts comparison (C++: evaluateMissionAttempts - always returns false)
            ConditionType::MissionAttempts => {
                // C++ evaluateMissionAttempts is a stub that always returns false
                Ok(false)
            }

            // Named unit reached end of waypoint path
            ConditionType::NamedReachedWaypointsEnd => {
                let unit_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "NamedReachedWaypointsEnd condition missing unit parameter".to_string(),
                    )
                })?;
                let waypoint_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "NamedReachedWaypointsEnd condition missing waypoint parameter".to_string(),
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
                let Some(ai) = obj_guard.get_ai_update_interface() else {
                    return Ok(false);
                };
                let Ok(ai_guard) = ai.lock() else {
                    return Ok(false);
                };
                let Some(completed_id) = ai_guard.get_completed_waypoint_id() else {
                    return Ok(false);
                };

                // C++ uses AsciiString::operator== here, which is a case-sensitive
                // strcmp.  `Waypoint::matches_path_label` intentionally serves other
                // terrain lookups with case-insensitive matching, so this script path
                // must compare the three labels directly.
                let waypoint_path_name = waypoint_param.get_string();
                let Ok(terrain) = get_terrain_logic().read() else {
                    return Ok(false);
                };
                let matches = terrain
                    .get_waypoint_by_id(completed_id)
                    .is_some_and(|waypoint| {
                        waypoint.get_path_label1().as_str() == waypoint_path_name
                            || waypoint.get_path_label2().as_str() == waypoint_path_name
                            || waypoint.get_path_label3().as_str() == waypoint_path_name
                    });
                Ok(matches)
            }

            // Team reached end of waypoint path (any member)
            ConditionType::TeamReachedWaypointsEnd => {
                let team_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "TeamReachedWaypointsEnd condition missing team parameter".to_string(),
                    )
                })?;
                let waypoint_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "TeamReachedWaypointsEnd condition missing waypoint parameter".to_string(),
                    )
                })?;

                let team_name = self.resolve_team_name_token(team_param.get_string());
                let waypoint_path_name = waypoint_param.get_string();
                // Resolve through TeamFactory before holding TerrainLogic: releasing the
                // factory guard may synchronously run queued team-create scripts.
                let team_instances = self.resolve_team_instances(&team_name);
                let Ok(terrain) = get_terrain_logic().read() else {
                    return Ok(false);
                };

                for team_arc in team_instances {
                    let Ok(team_guard) = team_arc.read() else {
                        continue;
                    };
                    for &member_id in team_guard.get_members() {
                        let Some(member_arc) = TheGameLogic::find_object_by_id(member_id) else {
                            continue;
                        };
                        let Ok(member_guard) = member_arc.read() else {
                            continue;
                        };
                        let Some(ai) = member_guard.get_ai_update_interface() else {
                            continue;
                        };
                        let Ok(ai_guard) = ai.lock() else {
                            continue;
                        };
                        let Some(completed_id) = ai_guard.get_completed_waypoint_id() else {
                            continue;
                        };
                        // C++ compares each of the completed waypoint's three path labels
                        // with the requested path.  Reaching a different path must not fire
                        // this campaign trigger.
                        if terrain
                            .get_waypoint_by_id(completed_id)
                            .is_some_and(|waypoint| {
                                waypoint.get_path_label1().as_str() == waypoint_path_name
                                    || waypoint.get_path_label2().as_str() == waypoint_path_name
                                    || waypoint.get_path_label3().as_str() == waypoint_path_name
                            })
                        {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }

            // Multiplayer: local player's alliance achieved victory
            ConditionType::MultiplayerAlliedVictory => {
                Ok(crate::helpers::TheVictoryConditions::is_local_allied_victory())
            }

            // Multiplayer: local player's alliance was defeated
            // C++: TheVictoryConditions->isLocalAlliedDefeat()
            ConditionType::MultiplayerAlliedDefeat => {
                let Ok(list) = player_list().read() else {
                    return Ok(false);
                };
                let Some(local_player_arc) = list.get_local_player() else {
                    return Ok(false);
                };
                let Ok(local_player) = local_player_arc.read() else {
                    return Ok(false);
                };

                let mut allied_count = 0usize;
                for player_arc in list.iter() {
                    if Arc::ptr_eq(player_arc, &local_player_arc) {
                        allied_count += 1;
                        if !local_player.is_defeated() {
                            return Ok(false);
                        }
                        continue;
                    }
                    let Ok(player) = player_arc.read() else {
                        continue;
                    };
                    if local_player.is_allied_with_player(&player) {
                        allied_count += 1;
                        if !player.is_defeated() {
                            return Ok(false);
                        }
                    }
                }
                Ok(allied_count > 0)
            }

            // Multiplayer: local player individually defeated (not whole alliance)
            // C++: TheVictoryConditions->isLocalDefeat() && !TheVictoryConditions->isLocalAlliedDefeat()
            ConditionType::MultiplayerPlayerDefeat => {
                let Ok(list) = player_list().read() else {
                    return Ok(false);
                };
                let Some(local_player_arc) = list.get_local_player() else {
                    return Ok(false);
                };
                let Ok(local_player) = local_player_arc.read() else {
                    return Ok(false);
                };

                if !local_player.is_player_dead() {
                    return Ok(false);
                }

                let mut has_alive_ally = false;
                for player_arc in list.iter() {
                    if Arc::ptr_eq(player_arc, &local_player_arc) {
                        continue;
                    }
                    let Ok(player) = player_arc.read() else {
                        continue;
                    };
                    if local_player.is_allied_with_player(&player) && !player.is_defeated() {
                        has_alive_ally = true;
                        break;
                    }
                }
                Ok(has_alive_ally)
            }

            // Named unit has sighted an enemy/friendly/neutral unit belonging to a side
            ConditionType::EnemySighted => {
                let unit_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "EnemySighted condition missing unit parameter".to_string(),
                    )
                })?;
                let alliance_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "EnemySighted condition missing alliance parameter".to_string(),
                    )
                })?;
                let player_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "EnemySighted condition missing player parameter".to_string(),
                    )
                })?;
                let unit_name = unit_param.get_string();
                let alliance = alliance_param.get_int();
                // Parameter::{REL_ENEMY, REL_NEUTRAL, REL_FRIEND} are the only
                // values accepted by C++ ScriptConditions::evaluateEnemySighted.
                // A malformed script must not accidentally make every nearby unit
                // satisfy the condition.
                let expected_relationship = match alliance {
                    0 => crate::common::Relationship::Enemies,
                    1 => crate::common::Relationship::Neutral,
                    2 => crate::common::Relationship::Allies,
                    _ => return Ok(false),
                };
                let Some(target_player) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };

                let tracker = get_named_object_tracker();
                let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
                    return Ok(false);
                };
                let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                    return Ok(false);
                };
                let (obj_pos, vision, source_off_map) = {
                    let Ok(obj_guard) = obj_arc.read() else {
                        return Ok(false);
                    };
                    (
                        *obj_guard.get_position(),
                        obj_guard.get_vision_range(),
                        obj_guard.is_off_map(),
                    )
                };

                let Some(partition) = crate::helpers::ThePartitionManager::get() else {
                    return Ok(false);
                };
                let nearby_objects = partition.get_objects_in_range(&obj_pos, vision);

                for nearby_id in nearby_objects {
                    if nearby_id == object_id {
                        continue;
                    }
                    let Some(nearby_arc) = TheGameLogic::find_object_by_id(nearby_id) else {
                        continue;
                    };
                    // Keep C++'s source-then-candidate read order.  Besides making the
                    // relation snapshot coherent, this avoids a reverse-lock order with
                    // gameplay code that evaluates a source object before its target.
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let Ok(nearby_guard) = nearby_arc.read() else {
                        continue;
                    };
                    if nearby_guard.is_effectively_dead() {
                        continue;
                    }
                    if nearby_guard.is_off_map() != source_off_map {
                        continue;
                    }

                    let status = nearby_guard.get_status_bits();
                    if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
                        && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
                        && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
                    {
                        continue;
                    }

                    if obj_guard.relationship_to(&nearby_guard) != expected_relationship {
                        continue;
                    }

                    // C++ compares Player* identity, not merely a player index.  Retain
                    // that exact ownership test so duplicate/stale player records cannot
                    // make the condition fire for the wrong side.
                    if nearby_guard
                        .get_controlling_player()
                        .is_some_and(|owner| Arc::ptr_eq(&owner, &target_player))
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }

            // Named bridge has been repaired (damage state changed to intact)
            ConditionType::BridgeRepaired => {
                let bridge_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "BridgeRepaired condition missing bridge parameter".to_string(),
                    )
                })?;

                let bridge_name = bridge_param.get_string();
                let tracker = get_named_object_tracker();
                let Some(object_id) = tracker.get_object_id(bridge_name).ok().flatten() else {
                    return Ok(false);
                };

                let Ok(terrain) = get_terrain_logic().read() else {
                    return Ok(false);
                };
                Ok(terrain.is_bridge_repaired(object_id))
            }

            // Named bridge has been broken (damage state changed to broken)
            ConditionType::BridgeBroken => {
                let bridge_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "BridgeBroken condition missing bridge parameter".to_string(),
                    )
                })?;

                let bridge_name = bridge_param.get_string();
                let tracker = get_named_object_tracker();
                let Some(object_id) = tracker.get_object_id(bridge_name).ok().flatten() else {
                    return Ok(false);
                };

                let Ok(terrain) = get_terrain_logic().read() else {
                    return Ok(false);
                };
                Ok(terrain.is_bridge_broken(object_id))
            }

            // Player has comparison count of a specific object type
            ConditionType::PlayerHasObjectComparison => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasObjectComparison condition missing player parameter".to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasObjectComparison condition missing comparison parameter"
                            .to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasObjectComparison condition missing count parameter".to_string(),
                    )
                })?;
                let type_param = condition.get_parameter(3).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasObjectComparison condition missing type parameter".to_string(),
                    )
                })?;

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                let types = self.resolve_object_types(type_param);

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut count = 0;
                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_effectively_dead() || obj_guard.is_destroyed() {
                        continue;
                    }
                    if types.contains_template(Some(obj_guard.get_template())) {
                        count += 1;
                    }
                }

                match comparison {
                    0 => Ok(count < target_count),  // LessThan
                    1 => Ok(count <= target_count), // LessEqual
                    2 => Ok(count == target_count), // Equal
                    3 => Ok(count >= target_count), // GreaterEqual
                    4 => Ok(count > target_count),  // Greater
                    5 => Ok(count != target_count), // NotEqual
                    _ => Ok(false),
                }
            }

            // Obsolete script conditions (no longer used in C++)
            ConditionType::ObsoleteScript1 | ConditionType::ObsoleteScript2 => {
                // C++ has no handler for these; they fall through to DEBUG_CRASH
                Ok(false)
            }

            // Player triggered a special power (any source unit)
            ConditionType::PlayerTriggeredSpecialPower => {
                self.evaluate_special_power_condition(condition, false, false)
            }

            // Player completed a special power (any source unit)
            ConditionType::PlayerCompletedSpecialPower => {
                self.evaluate_special_power_condition(condition, false, true)
            }

            // Player midway through special power (any source unit)
            ConditionType::PlayerMidwaySpecialPower => {
                self.evaluate_special_power_condition(condition, true, false)
            }

            // Player triggered special power from a specific named unit
            ConditionType::PlayerTriggeredSpecialPowerFromNamed => {
                self.evaluate_special_power_condition(condition, false, false)
            }

            // Player completed special power from a specific named unit
            ConditionType::PlayerCompletedSpecialPowerFromNamed => {
                self.evaluate_special_power_condition(condition, false, true)
            }

            // Player midway through special power from a specific named unit
            ConditionType::PlayerMidwaySpecialPowerFromNamed => {
                self.evaluate_special_power_condition(condition, true, false)
            }

            // Defunct: player selected general (removed in C++)
            ConditionType::DefunctPlayerSelectedGeneral
            | ConditionType::DefunctPlayerSelectedGeneralFromNamed => {
                // C++ DEBUG_CRASH: "PLAYER_SELECTED_GENERAL script conditions are no longer in use"
                Ok(false)
            }

            // Player built an upgrade (any source unit)
            ConditionType::PlayerBuiltUpgrade => self.evaluate_upgrade_condition(condition, false),

            // Player built an upgrade from a specific named unit
            ConditionType::PlayerBuiltUpgradeFromNamed => {
                self.evaluate_upgrade_condition(condition, true)
            }

            // Player destroyed N or more of opponent's buildings
            ConditionType::PlayerDestroyedNBuildingsPlayer => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerDestroyedNBuildingsPlayer condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let opponent_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerDestroyedNBuildingsPlayer condition missing opponent parameter"
                            .to_string(),
                    )
                })?;

                // C++ evaluatePlayerDestroyedNOrMoreBuildings resolves both players, ignores N,
                // then returns FALSE because the condition body is still a TODO.
                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(_player_guard) = player_arc.read() else {
                    return Ok(false);
                };
                let Some(opponent_arc) = self.resolve_player_from_param(opponent_param) else {
                    return Ok(false);
                };
                let Ok(_opponent_guard) = opponent_arc.read() else {
                    return Ok(false);
                };
                Ok(false)
            }

            // Unit completed sequential script execution
            // C++: NO case in switch — falls through to DEBUG_CRASH returning false.
            // ScriptEngine::hasUnitCompletedSequentialScript() always returns FALSE.
            ConditionType::UnitCompletedSequentialExecution => Ok(false),

            // Team completed sequential script execution
            // C++: NO case in switch — falls through to DEBUG_CRASH returning false.
            // ScriptEngine::hasTeamCompletedSequentialScript() always returns FALSE.
            ConditionType::TeamCompletedSequentialExecution => Ok(false),

            // Player has comparison count of unit type within a trigger area
            ConditionType::PlayerHasComparisonUnitTypeInTriggerArea => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitTypeInTriggerArea condition missing player parameter".to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitTypeInTriggerArea condition missing comparison parameter".to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitTypeInTriggerArea condition missing count parameter".to_string(),
                    )
                })?;
                let type_param = condition.get_parameter(3).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitTypeInTriggerArea condition missing type parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(4).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitTypeInTriggerArea condition missing trigger parameter".to_string(),
                    )
                })?;

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                let types = self.resolve_object_types(type_param);
                let area_name = trigger_param.get_string();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(false),
                };

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut count = 0;
                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if types.contains_template(Some(obj_guard.get_template())) {
                        if obj_guard.is_inside_trigger(&trigger) {
                            if Self::counts_for_unit_type_area_condition(
                                obj_guard.is_effectively_dead(),
                                obj_guard.is_kind_of(KindOf::Inert),
                                obj_guard.is_kind_of(KindOf::Crate),
                            ) {
                                count += 1;
                            }
                        }
                    }
                }

                match comparison {
                    0 => Ok(count < target_count),
                    1 => Ok(count <= target_count),
                    2 => Ok(count == target_count),
                    3 => Ok(count >= target_count),
                    4 => Ok(count > target_count),
                    5 => Ok(count != target_count),
                    _ => Ok(false),
                }
            }

            // Player has comparison count of unit kind within a trigger area
            // C++: evaluatePlayerHasUnitKindInArea filters by pObj->isKindOf((KindOfType)kindParam)
            ConditionType::PlayerHasComparisonUnitKindInTriggerArea => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitKindInTriggerArea condition missing player parameter".to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitKindInTriggerArea condition missing comparison parameter".to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitKindInTriggerArea condition missing count parameter".to_string(),
                    )
                })?;
                let kind_param = condition.get_parameter(3).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitKindInTriggerArea condition missing kind parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(4).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerHasComparisonUnitKindInTriggerArea condition missing trigger parameter".to_string(),
                    )
                })?;

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                let kind_of_type_int = kind_param.get_int();
                let area_name = trigger_param.get_string();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(false),
                };

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let kind_of_filter = Self::kind_of_type_to_mask(kind_of_type_int);
                let mut count = 0;
                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_effectively_dead() || obj_guard.is_kind_of(KindOf::Inert) {
                        continue;
                    }
                    if obj_guard.is_inside_trigger(&trigger) {
                        if let Some(kind) = kind_of_filter {
                            if !obj_guard.is_kind_of(kind) {
                                continue;
                            }
                        }
                        count += 1;
                    }
                }

                match comparison {
                    0 => Ok(count < target_count),
                    1 => Ok(count <= target_count),
                    2 => Ok(count == target_count),
                    3 => Ok(count >= target_count),
                    4 => Ok(count > target_count),
                    5 => Ok(count != target_count),
                    _ => Ok(false),
                }
            }

            // Named unit has sighted a specific object type
            ConditionType::TypeSighted => {
                let unit_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "TypeSighted condition missing unit parameter".to_string(),
                    )
                })?;
                let type_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "TypeSighted condition missing type parameter".to_string(),
                    )
                })?;
                let player_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "TypeSighted condition missing player parameter".to_string(),
                    )
                })?;

                let unit_name = unit_param.get_string();
                let types = self.resolve_object_types(type_param);
                let Some(target_player) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };

                let tracker = get_named_object_tracker();
                let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
                    return Ok(false);
                };
                let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                    return Ok(false);
                };
                let (obj_pos, vision, source_off_map) = {
                    let Ok(obj_guard) = obj_arc.read() else {
                        return Ok(false);
                    };
                    (
                        *obj_guard.get_position(),
                        obj_guard.get_vision_range(),
                        obj_guard.is_off_map(),
                    )
                };

                let Some(partition) = crate::helpers::ThePartitionManager::get() else {
                    return Ok(false);
                };

                for nearby_id in partition.get_objects_in_range(&obj_pos, vision) {
                    if nearby_id == object_id {
                        continue;
                    }
                    let Some(nearby_arc) = TheGameLogic::find_object_by_id(nearby_id) else {
                        continue;
                    };
                    let Ok(nearby_guard) = nearby_arc.read() else {
                        continue;
                    };
                    if nearby_guard.is_effectively_dead() {
                        continue;
                    }
                    if nearby_guard.is_off_map() != source_off_map {
                        continue;
                    }

                    let status = nearby_guard.get_status_bits();
                    if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
                        && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
                        && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
                    {
                        continue;
                    }

                    if types.contains_template(Some(nearby_guard.get_template()))
                        && nearby_guard
                            .get_controlling_player()
                            .is_some_and(|owner| Arc::ptr_eq(&owner, &target_player))
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }

            // --- Skirmish AI conditions ---

            // Skirmish: a specific special power is ready to use
            ConditionType::SkirmishSpecialPowerReady => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSpecialPowerReady condition missing player parameter".to_string(),
                    )
                })?;
                let power_name_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSpecialPowerReady condition missing power name parameter"
                            .to_string(),
                    )
                })?;

                let power_name = power_name_param.get_string();
                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_destroyed() {
                        continue;
                    }
                    if obj_guard
                        .with_special_power_module_interface_by_name(&power_name, |module| {
                            module.get_percent_ready() >= 1.0
                        })
                        .unwrap_or(false)
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }

            // Skirmish: total value of player's units inside an area meets comparison
            ConditionType::SkirmishValueInArea => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishValueInArea condition missing player parameter".to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishValueInArea condition missing comparison parameter".to_string(),
                    )
                })?;
                let value_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishValueInArea condition missing value parameter".to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(3).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishValueInArea condition missing trigger parameter".to_string(),
                    )
                })?;

                let comparison = comparison_param.get_int() as u32;
                let target_value = value_param.get_int();
                let area_name = trigger_param.get_string();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(false),
                };

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut total_cost = 0i32;
                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_kind_of(KindOf::Inert) {
                        continue;
                    }
                    if !obj_guard.is_effectively_dead() && obj_guard.is_inside_trigger(&trigger) {
                        total_cost += obj_guard.get_template().get_build_cost();
                    }
                }

                match comparison {
                    0 => Ok(total_cost < target_value),
                    1 => Ok(total_cost <= target_value),
                    2 => Ok(total_cost == target_value),
                    3 => Ok(total_cost >= target_value),
                    4 => Ok(total_cost > target_value),
                    5 => Ok(total_cost != target_value),
                    _ => Ok(false),
                }
            }

            // Skirmish: player is a specific faction
            ConditionType::SkirmishPlayerFaction => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerFaction condition missing player parameter".to_string(),
                    )
                })?;
                let faction_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerFaction condition missing faction parameter".to_string(),
                    )
                })?;

                let faction_name = faction_param.get_string();
                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                Ok(player_guard.get_side() == faction_name)
            }

            // Skirmish: supplies value within distance of a location meets threshold
            ConditionType::SkirmishSuppliesValueWithinDistance => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSuppliesValueWithinDistance condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let distance_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSuppliesValueWithinDistance condition missing distance parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSuppliesValueWithinDistance condition missing trigger parameter"
                            .to_string(),
                    )
                })?;
                let threshold_param = condition.get_parameter(3).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishSuppliesValueWithinDistance condition missing threshold parameter"
                            .to_string(),
                    )
                })?;

                let distance = distance_param.get_real();
                let area_name = trigger_param.get_string();
                let threshold = threshold_param.get_real();

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let Some(trigger) = self.get_trigger_area(area_name) else {
                    return Ok(false);
                };
                let Some(partition) = crate::helpers::ThePartitionManager::get() else {
                    return Ok(false);
                };

                let center = trigger.get_center_point();
                let radius = trigger.get_radius() + distance;
                let supply_box_value = player_guard.get_supply_box_value() as f32;
                let mut max_value = 0.0f32;

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
                    if !obj_guard.is_kind_of(KindOf::Structure) {
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
                                    player_guard.get_relationship(&owner_guard)
                                        == crate::common::Relationship::Neutral
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

                    let Some(module) = obj_guard.find_update_module("SupplyWarehouseDockUpdate")
                    else {
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

                    max_value = max_value.max(supply_box_value * boxes as f32);
                }

                Ok(max_value > threshold)
            }

            // Skirmish: tech building within distance of a location
            // C++: ThePartitionManager->getClosestObject with KindOf::TECH_BUILDING + player filters
            ConditionType::SkirmishTechBuildingWithinDistance => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishTechBuildingWithinDistance condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let distance_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishTechBuildingWithinDistance condition missing distance parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishTechBuildingWithinDistance condition missing trigger parameter"
                            .to_string(),
                    )
                })?;

                let distance = distance_param.get_real();
                let area_name = trigger_param.get_string();

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };
                let player_index = player_guard.get_player_index();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(false),
                };

                let center = trigger.get_center_point();
                let radius = trigger.get_radius() + distance;

                let Some(partition) = crate::helpers::ThePartitionManager::get() else {
                    return Ok(false);
                };

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
                    if !obj_guard.is_kind_of(KindOf::TechBuilding) {
                        continue;
                    }

                    let Some(owner_id) = obj_guard.get_controlling_player_id() else {
                        continue;
                    };
                    if owner_id == player_index as u32 {
                        continue;
                    }
                    if let Some(owner_arc) = player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_player(owner_id as i32).cloned())
                    {
                        if let Ok(owner_guard) = owner_arc.read() {
                            if !player_guard.is_allied_with_player(&owner_guard) {
                                continue;
                            }
                        }
                    }

                    return Ok(true);
                }
                Ok(false)
            }

            // Skirmish: all team members have command button ready
            ConditionType::SkirmishCommandButtonReadyAll => {
                let team_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishCommandButtonReadyAll condition missing team parameter"
                            .to_string(),
                    )
                })?;
                let button_name_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishCommandButtonReadyAll condition missing button name parameter"
                            .to_string(),
                    )
                })?;

                let button_name = button_name_param.get_string();
                let Some(bridge) = crate::control_bar::get_control_bar_bridge() else {
                    return Ok(false);
                };
                let Some(_button) = bridge.find_command_button_by_name(&button_name) else {
                    return Ok(false);
                };

                let team_name = self.resolve_team_name_token(team_param.get_string());
                let team_instances = self.resolve_team_instances(&team_name);
                if team_instances.is_empty() {
                    return Ok(false);
                }

                let mut all_ready = true;
                'outer: for team_arc in &team_instances {
                    let Ok(team_guard) = team_arc.read() else {
                        all_ready = false;
                        break;
                    };
                    for &member_id in team_guard.get_members() {
                        let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                            all_ready = false;
                            break 'outer;
                        };
                        let Ok(obj_guard) = obj_arc.read() else {
                            all_ready = false;
                            break 'outer;
                        };
                        if !obj_guard.is_destroyed() && _button.is_ready(&obj_guard) {
                            continue;
                        }
                        all_ready = false;
                        break 'outer;
                    }
                }
                Ok(all_ready)
            }

            // Skirmish: any team member has command button ready
            ConditionType::SkirmishCommandButtonReadyPartial => {
                let team_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishCommandButtonReadyPartial condition missing team parameter"
                            .to_string(),
                    )
                })?;
                let button_name_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishCommandButtonReadyPartial condition missing button name parameter"
                            .to_string(),
                    )
                })?;

                let button_name = button_name_param.get_string();
                let Some(bridge) = crate::control_bar::get_control_bar_bridge() else {
                    return Ok(false);
                };
                let Some(_button) = bridge.find_command_button_by_name(&button_name) else {
                    return Ok(false);
                };

                let team_name = self.resolve_team_name_token(team_param.get_string());
                for team_arc in self.resolve_team_instances(&team_name) {
                    let Ok(team_guard) = team_arc.read() else {
                        continue;
                    };
                    for &member_id in team_guard.get_members() {
                        let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                            continue;
                        };
                        let Ok(obj_guard) = obj_arc.read() else {
                            continue;
                        };
                        if !obj_guard.is_destroyed() && _button.is_ready(&obj_guard) {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            }

            // Skirmish: unowned (neutral) faction unit count meets comparison
            ConditionType::SkirmishUnownedFactionUnitExists => {
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishUnownedFactionUnitExists condition missing comparison parameter"
                            .to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishUnownedFactionUnitExists condition missing count parameter"
                            .to_string(),
                    )
                })?;

                // C++ counts neutral player objects with DISABLED_UNMANNED
                let Ok(list) = player_list().read() else {
                    return Ok(false);
                };
                let neutral_player = list.get_neutral_player();
                let Some(neutral_arc) = neutral_player else {
                    return Ok(false);
                };
                let Ok(neutral_guard) = neutral_arc.read() else {
                    return Ok(false);
                };

                let mut num_faction_units = 0i32;
                for obj_id in neutral_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_disabled_by_type(DisabledType::Unmanned) {
                        num_faction_units += 1;
                    }
                }

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                match comparison {
                    0 => Ok(num_faction_units < target_count),
                    1 => Ok(num_faction_units <= target_count),
                    2 => Ok(num_faction_units == target_count),
                    3 => Ok(num_faction_units >= target_count),
                    4 => Ok(num_faction_units > target_count),
                    5 => Ok(num_faction_units != target_count),
                    _ => Ok(false),
                }
            }

            // Skirmish: player has prerequisites to build a specific object type
            // C++: types.m_types->canBuildAny(player)
            ConditionType::SkirmishPlayerHasPrerequisiteToBuild => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasPrerequisiteToBuild condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let type_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasPrerequisiteToBuild condition missing type parameter"
                            .to_string(),
                    )
                })?;

                let types = self.resolve_object_types(type_param);

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                Ok(types.can_build_any(&player_guard))
            }

            // Skirmish: player's garrisoned building count meets comparison
            ConditionType::SkirmishPlayerHasComparisonGarrisoned => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonGarrisoned condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonGarrisoned condition missing comparison parameter".to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonGarrisoned condition missing count parameter"
                            .to_string(),
                    )
                })?;

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                // C++ counts buildings with ContainModuleInterface::isGarrisonable() && getContainCount() > 0
                let mut num_garrisoned = 0i32;
                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
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
                        num_garrisoned += 1;
                    }
                }

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                match comparison {
                    0 => Ok(num_garrisoned < target_count),
                    1 => Ok(num_garrisoned <= target_count),
                    2 => Ok(num_garrisoned == target_count),
                    3 => Ok(num_garrisoned >= target_count),
                    4 => Ok(num_garrisoned > target_count),
                    5 => Ok(num_garrisoned != target_count),
                    _ => Ok(false),
                }
            }

            // Skirmish: player's captured unit count meets comparison
            ConditionType::SkirmishPlayerHasComparisonCapturedUnits => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonCapturedUnits condition missing player parameter".to_string(),
                    )
                })?;
                let comparison_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonCapturedUnits condition missing comparison parameter".to_string(),
                    )
                })?;
                let count_param = condition.get_parameter(2).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasComparisonCapturedUnits condition missing count parameter".to_string(),
                    )
                })?;

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut num_captured = 0i32;
                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_captured() {
                        num_captured += 1;
                    }
                }

                let comparison = comparison_param.get_int() as u32;
                let target_count = count_param.get_int();
                match comparison {
                    0 => Ok(num_captured < target_count),
                    1 => Ok(num_captured <= target_count),
                    2 => Ok(num_captured == target_count),
                    3 => Ok(num_captured >= target_count),
                    4 => Ok(num_captured > target_count),
                    5 => Ok(num_captured != target_count),
                    _ => Ok(false),
                }
            }

            // Skirmish: named trigger area exists on the map
            ConditionType::SkirmishNamedAreaExist => {
                let trigger_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishNamedAreaExist condition missing trigger parameter".to_string(),
                    )
                })?;

                let area_name = trigger_param.get_string();
                Ok(self.get_trigger_area(area_name).is_some())
            }

            // Skirmish: player has units inside a trigger area
            ConditionType::SkirmishPlayerHasUnitsInArea => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasUnitsInArea condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasUnitsInArea condition missing trigger parameter"
                            .to_string(),
                    )
                })?;

                let area_name = trigger_param.get_string();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(false),
                };

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut count = 0;
                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_inside_trigger(&trigger) {
                        if !obj_guard.is_effectively_dead()
                            && !obj_guard.is_kind_of(KindOf::Inert)
                            && !obj_guard.is_kind_of(KindOf::Projectile)
                        {
                            count += 1;
                        }
                    }
                }

                Ok(count > 0)
            }

            // Skirmish: player has been attacked by another player
            ConditionType::SkirmishPlayerHasBeenAttackedByPlayer => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasBeenAttackedByPlayer condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let attacker_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasBeenAttackedByPlayer condition missing attacker parameter".to_string(),
                    )
                })?;

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let Some(attacker_arc) = self.resolve_player_from_param(attacker_param) else {
                    return Ok(false);
                };
                let Ok(attacker_guard) = attacker_arc.read() else {
                    return Ok(false);
                };

                Ok(player_guard.get_attacked_by(attacker_guard.get_player_index()))
            }

            // Skirmish: player has no units inside a trigger area
            ConditionType::SkirmishPlayerIsOutsideArea => {
                // C++: !evaluateSkirmishPlayerHasUnitsInArea

                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerIsOutsideArea condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let trigger_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerIsOutsideArea condition missing trigger parameter"
                            .to_string(),
                    )
                })?;

                let area_name = trigger_param.get_string();

                let trigger = match self.get_trigger_area(area_name) {
                    Some(t) => t,
                    None => return Ok(true), // No trigger = no units inside = outside
                };

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(true);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(true);
                };

                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if obj_guard.is_inside_trigger(&trigger) {
                        if !obj_guard.is_effectively_dead()
                            && !obj_guard.is_kind_of(KindOf::Inert)
                            && !obj_guard.is_kind_of(KindOf::Projectile)
                        {
                            return Ok(false); // Found a unit inside = not outside
                        }
                    }
                }

                Ok(true)
            }

            // Skirmish: player has discovered another player's units
            ConditionType::SkirmishPlayerHasDiscoveredPlayer => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasDiscoveredPlayer condition missing player parameter"
                            .to_string(),
                    )
                })?;
                let discovered_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SkirmishPlayerHasDiscoveredPlayer condition missing discovered-by parameter".to_string(),
                    )
                })?;

                let Some(discovered_by_arc) = self.resolve_player_from_param(discovered_param)
                else {
                    return Ok(false);
                };
                let Ok(discovered_by_guard) = discovered_by_arc.read() else {
                    return Ok(false);
                };
                let discovered_by_index = discovered_by_guard.get_player_index() as i32;

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                // C++: iterates player objects checking shroud status against discoveredByIndex
                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    let shroud = obj_guard.get_shrouded_status(discovered_by_index);
                    if matches!(
                        shroud,
                        ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear
                    ) {
                        return Ok(true);
                    }
                }

                Ok(false)
            }

            // Music track has completed playback
            // C++: TheAudio->hasMusicTrackCompleted(str, param)
            ConditionType::MusicTrackHasCompleted => {
                let music_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "MusicTrackHasCompleted condition missing music parameter".to_string(),
                    )
                })?;
                let int_param = condition.get_parameter(1);

                let track_name = music_param.get_string();
                let param = int_param.map(|p| p.get_int()).unwrap_or(0);

                let handler = self
                    .with_evaluation_engine_ref(|engine| engine.action_handler())
                    .flatten();
                Ok(handler
                    .map(|h| h.has_music_track_completed(&track_name, param))
                    .unwrap_or(false))
            }

            // Player lost all objects of a specific type (had them before, now fewer)
            ConditionType::PlayerLostObjectType => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerLostObjectType condition missing player parameter".to_string(),
                    )
                })?;
                let type_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "PlayerLostObjectType condition missing type parameter".to_string(),
                    )
                })?;

                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let player_index = player_arc.read().ok().map(|p| p.get_player_index() as i32);
                let Some(player_index) = player_index else {
                    return Ok(false);
                };

                let type_name = type_param.get_string();
                let types = self.resolve_object_types(type_param);

                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                let mut current_count = 0i32;
                for obj_id in player_guard.get_object_ids() {
                    let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id)
                        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                    else {
                        continue;
                    };
                    let Ok(obj_guard) = obj_arc.read() else {
                        continue;
                    };
                    if !obj_guard.is_destroyed()
                        && types.contains_template(Some(obj_guard.get_template()))
                    {
                        current_count += 1;
                    }
                }

                // C++ compares current count to previously stored count via
                // ScriptEngine.  ScriptEngine::update installs a lexical
                // active engine, so re-locking the global handle here would
                // deadlock the live campaign path.
                let stored_count = self
                    .with_evaluation_engine_ref(|engine| {
                        engine.get_object_count(player_index, type_name)
                    })
                    .unwrap_or(current_count);
                let _ = self.with_evaluation_engine_mut(|engine| {
                    engine.set_object_count(player_index, type_name, current_count);
                });

                Ok(current_count < stored_count)
            }

            // Skirmish: player's supply source is safe (above minimum amount)
            ConditionType::SupplySourceSafe => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SupplySourceSafe condition missing player parameter".to_string(),
                    )
                })?;
                let min_param = condition.get_parameter(1);

                let player_arc = self.resolve_player_from_param(player_param);
                let Some(player_arc) = player_arc else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };
                let player_id = player_guard.get_player_index() as u32;
                let min_supplies = min_param.as_ref().map(|p| p.get_int()).unwrap_or(0) as i32;

                let safe = crate::ai::integration::with_ai_integration(|manager| {
                    manager.with_ai_player(player_id, |ai| ai.is_supply_source_safe(min_supplies))
                })
                .flatten()
                .unwrap_or(false);
                Ok(safe)
            }

            // Skirmish: player's supply source is under attack
            ConditionType::SupplySourceAttacked => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "SupplySourceAttacked condition missing player parameter".to_string(),
                    )
                })?;

                let player_arc = self.resolve_player_from_param(player_param);
                let Some(player_arc) = player_arc else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };
                let player_id = player_guard.get_player_index() as u32;

                let attacked = crate::ai::integration::with_ai_integration_mut(|manager| {
                    manager.with_ai_player_mut(player_id, |ai| ai.is_supply_source_attacked())
                })
                .flatten()
                .unwrap_or(false);
                Ok(attacked)
            }

            // Skirmish: player's start position matches a specific index
            ConditionType::StartPositionIs => {
                let player_param = condition.get_parameter(0).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "StartPositionIs condition missing player parameter".to_string(),
                    )
                })?;
                let start_param = condition.get_parameter(1).ok_or_else(|| {
                    GameLogicError::Configuration(
                        "StartPositionIs condition missing start index parameter".to_string(),
                    )
                })?;

                // C++: ndx = pStartNdx->getInt()-1 (externally 1-based, internally 0-based)
                let ndx = start_param.get_int() - 1;
                let Some(player_arc) = self.resolve_player_from_param(player_param) else {
                    return Ok(false);
                };
                let Ok(player_guard) = player_arc.read() else {
                    return Ok(false);
                };

                Ok(player_guard.get_mp_start_index() == ndx)
            }

            _ => {
                let ctx = self.make_script_context();
                let mut evaluator = ScriptConditionEvaluator::new(ctx);
                match evaluator.evaluate_condition(condition) {
                    Ok(ScriptConditionResult::True) => Ok(true),
                    Ok(ScriptConditionResult::False) => Ok(false),
                    Ok(ScriptConditionResult::Error(msg)) => Err(GameLogicError::Configuration(
                        format!("Script condition evaluation error: {}", msg),
                    )),
                    Err(err) => Err(GameLogicError::Configuration(format!(
                        "Script condition evaluation failed: {}",
                        err
                    ))),
                }
            }
        };
        let elapsed = eval_started.elapsed();
        if elapsed >= std::time::Duration::from_millis(SLOW_SCRIPT_CONDITION_WARN_MS) {
            log::warn!(
                "Slow script condition evaluate: {:?} took {:?}",
                condition_type,
                elapsed
            );
        }
        result
    }
}
