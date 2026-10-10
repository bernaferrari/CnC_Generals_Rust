//! Same source OLD/NEW witness for exact C++ condition-team iteration.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_TEAM,
};
use gamelogic::scripting::engine::ScriptEngine;

fn admit(world: &GameLogic, name: &str, member: ObjectId, owner: u32) -> u32 {
    let mut factory = world.team_factory.lock().unwrap();
    if factory.find_team_prototype(name).is_none() {
        factory
            .init_team(name.into(), "".into(), false, None)
            .unwrap();
    }
    let team = factory.create_inactive_team(name).unwrap();
    let mut team = team.write().unwrap();
    team.add_member(member.0);
    team.set_controlling_player_id(Some(owner));
    team.get_id()
}
fn guard(name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, name.into()))
        .unwrap();
    action
}
fn increment(name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(ParameterType::Counter, name.into()))
        .unwrap();
    action
}
fn update(engine: &ScriptEngine, world: &mut GameLogic) {
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world),
        )
        .unwrap();
}

#[test]
fn condition_loop_guards_both_same_name_instances_even_after_oneshot_success() {
    let (mut world, first, second) = world();
    let first_id = admit(&world, "SiblingRoster", first, 1);
    let second_id = admit(&world, "SiblingRoster", second, 2);
    assert_ne!(first_id, second_id);
    // Names on objects are deliberately uninformative; actual rosters decide.
    world.host_object_mut(first).unwrap().team_instance_name = "Wrong".into();
    world.host_object_mut(second).unwrap().team_instance_name = "Wrong".into();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "VisitSiblingInstances".into();
    script.condition_team_name = "SiblingRoster".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    let mut action = guard(THIS_TEAM);
    action.next_action = Some(Box::new(increment("Visited")));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("Visited", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    update(&engine, &mut world);
    assert_eq!(engine.get_counter("Visited").unwrap().value, 2);
    for id in [first, second] {
        let unit = world.host_object(id).unwrap();
        assert_eq!(unit.ai_state, AIState::GuardingArea);
        assert_eq!(unit.guard_position, Some(unit.get_position()));
    }
    assert_eq!(engine.get_condition_team_name(), None);
    update(&engine, &mut world);
    assert_eq!(engine.get_counter("Visited").unwrap().value, 2);
}

#[test]
fn empty_authored_prototype_evaluates_once_with_null_condition_team() {
    let (mut world, first, second) = world();
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team("EmptyRoster".into(), "".into(), false, None)
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "EmptyConditionRoster".into();
    script.condition_team_name = "EmptyRoster".into();
    script.condition = Some(Box::new(branch));
    let mut action = guard(THIS_TEAM);
    action.next_action = Some(Box::new(increment("Visited")));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("Visited", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    update(&engine, &mut world);
    assert_eq!(engine.get_counter("Visited").unwrap().value, 1);
    assert_eq!(engine.get_condition_team_name(), None);
    assert_eq!(world.host_object(first).unwrap().guard_position, None);
    assert_eq!(world.host_object(second).unwrap().guard_position, None);
}
