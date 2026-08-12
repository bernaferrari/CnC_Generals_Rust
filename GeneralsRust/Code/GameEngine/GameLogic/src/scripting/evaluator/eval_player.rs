// Player economy, buildings, power, science, and media conditions
//
// Split from `scripting/evaluator.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEvaluator {
    fn evaluate_built_by_player_condition(
        &self,
        condition: &mut Condition,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let type_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuiltByPlayer condition missing type parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuiltByPlayer condition missing player parameter".to_string(),
            )
        })?;

        if condition.custom_data != 0
            && self
                .with_evaluation_engine_ref(|engine| engine.get_frame_object_count_changed())
                .is_some_and(|frame| frame == condition.custom_frame)
        {
            return Ok(condition.custom_data == 1);
        }

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        let types = self.resolve_object_types(type_param);

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
                count += 1;
            }
        }

        let result = count != 0;
        condition.custom_data = if result { 1 } else { -1 };
        if let Some(frame) =
            self.with_evaluation_engine_ref(|engine| engine.get_frame_object_count_changed())
        {
            condition.custom_frame = frame;
        }
        Ok(result)
    }

    fn evaluate_named_building_is_empty_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedBuildingIsEmpty condition missing unit parameter".to_string(),
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
        let Some(contain) = obj_guard.get_contain() else {
            return Ok(false);
        };
        let Ok(contain_guard) = contain.lock() else {
            return Ok(false);
        };
        Ok(contain_guard.get_contained_count() == 0)
    }

    fn evaluate_building_entered_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuildingEnteredByPlayer condition missing player parameter".to_string(),
            )
        })?;
        let building_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "BuildingEnteredByPlayer condition missing building parameter".to_string(),
            )
        })?;

        let building_name = building_param.get_string();
        let tracker = get_named_object_tracker();
        let Some(object_id) = tracker.get_object_id(building_name).ok().flatten() else {
            return Ok(false);
        };

        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return Ok(false);
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(false);
        };
        let Some(contain) = obj_guard.get_contain() else {
            return Ok(false);
        };
        let Ok(contain_guard) = contain.lock() else {
            return Ok(false);
        };

        let player_mask = contain_guard.get_player_who_entered();
        if player_mask == PlayerMaskType::none() {
            return Ok(false);
        }

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        Ok(player_mask.intersects(player_guard.get_player_mask()))
    }

    /// Evaluate named not destroyed condition
    fn evaluate_named_not_destroyed_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        Ok(!self.evaluate_named_destroyed_condition(condition)?)
    }

    fn evaluate_named_discovered_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedDiscovered condition missing unit parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedDiscovered condition missing player parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        // C++ evaluates visibility for the resolved Script `SIDE`, not only a
        // literal player display name.
        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let player_index = player_arc.read().ok().map(|p| p.get_player_index());
        let Some(player_index) = player_index else {
            return Ok(false);
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

        if obj_guard.is_disabled_by_type(DisabledType::Held) {
            return Ok(false);
        }

        if obj_guard.test_status(crate::common::ObjectStatusTypes::Stealthed)
            && !obj_guard.test_status(crate::common::ObjectStatusTypes::Detected)
            && !obj_guard.test_status(crate::common::ObjectStatusTypes::Disguised)
        {
            return Ok(false);
        }

        let shroud = obj_guard.get_shrouded_status(player_index as i32);
        Ok(matches!(
            shroud,
            ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear
        ))
    }

    fn evaluate_team_discovered_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamDiscovered condition missing team parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamDiscovered condition missing player parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let player_index = player_arc.read().ok().map(|p| p.get_player_index());
        let Some(player_index) = player_index else {
            return Ok(false);
        };

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

                if obj_guard.is_disabled_by_type(DisabledType::Held) {
                    continue;
                }
                if obj_guard.test_status(crate::common::ObjectStatusTypes::Stealthed)
                    && !obj_guard.test_status(crate::common::ObjectStatusTypes::Detected)
                    && !obj_guard.test_status(crate::common::ObjectStatusTypes::Disguised)
                {
                    continue;
                }

                let shroud = obj_guard.get_shrouded_status(player_index as i32);
                if matches!(
                    shroud,
                    ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear
                ) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Evaluate player has credits condition
    fn evaluate_player_has_credits_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let credits_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasCredits condition missing credits parameter".to_string(),
            )
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasCredits condition missing comparison parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasCredits condition missing player parameter".to_string(),
            )
        })?;

        let target_credits = credits_param.get_int();
        let comparison = comparison_param.get_int() as u32;
        let player_name = player_param.get_string();

        log::debug!(
            "Evaluating PlayerHasCredits for player: {} target: {} comparison: {}",
            player_name,
            target_credits,
            comparison
        );

        // C++ returns false when playerFromParam cannot resolve the Side.  Do
        // not manufacture a zero-credit player: equality with zero would turn
        // a missing player into a successful script condition.
        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(false);
        };
        let current_credits = player.get_money().get_money();

        match comparison {
            0 => Ok(target_credits < current_credits),  // LessThan
            1 => Ok(target_credits <= current_credits), // LessEqual
            2 => Ok(current_credits == target_credits), // Equal
            3 => Ok(target_credits >= current_credits), // GreaterEqual
            4 => Ok(target_credits > current_credits),  // Greater
            5 => Ok(current_credits != target_credits), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    /// Evaluate player has power condition
    fn evaluate_player_has_power_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasPower condition missing player parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        log::debug!("Evaluating PlayerHasPower for player: {}", player_name);

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player) = player_arc.read() else {
            return Ok(false);
        };
        // Player has power if production >= consumption (not low power).
        Ok(!player.get_energy().is_low_power())
    }

    /// Evaluate player has no power condition
    fn evaluate_player_has_no_power_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        Ok(!self.evaluate_player_has_power_condition(condition)?)
    }

    fn evaluate_named_owned_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        // Wave 343: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedOwnedByPlayer condition missing unit parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "NamedOwnedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let unit_name = unit_param.get_string();
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating NamedOwnedByPlayer for unit: {} player: {}",
            unit_name,
            player_name
        );

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Some(player_id) = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as UnsignedInt)
        else {
            return Ok(false);
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

        Ok(Some(player_id) == obj_guard.get_controlling_player_id())
    }

    fn evaluate_team_owned_by_player_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let team_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOwnedByPlayer condition missing team parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "TeamOwnedByPlayer condition missing player parameter".to_string(),
            )
        })?;

        let team_name = self.resolve_team_name_token(team_param.get_string());
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating TeamOwnedByPlayer for team: {} player: {}",
            team_name,
            player_name
        );

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Some(player_id) = player_arc
            .read()
            .ok()
            .map(|p| p.get_player_index() as UnsignedInt)
        else {
            return Ok(false);
        };

        for team_arc in self.resolve_team_instances(&team_name) {
            if let Ok(team_guard) = team_arc.read() {
                if team_guard.get_controlling_player_id() == Some(player_id) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn evaluate_player_has_n_or_fewer_buildings_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let building_count_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerBuildings condition missing building count parameter".to_string(),
            )
        })?;
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerBuildings condition missing player parameter".to_string(),
            )
        })?;

        let max_buildings = building_count_param.get_int();
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating PlayerHasNOrFewerBuildings for player: {}",
            player_name
        );

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let count = player_guard.count_buildings();

        Ok(max_buildings >= count)
    }

    fn evaluate_player_has_n_or_fewer_faction_buildings_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let building_count_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerFactionBuildings condition missing building count parameter"
                    .to_string(),
            )
        })?;
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerHasNOrFewerFactionBuildings condition missing player parameter".to_string(),
            )
        })?;

        let max_buildings = building_count_param.get_int();
        let player_name = player_param.get_string();
        log::debug!(
            "Evaluating PlayerHasNOrFewerFactionBuildings for player: {}",
            player_name
        );

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let mask =
            (1u64 << (KindOf::Structure as u32)) | (1u64 << (KindOf::CountsForVictory as u32));
        let count = player_guard.count_objects_by_kindof(mask, crate::common::KIND_OF_MASK_NONE);

        Ok(max_buildings >= count)
    }

    fn evaluate_player_power_compare_percent_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerPowerComparePercent condition missing player parameter".to_string(),
            )
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerPowerComparePercent condition missing comparison parameter".to_string(),
            )
        })?;
        let percent_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerPowerComparePercent condition missing percent parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let percent = percent_param.get_int() as f64 / 100.0;

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let ratio = player_guard.get_energy().supply_ratio() as f64;
        match comparison {
            0 => Ok(ratio < percent),  // LessThan
            1 => Ok(ratio <= percent), // LessEqual
            2 => Ok(ratio == percent), // Equal
            3 => Ok(ratio >= percent), // GreaterEqual
            4 => Ok(ratio > percent),  // Greater
            5 => Ok(ratio != percent), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    fn evaluate_player_excess_power_compare_value_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let player_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerExcessPowerCompareValue condition missing player parameter".to_string(),
            )
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerExcessPowerCompareValue condition missing comparison parameter".to_string(),
            )
        })?;
        let value_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerExcessPowerCompareValue condition missing value parameter".to_string(),
            )
        })?;

        let player_name = player_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let desired_excess = value_param.get_int() as i64;

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };

        let energy = player_guard.get_energy();
        let actual_excess = (energy.production() - energy.consumption()) as i64;

        match comparison {
            0 => Ok(actual_excess < desired_excess),  // LessThan
            1 => Ok(actual_excess <= desired_excess), // LessEqual
            2 => Ok(actual_excess == desired_excess), // Equal
            3 => Ok(actual_excess >= desired_excess), // GreaterEqual
            4 => Ok(actual_excess > desired_excess),  // Greater
            5 => Ok(actual_excess != desired_excess), // NotEqual
            _ => Err(GameLogicError::Configuration(format!(
                "Invalid comparison type: {}",
                comparison
            ))),
        }
    }

    /// Evaluate has finished video condition
    fn evaluate_has_finished_video_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let video_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "HasFinishedVideo condition missing video parameter".to_string(),
            )
        })?;

        let video_name = video_param.get_string();

        Ok(self
            .with_evaluation_engine_mut(|engine| engine.is_video_complete(video_name, true))
            .unwrap_or(false))
    }

    /// Evaluate has finished speech condition
    fn evaluate_has_finished_speech_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let speech_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "HasFinishedSpeech condition missing speech parameter".to_string(),
            )
        })?;

        let speech_name = speech_param.get_string();
        Ok(self
            .with_evaluation_engine_mut(|engine| engine.is_speech_complete(speech_name, true))
            .unwrap_or(false))
    }

    /// Evaluate has finished audio condition
    fn evaluate_has_finished_audio_condition(
        &self,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let audio_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "HasFinishedAudio condition missing audio parameter".to_string(),
            )
        })?;

        let audio_name = audio_param.get_string();
        Ok(self
            .with_evaluation_engine_mut(|engine| engine.is_audio_complete(audio_name, true))
            .unwrap_or(false))
    }
}
