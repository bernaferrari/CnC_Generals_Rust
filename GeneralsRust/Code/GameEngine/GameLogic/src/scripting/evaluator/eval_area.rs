// Named/team inside, outside, entered, and exited area conditions
//
// Split from `scripting/evaluator.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEvaluator {
    fn evaluate_named_inside_area_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedInsideArea condition missing unit parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedInsideArea condition missing trigger area parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let area_name = area_param.get_string();

        if dual_world_registry_unavailable() {
            return Ok(
                crate::scripting::host_script_named_unit_in_named_area(unit_name, area_name)
                    .unwrap_or(false),
            );
        }

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

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

        if !Self::is_object_considerable(&obj_guard) {
            return Ok(false);
        }

        Ok(Self::is_object_inside_trigger(&obj_guard, &trigger))
    }

    fn evaluate_named_outside_area_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        Ok(!self.evaluate_named_inside_area_condition(condition)?)
    }

    fn evaluate_team_inside_area_partially_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaPartially condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaPartially condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaPartially condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.some_inside_some_outside(&trigger, which_to_consider)
                || team_guard.all_inside(&trigger, which_to_consider)
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_inside_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamInsideAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.all_inside(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_outside_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOutsideAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOutsideAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOutsideAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;

        let trigger = match self.get_trigger_area(area_name) {
            Some(trigger) => trigger,
            None => return Ok(false),
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if !(team_guard.all_inside(&trigger, which_to_consider)
                || team_guard.some_inside_some_outside(&trigger, which_to_consider))
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_named_entered_area_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        if dual_world_registry_unavailable() {
            return Ok(false);
        }
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedEnteredArea condition missing unit parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedEnteredArea condition missing trigger area parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let area_name = area_param.get_string();
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        Ok(crate::object::registry::OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                if obj.is_kind_of(KindOf::Inert) {
                    return false;
                }
                obj.did_enter(&trigger)
            })
            .unwrap_or(false))
    }

    fn evaluate_named_exited_area_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        if dual_world_registry_unavailable() {
            return Ok(false);
        }
        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedExitedArea condition missing unit parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedExitedArea condition missing trigger area parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let area_name = area_param.get_string();
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(unit_name).ok().flatten() else {
            return Ok(false);
        };
        Ok(crate::object::registry::OBJECT_REGISTRY
            .with_object(object_id, |obj| obj.did_exit(&trigger))
            .unwrap_or(false))
    }

    fn evaluate_team_entered_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if !team_guard.did_enter_or_exit() {
                continue;
            }

            if team_guard.did_all_enter(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_entered_area_partially_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaPartially condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaPartially condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamEnteredAreaPartially condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.did_partial_enter(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_exited_area_entirely_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaEntirely condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaEntirely condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaEntirely condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if !team_guard.did_enter_or_exit() {
                continue;
            }

            if team_guard.did_all_exit(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn evaluate_team_exited_area_partially_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaPartially condition missing team parameter".to_string(),
            )
        })?;
        let area_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaPartially condition missing trigger area parameter".to_string(),
            )
        })?;
        let type_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamExitedAreaPartially condition missing type parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let area_name = area_param.get_string();
        let which_to_consider = type_param.get_int() as u32;
        let area_tracker = get_area_tracker();
        if !area_tracker.has_area(area_name).unwrap_or(false) {
            return Ok(false);
        }
        let Some(trigger) = self.get_trigger_area(area_name) else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            let Ok(team_guard) = team_arc.read() else {
                continue;
            };

            if team_guard.did_partial_exit(&trigger, which_to_consider) {
                return Ok(true);
            }
        }

        Ok(false)
    }
}
