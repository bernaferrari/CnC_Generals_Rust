//! Public borrowed dispatch must preserve its engine during retained callbacks.
//! CPP ScriptEngine.cpp:7558-7648 executes callbacks as part of one engine walk.
use super::*;
use crate::scripting::engine::{with_script_engine_mut, with_script_engine_ref};

struct ReenterBoundEngine {
    panic_after_action: bool,
}

impl ReenterBoundEngine {
    fn reenter(&self) {
        let result = with_script_engine_mut(|engine| engine.increment_counter("CallbackOwner", 1));
        assert!(matches!(result, Some(Ok(()))));
    }
}

impl ScriptActionHandler for ReenterBoundEngine {
    fn display_text(&self, _text: &str) -> GameLogicResult<()> {
        self.reenter();
        if self.panic_after_action {
            panic!("authored callback failure");
        }
        Ok(())
    }

    fn is_video_complete(&self, _name: &str, _flush: bool) -> bool {
        self.reenter();
        true
    }
}

fn display_action() -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::DisplayText);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            "Briefing".into(),
        ))
        .unwrap();
    action
}

fn video_condition() -> Condition {
    let mut condition = Condition::new(ConditionType::HasFinishedVideo);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Movie,
            "Briefing".into(),
        ))
        .unwrap();
    condition
}

fn foreign_counter() -> Option<i32> {
    with_script_engine_ref(|engine| engine.get_counter("ForeignOwner").map(|c| c.value)).flatten()
}

#[test]
fn public_action_callback_reenters_bound_engine_and_restores_foreign_scope() {
    let mut own = ScriptEngine::new().unwrap();
    own.set_action_handler(Some(Arc::new(ReenterBoundEngine {
        panic_after_action: false,
    })));
    let foreign = ScriptEngine::new().unwrap();
    foreign.set_counter("ForeignOwner", 19).unwrap();
    let state = RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&own, &state);
    foreign.with_active(|| {
        dispatcher.execute_action(&display_action()).unwrap();
        assert_eq!(foreign_counter(), Some(19));
    });
    assert_eq!(own.get_counter("CallbackOwner").map(|c| c.value), Some(1));
    assert!(foreign.get_counter("CallbackOwner").is_none());
}

#[test]
fn public_condition_and_or_callbacks_reenter_bound_engine() {
    let mut own = ScriptEngine::new().unwrap();
    own.set_action_handler(Some(Arc::new(ReenterBoundEngine {
        panic_after_action: false,
    })));
    let foreign = ScriptEngine::new().unwrap();
    foreign.set_counter("ForeignOwner", 19).unwrap();
    let state = RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&own, &state);
    let mut or = OrCondition::new();
    or.set_first_and_condition(Some(Box::new(video_condition())));
    foreign.with_active(|| {
        assert_eq!(
            evaluator
                .evaluate_condition(&mut video_condition())
                .unwrap(),
            ScriptConditionResult::True
        );
        assert!(evaluator.evaluate_or_condition(&mut or).unwrap());
        assert_eq!(foreign_counter(), Some(19));
    });
    assert_eq!(own.get_counter("CallbackOwner").map(|c| c.value), Some(2));
    assert!(foreign.get_counter("CallbackOwner").is_none());
}

#[test]
fn public_callback_unwind_restores_foreign_engine() {
    let mut own = ScriptEngine::new().unwrap();
    own.set_action_handler(Some(Arc::new(ReenterBoundEngine {
        panic_after_action: true,
    })));
    let foreign = ScriptEngine::new().unwrap();
    foreign.set_counter("ForeignOwner", 19).unwrap();
    let state = RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&own, &state);
    foreign.with_active(|| {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatcher.execute_action(&display_action()).unwrap();
        }));
        assert!(result.is_err());
        assert_eq!(foreign_counter(), Some(19));
    });
    assert_eq!(own.get_counter("CallbackOwner").map(|c| c.value), Some(1));
    assert!(foreign.get_counter("CallbackOwner").is_none());
}
