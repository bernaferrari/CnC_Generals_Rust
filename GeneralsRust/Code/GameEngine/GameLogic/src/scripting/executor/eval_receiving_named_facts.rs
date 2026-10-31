//! Bound named predicates observe the supplied Object owner before standalone discovery.
use super::*;
use crate::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};

impl ScriptConditionEvaluator<'_> {
    pub(crate) fn eval_receiving_named_facts(
        &self,
        condition: &Condition,
        driver: &dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptConditionResult>, ScriptError> {
        let kind = condition.get_condition_type();
        if !matches!(
            kind,
            ConditionType::NamedAttackedByObjecttype
                | ConditionType::NamedAttackedByPlayer
                | ConditionType::NamedDiscovered
                | ConditionType::UnitHasObjectStatus
                | ConditionType::BuildingEnteredByPlayer
                | ConditionType::UnitEmptied
        ) {
            return Ok(None);
        }
        let parameter = |index| {
            condition.get_parameter(index).ok_or_else(|| {
                ScriptError::ParameterNotFound(format!("Parameter {index} not found"))
            })
        };
        let engine = self.context.borrowed_engine();
        if kind == ConditionType::BuildingEnteredByPlayer {
            let current = engine.get_current_player_name();
            match driver.building_entered_by_player(
                parameter(0)?.get_string(),
                parameter(1)?.get_string(),
                current.as_deref(),
                engine.script_object_id(),
            ) {
                ScriptOwnerQuery::Present(entered) => return Ok(Some(Self::bool_result(entered))),
                ScriptOwnerQuery::Missing => return Ok(Some(Self::bool_result(false))),
                ScriptOwnerQuery::Unavailable => {}
            }
        }
        let name = parameter(if kind == ConditionType::BuildingEnteredByPlayer {
            1
        } else {
            0
        })?
        .get_string();
        let (frame, fact) = match driver.named_query_fact(name, engine.script_object_id()) {
            ScriptOwnerQuery::Unavailable => return Ok(None),
            ScriptOwnerQuery::Missing => return Ok(Some(Self::bool_result(false))),
            ScriptOwnerQuery::Present(fact) => fact,
        };
        let value = match kind {
            ConditionType::NamedAttackedByObjecttype => {
                let types = self.resolve_object_types_param(parameter(1)?.get_string());
                !fact.last_damage_template.is_empty()
                    && types.is_in_set(&AsciiString::from(fact.last_damage_template.as_str()))
            }
            ConditionType::UnitHasObjectStatus => {
                let mask = parameter(1)?.get_object_status().bits();
                fact.status_bits & mask != 0
            }
            ConditionType::UnitEmptied => engine
                .try_observe_receiving_transport_empty(fact.id, frame, fact.contain_count as usize)
                .ok_or_else(|| {
                    ScriptError::EvaluationFailed(
                        "Script engine state is borrowed while observing transport history".into(),
                    )
                })?,
            _ => {
                let token = parameter(if kind == ConditionType::BuildingEnteredByPlayer {
                    0
                } else {
                    1
                })?
                .get_string();
                let current = engine.get_current_player_name();
                let player = match driver.receiving_player_query_name(token, current.as_deref()) {
                    ScriptOwnerQuery::Unavailable => return Ok(None),
                    ScriptOwnerQuery::Missing => return Ok(Some(Self::bool_result(false))),
                    ScriptOwnerQuery::Present(player) => player,
                };
                match kind {
                    ConditionType::NamedAttackedByPlayer => {
                        !fact.last_damage_player.is_empty() && fact.last_damage_player == player
                    }
                    ConditionType::NamedDiscovered => {
                        !fact.held
                            && !fact.stealthed_hidden
                            && fact.discovered_by.iter().any(|name| name == &player)
                    }
                    ConditionType::BuildingEnteredByPlayer => {
                        fact.has_contain
                            && !fact.player_who_entered.is_empty()
                            && fact.player_who_entered == player
                    }
                    _ => unreachable!(),
                }
            }
        };
        Ok(Some(Self::bool_result(value)))
    }
}

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
        let first = ScriptEngine::new().unwrap();
        let second = ScriptEngine::new().unwrap();
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
        let engine = ScriptEngine::new().unwrap();
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
        crate::scripting::set_host_script_query_snapshot(
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

        let first = ScriptEngine::new().unwrap();
        let second = ScriptEngine::new().unwrap();
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
        crate::scripting::set_host_script_query_snapshot(
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

        crate::scripting::set_host_script_query_snapshot(
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
        let mut engine = ScriptEngine::new().unwrap();
        assert!(!engine.observe_receiving_transport_empty(7, u32::MAX, 1));
        assert!(engine.observe_receiving_transport_empty(7, 0, 0));
        assert!(engine.observe_receiving_transport_empty(7, 0, 0));
        let tail = engine.snapshot_xfer_tail_with_named_objects(Vec::new());
        engine.restore_receiving_xfer_tail(&tail);
        assert!(!engine.observe_receiving_transport_empty(7, 0, 0));
        assert!(!engine.observe_receiving_transport_empty(7, 5, 1));
        engine.reset();
        assert!(!engine.observe_receiving_transport_empty(7, 6, 0));
    }
}
