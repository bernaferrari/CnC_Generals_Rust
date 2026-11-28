//! Ready-frame latches and SIDE caching through the script condition entry.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery, ScriptSkirmishCensus};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::{ScriptEngine, ScriptSpecialPowerReadiness};
    struct Owner {
        exists: bool,
        ready: bool,
        next_frame: u32,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> crate::GameLogicResult<()> {
            Ok(())
        }
        fn condition_player_id(
            &mut self,
            _: &str,
            _: u32,
            _: Option<&str>,
        ) -> ScriptOwnerQuery<u32> {
            ScriptOwnerQuery::Present(1)
        }

        fn skirmish_special_power_exists(&self, _: &str) -> ScriptOwnerQuery<bool> {
            ScriptOwnerQuery::Present(self.exists)
        }

        fn skirmish_special_power_readiness(
            &self,
            _: u32,
            _: &str,
        ) -> ScriptOwnerQuery<ScriptSpecialPowerReadiness> {
            ScriptOwnerQuery::Present(if self.ready {
                ScriptSpecialPowerReadiness::Ready
            } else {
                ScriptSpecialPowerReadiness::NotReady {
                    earliest_ready_frame: Some(self.next_frame),
                }
            })
        }
    }

    #[test]
    fn invalid_power_definition_latches_before_missing_side() {
        struct MissingOwner;
        impl ScriptExecutionDriver for MissingOwner {
            fn after_action(&mut self) -> GameLogicResult<()> {
                Ok(())
            }

            fn skirmish_special_power_exists(&self, _: &str) -> ScriptOwnerQuery<bool> {
                ScriptOwnerQuery::Present(false)
            }

            fn condition_player_id(
                &mut self,
                _: &str,
                _: u32,
                _: Option<&str>,
            ) -> ScriptOwnerQuery<u32> {
                panic!("invalid power must not resolve the side")
            }
        }

        let engine = ScriptEngine::new_in_published_world().unwrap();
        let context = RefCell::new(ScriptContext::at_frame(20));
        let mut condition = Condition::new(ConditionType::SkirmishSpecialPowerReady);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "Missing".into(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::SpecialPower,
                "Invalid".into(),
            ))
            .unwrap();
        let result = ScriptConditionEvaluator::new(&engine, &context)
            .evaluate_condition_with_driver(&mut condition, &mut MissingOwner)
            .unwrap();
        assert_eq!(result, ScriptConditionResult::False);
        assert_eq!(condition.get_parameter(1).unwrap().get_int(), -1);
        assert_eq!(condition.get_parameter(0).unwrap().get_int(), 0);
    }

    #[test]
    fn invalid_census_comparison_still_caches_the_resolved_side() {
        struct CensusOwner;
        impl ScriptExecutionDriver for CensusOwner {
            fn after_action(&mut self) -> GameLogicResult<()> {
                Ok(())
            }

            fn condition_player_id(
                &mut self,
                _: &str,
                _: u32,
                _: Option<&str>,
            ) -> ScriptOwnerQuery<u32> {
                ScriptOwnerQuery::Present(2)
            }

            fn skirmish_census(&self, query: ScriptSkirmishCensus) -> ScriptOwnerQuery<i32> {
                assert!(matches!(query, ScriptSkirmishCensus::Captured(2)));
                ScriptOwnerQuery::Present(1)
            }
        }

        let engine = ScriptEngine::new_in_published_world().unwrap();
        let context = RefCell::new(ScriptContext::at_frame(20));
        let mut condition = Condition::new(ConditionType::SkirmishPlayerHasComparisonCapturedUnits);
        condition
            .add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Comparison, 99))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        let result = ScriptConditionEvaluator::new(&engine, &context)
            .evaluate_condition_with_driver(&mut condition, &mut CensusOwner)
            .unwrap();
        assert_eq!(result, ScriptConditionResult::False);
        assert_eq!(condition.get_parameter(0).unwrap().get_int(), 1 << 2);
    }

    #[test]
    fn power_parameter_retains_cpp_future_frame_and_missing_definition_latches() {
        let engine = crate::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
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
        let mut owner = Owner {
            exists: true,
            ready: false,
            next_frame: 60,
        };
        let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
        assert_eq!(
            ScriptConditionEvaluator::new(&engine, &context)
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::False
        );
        assert_eq!(condition.get_parameter(1).unwrap().get_int(), 60);
        let mut owner = Owner {
            ready: true,
            ..owner
        };
        let context = std::cell::RefCell::new(ScriptContext::at_frame(59));
        assert_eq!(
            ScriptConditionEvaluator::new(&engine, &context)
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::False
        );
        let context = std::cell::RefCell::new(ScriptContext::at_frame(60));
        assert_eq!(
            ScriptConditionEvaluator::new(&engine, &context)
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::True
        );
        condition.parameters[1].as_mut().unwrap().int_value = 0;
        let mut missing = Owner {
            exists: false,
            ..owner
        };
        assert_eq!(
            ScriptConditionEvaluator::new(&engine, &context)
                .evaluate_condition_with_driver(&mut condition, &mut missing)
                .unwrap(),
            ScriptConditionResult::False
        );
        assert_eq!(condition.get_parameter(1).unwrap().get_int(), -1);
        assert_eq!(
            ScriptConditionEvaluator::new(&engine, &context)
                .evaluate_condition_with_driver(&mut condition, &mut owner)
                .unwrap(),
            ScriptConditionResult::False
        );
    }
}
