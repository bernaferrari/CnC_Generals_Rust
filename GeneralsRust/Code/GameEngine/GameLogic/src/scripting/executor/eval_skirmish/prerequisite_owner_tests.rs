//! Owned prerequisite-query selection. Main tests exercise actual canBuild.
use super::*;
use crate::scripting::engine::{ScriptEngine, ScriptOwnerQuery};

#[derive(Debug, PartialEq, Eq)]
struct Query {
    player: String,
    candidates: Vec<String>,
    current_player: Option<String>,
}

struct Owner {
    result: ScriptOwnerQuery<bool>,
    calls: RefCell<Vec<Query>>,
}

impl ScriptExecutionDriver for Owner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }

    fn skirmish_player_can_build_any(
        &self,
        player: &str,
        candidates: &[String],
        current_player: Option<&str>,
    ) -> ScriptOwnerQuery<bool> {
        self.calls.borrow_mut().push(Query {
            player: player.into(),
            candidates: candidates.to_vec(),
            current_player: current_player.map(str::to_owned),
        });
        self.result
    }
}

fn owner(result: ScriptOwnerQuery<bool>) -> Owner {
    Owner {
        result,
        calls: RefCell::new(Vec::new()),
    }
}

fn condition(player: &str, types: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::SkirmishPlayerHasPrerequisiteToBuild);
    for (parameter_type, value) in [
        (ParameterType::Side, player),
        (ParameterType::ObjectType, types),
    ] {
        condition
            .add_parameter(Parameter::with_string(parameter_type, value.into()))
            .unwrap();
    }
    condition
}

#[test]
fn registered_empty_unknown_and_empty_parameter_remain_distinct() {
    let engine = ScriptEngine::new().unwrap();
    engine.set_object_types("EmptyList".into(), ObjectTypes::new());
    let mut empty_name = ObjectTypes::new();
    empty_name.add_object_type(AsciiString::from("MustNotAppear"));
    engine.set_object_types(String::new(), empty_name);
    let state = RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
    let mut driver = owner(ScriptOwnerQuery::Present(false));
    for name in ["EmptyList", "UnknownTemplate", ""] {
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition("Owned", name), &mut driver)
                .unwrap(),
            ScriptConditionResult::False
        );
    }
    assert_eq!(
        driver
            .calls
            .borrow()
            .iter()
            .map(|q| q.candidates.clone())
            .collect::<Vec<_>>(),
        vec![
            Vec::<String>::new(),
            vec!["UnknownTemplate".to_owned()],
            Vec::<String>::new()
        ]
    );
}

#[test]
fn missing_owner_is_authoritative_even_with_candidates() {
    let engine = ScriptEngine::new().unwrap();
    let state = RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
    let mut driver = owner(ScriptOwnerQuery::Missing);
    let mut condition = condition("Absent", "Candidate");
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut driver)
            .unwrap(),
        ScriptConditionResult::False
    );
    driver.result = ScriptOwnerQuery::Present(true);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut driver)
            .unwrap(),
        ScriptConditionResult::True
    );
    driver.result = ScriptOwnerQuery::Present(false);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut driver)
            .unwrap(),
        ScriptConditionResult::False
    );
}

#[test]
fn raw_player_and_type_tokens_use_borrowed_engine_context() {
    let mut own = ScriptEngine::new().unwrap();
    own.set_external_eval_context(Some("OwnedCurrent".into()), None);
    let mut own_types = ObjectTypes::new();
    own_types.add_object_type(AsciiString::from("OwnedCandidate"));
    own.set_object_types(LOCAL_PLAYER.into(), own_types);
    let mut foreign = ScriptEngine::new().unwrap();
    foreign.set_external_eval_context(Some("ForeignCurrent".into()), None);
    foreign.set_object_types(LOCAL_PLAYER.into(), ObjectTypes::new());
    let state = RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&own, &state);
    let mut driver = owner(ScriptOwnerQuery::Present(true));
    foreign.with_active(|| {
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(
                    &mut condition(THIS_PLAYER, LOCAL_PLAYER),
                    &mut driver
                )
                .unwrap(),
            ScriptConditionResult::True
        );
    });
    assert_eq!(
        *driver.calls.borrow(),
        vec![Query {
            player: THIS_PLAYER.into(),
            candidates: vec!["OwnedCandidate".into()],
            current_player: Some("OwnedCurrent".into()),
        }]
    );
}

#[test]
fn default_driver_preserves_unavailable_prerequisite_selection() {
    assert_eq!(
        CanonicalScriptExecutionDriver.skirmish_player_can_build_any(
            "Unadmitted",
            &["Candidate".into()],
            None
        ),
        ScriptOwnerQuery::Unavailable
    );
}

#[test]
fn missing_parameters_are_errors_before_owner_query() {
    let engine = ScriptEngine::new().unwrap();
    let state = RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
    let mut driver = owner(ScriptOwnerQuery::Missing);
    let mut condition = Condition::new(ConditionType::SkirmishPlayerHasPrerequisiteToBuild);
    for parameter_count in [0, 1] {
        if parameter_count == 1 {
            condition
                .add_parameter(Parameter::with_string(
                    ParameterType::Side,
                    "Missing".into(),
                ))
                .unwrap();
        }
        assert!(matches!(
            evaluator.evaluate_condition_with_driver(&mut condition, &mut driver),
            Err(ScriptError::ParameterNotFound(_))
        ));
    }
    assert!(driver.calls.borrow().is_empty());
}
