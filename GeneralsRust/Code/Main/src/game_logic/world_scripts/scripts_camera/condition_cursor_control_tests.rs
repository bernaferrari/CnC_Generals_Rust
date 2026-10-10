//! OLD/NEW controls: actual Main Guard effects with controlled synchronous
//! metadata callbacks. TEAM_DELETE deletes members, not these team records.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_TEAM,
};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver, ScriptOwnerQuery};

pub(super) fn admit(world: &GameLogic, name: &str, id: u32, member: Option<ObjectId>) {
    let mut factory = world.team_factory.lock().unwrap();
    if factory.find_team_prototype(name).is_none() {
        factory.replace_team_prototype(gamelogic::team::TeamPrototype::new(name.into()));
    }
    let team = factory
        .restore_owned_team_instance(name, id, Some(1))
        .unwrap();
    if let Some(member) = member {
        team.write().unwrap().restore_owned_members(&[member.0]);
    }
}
fn engine() -> ScriptEngine {
    let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    let mut count = ScriptAction::new(ScriptActionType::IncrementCounter);
    count
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    count
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "Visited".into(),
        ))
        .unwrap();
    action.next_action = Some(Box::new(count));
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.condition_team_name = "Copies".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("Visited", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}
#[derive(Clone, Copy)]
enum Mutation {
    RemoveNext,
    AddHead,
    RemoveCurrent,
}
struct Callback<'a> {
    world: &'a mut GameLogic,
    mutation: Mutation,
    changed: bool,
}
impl ScriptExecutionDriver for Callback<'_> {
    fn team_instances(&self, name: &str) -> ScriptOwnerQuery<Vec<u32>> {
        let factory = self.world.team_factory.lock().unwrap();
        if factory.find_team_prototype(name).is_none() {
            return ScriptOwnerQuery::Missing;
        }
        ScriptOwnerQuery::Present(
            factory
                .find_team_instances(name)
                .iter()
                .map(|team| team.read().unwrap().get_id())
                .collect(),
        )
    }
    fn team_guard(
        &mut self,
        name: &str,
        calling: Option<u32>,
        condition: Option<u32>,
    ) -> Option<gamelogic::GameLogicResult<()>> {
        HostScriptExecutionDriver::new(self.world).team_guard(name, calling, condition)
    }
    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        HostScriptExecutionDriver::new(self.world).after_action()?;
        if !self.changed {
            self.changed = true;
            match self.mutation {
                Mutation::AddHead => admit(self.world, "Copies", 40, None),
                Mutation::RemoveNext | Mutation::RemoveCurrent => {
                    let id = if matches!(self.mutation, Mutation::RemoveNext) {
                        20
                    } else {
                        30
                    };
                    let mut factory = self.world.team_factory.lock().unwrap();
                    let deletion = factory.prepare_host_team_deletion(id).unwrap();
                    assert!(factory.finalize_host_team_deletion(deletion));
                }
            }
        }
        Ok(())
    }
}
fn execute(mutation: Mutation) -> (GameLogic, ObjectId, ObjectId, i32) {
    let (mut world, first, second) = world();
    admit(&world, "Copies", 10, Some(first));
    admit(&world, "Copies", 20, Some(second));
    admit(&world, "Copies", 30, None);
    let engine = engine();
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(0),
            &mut Callback {
                world: &mut world,
                mutation,
                changed: false,
            },
        )
        .unwrap();
    assert_eq!(engine.get_condition_team_id(), None);
    let count = engine.get_counter("Visited").unwrap().value;
    (world, first, second, count)
}
#[test]
fn next_removed_in_callback_is_skipped_before_its_conditions_and_guard() {
    let (world, first, second, count) = execute(Mutation::RemoveNext);
    assert_eq!(
        count, 2,
        "deleted next instance must not execute the counter"
    );
    assert_eq!(
        world.host_object(first).unwrap().ai_state,
        AIState::GuardingArea
    );
    assert_eq!(world.host_object(second).unwrap().guard_position, None);
}
#[test]
fn new_head_callback_keeps_original_forward_walk_calibrated() {
    let (world, first, second, count) = execute(Mutation::AddHead);
    assert_eq!(count, 3);
    for id in [first, second] {
        assert_eq!(
            world.host_object(id).unwrap().ai_state,
            AIState::GuardingArea
        );
    }
}
#[test]
fn removed_current_ends_safe_walk_without_dispatching_siblings() {
    // Defined Rust safety outcome; C++ advance would dereference freed memory.
    let (world, first, second, count) = execute(Mutation::RemoveCurrent);
    assert_eq!(count, 1);
    for id in [first, second] {
        assert_eq!(world.host_object(id).unwrap().guard_position, None);
    }
}
