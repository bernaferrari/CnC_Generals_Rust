//! CPP ScriptConditions201/924: select the driving owner before standalone discovery.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};

impl ScriptConditionEvaluator<'_> {
    pub(crate) fn eval_owned_body_census(
        &self,
        condition: &Condition,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptConditionResult>, ScriptError> {
        let kind = condition.get_condition_type();
        if !matches!(
            kind,
            ConditionType::UnitHealth | ConditionType::PlayerAllDestroyed
        ) {
            return Ok(None);
        }
        let name = condition
            .get_parameter(0)
            .ok_or_else(|| ScriptError::ParameterNotFound("Parameter 0 not found".into()))?
            .get_string();
        let engine = self.context.borrowed_engine();
        let result = if kind == ConditionType::UnitHealth {
            let comparison = self.get_condition_comparison_param(condition, 1)?;
            let threshold = self.get_condition_int_param(condition, 2)?;
            match driver.unit_health_percent(name, engine.script_object_id()) {
                ScriptOwnerQuery::Unavailable => return Ok(None),
                ScriptOwnerQuery::Missing => false,
                ScriptOwnerQuery::Present(percent) => {
                    Self::compare_i32(comparison, percent, threshold)
                }
            }
        } else {
            let current_player = engine.get_current_player_name();
            let destroyed = match driver.player_has_any_objects(name, current_player.as_deref()) {
                ScriptOwnerQuery::Unavailable => return Ok(None),
                // CPP nonexistent player is all destroyed.
                ScriptOwnerQuery::Missing => true,
                ScriptOwnerQuery::Present(has_any) => !has_any,
            };
            match condition.get_parameter(1).map(|p| p.get_int() != 0) {
                Some(true) => !destroyed,
                Some(false) | None => destroyed,
            }
        };
        Ok(Some(Self::bool_result(result)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::ScriptEngine;

    struct Owner {
        health: ScriptOwnerQuery<i32>,
        objects: ScriptOwnerQuery<bool>,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn unit_health_percent(&self, _: &str, _: Option<ObjectID>) -> ScriptOwnerQuery<i32> {
            self.health
        }
        fn player_has_any_objects(&self, _: &str, _: Option<&str>) -> ScriptOwnerQuery<bool> {
            self.objects
        }
    }

    fn condition(kind: ConditionType) -> Condition {
        let mut result = Condition::new(kind);
        result
            .add_parameter(Parameter::with_string(
                if kind == ConditionType::UnitHealth {
                    ParameterType::Unit
                } else {
                    ParameterType::Side
                },
                "SameName".into(),
            ))
            .unwrap();
        if kind == ConditionType::UnitHealth {
            result
                .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
                .unwrap();
            result
                .add_parameter(Parameter::with_int(ParameterType::Int, 100))
                .unwrap();
        }
        result
    }

    #[test]
    fn owned_body_census_missing_is_authoritative_and_unavailable_retains_adapter() {
        let engine = ScriptEngine::new().unwrap();
        let state = RefCell::new(ScriptContext::at_frame(0));
        let evaluator = ScriptConditionEvaluator::new(&engine, &state);
        let owner = Owner {
            health: ScriptOwnerQuery::Missing,
            objects: ScriptOwnerQuery::Missing,
        };
        assert_eq!(
            evaluator
                .eval_owned_body_census(&condition(ConditionType::UnitHealth), &owner)
                .unwrap(),
            Some(ScriptConditionResult::False)
        );
        assert_eq!(
            evaluator
                .eval_owned_body_census(&condition(ConditionType::PlayerAllDestroyed), &owner)
                .unwrap(),
            Some(ScriptConditionResult::True)
        );
        let owner = Owner {
            health: ScriptOwnerQuery::Unavailable,
            objects: ScriptOwnerQuery::Unavailable,
        };
        assert_eq!(
            evaluator
                .eval_owned_body_census(&condition(ConditionType::UnitHealth), &owner)
                .unwrap(),
            None
        );
        assert_eq!(
            evaluator
                .eval_owned_body_census(&condition(ConditionType::PlayerAllDestroyed), &owner)
                .unwrap(),
            None
        );
        let owner = Owner {
            health: ScriptOwnerQuery::Present(100),
            objects: ScriptOwnerQuery::Present(true),
        };
        assert_eq!(
            evaluator
                .eval_owned_body_census(&condition(ConditionType::UnitHealth), &owner)
                .unwrap(),
            Some(ScriptConditionResult::True)
        );
        assert_eq!(
            evaluator
                .eval_owned_body_census(&condition(ConditionType::PlayerAllDestroyed), &owner)
                .unwrap(),
            Some(ScriptConditionResult::False)
        );
    }
}
