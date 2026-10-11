//! The real ScriptEngine walk selects the borrowed world for EVA control.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::helpers::EvaEvent;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

#[test]
fn eva_announcement_owner_tests_actual_script_toggles_only_driving_world() {
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    a.add_player(Player::new(0, Team::USA, "SamePlayer", true));
    b.add_player(Player::new(0, Team::USA, "SamePlayer", true));
    a.eva_announcements.request(EvaEvent::GeneralLevelUp);
    b.eva_announcements.request(EvaEvent::UpgradeComplete);
    let mut action = ScriptAction::new(ScriptActionType::EvaSetEnabledDisabled);
    action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 0))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnedEvaToggle".into();
    script.is_one_shot = false;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(a.get_frame()),
            &mut HostScriptExecutionDriver::new(&mut a),
        )
        .unwrap();
    assert!(!a.eva_enabled());
    assert!(b.eva_enabled());
    let admitted = a.eva_announcements.take_delivery();
    assert_eq!(
        admitted.operations,
        vec![
            crate::game_logic::host_eva_log::EvaOperation::Request(EvaEvent::GeneralLevelUp),
            crate::game_logic::host_eva_log::EvaOperation::SetEnabled(false)
        ]
    );
    assert_eq!(b.take_eva_requests(), vec![EvaEvent::UpgradeComplete]);
}
