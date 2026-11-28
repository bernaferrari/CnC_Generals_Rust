//! Bound named predicates observe the supplied Object owner before standalone discovery.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::{ScriptEngine, ScriptOwnerQuery};

    struct TransportDriver {
        frame: u32,
        count: u32,
    }

    impl ScriptExecutionDriver for TransportDriver {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }

        fn named_query_fact(
            &self,
            name: &str,
            _this_object: Option<u32>,
        ) -> ScriptOwnerQuery<(u32, crate::scripting::HostScriptQueryObject)> {
            ScriptOwnerQuery::Present((
                self.frame,
                crate::scripting::HostScriptQueryObject {
                    id: 77,
                    name: name.to_owned(),
                    contain_count: self.count,
                    ..Default::default()
                },
            ))
        }
    }

    fn emptied_condition() -> Condition {
        let mut condition = Condition::new(ConditionType::UnitEmptied);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "Transport".into(),
            ))
            .unwrap();
        condition
    }

    fn value(result: ScriptConditionResult) -> bool {
        match result {
            ScriptConditionResult::True => true,
            ScriptConditionResult::False => false,
            ScriptConditionResult::Error(error) => panic!("condition failed: {error}"),
        }
    }

    struct UnavailableDriver;

    impl ScriptExecutionDriver for UnavailableDriver {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
    }

    #[test]
    fn public_unit_emptied_dispatch_uses_each_borrowed_engine_with_same_object_id() {
        let first = ScriptEngine::new_in_published_world().unwrap();
        let second = ScriptEngine::new_in_published_world().unwrap();
        let first_state = std::cell::RefCell::new(ScriptContext::at_frame(10));
        let second_state = std::cell::RefCell::new(ScriptContext::at_frame(10));
        let mut first_eval = ScriptConditionEvaluator::new(&first, &first_state);
        let mut second_eval = ScriptConditionEvaluator::new(&second, &second_state);
        let mut first_driver = TransportDriver {
            frame: 10,
            count: 3,
        };
        let mut second_driver = TransportDriver {
            frame: 10,
            count: 0,
        };

        assert!(!value(
            first_eval
                .evaluate_condition_with_driver(&mut emptied_condition(), &mut first_driver,)
                .unwrap()
        ));
        assert!(!value(
            second_eval
                .evaluate_condition_with_driver(&mut emptied_condition(), &mut second_driver,)
                .unwrap()
        ));

        first_driver.frame = 11;
        first_driver.count = 0;
        second_driver.frame = 11;
        assert!(value(
            first_eval
                .evaluate_condition_with_driver(&mut emptied_condition(), &mut first_driver,)
                .unwrap()
        ));
        assert!(value(
            first_eval
                .evaluate_condition_with_driver(&mut emptied_condition(), &mut first_driver,)
                .unwrap()
        ));
        assert!(!value(
            second_eval
                .evaluate_condition_with_driver(&mut emptied_condition(), &mut second_driver,)
                .unwrap()
        ));

        // An unrelated active engine cannot redirect the explicitly borrowed B evaluator.
        first.with_active_for_test(|| {
            second_driver.frame = 12;
            second_driver.count = 2;
            assert!(!value(
                second_eval
                    .evaluate_condition_with_driver(&mut emptied_condition(), &mut second_driver,)
                    .unwrap()
            ));
            second_driver.frame = 13;
            second_driver.count = 0;
            assert!(value(
                second_eval
                    .evaluate_condition_with_driver(&mut emptied_condition(), &mut second_driver,)
                    .unwrap()
            ));
        });
    }

    #[test]
    fn public_unit_emptied_dispatch_reports_engine_borrow_conflict() {
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let state = std::cell::RefCell::new(ScriptContext::at_frame(4));
        let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
        let mut driver = TransportDriver { frame: 4, count: 0 };
        let result = engine.with_inner(|_| {
            evaluator.evaluate_condition_with_driver(&mut emptied_condition(), &mut driver)
        });
        assert!(matches!(result, Err(ScriptError::EvaluationFailed(_))));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn public_unit_emptied_unavailable_driver_fallback_uses_context_frame_and_owner() {
        let module = module_path!();
        let prefix = concat!(env!("CARGO_CRATE_NAME"), "::");
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            &format!(
                "{}::public_unit_emptied_unavailable_driver_fallback_uses_context_frame_and_owner",
                module.strip_prefix(prefix).unwrap_or(module)
            ),
            "GENERALS_UNIT_EMPTIED_FALLBACK_CHILD",
        ) {
            return;
        }

        // The host-query fallback is selected only when the process-wide
        // compatibility registry is empty. Run in the bounded child to keep
        // this read-only precondition isolated from parallel simulation tests.
        assert!(crate::object::registry::OBJECT_REGISTRY.is_empty());
        crate::scripting::HostScriptQuery::published_for_test().set_host_script_query_snapshot(
            crate::scripting::HostScriptQuerySnapshot {
                objects: vec![crate::scripting::HostScriptQueryObject {
                    id: 91,
                    name: "FallbackTransport".into(),
                    contain_count: 2,
                    ..Default::default()
                }],
                ..Default::default()
            },
        );

        let first = ScriptEngine::new_in_published_world().unwrap();
        let second = ScriptEngine::new_in_published_world().unwrap();
        let first_state = std::cell::RefCell::new(ScriptContext::at_frame(20));
        let second_state = std::cell::RefCell::new(ScriptContext::at_frame(20));
        let mut first_eval = ScriptConditionEvaluator::new(&first, &first_state);
        let mut second_eval = ScriptConditionEvaluator::new(&second, &second_state);
        let mut first_driver = UnavailableDriver;
        let mut second_driver = UnavailableDriver;
        let mut condition = || {
            let mut condition = Condition::new(ConditionType::UnitEmptied);
            condition
                .add_parameter(Parameter::with_string(
                    ParameterType::Unit,
                    "FallbackTransport".into(),
                ))
                .unwrap();
            condition
        };

        assert!(!value(
            first_eval
                .evaluate_condition_with_driver(&mut condition(), &mut first_driver)
                .unwrap()
        ));
        // The fallback snapshot is an ambient compatibility projection. Give
        // the second engine its own initial observation before interleaving.
        crate::scripting::HostScriptQuery::published_for_test().set_host_script_query_snapshot(
            crate::scripting::HostScriptQuerySnapshot {
                objects: vec![crate::scripting::HostScriptQueryObject {
                    id: 91,
                    name: "FallbackTransport".into(),
                    contain_count: 0,
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
        assert!(!value(
            second_eval
                .evaluate_condition_with_driver(&mut condition(), &mut second_driver)
                .unwrap()
        ));

        crate::scripting::HostScriptQuery::published_for_test().set_host_script_query_snapshot(
            crate::scripting::HostScriptQuerySnapshot {
                objects: vec![crate::scripting::HostScriptQueryObject {
                    id: 91,
                    name: "FallbackTransport".into(),
                    contain_count: 0,
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
        first_state.borrow_mut().current_frame = 21;
        second_state.borrow_mut().current_frame = 21;
        assert!(value(
            first_eval
                .evaluate_condition_with_driver(&mut condition(), &mut first_driver)
                .unwrap()
        ));
        assert!(value(
            first_eval
                .evaluate_condition_with_driver(&mut condition(), &mut first_driver)
                .unwrap()
        ));
        assert!(!value(
            second_eval
                .evaluate_condition_with_driver(&mut condition(), &mut second_driver)
                .unwrap()
        ));
    }

    #[test]
    fn receiving_transport_history_wraps_repeats_and_is_not_xfer_history() {
        let mut engine = ScriptEngine::new_in_published_world().unwrap();
        assert!(!engine.observe_receiving_transport_empty(7, u32::MAX, 1));
        assert!(engine.observe_receiving_transport_empty(7, 0, 0));
        assert!(engine.observe_receiving_transport_empty(7, 0, 0));
        let tail = engine.interpreter().capture_continuation(Vec::new());
        engine.interpreter().restore_archive_continuation(&tail);
        assert!(!engine.observe_receiving_transport_empty(7, 0, 0));
        assert!(!engine.observe_receiving_transport_empty(7, 5, 1));
        engine.reset();
        assert!(!engine.observe_receiving_transport_empty(7, 6, 0));
    }
}
