impl ScriptEvaluator {
    /// Helper for special power conditions (triggered, midway, complete).
    /// C++ evaluatePlayerSpecialPowerFromUnitTriggered/Midway/Complete with optional named source.
    fn evaluate_special_power_condition(
        &self,
        condition: &Condition,
        midway: bool,
        complete: bool,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "SpecialPower condition missing player parameter".to_string(),
            )
        })?;
        let power_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "SpecialPower condition missing power parameter".to_string(),
            )
        })?;

        let power_name = power_param.get_string();
        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as usize);
        let Some(player_index) = player_index else {
            return Ok(false);
        };

        let has_named = condition.get_parameter(2).is_some();
        let mut source_id = crate::common::INVALID_ID;
        if has_named {
            let named_param = condition.get_parameter(2).unwrap();
            let named_name = named_param.get_string();
            let tracker = get_named_object_tracker();
            if let Some(object_id) = tracker.get_object_id(named_name).ok().flatten() {
                if TheGameLogic::find_object_by_id(object_id).is_none() {
                    return Ok(false);
                }
                source_id = object_id;
            } else {
                return Ok(false);
            }
        }

        let result = self.with_evaluation_engine_mut(|engine| {
            if midway {
                engine.is_special_power_midway(player_index, power_name, true, source_id)
            } else if complete {
                engine.is_special_power_complete(player_index, power_name, true, source_id)
            } else {
                engine.is_special_power_triggered(player_index, power_name, true, source_id)
            }
        });

        Ok(result.unwrap_or(false))
    }

    /// Helper for upgrade conditions (built upgrade, built upgrade from named).
    /// C++ evaluateUpgradeFromUnitComplete with optional named source.
    fn evaluate_upgrade_condition(
        &self,
        condition: &Condition,
        from_named: bool,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerBuiltUpgrade condition missing player parameter".to_string(),
            )
        })?;
        let upgrade_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerBuiltUpgrade condition missing upgrade parameter".to_string(),
            )
        })?;

        let upgrade_name = upgrade_param.get_string();
        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as usize);
        let Some(player_index) = player_index else {
            return Ok(false);
        };

        let mut source_id = crate::common::INVALID_ID;
        if from_named {
            let named_param = condition.get_parameter(2).ok_or_else(|| {
                GameLogicError::Configuration(
                    "PlayerBuiltUpgradeFromNamed condition missing unit parameter".to_string(),
                )
            })?;
            let named_name = named_param.get_string();
            let tracker = get_named_object_tracker();
            if let Some(object_id) = tracker.get_object_id(named_name).ok().flatten() {
                if TheGameLogic::find_object_by_id(object_id).is_none() {
                    return Ok(false);
                }
                source_id = object_id;
            } else {
                return Ok(false);
            }
        }

        Ok(self
            .with_evaluation_engine_mut(|engine| {
                engine.is_upgrade_complete(player_index, upgrade_name, true, source_id)
            })
            .unwrap_or(false))
    }
}
