use gamelogic::scripting::core::{
    Parameter, ParameterType, ScriptAction, ScriptActionType, THIS_TEAM,
};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver};
use gamelogic::scripting::executor::ScriptContext;
use gamelogic::{GameLogicError, GameLogicResult};

#[derive(Default)]
struct Owner {
    calls: Vec<(String, Option<String>, Option<u32>, Option<u32>)>,
    reject: bool,
}

impl ScriptExecutionDriver for Owner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }

    fn team_wander(
        &mut self,
        team: &str,
        path: Option<&str>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        self.calls
            .push((team.into(), path.map(str::to_owned), calling, condition));
        Some(if self.reject {
            Err(GameLogicError::ModuleError("owner rejected wander".into()))
        } else {
            Ok(())
        })
    }
}

fn action(path: Option<&str>) -> ScriptAction {
    let mut action = ScriptAction::new(if path.is_some() {
        ScriptActionType::TeamWander
    } else {
        ScriptActionType::TeamWanderInPlace
    });
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    if let Some(path) = path {
        action
            .add_parameter(Parameter::with_string(
                ParameterType::WaypointPath,
                path.into(),
            ))
            .unwrap();
    }
    action
}

#[test]
fn wander_dispatches_synchronously_to_the_supplied_owner() {
    let mut engine = ScriptEngine::new().unwrap();
    let saved = engine.set_external_eval_context(None, Some(19));
    let mut first = Owner::default();
    let mut peer = Owner::default();
    engine
        .friend_execute_action_with_driver_checked(
            &action(Some("PathA")),
            Some(31),
            ScriptContext::at_frame(31),
            &mut first,
        )
        .unwrap();
    assert_eq!(
        first.calls,
        vec![(THIS_TEAM.into(), Some("PathA".into()), Some(31), Some(19))]
    );
    assert!(peer.calls.is_empty());
    engine
        .friend_execute_action_with_driver_checked(
            &action(None),
            Some(42),
            ScriptContext::at_frame(32),
            &mut peer,
        )
        .unwrap();
    assert_eq!(
        peer.calls,
        vec![(THIS_TEAM.into(), None, Some(42), Some(19))]
    );
    assert_eq!(first.calls.len(), 1);
    assert_eq!(engine.get_calling_team_id(), None);
    engine.restore_external_eval_context(saved);
}

#[test]
fn wander_owner_error_does_not_fall_back_to_another_world() {
    let engine = ScriptEngine::new().unwrap();
    let mut owner = Owner {
        reject: true,
        ..Default::default()
    };
    let error = engine.friend_execute_action_with_driver_checked(
        &action(None),
        None,
        ScriptContext::at_frame(0),
        &mut owner,
    );
    assert!(
        error
            .unwrap_err()
            .to_string()
            .contains("owner rejected wander")
    );
    assert_eq!(owner.calls.len(), 1);
}
