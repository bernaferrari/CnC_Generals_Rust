//! CPP2307/2368/2419/2624: canonical skirmish owner queries.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery, ScriptSkirmishCensus};
impl ScriptConditionEvaluator<'_> {
    pub(super) fn eval_owned_special_power(
        &self,
        condition: &mut Condition,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptConditionResult>, ScriptError> {
        if condition.get_condition_type() != ConditionType::SkirmishSpecialPowerReady {
            return Ok(None);
        }
        let player = self.get_condition_string_param(condition, 0)?;
        let power = self.get_condition_string_param(condition, 1)?;
        let latch = condition.get_parameter(1).unwrap().get_int();
        let frame = self.context.with_state(|state| state.current_frame);
        let current = self.context.borrowed_engine().get_current_player_name();
        // Establish that the supplied owner handles this family before applying
        // its condition-owned latch. Missing never falls back to an ambient owner.
        let state =
            match driver.skirmish_special_power_readiness(&player, &power, current.as_deref()) {
                ScriptOwnerQuery::Unavailable => return Ok(None),
                ScriptOwnerQuery::Missing => return Ok(Some(Self::bool_result(false))),
                ScriptOwnerQuery::Present(state) => state,
            };
        if latch == -1 || (latch > 0 && latch as u32 > frame) {
            return Ok(Some(Self::bool_result(false)));
        }
        if !state.template_exists {
            condition.parameters[1].as_mut().unwrap().int_value = -1;
            return Ok(Some(Self::bool_result(false)));
        }
        if state.ready {
            return Ok(Some(Self::bool_result(true)));
        }
        condition.parameters[1].as_mut().unwrap().int_value = state.next_frame as i32;
        Ok(Some(Self::bool_result(false)))
    }
    pub(super) fn eval_owned_waypoint_complete(
        &self,
        condition: &Condition,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptConditionResult>, ScriptError> {
        if condition.get_condition_type() == ConditionType::NamedReachedWaypointsEnd {
            let engine = self.context.borrowed_engine();
            return Ok(
                match driver.named_waypoint_complete(
                    &self.get_condition_string_param(condition, 0)?,
                    &self.get_condition_string_param(condition, 1)?,
                    engine.script_object_id(),
                ) {
                    ScriptOwnerQuery::Unavailable => None,
                    ScriptOwnerQuery::Missing => Some(Self::bool_result(false)),
                    ScriptOwnerQuery::Present(value) => Some(Self::bool_result(value)),
                },
            );
        }
        if condition.get_condition_type() != ConditionType::TeamReachedWaypointsEnd {
            return Ok(None);
        }
        let engine = self.context.borrowed_engine();
        Ok(
            match driver.team_waypoint_complete(
                &self.get_condition_string_param(condition, 0)?,
                &self.get_condition_string_param(condition, 1)?,
                engine.get_calling_team_id(),
                engine.get_condition_team_id(),
            ) {
                ScriptOwnerQuery::Unavailable => None,
                ScriptOwnerQuery::Missing => Some(Self::bool_result(false)),
                ScriptOwnerQuery::Present(value) => Some(Self::bool_result(value)),
            },
        )
    }
    pub(super) fn eval_owned_skirmish(
        &self,
        condition: &Condition,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptConditionResult>, ScriptError> {
        let kind = condition.get_condition_type();
        let query = match kind {
            ConditionType::SkirmishUnownedFactionUnitExists => {
                ScriptSkirmishCensus::NeutralUnmanned
            }
            ConditionType::SkirmishPlayerHasComparisonGarrisoned => {
                ScriptSkirmishCensus::Garrisoned
            }
            ConditionType::SkirmishPlayerHasComparisonCapturedUnits => {
                ScriptSkirmishCensus::Captured
            }
            ConditionType::SkirmishPlayerHasDiscoveredPlayer => {
                let current = self.context.borrowed_engine().get_current_player_name();
                return Ok(
                    match driver.player_discovered_by(
                        &self.get_condition_string_param(condition, 0)?,
                        &self.get_condition_string_param(condition, 1)?,
                        current.as_deref(),
                    ) {
                        ScriptOwnerQuery::Unavailable => None,
                        ScriptOwnerQuery::Missing => Some(Self::bool_result(false)),
                        ScriptOwnerQuery::Present(value) => Some(Self::bool_result(value)),
                    },
                );
            }
            _ => return Ok(None),
        };
        let current = self.context.borrowed_engine().get_current_player_name();
        let count = match driver.skirmish_census(
            &self.get_condition_string_param(condition, 0)?,
            current.as_deref(),
            query,
        ) {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing => return Ok(Some(Self::bool_result(false))),
            ScriptOwnerQuery::Present(count) => count,
        };
        Ok(Some(Self::bool_result(Self::compare_i32(
            self.get_condition_comparison_param(condition, 1)?,
            count,
            self.get_condition_int_param(condition, 2)?,
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::ScriptSpecialPowerReadiness;
    struct Owner {
        exists: bool,
        ready: bool,
        next_frame: u32,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> crate::GameLogicResult<()> {
            Ok(())
        }
        fn skirmish_special_power_readiness(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
        ) -> ScriptOwnerQuery<ScriptSpecialPowerReadiness> {
            ScriptOwnerQuery::Present(ScriptSpecialPowerReadiness {
                template_exists: self.exists,
                ready: self.ready,
                next_frame: self.next_frame,
            })
        }
    }
    #[test]
    fn power_parameter_retains_cpp_future_frame_and_missing_definition_latches() {
        let engine = crate::scripting::engine::ScriptEngine::new().unwrap();
        let mut condition = Condition::new(ConditionType::SkirmishSpecialPowerReady);
        condition
            .add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::SpecialPower,
                "Power".into(),
            ))
            .unwrap();
        let owner = Owner {
            exists: true,
            ready: false,
            next_frame: 60,
        };
        let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
        assert_eq!(
            ScriptConditionEvaluator::new(&engine, &context)
                .eval_owned_special_power(&mut condition, &owner)
                .unwrap(),
            Some(ScriptConditionResult::False)
        );
        assert_eq!(condition.get_parameter(1).unwrap().get_int(), 60);
        let owner = Owner {
            ready: true,
            ..owner
        };
        let context = std::cell::RefCell::new(ScriptContext::at_frame(59));
        assert_eq!(
            ScriptConditionEvaluator::new(&engine, &context)
                .eval_owned_special_power(&mut condition, &owner)
                .unwrap(),
            Some(ScriptConditionResult::False)
        );
        let context = std::cell::RefCell::new(ScriptContext::at_frame(60));
        assert_eq!(
            ScriptConditionEvaluator::new(&engine, &context)
                .eval_owned_special_power(&mut condition, &owner)
                .unwrap(),
            Some(ScriptConditionResult::True)
        );
        condition.parameters[1].as_mut().unwrap().int_value = 0;
        let missing = Owner {
            exists: false,
            ..owner
        };
        assert_eq!(
            ScriptConditionEvaluator::new(&engine, &context)
                .eval_owned_special_power(&mut condition, &missing)
                .unwrap(),
            Some(ScriptConditionResult::False)
        );
        assert_eq!(condition.get_parameter(1).unwrap().get_int(), -1);
        assert_eq!(
            ScriptConditionEvaluator::new(&engine, &context)
                .eval_owned_special_power(&mut condition, &owner)
                .unwrap(),
            Some(ScriptConditionResult::False)
        );
    }
}
