use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::scripting::executor::ScriptContext;
use gamelogic::{GameLogicError, GameLogicResult};
use generals_sim::scripting::driver::ScriptExecutionDriver;
use generals_sim::scripting::program::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use generals_sim::scripting::world::ScriptOwnerQuery;

struct SideDriver {
    fail: bool,
    team_phase_player: Option<Option<String>>,
}

impl ScriptExecutionDriver<ScriptEngine> for SideDriver {
    fn side_script_current_player(&self, side: usize) -> ScriptOwnerQuery<Option<String>> {
        ScriptOwnerQuery::Present((side == 1).then(|| "AuthoredSide".into()))
    }

    fn after_action(&mut self) -> GameLogicResult<()> {
        if self.fail {
            return Err(GameLogicError::Configuration(
                "side callback rejected".into(),
            ));
        }
        Ok(())
    }

    fn update_team_states(&mut self, engine: &ScriptEngine) -> Option<GameLogicResult<()>> {
        self.team_phase_player = Some(engine.get_current_player_name());
        Some(Ok(()))
    }
}

fn install_side_script(engine: &ScriptEngine) {
    let mut action = ScriptAction::new(ScriptActionType::SetFlag);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Flag,
            "RanSide".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "SideScope".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.append_action(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    engine
        .set_script_list_for_player(1, Some(Box::new(list)))
        .unwrap();
}

#[test]
fn failed_side_callback_restores_the_calling_player_before_retry() {
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_external_eval_context(Some("Outer".into()), Some(77));
    install_side_script(&engine);
    let mut owner = SideDriver {
        fail: true,
        team_phase_player: None,
    };
    assert!(matches!(
        engine.update_with_driver(ScriptContext::at_frame(5), &mut owner),
        Err(GameLogicError::Configuration(message)) if message == "side callback rejected"
    ));
    assert!(engine.get_flag("RanSide").unwrap().value);
    assert_eq!(owner.team_phase_player, None);
    assert_eq!(engine.get_current_player_name().as_deref(), Some("Outer"));
    assert_eq!(engine.get_condition_team_id(), Some(77));
    assert!(engine.capture_side_script_continuation().is_ok());

    owner.fail = false;
    install_side_script(&engine);
    engine
        .update_with_driver(ScriptContext::at_frame(6), &mut owner)
        .unwrap();
    assert_eq!(owner.team_phase_player, Some(None));
    assert_eq!(engine.get_current_player_name(), None);
    assert_eq!(engine.get_condition_team_id(), Some(77));
}
