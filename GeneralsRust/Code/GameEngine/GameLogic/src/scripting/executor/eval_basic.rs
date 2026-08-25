//! Condition dispatch plus OR/AND, counter, flag, timer, player, and team evaluators
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;

/// C++ ScriptEngine.cpp:5810 / 5935 — THE_PLAYER aliases are Challenge-only.
fn is_generals_challenge_context() -> bool {
    crate::scripting::core::is_generals_challenge_campaign()
}

/// C++ Team.cpp:142-145 locoSetMatches — WB bit1 (air) shifts to locomotor AIR.
fn loco_set_matches(lstm: u32, surface_bit_flags: u32) -> bool {
    let remapped = (surface_bit_flags & 0x01) | ((surface_bit_flags & 0x02) << 2);
    (remapped & lstm) != 0
}

fn bool_result(value: bool) -> ScriptConditionResult {
    if value {
        ScriptConditionResult::True
    } else {
        ScriptConditionResult::False
    }
}

impl ScriptConditionEvaluator {
    pub(crate) fn resolve_string_token(&self, raw: &str) -> String {
        match raw {
            THE_PLAYER => {
                // C++ ScriptEngine::getPlayerFromAsciiString (ScriptEngine.cpp:5809-5814):
                // remap ThePlayer to the local player only in Generals Challenge.
                if !is_generals_challenge_context() {
                    raw.to_string()
                } else {
                    player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_local_player().cloned())
                        .and_then(|p| {
                            p.read().ok().and_then(|p| {
                                NameKeyGenerator::key_to_name(p.get_player_name_key())
                            })
                        })
                        .unwrap_or_else(|| raw.to_string())
                }
            }
            THIS_PLAYER => with_script_engine_ref(|engine| engine.get_current_player_name())
                .flatten()
                .unwrap_or_else(|| raw.to_string()),
            LOCAL_PLAYER => player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|p| {
                    p.read()
                        .ok()
                        .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
                })
                .unwrap_or_else(|| raw.to_string()),
            THIS_TEAM => with_script_engine_ref(|engine| {
                engine
                    .get_condition_team_name()
                    .or_else(|| engine.get_calling_team_name())
            })
            .flatten()
            .unwrap_or_else(|| raw.to_string()),
            TEAM_THE_PLAYER => {
                // C++ ScriptEngine::getTeamNamed (ScriptEngine.cpp:5935-5939):
                // remap teamThePlayer only in Generals Challenge campaigns.
                if !is_generals_challenge_context() {
                    return raw.to_string();
                }
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_local_player().cloned())
                    .and_then(|p| p.read().ok().and_then(|p| p.get_default_team()))
                    .and_then(|team| team.read().ok().map(|t| t.get_name().to_string()))
                    .unwrap_or_else(|| raw.to_string())
            }
            _ => raw.to_string(),
        }
    }

    pub(crate) fn get_team_by_name(
        &self,
        team_name: &str,
    ) -> Result<Arc<RwLock<crate::team::Team>>, ScriptError> {
        self.lookup_condition_team(team_name)
            .ok_or_else(|| ScriptError::TeamNotFound(team_name.to_string()))
    }

    /// C++ ScriptEngine::getTeamNamed — first instance, never every prototype copy.
    /// Singleton prototypes return NULL unless that instance is active.
    pub(crate) fn lookup_condition_team(
        &self,
        team_name: &str,
    ) -> Option<Arc<RwLock<crate::team::Team>>> {
        let resolved = self.resolve_string_token(team_name);
        let calling = with_script_engine_ref(|engine| {
            engine
                .get_calling_team_name()
                .or_else(|| engine.get_condition_team_name())
        })
        .flatten();
        let preferred = calling.filter(|name| name == &resolved);

        let factory = get_team_factory();
        let factory_guard = factory.lock().ok()?;
        let pick = |name: &str| -> Option<Arc<RwLock<crate::team::Team>>> {
            let team = factory_guard.find_team_instances(name).into_iter().next()?;
            if factory_guard
                .find_team_prototype(name)
                .is_some_and(|proto| proto.is_singleton())
            {
                let active = team.read().ok().is_some_and(|guard| guard.is_active());
                if !active {
                    return None;
                }
            }
            Some(team)
        };
        if let Some(name) = preferred {
            if let Some(team) = pick(&name) {
                return Some(team);
            }
        }
        pick(&resolved)
    }

    pub(crate) fn get_trigger_area(
        &self,
        area_name: &str,
    ) -> Result<crate::polygon_trigger::PolygonTrigger, ScriptError> {
        if let Some(trigger) =
            with_script_engine_ref(|engine| engine.get_qualified_trigger_area_by_name(area_name))
                .flatten()
        {
            return Ok(trigger);
        }

        let resolved = crate::scripting::engine::qualify_trigger_area_name(area_name, None)
            .unwrap_or_else(|| area_name.to_string());
        if let Ok(terrain) = get_terrain_logic().read() {
            if let Some(trigger) = terrain.get_trigger_area_by_name(&resolved) {
                Ok(trigger.clone())
            } else {
                Err(ScriptError::ObjectNotFound(format!(
                    "Trigger area '{}' not found",
                    area_name
                )))
            }
        } else {
            Err(ScriptError::ExecutionFailed(
                "Failed to lock terrain logic".to_string(),
            ))
        }
    }

    /// Evaluate a script condition
    ///
    /// C++ Reference: ScriptConditions::evaluateCondition(Condition *pCondition)
    pub fn evaluate_condition(
        &mut self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let condition_type = condition.get_condition_type();

        // Dispatch to the appropriate handler based on condition type
        match condition_type {
            // ============================================================================
            // BASIC CONDITIONS
            // ============================================================================
            ConditionType::ConditionFalse => Ok(ScriptConditionResult::False),
            ConditionType::ConditionTrue => Ok(ScriptConditionResult::True),
            ConditionType::Counter => self.eval_counter(condition),
            ConditionType::Flag => self.eval_flag(condition),
            ConditionType::TimerExpired => self.eval_timer_expired(condition),

            // ============================================================================
            // PLAYER CONDITIONS
            // ============================================================================
            ConditionType::PlayerAllDestroyed => self.eval_player_all_destroyed(condition),
            ConditionType::PlayerAllBuildfacilitiesDestroyed => {
                self.eval_player_all_buildfacilities_destroyed(condition)
            }
            ConditionType::PlayerHasCredits => self.eval_player_has_credits(condition),
            ConditionType::PlayerHasNOrFewerBuildings => {
                self.eval_player_has_n_or_fewer_buildings(condition)
            }
            ConditionType::PlayerHasPower => self.eval_player_has_power(condition),
            ConditionType::PlayerHasNoPower => self.eval_player_has_no_power(condition),
            ConditionType::PlayerHasNOrFewerFactionBuildings => {
                self.eval_player_has_n_or_fewer_faction_buildings(condition)
            }
            ConditionType::PlayerPowerComparePercent => {
                self.eval_player_power_compare_percent(condition)
            }
            ConditionType::PlayerExcessPowerCompareValue => {
                self.eval_player_excess_power_compare_value(condition)
            }
            ConditionType::PlayerAcquiredScience => self.eval_player_acquired_science(condition),
            ConditionType::PlayerHasSciencepurchasepoints => {
                self.eval_player_has_science_purchase_points(condition)
            }
            ConditionType::PlayerCanPurchaseScience => {
                self.eval_player_can_purchase_science(condition)
            }
            ConditionType::PlayerLostObjectType => self.eval_player_lost_object_type(condition),
            ConditionType::PlayerDestroyedNBuildingsPlayer => {
                self.eval_player_destroyed_n_buildings_player(condition)
            }
            ConditionType::PlayerHasObjectComparison => {
                self.eval_player_has_object_comparison(condition)
            }

            // ============================================================================
            // TEAM CONDITIONS
            // ============================================================================
            ConditionType::TeamInsideAreaPartially => {
                self.eval_team_inside_area_partially(condition)
            }
            ConditionType::TeamDestroyed => self.eval_team_destroyed(condition),
            ConditionType::TeamHasUnits => self.eval_team_has_units(condition),
            ConditionType::TeamStateIs => self.eval_team_state_is(condition),
            ConditionType::TeamStateIsNot => self.eval_team_state_is_not(condition),
            ConditionType::TeamInsideAreaEntirely => self.eval_team_inside_area_entirely(condition),
            ConditionType::TeamOutsideAreaEntirely => {
                self.eval_team_outside_area_entirely(condition)
            }
            ConditionType::TeamAttackedByObjecttype => {
                self.eval_team_attacked_by_object_type(condition)
            }
            ConditionType::TeamAttackedByPlayer => self.eval_team_attacked_by_player(condition),
            ConditionType::TeamCreated => self.eval_team_created(condition),
            ConditionType::TeamDiscovered => self.eval_team_discovered(condition),
            ConditionType::TeamOwnedByPlayer => self.eval_team_owned_by_player(condition),
            ConditionType::TeamReachedWaypointsEnd => {
                self.eval_team_reached_waypoints_end(condition)
            }
            ConditionType::TeamEnteredAreaEntirely => {
                self.eval_team_entered_area_entirely(condition)
            }
            ConditionType::TeamEnteredAreaPartially => {
                self.eval_team_entered_area_partially(condition)
            }
            ConditionType::TeamExitedAreaEntirely => self.eval_team_exited_area_entirely(condition),
            ConditionType::TeamExitedAreaPartially => {
                self.eval_team_exited_area_partially(condition)
            }
            ConditionType::TeamCompletedSequentialExecution => {
                self.eval_team_completed_sequential_execution(condition)
            }
            ConditionType::TeamAllHasObjectStatus => {
                self.eval_team_all_has_object_status(condition)
            }
            ConditionType::TeamSomeHaveObjectStatus => {
                self.eval_team_some_have_object_status(condition)
            }

            // ============================================================================
            // NAMED OBJECT CONDITIONS
            // ============================================================================
            ConditionType::NamedInsideArea => self.eval_named_inside_area(condition),
            ConditionType::NamedOutsideArea => self.eval_named_outside_area(condition),
            ConditionType::NamedDestroyed => self.eval_named_destroyed(condition),
            ConditionType::NamedNotDestroyed => self.eval_named_not_destroyed(condition),
            ConditionType::NamedAttackedByObjecttype => {
                self.eval_named_attacked_by_object_type(condition)
            }
            ConditionType::NamedAttackedByPlayer => self.eval_named_attacked_by_player(condition),
            ConditionType::NamedCreated => self.eval_named_created(condition),
            ConditionType::NamedDiscovered => self.eval_named_discovered(condition),
            ConditionType::NamedOwnedByPlayer => self.eval_named_owned_by_player(condition),
            ConditionType::NamedReachedWaypointsEnd => {
                self.eval_named_reached_waypoints_end(condition)
            }
            ConditionType::NamedSelected => self.eval_named_selected(condition),
            ConditionType::NamedEnteredArea => self.eval_named_entered_area(condition),
            ConditionType::NamedExitedArea => self.eval_named_exited_area(condition),
            ConditionType::NamedDying => self.eval_named_dying(condition),
            ConditionType::NamedTotallyDead => self.eval_named_totally_dead(condition),
            ConditionType::NamedBuildingIsEmpty => self.eval_named_building_is_empty(condition),
            ConditionType::NamedHasFreeContainerSlots => {
                self.eval_named_has_free_container_slots(condition)
            }

            // ============================================================================
            // UNIT CONDITIONS
            // ============================================================================
            ConditionType::UnitHealth => self.eval_unit_health(condition),
            ConditionType::UnitCompletedSequentialExecution => {
                self.eval_unit_completed_sequential_execution(condition)
            }
            ConditionType::UnitEmptied => self.eval_unit_emptied(condition),
            ConditionType::UnitHasObjectStatus => self.eval_unit_has_object_status(condition),

            // ============================================================================
            // CAMERA CONDITIONS
            // ============================================================================
            ConditionType::CameraMovementFinished => self.eval_camera_movement_finished(condition),

            // ============================================================================
            // BUILDING CONDITIONS
            // ============================================================================
            ConditionType::BuiltByPlayer => self.eval_built_by_player(condition),
            ConditionType::BuildingEnteredByPlayer => {
                self.eval_building_entered_by_player(condition)
            }
            ConditionType::BridgeRepaired => self.eval_bridge_repaired(condition),
            ConditionType::BridgeBroken => self.eval_bridge_broken(condition),

            // ============================================================================
            // SPECIAL POWER CONDITIONS
            // ============================================================================
            ConditionType::PlayerTriggeredSpecialPower => {
                self.eval_player_triggered_special_power(condition)
            }
            ConditionType::PlayerCompletedSpecialPower => {
                self.eval_player_completed_special_power(condition)
            }
            ConditionType::PlayerMidwaySpecialPower => {
                self.eval_player_midway_special_power(condition)
            }
            ConditionType::PlayerTriggeredSpecialPowerFromNamed => {
                self.eval_player_triggered_special_power_from_named(condition)
            }
            ConditionType::PlayerCompletedSpecialPowerFromNamed => {
                self.eval_player_completed_special_power_from_named(condition)
            }
            ConditionType::PlayerMidwaySpecialPowerFromNamed => {
                self.eval_player_midway_special_power_from_named(condition)
            }

            // ============================================================================
            // UPGRADE CONDITIONS
            // ============================================================================
            ConditionType::PlayerBuiltUpgrade => self.eval_player_built_upgrade(condition),
            ConditionType::PlayerBuiltUpgradeFromNamed => {
                self.eval_player_built_upgrade_from_named(condition)
            }

            // ============================================================================
            // MULTIPLAYER CONDITIONS
            // ============================================================================
            ConditionType::MultiplayerAlliedVictory => {
                self.eval_multiplayer_allied_victory(condition)
            }
            ConditionType::MultiplayerAlliedDefeat => {
                self.eval_multiplayer_allied_defeat(condition)
            }
            ConditionType::MultiplayerPlayerDefeat => {
                self.eval_multiplayer_player_defeat(condition)
            }

            // ============================================================================
            // MEDIA CONDITIONS
            // ============================================================================
            ConditionType::HasFinishedVideo => self.eval_has_finished_video(condition),
            ConditionType::HasFinishedSpeech => self.eval_has_finished_speech(condition),
            ConditionType::HasFinishedAudio => self.eval_has_finished_audio(condition),
            ConditionType::MusicTrackHasCompleted => self.eval_music_track_has_completed(condition),

            // ============================================================================
            // MISCELLANEOUS CONDITIONS
            // ============================================================================
            ConditionType::EnemySighted => self.eval_enemy_sighted(condition),
            ConditionType::TypeSighted => self.eval_type_sighted(condition),
            ConditionType::MissionAttempts => self.eval_mission_attempts(condition),
            ConditionType::SupplySourceSafe => self.eval_supply_source_safe(condition),
            ConditionType::SupplySourceAttacked => self.eval_supply_source_attacked(condition),
            ConditionType::StartPositionIs => self.eval_start_position_is(condition),

            // ============================================================================
            // SKIRMISH CONDITIONS
            // ============================================================================
            ConditionType::SkirmishSpecialPowerReady => {
                self.eval_skirmish_special_power_ready(condition)
            }
            ConditionType::SkirmishValueInArea => self.eval_skirmish_value_in_area(condition),
            ConditionType::SkirmishPlayerFaction => self.eval_skirmish_player_faction(condition),
            ConditionType::SkirmishSuppliesValueWithinDistance => {
                self.eval_skirmish_supplies_value_within_distance(condition)
            }
            ConditionType::SkirmishTechBuildingWithinDistance => {
                self.eval_skirmish_tech_building_within_distance(condition)
            }
            ConditionType::SkirmishCommandButtonReadyAll => {
                self.eval_skirmish_command_button_ready_all(condition)
            }
            ConditionType::SkirmishCommandButtonReadyPartial => {
                self.eval_skirmish_command_button_ready_partial(condition)
            }
            ConditionType::SkirmishUnownedFactionUnitExists => {
                self.eval_skirmish_unowned_faction_unit_exists(condition)
            }
            ConditionType::SkirmishPlayerHasPrerequisiteToBuild => {
                self.eval_skirmish_player_has_prerequisite_to_build(condition)
            }
            ConditionType::SkirmishPlayerHasComparisonGarrisoned => {
                self.eval_skirmish_player_has_comparison_garrisoned(condition)
            }
            ConditionType::SkirmishPlayerHasComparisonCapturedUnits => {
                self.eval_skirmish_player_has_comparison_captured_units(condition)
            }
            ConditionType::SkirmishNamedAreaExist => self.eval_skirmish_named_area_exist(condition),
            ConditionType::SkirmishPlayerHasUnitsInArea => {
                self.eval_skirmish_player_has_units_in_area(condition)
            }
            ConditionType::SkirmishPlayerHasBeenAttackedByPlayer => {
                self.eval_skirmish_player_has_been_attacked_by_player(condition)
            }
            ConditionType::SkirmishPlayerIsOutsideArea => {
                self.eval_skirmish_player_is_outside_area(condition)
            }
            ConditionType::SkirmishPlayerHasDiscoveredPlayer => {
                self.eval_skirmish_player_has_discovered_player(condition)
            }

            // ============================================================================
            // AREA CONDITIONS
            // ============================================================================
            ConditionType::PlayerHasComparisonUnitTypeInTriggerArea => {
                self.eval_player_has_comparison_unit_type_in_trigger_area(condition)
            }
            ConditionType::PlayerHasComparisonUnitKindInTriggerArea => {
                self.eval_player_has_comparison_unit_kind_in_trigger_area(condition)
            }

            // ============================================================================
            // OBSOLETE/DEFUNCT CONDITIONS
            // ============================================================================
            ConditionType::ObsoleteScript1 => Ok(ScriptConditionResult::False),
            ConditionType::ObsoleteScript2 => Ok(ScriptConditionResult::False),
            ConditionType::DefunctPlayerSelectedGeneral => Ok(ScriptConditionResult::False),
            ConditionType::DefunctPlayerSelectedGeneralFromNamed => {
                Ok(ScriptConditionResult::False)
            }

            ConditionType::NumItems => Ok(ScriptConditionResult::False),
        }
    }

    /// Evaluate an OR condition (disjunction of AND conditions)
    pub fn evaluate_or_condition(
        &mut self,
        or_condition: &mut OrCondition,
    ) -> Result<bool, ScriptError> {
        // Iterate through all OR branches
        let mut current_or = Some(or_condition);
        while let Some(or_cond) = current_or {
            // Evaluate the AND chain for this OR branch
            if let Some(and_cond) = or_cond.first_and.as_deref_mut() {
                if self.evaluate_and_chain(and_cond)? {
                    return Ok(true);
                }
            }
            current_or = or_cond.next_or.as_deref_mut();
        }
        Ok(false)
    }

    /// Evaluate an AND chain of conditions
    pub(crate) fn evaluate_and_chain(
        &mut self,
        condition: &mut Condition,
    ) -> Result<bool, ScriptError> {
        let mut current = Some(condition);
        while let Some(cond) = current {
            match self.evaluate_condition(cond)? {
                ScriptConditionResult::True => {
                    // Continue to next AND condition
                    current = cond.next_and_condition.as_deref_mut();
                }
                ScriptConditionResult::False => {
                    // AND chain failed
                    return Ok(false);
                }
                ScriptConditionResult::Error(msg) => {
                    return Err(ScriptError::EvaluationFailed(msg));
                }
            }
        }
        // All conditions in the AND chain passed
        Ok(true)
    }

    // ============================================================================
    // BASIC CONDITION HANDLERS
    // ============================================================================

    /// C++ Reference: ScriptEngine::evaluateCounter() line 6319-6332
    pub(crate) fn eval_counter(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let counter_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_value = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating counter '{}' {:?} {}",
            counter_name,
            comparison,
            target_value
        );

        // Get counter value from script engine
        let counter_value = with_script_engine_ref(|engine| {
            engine
                .get_counter(&counter_name)
                .map(|counter| counter.value)
                .unwrap_or(0)
        })
        .unwrap_or(0);

        // Perform comparison matching C++ ScriptEngine::evaluateCounter()
        let result = match comparison {
            ComparisonType::LessThan => counter_value < target_value,
            ComparisonType::LessEqual => counter_value <= target_value,
            ComparisonType::Equal => counter_value == target_value,
            ComparisonType::GreaterEqual => counter_value >= target_value,
            ComparisonType::Greater => counter_value > target_value,
            ComparisonType::NotEqual => counter_value != target_value,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptEngine::evaluateFlag() line 6442-6460
    pub(crate) fn eval_flag(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let flag_name = self.get_condition_string_param(condition, 0)?;
        let expected = self.get_condition_bool_param(condition, 1)?;
        log::debug!("Evaluating flag '{}' == {}", flag_name, expected);

        // Re-entrant: nested under CALL_SUBROUTINE may hold the engine write lock.
        // C++: stored flag == expected, else any matching `m_uiInteractions` name
        // is true for this frame (cleared at the end of ScriptEngine::update).
        let matched = with_script_engine_ref(|engine| {
            let flag_value = engine
                .get_flag(&flag_name)
                .map(|f| f.value)
                .unwrap_or(false);
            flag_value == expected || engine.has_ui_interaction(&flag_name)
        })
        .unwrap_or(false);

        Ok(if matched {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    /// C++ Reference: ScriptEngine::evaluateTimerExpired() line 6700-6710
    /// Timers are counters with is_countdown_timer=true. Expired when value <= 0.
    pub(crate) fn eval_timer_expired(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let timer_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if timer '{}' expired", timer_name);

        // Re-entrant: nested under CALL_SUBROUTINE may hold the engine write lock.
        // Timers are counters with is_countdown_timer; expired when value <= 0.
        let is_expired = with_script_engine_ref(|engine| {
            engine
                .get_counter(&timer_name)
                .map(|counter| counter.is_countdown_timer && counter.value <= 0)
                .unwrap_or(false)
        })
        .unwrap_or(false);

        Ok(if is_expired {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // PLAYER CONDITION HANDLERS
    // ============================================================================

    pub(crate) fn eval_player_all_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if all of player '{}' destroyed", player_name);

        // Parser compatibility: `PLAYER_ALIVE player TRUE/FALSE` is mapped onto this condition with
        // an optional second boolean that inverts the meaning (TRUE => player alive).
        let wants_alive = condition.get_parameter(1).map(|p| p.get_int() != 0);

        let all_destroyed =
            if let Some(census) = crate::scripting::host_query_player_census(&player_name) {
                // Live host: leftover OBJECT_REGISTRY is empty so Player::hasAnyObjects is false.
                !census.has_any_objects
            } else if let Ok(players) = player_list().read() {
                if let Some(player_arc) = players.find_player_by_name(&player_name) {
                    if let Ok(player) = player_arc.read() {
                        // C++: player is all destroyed if Player::hasAnyObjects() is false.
                        !player.has_any_objects()
                    } else {
                        true
                    }
                } else {
                    true
                }
            } else {
                true
            };
        let result = match wants_alive {
            Some(true) => !all_destroyed,
            Some(false) => all_destroyed,
            None => all_destroyed,
        };
        Ok(bool_result(result))
    }

    pub(crate) fn eval_player_all_buildfacilities_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!(
            "Evaluating if all build facilities of player '{}' destroyed",
            player_name
        );

        if let Some(census) = crate::scripting::host_query_player_census(&player_name) {
            return Ok(bool_result(!census.has_any_build_facility));
        }

        // Look up the player and check if they have any build facilities
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    // All build facilities are destroyed if player has none
                    return Ok(bool_result(!player.has_any_build_facility()));
                }
            }
        }
        // Player not found - consider build facilities as destroyed
        Ok(ScriptConditionResult::True)
    }

    pub(crate) fn eval_player_has_credits(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ ScriptConditions::evaluatePlayerHasCredits (ScriptConditions.cpp:952-972)
        // Template [INT, COMPARISON, SIDE]; compare credits param to countMoney().
        let target_credits = self.get_condition_int_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let player_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating if {} {:?} player '{}' credits",
            target_credits,
            comparison,
            player_name
        );

        let current_credits =
            if let Some(census) = crate::scripting::host_query_player_census(&player_name) {
                census.money
            } else {
                // C++ returns false when playerFromParam cannot resolve the Side.
                let Ok(players) = player_list().read() else {
                    return Ok(ScriptConditionResult::False);
                };
                let Some(player_arc) = players.find_player_by_name(&player_name) else {
                    return Ok(ScriptConditionResult::False);
                };
                let Ok(player) = player_arc.read() else {
                    return Ok(ScriptConditionResult::False);
                };
                player.get_money().get_money()
            };

        let result = match comparison {
            ComparisonType::LessThan => target_credits < current_credits,
            ComparisonType::LessEqual => target_credits <= current_credits,
            ComparisonType::Equal => target_credits == current_credits,
            ComparisonType::GreaterEqual => target_credits >= current_credits,
            ComparisonType::Greater => target_credits > current_credits,
            ComparisonType::NotEqual => target_credits != current_credits,
        };

        Ok(bool_result(result))
    }

    pub(crate) fn eval_player_has_n_or_fewer_buildings(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let count = self.get_condition_int_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' has {} or fewer buildings",
            player_name,
            count
        );

        // C++ parity: ScriptConditions::evaluatePlayerHasNOrFewerBuildings
        let building_count =
            if let Some(census) = crate::scripting::host_query_player_census(&player_name) {
                census.building_count
            } else {
                let Ok(players) = player_list().read() else {
                    return Ok(ScriptConditionResult::False);
                };
                let Some(player_arc) = players.find_player_by_name(&player_name) else {
                    return Ok(ScriptConditionResult::False);
                };
                let Ok(player) = player_arc.read() else {
                    return Ok(ScriptConditionResult::False);
                };
                player.count_buildings()
            };

        Ok(bool_result(count >= building_count))
    }

    pub(crate) fn eval_player_has_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if player '{}' has power", player_name);

        if let Some(census) = crate::scripting::host_query_player_census(&player_name) {
            return Ok(bool_result(census.has_sufficient_power()));
        }

        // Look up the player and check their power status
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    // C++ parity: Energy::hasSufficientPower
                    return Ok(bool_result(player.get_energy().has_sufficient_power()));
                }
            }
        }
        // If player doesn't exist, default to no power
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_player_has_no_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if player '{}' has no power", player_name);

        // Invert the has_power check
        match self.eval_player_has_power(condition)? {
            ScriptConditionResult::True => Ok(ScriptConditionResult::False),
            ScriptConditionResult::False => Ok(ScriptConditionResult::True),
            ScriptConditionResult::Error(e) => Ok(ScriptConditionResult::Error(e)),
        }
    }

    pub(crate) fn eval_player_has_n_or_fewer_faction_buildings(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let count = self.get_condition_int_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' has {} or fewer faction buildings",
            player_name,
            count
        );

        // C++ parity: ScriptConditions::evaluatePlayerHasNOrFewerFactionBuildings
        // Uses KINDOF_MP_COUNT_FOR_VICTORY + KINDOF_STRUCTURE.
        let building_count =
            if let Some(census) = crate::scripting::host_query_player_census(&player_name) {
                census.faction_building_count
            } else {
                let Ok(players) = player_list().read() else {
                    return Ok(ScriptConditionResult::False);
                };
                let Some(player_arc) = players.find_player_by_name(&player_name) else {
                    return Ok(ScriptConditionResult::False);
                };
                let Ok(player) = player_arc.read() else {
                    return Ok(ScriptConditionResult::False);
                };
                let mask = crate::common::KindOf::Structure.cpp_mask()
                    | crate::common::KindOf::CountsForVictory.cpp_mask();
                player.count_objects_by_kindof(mask, crate::common::KIND_OF_MASK_NONE)
            };

        Ok(bool_result(count >= building_count))
    }

    pub(crate) fn eval_player_power_compare_percent(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let percent = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating player '{}' power percent {:?} {}",
            player_name,
            comparison,
            percent
        );

        let power_ratio =
            if let Some(census) = crate::scripting::host_query_player_census(&player_name) {
                census.supply_ratio()
            } else {
                let Ok(players) = player_list().read() else {
                    return Ok(ScriptConditionResult::False);
                };
                let Some(player_arc) = players.find_player_by_name(&player_name) else {
                    return Ok(ScriptConditionResult::False);
                };
                let Ok(player) = player_arc.read() else {
                    return Ok(ScriptConditionResult::False);
                };
                player.get_energy().supply_ratio()
            };
        let test_ratio = percent as f32 / 100.0;
        Ok(bool_result(Self::compare_f32(
            comparison,
            power_ratio,
            test_ratio,
        )))
    }

    pub(crate) fn eval_player_excess_power_compare_value(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let desired_excess = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating player '{}' excess power {:?} {}",
            player_name,
            comparison,
            desired_excess
        );

        let actual_excess =
            if let Some(census) = crate::scripting::host_query_player_census(&player_name) {
                census.excess_power()
            } else {
                let Ok(players) = player_list().read() else {
                    return Ok(ScriptConditionResult::False);
                };
                let Some(player_arc) = players.find_player_by_name(&player_name) else {
                    return Ok(ScriptConditionResult::False);
                };
                let Ok(player) = player_arc.read() else {
                    return Ok(ScriptConditionResult::False);
                };
                let energy = player.get_energy();
                energy.production() - energy.consumption()
            };
        Ok(bool_result(Self::compare_i32(
            comparison,
            actual_excess,
            desired_excess,
        )))
    }

    /// C++ Reference: ScriptConditions::evaluateScienceAcquired() line 1543-1553
    pub(crate) fn eval_player_acquired_science(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let science_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' acquired science '{}'",
            player_name,
            science_name
        );

        let science = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            SCIENCE_INVALID
        };
        if science == SCIENCE_INVALID {
            log::warn!("Science '{}' not found in store", science_name);
            return Ok(ScriptConditionResult::False);
        }

        let player_index = if let Ok(players) = player_list().read() {
            players
                .find_player_by_name(&player_name)
                .and_then(|arc| arc.read().ok().map(|p| p.get_player_index() as usize))
        } else {
            None
        };

        if let Some(player_index) = player_index {
            let acquired = with_script_engine_mut(|engine| {
                engine.is_science_acquired(player_index, science, true)
            })
            .unwrap_or(false);
            if acquired {
                return Ok(ScriptConditionResult::True);
            }
        }

        // Live host rank-up leftover-notifies via addScience. If leftover
        // PlayerList missed the notify, consume a one-shot host census edge.
        if crate::scripting::host_query_player_has_science(&player_name, &science_name)
            .unwrap_or(false)
        {
            if let Some(idx) = player_index {
                let already = with_script_engine_mut(|engine| {
                    engine.is_science_acquired(idx, science, false)
                })
                .unwrap_or(false);
                if !already {
                    let _ = with_script_engine_mut(|engine| {
                        engine.notify_of_acquired_science(idx, science);
                        engine.is_science_acquired(idx, science, true)
                    });
                    return Ok(ScriptConditionResult::True);
                }
            } else {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_player_has_science_purchase_points(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        // C++ PLAYER_HAS_SCIENCEPURCHASEPOINTS is [SIDE, INT] leftover points >= N.
        // Accept a leftover 3-param COMPARISON form if authored that way.
        let (target_points, comparison) = if condition.get_parameter(2).is_some() {
            (
                self.get_condition_int_param(condition, 2)?,
                Some(self.get_condition_comparison_param(condition, 1)?),
            )
        } else {
            (self.get_condition_int_param(condition, 1)?, None)
        };
        log::debug!(
            "Evaluating if player '{}' science points {:?} {}",
            player_name,
            comparison,
            target_points
        );

        let current_points =
            crate::scripting::host_query_player_science_purchase_points(&player_name)
                .or_else(|| {
                    let players = player_list().read().ok()?;
                    let player_arc = players.find_player_by_name(&player_name)?;
                    player_arc
                        .read()
                        .ok()
                        .map(|p| p.get_science_purchase_points())
                })
                .unwrap_or(0);

        let result = match comparison {
            Some(ComparisonType::LessThan) => current_points < target_points,
            Some(ComparisonType::LessEqual) => current_points <= target_points,
            Some(ComparisonType::Equal) => current_points == target_points,
            Some(ComparisonType::GreaterEqual) => current_points >= target_points,
            Some(ComparisonType::Greater) => current_points > target_points,
            Some(ComparisonType::NotEqual) => current_points != target_points,
            None => current_points >= target_points,
        };

        Ok(bool_result(result))
    }

    /// C++ Reference: ScriptConditions::evaluateCanPurchaseScience() line 1559-1568
    pub(crate) fn eval_player_can_purchase_science(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let science_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' can purchase science '{}'",
            player_name,
            science_name
        );

        let science = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            SCIENCE_INVALID
        };

        if science == SCIENCE_INVALID {
            log::warn!("Science '{}' not found in store", science_name);
            return Ok(ScriptConditionResult::False);
        }

        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    if player.is_capable_of_purchasing_science(science) {
                        return Ok(ScriptConditionResult::True);
                    }
                }
            }
        }

        // Live host leftover Player SPP/sciences are stale until rank-up sync.
        if let Some(census) = crate::scripting::host_query_player_census(&player_name) {
            if crate::scripting::host_query_player_has_science(&player_name, &science_name)
                .unwrap_or(false)
            {
                return Ok(ScriptConditionResult::False);
            }
            if let Some(store) = get_science_store() {
                let cost = store.get_science_purchase_cost(science);
                if cost > 0 && cost <= census.science_purchase_points {
                    let owned: Vec<ScienceType> = census
                        .unlocked_sciences
                        .iter()
                        .map(|n| store.get_science_from_internal_name(n))
                        .filter(|s| *s != SCIENCE_INVALID)
                        .collect();
                    struct Access(Vec<ScienceType>);
                    impl game_engine::common::rts::science::ScienceAccess for Access {
                        fn has_science(&self, s: ScienceType) -> bool {
                            self.0.contains(&s)
                        }
                    }
                    if store.player_has_prereqs_for_science(&Access(owned), science) {
                        return Ok(ScriptConditionResult::True);
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluatePlayerLostObjectType() line 2671-2698
    pub(crate) fn eval_player_lost_object_type(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let object_type = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if player '{}' lost object type '{}'",
            player_name,
            object_type
        );

        let leftover_player = player_list().read().ok().and_then(|players| {
            players
                .find_player_by_name(&player_name)
                .and_then(|arc| arc.read().ok().map(|p| p.get_player_index()))
        });

        if let Some(sum_of_objs) =
            self.host_player_object_type_count(&player_name, &object_type, true)
        {
            let player_index = leftover_player.unwrap_or(0);
            return Ok(self.finish_player_lost_object_type(
                player_index,
                &object_type,
                sum_of_objs,
            ));
        }

        let Some(player_index) = leftover_player else {
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

        let types = self.resolve_object_types_param(&object_type);
        let (templates, mut counts) = types.prep_for_player_counting();
        if templates.is_empty() {
            return Ok(ScriptConditionResult::False);
        }
        // C++ countObjectsByThingTemplate(..., ignoreDead=TRUE)
        player.count_objects_by_thing_template(&templates, true, true, &mut counts);
        let sum_of_objs: i32 = counts.iter().copied().sum();
        Ok(self.finish_player_lost_object_type(player_index, &object_type, sum_of_objs))
    }

    fn finish_player_lost_object_type(
        &self,
        player_index: i32,
        object_type: &str,
        sum_of_objs: i32,
    ) -> ScriptConditionResult {
        let current_count =
            with_script_engine_ref(|engine| engine.get_object_count(player_index, object_type))
                .unwrap_or(0);

        if sum_of_objs != current_count {
            let _ = with_script_engine_mut(|engine| {
                engine.set_object_count(player_index, object_type, sum_of_objs);
            });
        }

        bool_result(sum_of_objs < current_count)
    }

    /// C++ Reference: ScriptConditions::evaluatePlayerDestroyedNOrMoreBuildings()
    pub(crate) fn eval_player_destroyed_n_buildings_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let _target_count = self.get_condition_int_param(condition, 1)?;
        let opponent_name = self.get_condition_string_param(condition, 2)?;
        log::debug!(
            "Evaluating unimplemented C++ destroyed-buildings condition for '{}' against '{}'",
            player_name,
            opponent_name
        );

        // C++ resolves both players, ignores N, then returns FALSE because this helper
        // still contains only `@todo CLH implement me!`.
        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(opponent_arc) = players.find_player_by_name(&opponent_name) else {
            return Ok(ScriptConditionResult::False);
        };
        if player_arc.read().is_err() || opponent_arc.read().is_err() {
            return Ok(ScriptConditionResult::False);
        }

        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluatePlayerHasObjectComparison()
    /// Check if player has N objects of a specific type
    pub(crate) fn eval_player_has_object_comparison(
        &self,
        condition: &mut Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ parameter order: [player, comparison, count, type_or_list]
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_count = self.get_condition_int_param(condition, 2)?;
        let object_type = self.get_condition_string_param(condition, 3)?;
        log::debug!(
            "Evaluating player '{}' has {:?} {} of type '{}'",
            player_name,
            comparison,
            target_count,
            object_type
        );

        if condition.custom_data != 0 {
            if with_script_engine_ref(|engine| {
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
        }

        let object_count = if let Some(count) =
            self.host_player_object_type_count(&player_name, &object_type, false)
        {
            count
        } else {
            let Ok(players) = player_list().read() else {
                return Ok(ScriptConditionResult::False);
            };
            let Some(player_arc) = players.find_player_by_name(&player_name) else {
                return Ok(ScriptConditionResult::False);
            };
            let Ok(player) = player_arc.read() else {
                return Ok(ScriptConditionResult::False);
            };

            let types = self.resolve_object_types_param(&object_type);
            let (templates, mut counts) = types.prep_for_player_counting();
            // C++ countObjectsByThingTemplate(..., ignoreDead=FALSE)
            if !templates.is_empty() {
                player.count_objects_by_thing_template(&templates, false, true, &mut counts);
            }
            counts.iter().copied().sum()
        };

        let result = match comparison {
            ComparisonType::LessThan => object_count < target_count,
            ComparisonType::LessEqual => object_count <= target_count,
            ComparisonType::GreaterEqual => object_count >= target_count,
            ComparisonType::Greater => object_count > target_count,
            ComparisonType::Equal => object_count == target_count,
            ComparisonType::NotEqual => object_count != target_count,
        };

        condition.custom_data = if result { 1 } else { -1 };
        if let Some(frame) =
            with_script_engine_ref(|engine| engine.get_frame_object_count_changed())
        {
            condition.custom_frame = frame;
        }

        Ok(bool_result(result))
    }

    // ============================================================================
    // TEAM CONDITION HANDLERS
    // ============================================================================

    /// C++ ScriptConditions::evaluateTeamInsideAreaPartially (line 378-392)
    pub(crate) fn eval_team_inside_area_partially(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let counts = self.team_inside_consideration(condition)?;
        Ok(bool_result(counts.any_considered && counts.any_inside))
    }

    pub(crate) fn eval_team_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if team '{}' is destroyed", team_name);

        if crate::object::registry::OBJECT_REGISTRY.is_empty()
            && crate::scripting::host_script_query_has_any()
        {
            if !crate::scripting::host_team_was_fielded(&team_name) {
                return Ok(ScriptConditionResult::False);
            }
            return Ok(bool_result(
                !crate::scripting::host_team_has_any_live_objects(&team_name),
            ));
        }

        // C++: non-existent team is not destroyed; existing team uses Team::hasAnyObjects().
        if let Some(team_arc) = self.lookup_condition_team(&team_name) {
            if let Ok(team) = team_arc.read() {
                return Ok(bool_result(!team.has_any_objects()));
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_has_units(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if team '{}' has units", team_name);

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(bool_result(crate::scripting::host_team_has_any_live_units(
                &team_name,
            )));
        }

        // C++ evaluateHasUnits iterates every prototype instance for non-THIS names.
        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.find_team_instances(&team_name) {
                if let Ok(team) = team_arc.read() {
                    if team.has_any_units() {
                        return Ok(ScriptConditionResult::True);
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_state_is(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let expected_state = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' state is '{}'",
            team_name,
            expected_state
        );

        if let Some(team_arc) = self.lookup_condition_team(&team_name) {
            if let Ok(team) = team_arc.read() {
                return Ok(bool_result(team.get_state().as_str() == expected_state));
            }
        }
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluateTeamStateIsNot() line 608-620
    pub(crate) fn eval_team_state_is_not(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let expected_state = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' state is not '{}'",
            team_name,
            expected_state
        );

        if let Some(team_arc) = self.lookup_condition_team(&team_name) {
            if let Ok(team) = team_arc.read() {
                return Ok(bool_result(team.get_state().as_str() != expected_state));
            }
        }
        Ok(ScriptConditionResult::False)
    }

    /// C++ ScriptConditions::evaluateTeamInsideAreaEntirely (line 632-649)
    pub(crate) fn eval_team_inside_area_entirely(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let counts = self.team_inside_consideration(condition)?;
        Ok(bool_result(counts.any_considered && !counts.any_outside))
    }

    /// C++ ScriptConditions::evaluateTeamOutsideAreaEntirely (line 652-658)
    pub(crate) fn eval_team_outside_area_entirely(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let entirely_inside = self.eval_team_inside_area_entirely(condition)?;
        let partially_inside = self.eval_team_inside_area_partially(condition)?;
        let any_inside = matches!(entirely_inside, ScriptConditionResult::True)
            || matches!(partially_inside, ScriptConditionResult::True);
        Ok(bool_result(!any_inside))
    }

    pub(crate) fn eval_team_attacked_by_object_type(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ ScriptConditions::evaluateTeamAttackedByType + objectTypesFromParam
        let team_name = self.get_condition_string_param(condition, 0)?;
        let types_param = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' attacked by object type '{}'",
            team_name,
            types_param
        );

        let types = self.resolve_object_types_param(&types_param);
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(bool_result(host_team_attacked_by_object_types(
                &team_name, &types,
            )));
        }

        let Some(team_arc) = self.lookup_condition_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            if last_damage_matches_object_types(member_id, &types) {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_attacked_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' attacked by player '{}'",
            team_name,
            player_name
        );

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(bool_result(host_team_attacked_by_player(
                &team_name,
                &player_name,
            )));
        }

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

        let Some(team_arc) = self.lookup_condition_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            let Some(body) = obj.get_body_module() else {
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
            let Ok(attacker) = attacker_arc.read() else {
                continue;
            };
            let Some(attacker_owner) = attacker.get_controlling_player_id() else {
                continue;
            };

            if attacker_owner as i32 == victim_index {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_created(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if team '{}' was created", team_name);

        if crate::object::registry::OBJECT_REGISTRY.is_empty()
            && crate::scripting::host_script_query_has_any()
        {
            return Ok(bool_result(crate::scripting::host_team_was_fielded(
                &team_name,
            )));
        }

        if let Some(team_arc) = self.lookup_condition_team(&team_name) {
            if let Ok(team) = team_arc.read() {
                return Ok(bool_result(team.is_created()));
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_discovered(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++ ScriptConditions::evaluateTeamDiscovered — CLEAR | PARTIAL_CLEAR only
        let team_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' was discovered by player '{}'",
            team_name,
            player_name
        );

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(bool_result(host_team_discovered_by_player(
                &team_name,
                &player_name,
            )));
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
        let player_index = player.get_player_index();

        let Some(team_arc) = self.lookup_condition_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if object_is_discovered_by_player(&obj, player_index) {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_owned_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' is owned by player '{}'",
            team_name,
            player_name
        );

        // Get the team and its controlling player ID
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    if let Some(controlling_player_id) = team.get_controlling_player_id() {
                        // Find the player and compare names
                        if let Ok(players) = player_list().read() {
                            if let Some(player_arc) =
                                players.get_player(controlling_player_id as i32)
                            {
                                if let Ok(player) = player_arc.read() {
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
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_reached_waypoints_end(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let waypoint_path = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' reached waypoints end for path '{}'",
            team_name,
            waypoint_path
        );

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            let any = crate::scripting::host_script_team_member_ids(&team_name)
                .into_iter()
                .filter_map(crate::scripting::host_script_query_object_by_id)
                .any(|obj| {
                    obj.waypoint_labels
                        .iter()
                        .any(|label| label == &waypoint_path)
                });
            return Ok(bool_result(any));
        }

        // C++ parity: ScriptConditions::evaluateTeamReachedWaypointsEnd
        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let Ok(terrain) = crate::terrain::get_terrain_logic().read() else {
            return Ok(ScriptConditionResult::False);
        };

        let mut any_at_end = false;
        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            let Some(ai_arc) = obj.get_ai_update_interface() else {
                // C++: no AI -> continue (e.g. rocks/trees in team)
                continue;
            };
            let Ok(ai) = ai_arc.lock() else {
                continue;
            };
            let Some(completed_waypoint_id) = ai.get_completed_waypoint_id() else {
                continue;
            };
            let Some(target_waypoint) = terrain.get_waypoint_by_id(completed_waypoint_id) else {
                continue;
            };

            let found = target_waypoint.get_path_label1().as_str() == waypoint_path
                || target_waypoint.get_path_label2().as_str() == waypoint_path
                || target_waypoint.get_path_label3().as_str() == waypoint_path;
            if found {
                any_at_end = true;
            }
        }

        Ok(if any_at_end {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_team_entered_area_entirely(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        let which_to_consider = self.get_condition_int_param(condition, 2).unwrap_or(0) as u32;
        let Ok(trigger) = self.get_trigger_area(&area_name) else {
            return Ok(ScriptConditionResult::False);
        };
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(bool_result(
                crate::scripting::conditions::host_team_did_all_enter(
                    &team_name,
                    &trigger,
                    which_to_consider,
                ),
            ));
        }
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    return Ok(if team.did_all_enter(&trigger, which_to_consider) {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_entered_area_partially(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        let which_to_consider = self.get_condition_int_param(condition, 2).unwrap_or(0) as u32;
        let Ok(trigger) = self.get_trigger_area(&area_name) else {
            return Ok(ScriptConditionResult::False);
        };
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(bool_result(
                crate::scripting::conditions::host_team_did_partial_enter(
                    &team_name,
                    &trigger,
                    which_to_consider,
                ),
            ));
        }
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    return Ok(if team.did_partial_enter(&trigger, which_to_consider) {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_exited_area_entirely(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        let which_to_consider = self.get_condition_int_param(condition, 2).unwrap_or(0) as u32;
        let Ok(trigger) = self.get_trigger_area(&area_name) else {
            return Ok(ScriptConditionResult::False);
        };
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(bool_result(
                crate::scripting::conditions::host_team_did_all_exit(
                    &team_name,
                    &trigger,
                    which_to_consider,
                ),
            ));
        }
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    return Ok(if team.did_all_exit(&trigger, which_to_consider) {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_exited_area_partially(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        let which_to_consider = self.get_condition_int_param(condition, 2).unwrap_or(0) as u32;
        let Ok(trigger) = self.get_trigger_area(&area_name) else {
            return Ok(ScriptConditionResult::False);
        };
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(bool_result(
                crate::scripting::conditions::host_team_did_partial_exit(
                    &team_name,
                    &trigger,
                    which_to_consider,
                ),
            ));
        }
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    return Ok(if team.did_partial_exit(&trigger, which_to_consider) {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_completed_sequential_execution(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        log::debug!(
            "Evaluating if team '{}' completed sequential execution",
            team_name
        );
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_all_has_object_status(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let status_mask = condition
            .get_parameter(1)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 1 not found".to_string()))?
            .get_object_status();
        log::debug!(
            "Evaluating if all of team '{}' has object status",
            team_name
        );

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(bool_result(
                crate::scripting::host_eval_team_has_object_status(
                    &team_name,
                    status_mask.bits(),
                    true,
                )
                .unwrap_or(false),
            ));
        }

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                return Ok(ScriptConditionResult::False);
            };
            let Ok(obj) = obj_arc.read() else {
                return Ok(ScriptConditionResult::False);
            };
            if !obj.get_status_bits().intersects(status_mask) {
                return Ok(ScriptConditionResult::False);
            }
        }

        Ok(ScriptConditionResult::True)
    }

    pub(crate) fn eval_team_some_have_object_status(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let status_mask = condition
            .get_parameter(1)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 1 not found".to_string()))?
            .get_object_status();
        log::debug!(
            "Evaluating if some of team '{}' have object status",
            team_name
        );

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(bool_result(
                crate::scripting::host_eval_team_has_object_status(
                    &team_name,
                    status_mask.bits(),
                    false,
                )
                .unwrap_or(false),
            ));
        }

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                return Ok(ScriptConditionResult::False);
            };
            let Ok(obj) = obj_arc.read() else {
                return Ok(ScriptConditionResult::False);
            };
            if obj.get_status_bits().intersects(status_mask) {
                return Ok(ScriptConditionResult::True);
            }
        }

        Ok(ScriptConditionResult::False)
    }

    // ============================================================================
    // PARAMETER HELPERS
    // ============================================================================

    pub(crate) fn resolve_object_types_param(&self, type_or_list_name: &str) -> ObjectTypes {
        let mut types = ObjectTypes::new();
        if type_or_list_name.is_empty() {
            return types;
        }

        if let Some(Some(found)) =
            with_script_engine_ref(|engine| engine.get_object_types(type_or_list_name))
        {
            return found;
        }

        types.add_object_type(AsciiString::from(type_or_list_name));
        types
    }

    /// C++ Player::countObjectsByThingTemplate via host census when leftover
    /// OBJECT_REGISTRY is empty. None when no census row exists for the side.
    pub(crate) fn host_player_object_type_count(
        &self,
        player_name: &str,
        type_or_list: &str,
        ignore_dead: bool,
    ) -> Option<i32> {
        let census = crate::scripting::host_query_player_census(player_name)?;
        let types = self.resolve_object_types_param(type_or_list);
        let (templates, _) = types.prep_for_player_counting();
        let mut names: Vec<String> = templates
            .iter()
            .map(|template| template.get_name().to_string())
            .collect();
        if names.is_empty() {
            names.extend(types.iter().map(|name| name.to_string()));
        }
        if names.is_empty() && !type_or_list.is_empty() {
            names.push(type_or_list.to_string());
        }
        Some(census.count_templates(&names, ignore_dead))
    }

    pub(crate) fn get_condition_string_param(
        &self,
        condition: &Condition,
        index: usize,
    ) -> Result<String, ScriptError> {
        condition
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| self.resolve_string_token(p.get_string()))
    }

    pub(crate) fn get_condition_int_param(
        &self,
        condition: &Condition,
        index: usize,
    ) -> Result<i32, ScriptError> {
        condition
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_int())
    }

    pub(crate) fn get_condition_real_param(
        &self,
        condition: &Condition,
        index: usize,
    ) -> Result<f32, ScriptError> {
        condition
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_real())
    }

    pub(crate) fn get_condition_bool_param(
        &self,
        condition: &Condition,
        index: usize,
    ) -> Result<bool, ScriptError> {
        condition
            .get_parameter(index)
            .ok_or_else(|| ScriptError::ParameterNotFound(format!("Parameter {} not found", index)))
            .map(|p| p.get_int() != 0)
    }

    pub(crate) fn get_condition_comparison_param(
        &self,
        condition: &Condition,
        index: usize,
    ) -> Result<ComparisonType, ScriptError> {
        let value = condition
            .get_parameter(index)
            .ok_or_else(|| {
                ScriptError::ParameterNotFound(format!("Parameter {} not found", index))
            })?
            .get_int();
        match value {
            0 => Ok(ComparisonType::LessThan),
            1 => Ok(ComparisonType::LessEqual),
            2 => Ok(ComparisonType::Equal),
            3 => Ok(ComparisonType::GreaterEqual),
            4 => Ok(ComparisonType::Greater),
            5 => Ok(ComparisonType::NotEqual),
            _ => Ok(ComparisonType::Equal),
        }
    }

    pub(crate) fn compare_i32(comparison: ComparisonType, lhs: i32, rhs: i32) -> bool {
        match comparison {
            ComparisonType::LessThan => lhs < rhs,
            ComparisonType::LessEqual => lhs <= rhs,
            ComparisonType::Equal => lhs == rhs,
            ComparisonType::GreaterEqual => lhs >= rhs,
            ComparisonType::Greater => lhs > rhs,
            ComparisonType::NotEqual => lhs != rhs,
        }
    }

    pub(crate) fn compare_f32(comparison: ComparisonType, lhs: f32, rhs: f32) -> bool {
        match comparison {
            ComparisonType::LessThan => lhs < rhs,
            ComparisonType::LessEqual => lhs <= rhs,
            ComparisonType::Equal => lhs == rhs,
            ComparisonType::GreaterEqual => lhs >= rhs,
            ComparisonType::Greater => lhs > rhs,
            ComparisonType::NotEqual => lhs != rhs,
        }
    }

    /// C++ Team::allInside / someInsideSomeOutside (Team.cpp:2079-2203)
    fn team_inside_consideration(
        &self,
        condition: &Condition,
    ) -> Result<TeamInsideCounts, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        let which_to_consider = self.get_condition_int_param(condition, 2).unwrap_or(1) as u32;

        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            let Ok(trigger) = self.get_trigger_area(&area_name) else {
                return Ok(TeamInsideCounts::default());
            };
            let all_inside = crate::scripting::conditions::host_team_all_inside(
                &team_name,
                &trigger,
                which_to_consider,
            );
            let mixed = crate::scripting::conditions::host_team_some_inside_some_outside(
                &team_name,
                &trigger,
                which_to_consider,
            );
            if !all_inside && !mixed {
                return Ok(TeamInsideCounts::default());
            }
            return Ok(TeamInsideCounts {
                any_considered: true,
                any_inside: all_inside || mixed,
                any_outside: mixed || !all_inside,
            });
        }

        let Some(team_arc) = self.lookup_condition_team(&team_name) else {
            return Ok(TeamInsideCounts::default());
        };
        let Ok(team) = team_arc.read() else {
            return Ok(TeamInsideCounts::default());
        };
        if !team.has_any_objects() {
            return Ok(TeamInsideCounts::default());
        }

        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .unwrap_or_default();

        let mut counts = TeamInsideCounts::default();
        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            if !member_counts_for_team_area(&obj, which_to_consider) {
                continue;
            }
            counts.any_considered = true;
            if objects_in_area.contains(&member_id) {
                counts.any_inside = true;
            } else {
                counts.any_outside = true;
            }
        }
        Ok(counts)
    }

    pub(crate) fn bool_result(value: bool) -> ScriptConditionResult {
        bool_result(value)
    }

    pub(crate) fn last_damage_matches_object_types(
        &self,
        member_id: crate::common::ObjectID,
        types: &crate::object::object_types::ObjectTypes,
    ) -> bool {
        last_damage_matches_object_types(member_id, types)
    }

    pub(crate) fn object_is_discovered_by_player(
        &self,
        obj: &crate::object::Object,
        player_index: i32,
    ) -> bool {
        object_is_discovered_by_player(obj, player_index)
    }
}

#[derive(Default)]
struct TeamInsideCounts {
    any_considered: bool,
    any_inside: bool,
    any_outside: bool,
}

fn member_counts_for_team_area(obj: &crate::object::Object, which_to_consider: u32) -> bool {
    let surfaces = if let Some(ai) = obj.get_ai() {
        if let Ok(ai_guard) = ai.lock() {
            ai_guard
                .get_locomotor_set_clone()
                .map(|set| set.get_valid_surfaces())
                .unwrap_or(crate::path::SURFACE_GROUND)
        } else {
            crate::path::SURFACE_GROUND
        }
    } else {
        crate::path::SURFACE_GROUND
    };
    if !loco_set_matches(surfaces, which_to_consider) {
        return false;
    }
    if obj.is_effectively_dead() {
        return false;
    }
    if obj.is_kind_of(crate::common::KindOf::Inert) {
        return false;
    }
    true
}

fn last_damage_matches_object_types(
    member_id: crate::common::ObjectID,
    types: &crate::object::object_types::ObjectTypes,
) -> bool {
    let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
        return false;
    };
    let Ok(obj) = obj_arc.read() else {
        return false;
    };
    let Some(body) = obj.get_body_module() else {
        return false;
    };
    let Ok(body_guard) = body.lock() else {
        return false;
    };
    let Some(last) = body_guard.get_last_damage_info() else {
        return false;
    };
    if let Some(template) = last.input.source_template.as_deref() {
        return types.contains_template(Some(template));
    }
    let Some(attacker_arc) = TheGameLogic::find_object_by_id(last.input.source_id) else {
        return false;
    };
    let Ok(attacker) = attacker_arc.read() else {
        return false;
    };
    types.contains_template(Some(attacker.get_template().as_ref()))
}

fn object_is_discovered_by_player(obj: &crate::object::Object, player_index: i32) -> bool {
    if obj.is_disabled_by_type(crate::common::DisabledType::Held) {
        return false;
    }
    let status = obj.get_status_bits();
    if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
        && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
        && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
    {
        return false;
    }
    matches!(
        obj.get_shrouded_status(player_index),
        crate::common::ObjectShroudStatus::Clear | crate::common::ObjectShroudStatus::PartialClear
    )
}

/// C++ evaluateTeamAttackedByType over host team_instance_ids + last_damage_*.
fn host_team_attacked_by_object_types(
    team_name: &str,
    types: &crate::object::object_types::ObjectTypes,
) -> bool {
    for id in crate::scripting::host_script_team_member_ids(team_name) {
        let Some(obj) = crate::scripting::host_script_query_object_by_id(id) else {
            continue;
        };
        if !obj.last_damage_template.is_empty() {
            if types.is_in_set(&crate::common::AsciiString::from(
                obj.last_damage_template.as_str(),
            )) {
                return true;
            }
            continue;
        }
        if obj.last_damage_source_id == 0 {
            continue;
        }
        let Some(src) = crate::scripting::host_script_query_object_by_id(obj.last_damage_source_id)
        else {
            continue;
        };
        if types.is_in_set(&crate::common::AsciiString::from(
            src.template_name.as_str(),
        )) {
            return true;
        }
    }
    false
}

/// C++ evaluateTeamAttackedByPlayer over host last_damage_player (live attacker owner).
fn host_team_attacked_by_player(team_name: &str, player_name: &str) -> bool {
    crate::scripting::host_script_team_member_ids(team_name)
        .into_iter()
        .filter_map(crate::scripting::host_script_query_object_by_id)
        .any(|obj| {
            !obj.last_damage_player.is_empty()
                && obj.last_damage_player.eq_ignore_ascii_case(player_name)
        })
}

/// C++ evaluateTeamDiscovered over host discovered_by (CLEAR|PARTIAL_CLEAR census).
fn host_team_discovered_by_player(team_name: &str, player_name: &str) -> bool {
    crate::scripting::host_script_team_member_ids(team_name)
        .into_iter()
        .filter_map(crate::scripting::host_script_query_object_by_id)
        .any(|obj| {
            if obj.held || obj.stealthed_hidden {
                return false;
            }
            obj.discovered_by
                .iter()
                .any(|name| name.eq_ignore_ascii_case(player_name))
        })
}
