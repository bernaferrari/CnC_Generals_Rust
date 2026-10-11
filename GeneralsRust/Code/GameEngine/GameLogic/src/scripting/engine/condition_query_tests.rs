//! Borrowed evaluateConditions uses real local cash and original ordered chains.
use super::*;
use crate::scripting::executor::ScriptContext;
use std::cell::RefCell;

struct Owner<'a> {
    engine: &'a ScriptEngine,
    cash: crate::player::Player,
    seen: RefCell<Vec<(String, Option<String>, Option<TeamID>, Option<ObjectID>)>>,
    panic: bool,
    nested: bool,
}
impl<'a> Owner<'a> {
    fn new(engine: &'a ScriptEngine, money: i32) -> Self {
        let mut cash = crate::player::Player::new(7);
        cash.get_money_mut().set_money(money);
        Self {
            engine,
            cash,
            seen: RefCell::new(Vec::new()),
            panic: false,
            nested: false,
        }
    }
}
impl ScriptExecutionDriver for Owner<'_> {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn team_name(&self, id: TeamID) -> ScriptOwnerQuery<String> {
        if id == 5 {
            ScriptOwnerQuery::Present("OwnedTeam".into())
        } else {
            ScriptOwnerQuery::Missing
        }
    }
    fn team_current_player(&self, id: TeamID) -> ScriptOwnerQuery<Option<String>> {
        match id {
            5 => ScriptOwnerQuery::Present(Some("cash".into())),
            6 => ScriptOwnerQuery::Present(None),
            _ => ScriptOwnerQuery::Missing,
        }
    }
    fn player_census(
        &self,
        name: &str,
        current: Option<&str>,
    ) -> ScriptOwnerQuery<(i32, crate::scripting::HostScriptPlayerCensus)> {
        self.seen.borrow_mut().push((
            name.into(),
            current.map(str::to_owned),
            self.engine.get_calling_team_id(),
            self.engine.script_object_id(),
        ));
        assert!(!self.panic, "owned callback panic control");
        if self.nested {
            let before = (
                self.engine.get_calling_team_id(),
                self.engine.get_current_player_name(),
            );
            let mut nested_script = with_condition(Condition::new(ConditionType::ConditionTrue));
            let mut nested_owner = Owner::new(self.engine, 0);
            assert!(self.engine.evaluate_conditions_with_driver(
                &mut nested_script,
                None,
                Some("nested"),
                ScriptContext::at_frame(52),
                &mut nested_owner
            ));
            assert_eq!(
                (
                    self.engine.get_calling_team_id(),
                    self.engine.get_current_player_name()
                ),
                before
            );
        }

        if name != "cash" {
            return ScriptOwnerQuery::Missing;
        }
        ScriptOwnerQuery::Present((
            7,
            crate::scripting::HostScriptPlayerCensus {
                money: self.cash.get_money().get_money(),
                ..Default::default()
            },
        ))
    }
}
fn credits(name: &str, money: i32) -> Condition {
    let mut condition = Condition::new(ConditionType::PlayerHasCredits);
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, money))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(
            ParameterType::Comparison,
            ComparisonType::Equal as i32,
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, name.into()))
        .unwrap();
    condition
}
fn with_condition(condition: Condition) -> Script {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut script = Script::new();
    script.condition = Some(Box::new(branch));
    script
}
fn engine() -> ScriptEngine {
    let engine = ScriptEngine::new().unwrap();
    let mut inner = engine.lock_inner_mut();
    inner.current_player = Some("outer".into());
    inner.calling_team = Some(777);
    inner.condition_team = Some(888);
    inner.calling_object = Some(99);
    drop(inner);
    engine
}
fn assert_restored(engine: &ScriptEngine) {
    assert_eq!(engine.get_current_player_name().as_deref(), Some("outer"));
    assert_eq!(engine.get_calling_team_id(), Some(777));
    assert_eq!(engine.get_condition_team_id(), Some(888));
    assert_eq!(engine.script_object_id(), Some(99));
}

