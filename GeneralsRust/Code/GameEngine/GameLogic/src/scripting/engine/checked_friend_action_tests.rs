//! Actual checked script action chain errors and calling-context restoration.
use super::*;

fn create(template: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::CreateObject);
    for parameter in [
        Parameter::with_string(ParameterType::ObjectType, template.into()),
        Parameter::with_string(ParameterType::Team, "CheckedTeam".into()),
        Parameter::with_coord(
            ParameterType::Coord3D,
            crate::common::Coord3D::new(0., 0., 0.),
        ),
        Parameter::with_real(ParameterType::Angle, 0.),
    ] {
        action.add_parameter(parameter).unwrap();
    }
    action
}
#[derive(Default)]
struct Owner {
    fail: bool,
    seen: Vec<String>,
}
impl ScriptExecutionDriver for Owner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn team_current_player(&self, id: TeamID) -> ScriptOwnerQuery<Option<String>> {
        ScriptOwnerQuery::Present(Some(format!("owner-{id}")))
    }
    fn create_object(
        &mut self,
        request: ScriptObjectCreation<'_>,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<GameLogicResult<()>> {
        assert_eq!(calling, Some(3));
        assert_eq!(condition, Some(77));
        self.seen.push(request.template.to_owned());
        Some(if self.fail {
            Err(GameLogicError::Configuration(
                "receiving constructor rejected".into(),
            ))
        } else {
            Ok(())
        })
    }
}
#[test]
fn checked_friend_action_returns_constructor_failure_before_next_action_and_restores_context() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_external_eval_context(Some("before".into()), Some(77));
    let mut first = create("First");
    first.next_action = Some(Box::new(create("After")));
    let mut owner = Owner {
        fail: true,
        ..Default::default()
    };
    let error = engine
        .friend_execute_action_with_driver_checked(
            &first,
            Some(3),
            crate::scripting::executor::ScriptContext::new(),
            &mut owner,
        )
        .unwrap_err();
    assert!(error.to_string().contains("receiving constructor rejected"));
    assert_eq!(owner.seen, ["First"]);
    assert_eq!(engine.get_calling_team_id(), None);
    assert_eq!(engine.get_current_player_name().as_deref(), Some("before"));
    assert_eq!(engine.get_condition_team_id(), Some(77));
    owner.fail = false;
    owner.seen.clear();
    engine
        .friend_execute_action_with_driver_checked(
            &first,
            Some(3),
            crate::scripting::executor::ScriptContext::new(),
            &mut owner,
        )
        .unwrap();
    assert_eq!(owner.seen, ["First", "After"]);
    assert_eq!(engine.get_calling_team_id(), None);
    assert_eq!(engine.get_current_player_name().as_deref(), Some("before"));
    assert_eq!(engine.get_condition_team_id(), Some(77));
}
