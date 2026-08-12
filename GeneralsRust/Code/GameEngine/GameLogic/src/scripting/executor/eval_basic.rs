//! Condition dispatch plus OR/AND, counter, flag, timer, player, and team evaluators
//!
//! Split from `scripting/executor.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::*;

impl ScriptConditionEvaluator {
    pub(crate) fn resolve_string_token(&self, raw: &str) -> String {
        match raw {
            THE_PLAYER | THIS_PLAYER => with_script_engine_ref(|engine| engine.get_current_player_name())
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
                let current_player =
                    with_script_engine_ref(|engine| engine.get_current_player_name()).flatten();
                let Some(player_name) = current_player else {
                    return raw.to_string();
                };

                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.find_player_by_name(&player_name))
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
        let team_name = self.resolve_string_token(team_name);
        let factory = get_team_factory();
        if let Ok(mut factory_guard) = factory.lock() {
            factory_guard
                .find_team(&team_name)
                .ok_or_else(|| ScriptError::TeamNotFound(team_name.to_string()))
        } else {
            Err(ScriptError::ExecutionFailed(
                "Failed to lock team factory".to_string(),
            ))
        }
    }

    pub(crate) fn get_trigger_area(
        &self,
        area_name: &str,
    ) -> Result<crate::polygon_trigger::PolygonTrigger, ScriptError> {
        if let Ok(terrain) = get_terrain_logic().read() {
            if let Some(trigger) = terrain.get_trigger_area_by_name(area_name) {
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
    pub(crate) fn evaluate_and_chain(&mut self, condition: &mut Condition) -> Result<bool, ScriptError> {
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
    pub(crate) fn eval_counter(&self, condition: &Condition) -> Result<ScriptConditionResult, ScriptError> {
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

    /// C++ Reference: ScriptEngine::evaluateFlag() line 6442-6450
    pub(crate) fn eval_flag(&self, condition: &Condition) -> Result<ScriptConditionResult, ScriptError> {
        let flag_name = self.get_condition_string_param(condition, 0)?;
        let expected = self.get_condition_bool_param(condition, 1)?;
        log::debug!("Evaluating flag '{}' == {}", flag_name, expected);

        // Re-entrant: nested under CALL_SUBROUTINE may hold the engine write lock.
        let flag_value = with_script_engine_ref(|engine| {
            engine
                .get_flag(&flag_name)
                .map(|f| f.value)
                .unwrap_or(false)
        })
        .unwrap_or(false);

        // Compare flag value with expected (C++ compares boolFlag == value)
        Ok(if flag_value == expected {
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

        // Look up the player and check if they have any units
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    // C++: player is all destroyed if Player::hasAnyObjects() is false.
                    let all_destroyed = !player.has_any_objects();
                    let result = match wants_alive {
                        Some(true) => !all_destroyed,
                        Some(false) => all_destroyed,
                        None => all_destroyed,
                    };
                    return Ok(if result {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // Player not found - consider as destroyed
        Ok(if wants_alive == Some(true) {
            ScriptConditionResult::False
        } else {
            ScriptConditionResult::True
        })
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

        // Look up the player and check if they have any build facilities
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    // All build facilities are destroyed if player has none
                    return Ok(if !player.has_any_build_facility() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
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
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_credits = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' credits {:?} {}",
            player_name,
            comparison,
            target_credits
        );

        // Look up the player and get their credits
        let current_credits = if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    player.get_money().get_money()
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        let result = match comparison {
            ComparisonType::LessThan => target_credits < current_credits,
            ComparisonType::LessEqual => target_credits <= current_credits,
            ComparisonType::Equal => target_credits == current_credits,
            ComparisonType::GreaterEqual => target_credits >= current_credits,
            ComparisonType::Greater => target_credits > current_credits,
            ComparisonType::NotEqual => target_credits != current_credits,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
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
        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let building_count = player.count_buildings();

        Ok(if count >= building_count {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_player_has_power(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if player '{}' has power", player_name);

        // Look up the player and check their power status
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    // C++ parity: Energy::hasSufficientPower
                    return Ok(if player.get_energy().has_sufficient_power() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
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
        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let mask = (1u64 << (crate::common::KindOf::Structure as u32))
            | (1u64 << (crate::common::KindOf::CountsForVictory as u32));
        let building_count = player.count_objects_by_kindof(mask, crate::common::KIND_OF_MASK_NONE);

        Ok(if count >= building_count {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
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

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let power_ratio = player.get_energy().supply_ratio();
        let test_ratio = percent as f32 / 100.0;
        Ok(if Self::compare_f32(comparison, power_ratio, test_ratio) {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
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
        let actual_excess = energy.production() - energy.consumption();
        Ok(
            if Self::compare_i32(comparison, actual_excess, desired_excess) {
                ScriptConditionResult::True
            } else {
                ScriptConditionResult::False
            },
        )
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

        let Ok(players) = player_list().read() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(player_arc) = players.find_player_by_name(&player_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let player_index = match player_arc.read() {
            Ok(player) => player.get_player_index() as usize,
            Err(_) => return Ok(ScriptConditionResult::False),
        };

        let acquired = with_script_engine_mut(|engine| {
            engine.is_science_acquired(player_index, science, true)
        })
        .unwrap_or(false);

        Ok(if acquired {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    pub(crate) fn eval_player_has_science_purchase_points(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let player_name = self.get_condition_string_param(condition, 0)?;
        let comparison = self.get_condition_comparison_param(condition, 1)?;
        let target_points = self.get_condition_int_param(condition, 2)?;
        log::debug!(
            "Evaluating if player '{}' science points {:?} {}",
            player_name,
            comparison,
            target_points
        );

        // Look up the player and get their science purchase points
        let current_points = if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    player.get_science_purchase_points()
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };

        let result = match comparison {
            ComparisonType::LessThan => current_points < target_points,
            ComparisonType::LessEqual => current_points <= target_points,
            ComparisonType::Equal => current_points == target_points,
            ComparisonType::GreaterEqual => current_points >= target_points,
            ComparisonType::Greater => current_points > target_points,
            ComparisonType::NotEqual => current_points != target_points,
        };

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
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

        // Look up science type from name using science store
        let science = if let Some(store) = get_science_store() {
            store.get_science_from_internal_name(&science_name)
        } else {
            SCIENCE_INVALID
        };

        if science == SCIENCE_INVALID {
            log::warn!("Science '{}' not found in store", science_name);
            return Ok(ScriptConditionResult::False);
        }

        // Look up the player and check if they can purchase the science
        if let Ok(players) = player_list().read() {
            if let Some(player_arc) = players.find_player_by_name(&player_name) {
                if let Ok(player) = player_arc.read() {
                    // Check prerequisites for this science
                    return Ok(if player.has_prereqs_for_science(science) {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluatePlayerLostObjectType() - requires event tracking
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

        let Some(player_index) = player_list()
            .read()
            .ok()
            .and_then(|players| players.find_player_by_name(&player_name))
            .and_then(|player_arc| {
                player_arc
                    .read()
                    .ok()
                    .map(|player| player.get_player_index())
            })
        else {
            return Ok(ScriptConditionResult::False);
        };

        let current_count = with_script_engine_ref(|engine| {
            engine.get_object_count(player_index, &object_type)
        })
        .unwrap_or(0);

        let object_manager = get_object_manager();
        let sum_of_objs = object_manager
            .read()
            .ok()
            .map(|manager| {
                manager
                    .all_object_ids()
                    .into_iter()
                    .filter(|object_id| {
                        let Some(obj_arc) = manager.get_object(*object_id) else {
                            return false;
                        };
                        let Ok(obj_guard) = obj_arc.read() else {
                            return false;
                        };
                        if obj_guard.is_destroyed() {
                            return false;
                        }
                        let owner = obj_guard
                            .base()
                            .read()
                            .ok()
                            .and_then(|base| base.get_controlling_player_id())
                            .map(|id| id as i32)
                            .unwrap_or(-1);
                        if owner != player_index {
                            return false;
                        }
                        obj_guard
                            .template
                            .as_ref()
                            .map(|template| template.get_name().as_str() == object_type.as_str())
                            .unwrap_or(false)
                    })
                    .count() as i32
            })
            .unwrap_or(0);

        if sum_of_objs != current_count {
            let _ = with_script_engine_mut(|engine| {
                engine.set_object_count(player_index, &object_type, sum_of_objs);
            });
        }

        Ok(if sum_of_objs < current_count {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
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
        let mut object_count = 0i32;
        for object_id in player.get_all_objects() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.is_effectively_dead() {
                continue;
            }
            if types.contains_template(Some(obj_guard.get_template().as_ref())) {
                object_count += 1;
            }
        }

        let result = match comparison {
            ComparisonType::LessThan => object_count < target_count,
            ComparisonType::LessEqual => object_count <= target_count,
            ComparisonType::Equal => object_count == target_count,
            ComparisonType::GreaterEqual => object_count >= target_count,
            ComparisonType::Greater => object_count > target_count,
            ComparisonType::NotEqual => object_count != target_count,
        };

        condition.custom_data = if result { 1 } else { -1 };
        if let Some(frame) = with_script_engine_ref(|engine| engine.get_frame_object_count_changed()) {
            condition.custom_frame = frame;
        }

        Ok(if result {
            ScriptConditionResult::True
        } else {
            ScriptConditionResult::False
        })
    }

    // ============================================================================
    // TEAM CONDITION HANDLERS
    // ============================================================================

    /// C++ Reference: ScriptConditions::evaluateTeamInsideAreaPartially() line 378-392
    /// C++ pattern: theTeam->someInsideSomeOutside(pTrig, type) || theTeam->allInside(pTrig, type)
    /// Returns true if ANY team member is inside the area
    pub(crate) fn eval_team_inside_area_partially(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' is partially inside area '{}'",
            team_name,
            area_name
        );

        // Get team members and check if ANY are inside the area
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if ANY team member is in the area
                        for &member_id in members {
                            if objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::True);
                            }
                        }
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_destroyed(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        log::debug!("Evaluating if team '{}' is destroyed", team_name);

        // C++: non-existent team is not destroyed; existing team uses Team::hasAnyObjects().
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    return Ok(if !team.has_any_objects() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
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

        // C++: non-THIS team names scan every instance of the prototype via
        // TeamPrototype::iterate_TeamInstanceList().
        if let Ok(factory) = get_team_factory().lock() {
            for team_arc in factory.find_team_instances(&team_name) {
                if let Ok(team) = team_arc.read() {
                    if team.has_any_units() {
                        return Ok(ScriptConditionResult::True);
                    }
                }
            }
        }
        // If team doesn't exist, it has no units
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

        // Look up the team and check its state
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let current_state = team.get_state();
                    return Ok(if current_state.as_str() == expected_state {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
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

        // Look up the team and check its state
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let current_state = team.get_state();
                    return Ok(if current_state.as_str() != expected_state {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        // C++: return false; // Non existent team isn't in any state.
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluateTeamInsideAreaEntirely() line 632-649
    /// C++ pattern: theTeam->allInside(pTrig, type)
    /// Returns true if ALL team members are inside the area
    pub(crate) fn eval_team_inside_area_entirely(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        let team_name = self.get_condition_string_param(condition, 0)?;
        let area_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' is entirely inside area '{}'",
            team_name,
            area_name
        );

        // Get team members and check if ALL are inside the area
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if ALL team members are in the area
                        for &member_id in members {
                            if !objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::False);
                            }
                        }
                        return Ok(ScriptConditionResult::True);
                    }
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    /// C++ Reference: ScriptConditions::evaluateTeamOutsideAreaEntirely() line 652-658
    /// C++ pattern: return !(evaluateTeamInsideAreaEntirely(...) || evaluateTeamInsideAreaPartially(...));
    pub(crate) fn eval_team_outside_area_entirely(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        log::debug!("Evaluating team outside area entirely (using C++ pattern)");

        // C++ pattern: return !(evaluateTeamInsideAreaEntirely(...) || evaluateTeamInsideAreaPartially(...));
        let entirely_inside = self.eval_team_inside_area_entirely(condition)?;
        let partially_inside = self.eval_team_inside_area_partially(condition)?;

        // If either entirely or partially inside, team is NOT entirely outside
        let any_inside = matches!(entirely_inside, ScriptConditionResult::True)
            || matches!(partially_inside, ScriptConditionResult::True);

        Ok(if any_inside {
            ScriptConditionResult::False
        } else {
            ScriptConditionResult::True
        })
    }

    pub(crate) fn eval_team_attacked_by_object_type(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateTeamAttackedByType
        let team_name = self.get_condition_string_param(condition, 0)?;
        let types_param = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' attacked by object type '{}'",
            team_name,
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

            if let Some(template) = &last.input.source_template {
                if wanted_types
                    .iter()
                    .any(|wanted| template.get_name().as_str() == *wanted)
                {
                    return Ok(ScriptConditionResult::True);
                }
            } else {
                // Old system: consult the attacker object template if the source template wasn't set.
                let attacker_id = last.input.source_id;
                let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                    // C++ explicitly continues here so other team members can still satisfy.
                    continue;
                };
                let Ok(attacker) = attacker_arc.read() else {
                    continue;
                };
                let attacker_template = attacker.get_template();
                if wanted_types
                    .iter()
                    .any(|wanted| attacker_template.get_name().as_str() == *wanted)
                {
                    return Ok(ScriptConditionResult::True);
                }
            }
        }

        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_attacked_by_player(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateTeamAttackedByPlayer
        let team_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' attacked by player '{}'",
            team_name,
            player_name
        );

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

        // Look up the team and check if it was just created
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    return Ok(if team.is_created() {
                        ScriptConditionResult::True
                    } else {
                        ScriptConditionResult::False
                    });
                }
            }
        }
        Ok(ScriptConditionResult::False)
    }

    pub(crate) fn eval_team_discovered(
        &self,
        condition: &Condition,
    ) -> Result<ScriptConditionResult, ScriptError> {
        // C++: ScriptConditions::evaluateTeamDiscovered
        let team_name = self.get_condition_string_param(condition, 0)?;
        let player_name = self.get_condition_string_param(condition, 1)?;
        log::debug!(
            "Evaluating if team '{}' was discovered by player '{}'",
            team_name,
            player_name
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
        let player_id: u32 = match player.get_player_index().try_into() {
            Ok(value) => value,
            Err(_) => return Ok(ScriptConditionResult::False),
        };

        let Ok(mut factory) = get_team_factory().lock() else {
            return Ok(ScriptConditionResult::False);
        };
        let Some(team_arc) = factory.find_team(&team_name) else {
            return Ok(ScriptConditionResult::False);
        };
        let Ok(team) = team_arc.read() else {
            return Ok(ScriptConditionResult::False);
        };

        let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
        let Ok(shroud_mgr) = shroud_mgr.lock() else {
            return Ok(ScriptConditionResult::False);
        };

        for &member_id in team.get_members() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };

            // We are held, so we are not visible.
            if obj.is_disabled_by_type(crate::common::DisabledType::Held) {
                continue;
            }

            // If we are stealthed we are not visible (unless DETECTED or DISGUISED).
            let status = obj.get_status_bits();
            if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
                && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
                && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
            {
                continue;
            }

            let shroud_state = shroud_mgr.get_shroud_state(player_id, obj.get_position());
            if matches!(
                shroud_state,
                crate::system::shroud_manager::ShroudState::Visible
                    | crate::system::shroud_manager::ShroudState::Explored
            ) {
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
        log::debug!(
            "Evaluating if team '{}' entered area '{}' entirely",
            team_name,
            area_name
        );

        // Check if team had enter/exit event and is now entirely inside
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Only check if there was an enter/exit event this frame
                    if !team.did_enter_or_exit() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if ALL team members are now in the area
                        for &member_id in members {
                            if !objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::False);
                            }
                        }
                        return Ok(ScriptConditionResult::True);
                    }
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
        log::debug!(
            "Evaluating if team '{}' entered area '{}' partially",
            team_name,
            area_name
        );

        // Check if team had enter/exit event and at least one member is now inside
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Only check if there was an enter/exit event this frame
                    if !team.did_enter_or_exit() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if ANY team member is now in the area
                        for &member_id in members {
                            if objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::True);
                            }
                        }
                    }
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
        log::debug!(
            "Evaluating if team '{}' exited area '{}' entirely",
            team_name,
            area_name
        );

        // Check if team had enter/exit event and is now entirely outside
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Only check if there was an enter/exit event this frame
                    if !team.did_enter_or_exit() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let members = team.get_members();
                    if members.is_empty() {
                        // Empty team considered to have exited
                        return Ok(ScriptConditionResult::True);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if NO team members are in the area
                        for &member_id in members {
                            if objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::False);
                            }
                        }
                        return Ok(ScriptConditionResult::True);
                    }
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
        log::debug!(
            "Evaluating if team '{}' exited area '{}' partially",
            team_name,
            area_name
        );

        // Check if team had enter/exit event and at least one member is now outside
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Only check if there was an enter/exit event this frame
                    if !team.did_enter_or_exit() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(ScriptConditionResult::False);
                    }

                    let area_tracker = get_area_tracker();
                    if let Ok(objects_in_area) = area_tracker.get_objects_in_area(&area_name) {
                        // Check if ANY team member is now outside the area
                        for &member_id in members {
                            if !objects_in_area.contains(&member_id) {
                                return Ok(ScriptConditionResult::True);
                            }
                        }
                    }
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
}
