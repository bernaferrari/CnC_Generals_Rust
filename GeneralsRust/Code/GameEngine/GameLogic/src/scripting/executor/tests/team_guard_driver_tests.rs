//! CPP ScriptActions.cpp:1882-1900: TeamGuard selects one named team's AI.
//! Main owns the real GuardPosition/SCRIPT/formation/locomotor assertions;
//! these tests prove the executor selects that borrowed owner before lookup.
use super::*;
use crate::scripting::core::THIS_TEAM;
use crate::scripting::engine::ScriptExecutionDriver;

type GuardCall = (String, Option<String>, Option<String>);

#[derive(Default)]
struct GuardOwner {
    calls: Vec<GuardCall>,
    reject: bool,
    completed_actions: usize,
}

impl ScriptExecutionDriver for GuardOwner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        self.completed_actions += 1;
        Ok(())
    }

    fn team_guard(
        &mut self,
        team: &str,
        calling_team: Option<&str>,
        condition_team: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        self.calls.push((
            team.into(),
            calling_team.map(str::to_owned),
            condition_team.map(str::to_owned),
        ));
        Some(if self.reject {
            Err(GameLogicError::ModuleError(
                "team guard owner rejected command".into(),
            ))
        } else {
            // This selected owner deliberately admits missing teams as the
            // original no-op; it never delegates them to another registry.
            Ok(())
        })
    }
}

fn guard_action(team: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    action
}

#[test]
fn team_guard_dispatches_raw_token_and_condition_from_borrowed_engine() {
    let mut engine = ScriptEngine::new().unwrap();
    let saved = engine.set_external_eval_context(None, Some("OwnedConditionTeam".into()));
    let context = std::cell::RefCell::new(ScriptContext::at_frame(19));
    let mut owner = GuardOwner::default();
    {
        let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
        for team in [THIS_TEAM, "LiteralGuardTeam"] {
            assert_eq!(
                dispatcher
                    .execute_action_with_driver(&guard_action(team), &mut owner)
                    .unwrap(),
                ScriptActionResult::Success
            );
        }
    }
    assert_eq!(
        owner.calls,
        vec![
            (THIS_TEAM.into(), None, Some("OwnedConditionTeam".into())),
            (
                "LiteralGuardTeam".into(),
                None,
                Some("OwnedConditionTeam".into())
            ),
        ],
        "the owner must receive the raw token before ambient THIS_TEAM resolution"
    );
    engine.restore_external_eval_context(saved);
    assert_eq!(engine.get_condition_team_name(), None);
}

#[test]
fn team_guard_friend_action_passes_both_contexts_and_restores_calling_team() {
    let mut engine = ScriptEngine::new().unwrap();
    let saved = engine.set_external_eval_context(None, Some("OwnedConditionTeam".into()));
    let mut owner = GuardOwner::default();
    engine.friend_execute_action_with_driver(
        &guard_action(THIS_TEAM),
        Some("OwnedCallingTeam"),
        ScriptContext::at_frame(31),
        &mut owner,
    );
    assert_eq!(
        owner.calls,
        vec![(
            THIS_TEAM.into(),
            Some("OwnedCallingTeam".into()),
            Some("OwnedConditionTeam".into())
        )],
        "calling and condition teams remain distinct until owner resolution"
    );
    assert_eq!(owner.completed_actions, 1);
    assert_eq!(engine.get_calling_team_name(), None);
    assert_eq!(
        engine.get_condition_team_name().as_deref(),
        Some("OwnedConditionTeam")
    );
    engine.restore_external_eval_context(saved);
}

#[test]
fn team_guard_missing_owner_result_is_authoritative_success() {
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = GuardOwner::default();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&guard_action("AbsentOwnedTeam"), &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(owner.calls, vec![("AbsentOwnedTeam".into(), None, None)]);
}

#[test]
fn team_guard_owner_error_is_authoritative_without_standalone_retry() {
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = GuardOwner {
        reject: true,
        ..GuardOwner::default()
    };
    assert!(matches!(
        dispatcher.execute_action_with_driver(&guard_action("RejectedOwnedTeam"), &mut owner),
        Err(ScriptError::ExecutionFailed(message)) if message.contains("team guard owner rejected command")
    ));
    assert_eq!(owner.calls, vec![("RejectedOwnedTeam".into(), None, None)]);
}

#[test]
fn team_guard_requires_team_parameter_before_owner_dispatch() {
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = GuardOwner::default();
    assert!(
        dispatcher
            .execute_action_with_driver(&ScriptAction::new(ScriptActionType::TeamGuard), &mut owner)
            .is_err()
    );
    assert!(owner.calls.is_empty());
}

#[test]
fn default_team_guard_driver_keeps_standalone_adapter_available() {
    struct UnavailableOwner;
    impl ScriptExecutionDriver for UnavailableOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
    }
    assert!(
        UnavailableOwner
            .team_guard(THIS_TEAM, Some("Calling"), Some("Condition"))
            .is_none()
    );
    // Do not execute the standalone branch: its separate registry fixtures
    // must not be installed merely to verify the default selection contract.
}
