// Destroyed, created, and team-state condition evaluators
//
// Split from `scripting/evaluator.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEvaluator {
    fn evaluate_player_all_destroyed_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAllDestroyed condition missing player parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        log::debug!("Evaluating PlayerAllDestroyed for player: {}", player_name);

        let Ok(list) = player_list().read() else {
            return Ok(true);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(true);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(true);
        };

        Ok(!player_guard.has_any_objects())
    }

    /// Evaluate player all build facilities destroyed condition
    fn evaluate_player_all_buildfacilities_destroyed_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerAllBuildfacilitiesDestroyed condition missing player parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating PlayerAllBuildfacilitiesDestroyed for player: {}",
            player_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(true);
        };
        let Some(player_arc) = list.find_player_by_name(player_name) else {
            return Ok(true);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(true);
        };

        Ok(!player_guard.has_any_build_facility())
    }

    /// Evaluate team destroyed condition
    fn evaluate_team_destroyed_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamDestroyed condition missing team parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        log::debug!("Evaluating TeamDestroyed for team: {}", team_name);

        let teams = self.resolve_team_instances(&team_name);
        if teams.is_empty() {
            return Ok(false);
        }

        for team_arc in teams {
            if let Ok(team) = team_arc.read() {
                if team.has_any_objects() {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Evaluate team has units condition
    fn evaluate_team_has_units_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamHasUnits condition missing team parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        log::debug!("Evaluating TeamHasUnits for team: {}", team_name);

        for team_arc in self.resolve_team_instances(&team_name) {
            if let Ok(team) = team_arc.read() {
                if team.has_any_units() {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Evaluate named destroyed condition
    fn evaluate_named_destroyed_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedDestroyed condition missing unit parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        log::debug!("Evaluating NamedDestroyed for unit: {}", unit_name);

        // Look up the named object using the tracker
        let tracker = get_named_object_tracker();
        if let Ok(Some(object_id)) = tracker.get_object_id(unit_name) {
            // Check if the object exists and is destroyed
            if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    return Ok(obj.is_destroyed());
                }
            }
            // Object ID exists but object not found - considered destroyed
            return Ok(true);
        }
        // Object not in tracker - considered destroyed
        Ok(true)
    }

    fn evaluate_named_created_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedCreated condition missing unit parameter".to_string(),
            )
        })?;
        let unit_name = unit_param.get_string();

        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        Ok(TheGameLogic::find_object_by_id(object_id).is_some())
    }

    fn evaluate_team_created_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamCreated condition missing team parameter".to_string(),
            )
        })?;
        let team_name = self.resolve_team_name_token(team_param.get_string());

        let Some(team_arc) = self.resolve_team_instances(&team_name).into_iter().next() else {
            return Ok(false);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(false);
        };
        Ok(team_guard.is_created())
    }

    fn evaluate_team_state_is_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIs condition missing team parameter".to_string(),
            )
        })?;
        let state_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIs condition missing state parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let expected_state = state_param.get_string();

        let Some(team_arc) = self.resolve_team_instances(&team_name).into_iter().next() else {
            return Ok(false);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(false);
        };
        Ok(team_guard.get_state().as_str() == expected_state)
    }

    fn evaluate_team_state_is_not_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIsNot condition missing team parameter".to_string(),
            )
        })?;
        let state_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamStateIsNot condition missing state parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let expected_state = state_param.get_string();

        let Some(team_arc) = self.resolve_team_instances(&team_name).into_iter().next() else {
            return Ok(false);
        };
        let Ok(team_guard) = team_arc.read() else {
            return Ok(false);
        };
        Ok(team_guard.get_state().as_str() != expected_state)
    }
}
