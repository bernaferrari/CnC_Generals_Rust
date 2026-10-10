//! Synchronous script dispatch borrows its engine and operation state.
//! C++ ScriptEngine.cpp:7558-7648 runs conditions/actions immediately on one engine.
use super::*;
use crate::scripting::engine::ScriptEngine;
use std::cell::RefCell;

/// Both dependencies belong to the operation's driving engine. Neither is
/// resolved from ambient state or retained behind a synchronization handle.
#[derive(Clone, Copy)]
pub(super) struct ExecutionContext<'engine> {
    engine: &'engine ScriptEngine,
    state: &'engine RefCell<ScriptContext>,
}

impl<'engine> ExecutionContext<'engine> {
    pub(super) fn borrowed(
        engine: &'engine ScriptEngine,
        state: &'engine RefCell<ScriptContext>,
    ) -> Self {
        Self { engine, state }
    }

    /// Direct, callback-free operations use the driving engine without
    /// publishing an active slot or treating a borrow conflict as an absent
    /// gameplay event.
    pub(super) fn borrowed_engine(&self) -> &'engine ScriptEngine {
        self.engine
    }

    pub(super) fn with_state<R>(&self, f: impl FnOnce(&ScriptContext) -> R) -> R {
        f(&self.state.borrow())
    }

    pub(super) fn with_state_mut<R>(&self, f: impl FnOnce(&mut ScriptContext) -> R) -> R {
        f(&mut self.state.borrow_mut())
    }

    /// Public operations establish this exact owner for retained callbacks.
    /// Copies of this context borrow the same state; no storage is cloned.
    pub(super) fn with_active_scope<R>(&self, f: impl FnOnce() -> R) -> R {
        self.engine.with_active(f)
    }

    pub(super) fn with_engine_ref<R>(&self, f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
        self.engine.with_execution_read(f)
    }

    pub(super) fn with_engine_mut<R>(&self, f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
        // Existing lexical scope is only a temporary adapter for callbacks
        // still using ambient helpers. It never chooses the borrowed owner.
        Some(self.engine.with_active(|| f(self.engine)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripting::engine::{ScriptEngine, with_script_engine_ref};

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
        let own = ScriptEngine::new().unwrap();
        let foreign = ScriptEngine::new().unwrap();
        own.set_priority_default("Shared", 4);
        foreign.set_priority_default("Shared", 19);
        let state = RefCell::new(ScriptContext::at_frame(0));
        let mut dispatch = ScriptActionDispatcher::new(&own, &state);
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
        let own = ScriptEngine::new().unwrap();
        let foreign = ScriptEngine::new().unwrap();
        own.set_flag("Shared", true);
        foreign.set_flag("Shared", false);
        let state = RefCell::new(ScriptContext::at_frame(0));
        let mut evaluator = ScriptConditionEvaluator::new(&own, &state);
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
        let engine = ScriptEngine::new().unwrap();
        let state = RefCell::new(ScriptContext::at_frame(47));
        state.borrow_mut().suppress_new_windows = true;
        assert!(!crate::scripting::engine::is_script_engine_active());
        let dispatcher = ScriptActionDispatcher::new(&engine, &state);
        let evaluator = ScriptConditionEvaluator::new(&engine, &state);
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
