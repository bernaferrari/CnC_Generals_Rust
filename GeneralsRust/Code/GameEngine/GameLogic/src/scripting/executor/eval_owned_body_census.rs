//! Body and player census dispatch against a supplied world.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::ScriptEngine;

    struct Owner {
        health: ScriptOwnerQuery<i32>,
        objects: ScriptOwnerQuery<bool>,
    }
    impl ScriptExecutionDriver<crate::scripting::engine::ScriptEngine> for Owner {
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
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let mut owner = Owner {
            health: ScriptOwnerQuery::Missing,
            objects: ScriptOwnerQuery::Missing,
        };
        assert_eq!(
            engine.interpreter().try_evaluate_receiving_condition(
                &mut condition(ConditionType::UnitHealth),
                0,
                &mut engine.condition_queries(&mut owner),
            )
                .unwrap(),
            Some(false)
        );
        assert_eq!(
            engine.interpreter().try_evaluate_receiving_condition(
                &mut condition(ConditionType::PlayerAllDestroyed),
                0,
                &mut engine.condition_queries(&mut owner),
            )
                .unwrap(),
            Some(true)
        );
        let mut owner = Owner {
            health: ScriptOwnerQuery::Unavailable,
            objects: ScriptOwnerQuery::Unavailable,
        };
        assert_eq!(
            engine.interpreter().try_evaluate_receiving_condition(
                &mut condition(ConditionType::UnitHealth),
                0,
                &mut engine.condition_queries(&mut owner),
            )
                .unwrap(),
            None
        );
        assert_eq!(
            engine.interpreter().try_evaluate_receiving_condition(
                &mut condition(ConditionType::PlayerAllDestroyed),
                0,
                &mut engine.condition_queries(&mut owner),
            )
                .unwrap(),
            None
        );
        let mut owner = Owner {
            health: ScriptOwnerQuery::Present(100),
            objects: ScriptOwnerQuery::Present(true),
        };
        assert_eq!(
            engine.interpreter().try_evaluate_receiving_condition(
                &mut condition(ConditionType::UnitHealth),
                0,
                &mut engine.condition_queries(&mut owner),
            )
                .unwrap(),
            Some(true)
        );
        assert_eq!(
            engine.interpreter().try_evaluate_receiving_condition(
                &mut condition(ConditionType::PlayerAllDestroyed),
                0,
                &mut engine.condition_queries(&mut owner),
            )
                .unwrap(),
            Some(false)
        );
    }
}
