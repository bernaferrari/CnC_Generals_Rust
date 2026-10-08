//! Synchronous script dispatch borrows its engine and operation state.
//! C++ ScriptEngine.cpp:7558-7648 runs conditions/actions immediately on one engine.
use super::*;
use crate::scripting::engine::ScriptEngine;
use std::cell::RefCell;

/// The live engine owns the operation state. Standalone callers retain their
/// existing shared context until their owner/retained-alias boundary is migrated.
pub(super) enum ExecutionContext<'engine> {
    Borrowed {
        engine: &'engine ScriptEngine,
        state: &'engine RefCell<ScriptContext>,
    },
    Standalone(Arc<RwLock<ScriptContext>>),
}

impl<'engine> ExecutionContext<'engine> {
    pub(super) fn borrowed(
        engine: &'engine ScriptEngine,
        state: &'engine RefCell<ScriptContext>,
    ) -> Self {
        Self::Borrowed { engine, state }
    }

    pub(super) fn with_state<R>(&self, f: impl FnOnce(&ScriptContext) -> R) -> R {
        match self {
            Self::Borrowed { state, .. } => f(&state.borrow()),
            Self::Standalone(state) => f(&state.read().unwrap_or_else(|error| error.into_inner())),
        }
    }

    pub(super) fn with_state_mut<R>(&self, f: impl FnOnce(&mut ScriptContext) -> R) -> R {
        match self {
            Self::Borrowed { state, .. } => f(&mut state.borrow_mut()),
            Self::Standalone(state) => f(&mut state.write().unwrap()),
        }
    }

    pub(super) fn with_engine_ref<R>(&self, f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
        match self {
            Self::Borrowed { engine, .. } => engine.with_execution_read(f),
            Self::Standalone(_) => with_script_engine_ref(f),
        }
    }

    pub(super) fn with_engine_mut<R>(&self, f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
        match self {
            // Existing lexical scope is only a temporary adapter for callbacks
            // still using ambient helpers. It never chooses the borrowed owner.
            Self::Borrowed { engine, .. } => Some(engine.with_active(|| f(engine))),
            Self::Standalone(_) => with_script_engine_mut(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::ScriptEngine;

    fn default_action(name: &str, value: i32) -> ScriptAction {
        let mut action = ScriptAction::new(ScriptActionType::SetDefaultAttackPriority);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::AttackPrioritySet,
                name.into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, value))
            .unwrap();
        action
    }

    #[test]
    fn borrowed_dispatch_uses_its_engine_inside_a_foreign_lexical_scope() {
        let _serial = crate::test_sync::lock();
        let own = ScriptEngine::new().unwrap();
        let foreign = ScriptEngine::new().unwrap();
        own.set_priority_default("Shared", 4);
        foreign.set_priority_default("Shared", 19);
        let state = RefCell::new(ScriptContext::new());
        let mut dispatch = ScriptActionDispatcher::for_engine(&own, &state);
        foreign.with_active(|| {
            dispatch
                .execute_action(&default_action("Shared", 13))
                .unwrap();
            assert_eq!(
                with_script_engine_ref(|engine| engine
                    .get_attack_info("Shared")
                    .unwrap()
                    .default_priority),
                Some(19)
            );
        });
        assert_eq!(own.get_attack_info("Shared").unwrap().default_priority, 13);
        assert_eq!(
            foreign.get_attack_info("Shared").unwrap().default_priority,
            19
        );
    }

    #[test]
    fn borrowed_condition_uses_its_engine_inside_a_foreign_lexical_scope() {
        let _serial = crate::test_sync::lock();
        let own = ScriptEngine::new().unwrap();
        let foreign = ScriptEngine::new().unwrap();
        own.set_flag("Shared", true);
        foreign.set_flag("Shared", false);
        let state = RefCell::new(ScriptContext::new());
        let mut evaluator = ScriptConditionEvaluator::for_engine(&own, &state);
        let mut condition = Condition::new(ConditionType::Flag);
        condition
            .add_parameter(Parameter::with_string(ParameterType::Flag, "Shared".into()))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .unwrap();
        foreign.with_active(|| {
            assert_eq!(
                evaluator.evaluate_condition(&mut condition).unwrap(),
                ScriptConditionResult::True
            );
        });
        assert!(!foreign.get_flag("Shared").unwrap().value);
    }

    #[test]
    fn borrowed_operation_context_shares_state_and_construction_is_inert() {
        let _serial = crate::test_sync::lock();
        let engine = ScriptEngine::new().unwrap();
        let state = RefCell::new(ScriptContext::new());
        state.borrow_mut().current_frame = 47;
        state.borrow_mut().suppress_new_windows = true;
        assert!(!crate::scripting::engine::is_script_engine_active());
        let dispatcher = ScriptActionDispatcher::for_engine(&engine, &state);
        let evaluator = ScriptConditionEvaluator::for_engine(&engine, &state);
        assert!(!crate::scripting::engine::is_script_engine_active());
        dispatcher
            .context
            .with_state_mut(|state| state.suppress_new_windows = false);
        assert_eq!(
            evaluator
                .context
                .with_state(|state| (state.current_frame, state.suppress_new_windows)),
            (47, false)
        );
    }
}
