//! CPP ScriptConditions420/513/2088: an owner-selected polygon census.
use super::*;
use crate::scripting::engine::{
    ScriptExecutionDriver, ScriptOwnerQuery, ScriptPlayerAreaCensus, ScriptPlayerAreaQuery,
};
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::ScriptEngine;
    struct Owner {
        frame: u32,
        count: i32,
        changed: bool,
        missing: bool,
    }
    impl ScriptExecutionDriver<crate::scripting::engine::ScriptEngine> for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn player_area_value(
            &mut self,
            _: &str,
            _: u32,
            _: &str,
            _: Option<&str>,
            _: ScriptPlayerAreaQuery<'_>,
        ) -> ScriptOwnerQuery<ScriptPlayerAreaCensus> {
            if self.missing {
                ScriptOwnerQuery::Missing
            } else {
                ScriptOwnerQuery::Present(ScriptPlayerAreaCensus::Measured {
                    player_mask: 1,
                    value: self.count,
                    membership_changed: self.changed,
                })
            }
        }
        fn object_count_changed_frame(&self) -> ScriptOwnerQuery<u32> {
            ScriptOwnerQuery::Present(self.frame)
        }
    }
    fn condition(kind: ConditionType) -> Condition {
        let mut condition = Condition::new(kind);
        condition
            .add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        if kind != ConditionType::SkirmishValueInArea {
            condition
                .add_parameter(Parameter::with_string(
                    ParameterType::ObjectType,
                    "Infantry".into(),
                ))
                .unwrap();
        }
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::TriggerArea,
                "Area".into(),
            ))
            .unwrap();
        condition
    }
    #[test]
    fn area_type_and_value_cache_keep_distinct_cpp_frame_gates_and_missing_is_final() {
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
        let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
        for kind in [
            ConditionType::PlayerHasComparisonUnitTypeInTriggerArea,
            ConditionType::SkirmishValueInArea,
        ] {
            let mut condition = condition(kind);
            let mut owner = Owner {
                frame: 10,
                count: 1,
                changed: false,
                missing: false,
            };
            assert_eq!(
                evaluator
                    .evaluate_condition_with_driver(&mut condition, &mut owner)
                    .unwrap(),
                ScriptConditionResult::True
            );
            owner.frame = 9;
            owner.count = 0;
            let expected = if kind == ConditionType::SkirmishValueInArea {
                ScriptConditionResult::False
            } else {
                ScriptConditionResult::True
            };
            assert_eq!(
                evaluator
                    .evaluate_condition_with_driver(&mut condition, &mut owner)
                    .unwrap(),
                expected
            );
            owner.changed = true;
            assert_eq!(
                evaluator
                    .evaluate_condition_with_driver(&mut condition, &mut owner)
                    .unwrap(),
                ScriptConditionResult::False
            );
            owner.missing = true;
            owner.count = 1;
            assert_eq!(
                evaluator
                    .evaluate_condition_with_driver(&mut condition, &mut owner)
                    .unwrap(),
                ScriptConditionResult::False
            );
        }
    }
    #[test]
    fn area_kind_preserves_cpp_frame_in_custom_data_quirk() {
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
        let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
        let mut condition = condition(ConditionType::PlayerHasComparisonUnitKindInTriggerArea);
        let mut owner = Owner {
            frame: 18,
            count: 1,
            changed: false,
            missing: false,
        };
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::True
        );
        assert_eq!((condition.custom_data, condition.custom_frame), (18, 0));
    }
}
