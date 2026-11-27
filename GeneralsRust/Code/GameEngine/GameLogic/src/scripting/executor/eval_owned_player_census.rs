//! CPP ScriptConditions player queries use the driving player's completed census.
use super::*;
use crate::scripting::engine::{
    ScriptExecutionDriver, ScriptOwnerQuery, ScriptPlayerCensusScope,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::ScriptEngine;

    struct Owner {
        frame: u32,
        count: i32,
    }
    impl ScriptExecutionDriver<crate::scripting::engine::ScriptEngine> for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn literal_template_exists(&self, _: &str) -> ScriptOwnerQuery<bool> {
            ScriptOwnerQuery::Present(true)
        }
        fn object_count_changed_frame(&self) -> ScriptOwnerQuery<u32> {
            ScriptOwnerQuery::Present(self.frame)
        }
        fn player_census(
            &mut self,
            _: &str,
            _: u32,
            _: Option<&str>,
            scope: ScriptPlayerCensusScope,
        ) -> ScriptOwnerQuery<(i32, crate::scripting::HostScriptPlayerCensus)> {
            assert_eq!(scope, ScriptPlayerCensusScope::TemplateCounts);
            let mut census = crate::scripting::HostScriptPlayerCensus::default();
            census.template_counts.insert("ranger".into(), self.count);
            ScriptOwnerQuery::Present((0, census))
        }
    }
    #[test]
    fn built_by_player_cache_uses_only_the_driving_object_count_frame() {
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
        let mut evaluator = ScriptConditionEvaluator::new(&engine, &context);
        let mut condition = Condition::new(ConditionType::BuiltByPlayer);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                "Ranger".into(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
            .unwrap();
        let mut owner = Owner {
            frame: 10,
            count: 1,
        };
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::True
        );
        assert_eq!((condition.custom_data, condition.custom_frame), (1, 10));
        owner.count = 0;
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::True
        );
        owner.frame = 11;
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::False
        );
        assert_eq!((condition.custom_data, condition.custom_frame), (-1, 11));
    }
}