#[test]
fn borrowed_condition_queries_short_circuit_and_restore_the_driving_scope() {
    let engine = engine();
    let mut owner = Owner::new(&engine, 100);
    let peer_engine = ScriptEngine::new().unwrap();
    let mut peer = Owner::new(&peer_engine, 10);
    let mut first = credits("missing", 0);
    first.next_and_condition = Some(Box::new(credits("must-not-run-and", 0)));
    let mut script = with_condition(first);
    let mut second = OrCondition::new();
    second.set_first_and_condition(Some(Box::new(credits("cash", 100))));
    let mut third = OrCondition::new();
    third.set_first_and_condition(Some(Box::new(credits("must-not-run-or", 0))));
    second.next_or = Some(Box::new(third));
    script.condition.as_mut().unwrap().next_or = Some(Box::new(second));
    assert!(engine.evaluate_conditions_with_driver(
        &mut script,
        Some(5),
        Some("ignored"),
        ScriptContext::at_frame(51),
        &mut owner
    ));
    assert_eq!(
        *owner.seen.borrow(),
        [
            ("missing".into(), Some("cash".into()), Some(5), Some(99)),
            ("cash".into(), Some("cash".into()), Some(5), Some(99))
        ]
    );
    assert_restored(&engine);
    let mut peer_script = with_condition(credits("cash", 100));
    assert!(!peer_engine.evaluate_conditions_with_driver(
        &mut peer_script,
        None,
        Some("cash"),
        ScriptContext::at_frame(51),
        &mut peer
    ));
    owner.cash.get_money_mut().set_money(200);
    let mut updated = with_condition(credits("cash", 200));
    assert!(engine.evaluate_conditions_with_driver(
        &mut updated,
        None,
        Some("cash"),
        ScriptContext::at_frame(52),
        &mut owner
    ));
    assert_restored(&engine);
}

#[test]
fn borrowed_condition_queries_keep_null_controller_fallback_and_fail_closed_on_missing_owner() {
    let engine = engine();
    let mut owner = Owner::new(&engine, 100);
    let mut script = with_condition(credits("cash", 100));
    assert!(engine.evaluate_conditions_with_driver(
        &mut script,
        Some(6),
        Some("ignored"),
        ScriptContext::at_frame(0),
        &mut owner
    ));
    assert_eq!(owner.seen.borrow()[0].1.as_deref(), Some("outer"));
    owner.seen.borrow_mut().clear();
    assert!(!engine.evaluate_conditions_with_driver(
        &mut script,
        Some(404),
        Some("cash"),
        ScriptContext::at_frame(0),
        &mut owner
    ));
    assert!(owner.seen.borrow().is_empty());
    assert_restored(&engine);
    let mut empty = Script::new();
    assert!(!engine.evaluate_conditions_with_driver(
        &mut empty,
        None,
        None,
        ScriptContext::at_frame(0),
        &mut owner
    ));
    let mut malformed = with_condition(Condition::new(ConditionType::Counter));
    assert!(!engine.evaluate_conditions_with_driver(
        &mut malformed,
        None,
        None,
        ScriptContext::at_frame(0),
        &mut owner
    ));
    assert_restored(&engine);
}

#[test]
fn borrowed_condition_queries_restore_scope_after_callback_panic() {
    let engine = engine();
    let mut owner = Owner::new(&engine, 100);
    owner.panic = true;
    let mut script = with_condition(credits("cash", 100));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            engine.evaluate_conditions_with_driver(
                &mut script,
                Some(5),
                Some("ignored"),
                ScriptContext::at_frame(51),
                &mut owner,
            )
        }))
        .is_err()
    );
    assert_restored(&engine);
}

#[test]
fn borrowed_condition_queries_allow_bounded_same_interpreter_reentry() {
    let engine = engine();
    let mut owner = Owner::new(&engine, 100);
    owner.nested = true;
    let mut script = with_condition(credits("cash", 100));
    assert!(engine.evaluate_conditions_with_driver(
        &mut script,
        Some(5),
        Some("ignored"),
        ScriptContext::at_frame(51),
        &mut owner
    ));
    assert_restored(&engine);
}
