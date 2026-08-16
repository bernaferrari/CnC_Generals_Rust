// Evaluator action execution paths and leftover special/upgrade eval
//
// Split from `scripting/evaluator.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEvaluator {
    /// Execute a sequence of actions
    pub fn execute_action_sequence(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let mut current_action = Some(action);

        while let Some(act) = current_action {
            self.execute_action(act)?;
            current_action = act.get_next();
        }

        Ok(())
    }

    /// Execute a single action matching C++ DoAction
    pub fn execute_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        log::debug!("Executing action: {:?}", action.get_action_type());

        match action.get_action_type() {
            ScriptActionType::NoOp => Ok(()), // Do nothing
            ScriptActionType::Victory => self.execute_victory_action(action),
            ScriptActionType::Defeat => self.execute_defeat_action(action),
            ScriptActionType::SetFlag => self.execute_set_flag_action(action),
            ScriptActionType::SetCounter => self.execute_set_counter_action(action),
            ScriptActionType::IncrementCounter => self.execute_increment_counter_action(action),
            ScriptActionType::DecrementCounter => self.execute_decrement_counter_action(action),
            ScriptActionType::SetTimer => self.execute_set_timer_action(action),
            ScriptActionType::SetMillisecondTimer => {
                self.execute_set_millisecond_timer_action(action)
            }
            ScriptActionType::DisplayText => self.execute_display_text_action(action),
            ScriptActionType::PlaySoundEffect => self.execute_play_sound_effect_action(action),
            ScriptActionType::EnableScript => self.execute_enable_script_action(action),
            ScriptActionType::DisableScript => self.execute_disable_script_action(action),
            ScriptActionType::FreezeTime => self.execute_freeze_time_action(action),
            ScriptActionType::UnfreezeTime => self.execute_unfreeze_time_action(action),
            ScriptActionType::PlayerSetMoney => self.execute_player_set_money_action(action),
            ScriptActionType::PlayerGiveMoney => self.execute_player_give_money_action(action),
            ScriptActionType::Quickvictory => self.execute_quick_victory_action(action),
            // Keep CALL_SUBROUTINE on this evaluator's injected ScriptEngine.
            // MissionScriptRuntime and trigger evaluators may carry a private
            // engine; routing through the process-global dispatcher would make
            // the callee list invisible and break C++'s immediate re-entry.
            ScriptActionType::CallSubroutine => self.execute_call_subroutine_action(action),
            _ => {
                let ctx = self.make_script_context();
                let mut dispatcher = ScriptActionDispatcher::new(ctx);
                match dispatcher.execute_action(action) {
                    Ok(ScriptActionResult::Success) => Ok(()),
                    Ok(ScriptActionResult::Pending(_frames)) => Ok(()),
                    Ok(ScriptActionResult::Failed(msg)) => Err(GameLogicError::Configuration(
                        format!("Script action failed: {}", msg),
                    )),
                    Err(err) => Err(GameLogicError::Configuration(format!(
                        "Script action dispatch failed: {}",
                        err
                    ))),
                }
            }
        }
    }

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

    fn make_script_context(&self) -> Arc<RwLock<ScriptContext>> {
        let mut context = ScriptContext::new();
        context.current_frame = TheGameLogic::get_frame();
        Arc::new(RwLock::new(context))
    }

    fn with_action_handler<F>(&self, f: F) -> GameLogicResult<()>
    where
        F: FnOnce(&dyn ScriptActionHandler) -> GameLogicResult<()>,
    {
        if let Some(handler) = self.get_action_handler()? {
            f(handler.as_ref())
        } else {
            Ok(())
        }
    }

    fn get_action_handler(&self) -> GameLogicResult<Option<Arc<dyn ScriptActionHandler>>> {
        Ok(self
            .with_evaluation_engine_ref(|engine| engine.action_handler())
            .flatten())
    }

    /// Execute victory action
    fn execute_victory_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        log::info!("Victory action executed");

        let _ = self.with_evaluation_engine_mut(|engine| {
            engine.set_campaign_victorious(true);
            engine.start_end_game_timer();
        });
        Ok(())
    }

    /// Execute defeat action
    fn execute_defeat_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        log::info!("Defeat action executed");

        let _ = self.with_evaluation_engine_mut(|engine| {
            engine.set_campaign_victorious(false);
            engine.start_end_game_timer();
        });
        Ok(())
    }

    /// Execute quick victory action
    fn execute_quick_victory_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        log::info!("Quick victory action executed");

        let _ = self.with_evaluation_engine_mut(|engine| {
            engine.set_campaign_victorious(true);
            engine.start_quick_end_game_timer();
        });
        Ok(())
    }

    /// Execute set flag action
    fn execute_set_flag_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let flag_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetFlag action missing flag parameter".to_string())
        })?;
        let value_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetFlag action missing value parameter".to_string())
        })?;

        let flag_name = flag_param.get_string();
        let flag_value = value_param.get_int() != 0;

        if let Some(result) = self
            .with_evaluation_engine_mut(|engine| engine.set_flag(flag_name, flag_value))
        {
            result?;
        }
        log::debug!("Set flag '{}' to {}", flag_name, flag_value);
        Ok(())
    }

    /// Execute set counter action
    fn execute_set_counter_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetCounter action missing counter parameter".to_string())
        })?;
        let value_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetCounter action missing value parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let counter_value = value_param.get_int();

        if let Some(result) = self
            .with_evaluation_engine_mut(|engine| engine.set_counter(counter_name, counter_value))
        {
            result?;
        }
        log::debug!("Set counter '{}' to {}", counter_name, counter_value);
        Ok(())
    }

    /// Execute increment counter action
    fn execute_increment_counter_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "IncrementCounter action missing counter parameter".to_string(),
            )
        })?;
        let value_param = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(1);

        let counter_name = counter_param.get_string();

        let current_value = self
            .with_evaluation_engine_mut(|engine| {
                let current_value = engine
                    .get_counter(counter_name)
                    .map(|counter| counter.value)
                    .unwrap_or(0);
                engine.set_counter(counter_name, current_value + value_param)?;
                Ok::<_, GameLogicError>(current_value)
            })
            .transpose()?
            .unwrap_or(0);
        log::debug!(
            "Incremented counter '{}' by {} to {}",
            counter_name,
            value_param,
            current_value + value_param
        );
        Ok(())
    }

    /// Execute decrement counter action
    fn execute_decrement_counter_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "DecrementCounter action missing counter parameter".to_string(),
            )
        })?;
        let value_param = action.get_parameter(1).map(|p| p.get_int()).unwrap_or(1);

        let counter_name = counter_param.get_string();

        let current_value = self
            .with_evaluation_engine_mut(|engine| {
                let current_value = engine
                    .get_counter(counter_name)
                    .map(|counter| counter.value)
                    .unwrap_or(0);
                engine.set_counter(counter_name, current_value - value_param)?;
                Ok::<_, GameLogicError>(current_value)
            })
            .transpose()?
            .unwrap_or(0);
        log::debug!(
            "Decremented counter '{}' by {} to {}",
            counter_name,
            value_param,
            current_value - value_param
        );
        Ok(())
    }

    /// Execute set timer action
    fn execute_set_timer_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetTimer action missing counter parameter".to_string())
        })?;
        let seconds_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetTimer action missing seconds parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let seconds = seconds_param.get_int();
        let frames = seconds * LOGICFRAMES_PER_SECOND as i32;

        if let Some(result) =
            self.with_evaluation_engine_mut(|engine| engine.set_timer(counter_name, frames))
        {
            result?;
        }

        log::debug!(
            "Set timer '{}' to {} seconds ({} frames)",
            counter_name,
            seconds,
            frames
        );
        Ok(())
    }

    /// Execute set millisecond timer action
    fn execute_set_millisecond_timer_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "SetMillisecondTimer action missing counter parameter".to_string(),
            )
        })?;
        let milliseconds_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "SetMillisecondTimer action missing milliseconds parameter".to_string(),
            )
        })?;

        let counter_name = counter_param.get_string();
        let seconds = milliseconds_param.get_real();
        let frames = (seconds.max(0.0) * LOGICFRAMES_PER_SECOND as f32).ceil() as i32;

        if let Some(result) = self.with_evaluation_engine_mut(|engine| {
            engine.set_timer_millisecond_script_seconds(counter_name, seconds)
        }) {
            result?;
        }

        log::debug!(
            "Set millisecond timer '{}' to {} script-seconds ({} frames)",
            counter_name,
            seconds,
            frames
        );
        Ok(())
    }

    /// Execute display text action
    fn execute_display_text_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let text_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("DisplayText action missing text parameter".to_string())
        })?;

        let text = text_param.get_string().to_string();
        self.with_action_handler(|handler| handler.display_text(&text))
    }

    /// Execute play sound effect action
    fn execute_play_sound_effect_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let sound_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlaySoundEffect action missing sound parameter".to_string(),
            )
        })?;

        let sound_name = sound_param.get_string().to_string();
        self.with_action_handler(|handler| handler.play_sound_effect(&sound_name))
    }

    /// Execute enable script action
    fn execute_enable_script_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let script_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "EnableScript action missing script parameter".to_string(),
            )
        })?;

        let script_name = script_param.get_string().to_string();
        // C++ mutates ScriptEngine state immediately, before the next action
        // in this chain (including CALL_SUBROUTINE) can run.  The host handler
        // mirrors that change into MissionScriptRuntime after the evaluator
        // returns its current borrowed entry.
        if self
            .with_evaluation_engine_mut(|engine| {
                engine.set_script_active_by_name(&script_name, true)
            })
            .is_none()
        {
            self.with_action_handler(|handler| handler.enable_script(&script_name, true))?;
        }
        Ok(())
    }

    /// Execute disable script action
    fn execute_disable_script_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let script_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "DisableScript action missing script parameter".to_string(),
            )
        })?;

        let script_name = script_param.get_string().to_string();
        // Keep DISABLE_SCRIPT immediate for the same C++ action-chain and
        // subroutine re-entry semantics as ENABLE_SCRIPT above.
        if self
            .with_evaluation_engine_mut(|engine| {
                engine.set_script_active_by_name(&script_name, false)
            })
            .is_none()
        {
            self.with_action_handler(|handler| handler.enable_script(&script_name, false))?;
        }
        Ok(())
    }

    /// C++ `ScriptEngine::callSubroutine` executes the named callee before the
    /// outer action chain continues.  Use this evaluator's engine handle so a
    /// caller with a private/lexically active engine never falls back to an
    /// unrelated global ScriptList.
    fn execute_call_subroutine_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let subroutine_name = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "CallSubroutine action missing subroutine parameter".to_string(),
            )
        })?;
        let subroutine_name = subroutine_name.get_string().to_string();
        let found = self
            .with_evaluation_engine_mut(|engine| {
                engine.execute_subroutine_by_name(&subroutine_name)
            })
            .transpose()?
            .unwrap_or(false);

        if !found {
            log::warn!(
                "CALL_SUBROUTINE: subroutine '{}' not found",
                subroutine_name
            );
        }
        Ok(())
    }

    /// Execute freeze time action
    fn execute_freeze_time_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        let _ = self.with_evaluation_engine_mut(|engine| engine.do_freeze_time());
        Ok(())
    }

    /// Execute unfreeze time action
    fn execute_unfreeze_time_action(&self, _action: &ScriptAction) -> GameLogicResult<()> {
        let _ = self.with_evaluation_engine_mut(|engine| engine.do_unfreeze_time());
        Ok(())
    }

    /// Execute player set money action
    fn execute_player_set_money_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let player_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerSetMoney action missing player parameter".to_string(),
            )
        })?;
        let amount_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerSetMoney action missing amount parameter".to_string(),
            )
        })?;

        let amount = amount_param.get_int();
        let player_name = player_param.get_string();

        log::info!("Set player '{}' money to {}", player_name, amount);

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(());
        };
        if let Ok(mut player) = player_arc.write() {
            player.get_money_mut().set_money(amount);
        }

        Ok(())
    }

    /// Execute player give money action
    fn execute_player_give_money_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let player_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerGiveMoney action missing player parameter".to_string(),
            )
        })?;
        let amount_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "PlayerGiveMoney action missing amount parameter".to_string(),
            )
        })?;

        let amount = amount_param.get_int();
        let player_name = player_param.get_string();

        log::info!("Give player '{}' {} money", player_name, amount);

        let Some(player_arc) = self.resolve_player_from_param(player_param) else {
            return Ok(());
        };
        if let Ok(mut player) = player_arc.write() {
            let current = player.get_money().get_money();
            let updated = if amount < 0 {
                current.saturating_sub(amount.saturating_neg().min(current.max(0)))
            } else {
                current.saturating_add(amount)
            };
            player.get_money_mut().set_money(updated);
        }

        Ok(())
    }
}
