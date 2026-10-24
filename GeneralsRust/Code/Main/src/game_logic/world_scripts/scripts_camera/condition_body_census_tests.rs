//! Actual borrowed script walks for InitialHealth and player object census.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

impl GameLogic {
    pub(crate) fn owned_script_condition_for_test(&mut self, condition: Condition) -> bool {
        let mut branch = OrCondition::new();
        branch.set_first_and_condition(Some(Box::new(condition)));
        let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "OwnedConditionObserved".into(),
            ))
            .unwrap();
        let mut script = Script::new();
        script.script_name = "OwnedBodyCensusCondition".into();
        script.is_one_shot = false;
        script.condition = Some(Box::new(branch));
        script.action = Some(Box::new(action));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        let mut engine = ScriptEngine::new().unwrap();
        engine.set_counter("OwnedConditionObserved", 0).unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        engine
            .update_with_driver(
                gamelogic::scripting::executor::ScriptContext::at_frame(self.frame),
                &mut HostScriptExecutionDriver::new(self),
            )
            .unwrap();
        engine.get_counter("OwnedConditionObserved").unwrap().value == 1
    }
}

fn health(name: &str, threshold: i32) -> Condition {
    let mut condition = Condition::new(ConditionType::UnitHealth);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, threshold))
        .unwrap();
    condition
}

fn destroyed(name: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::PlayerAllDestroyed);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, name.into()))
        .unwrap();
    condition
}

fn world(current: f32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(
        Player::new(0, Team::USA, "OwnedBodyPlayer", true).with_script_identity("OwnedBodyPlayer"),
    );
    let mut template = ThingTemplate::new("OwnedBodyUnit");
    template.set_health(100.0).add_kind_of(KindOf::Infantry);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("OwnedBodyUnit", 0, Vec3::ZERO)
        .unwrap();
    let object = world.host_object_mut(id).unwrap();
    object.name = "OwnedBodyName".into();
    object.initial_health = 80.0;
    object.health.current = current;
    world.initialize_receiving_named_cache_for_fixture();
    (world, id)
}

#[test]
fn owned_condition_health_and_census_ignore_foreign_same_ids() {
    let (mut source, id) = world(80.0);
    let (mut foreign, foreign_id) = world(1.0);
    assert_eq!(id, foreign_id);
    foreign
        .host_object_mut(foreign_id)
        .unwrap()
        .status
        .effectively_dead = true;
    assert!(source.owned_script_condition_for_test(health("OwnedBodyName", 100)));
    assert!(!foreign.owned_script_condition_for_test(health("OwnedBodyName", 100)));
    assert!(!source.owned_script_condition_for_test(destroyed("OwnedBodyPlayer")));
    assert!(foreign.owned_script_condition_for_test(destroyed("OwnedBodyPlayer")));
    source.host_object_mut(id).unwrap().health.current = 79.0;
    assert!(!source.owned_script_condition_for_test(health("OwnedBodyName", 100)));
    assert!(!source.owned_script_condition_for_test(health("AbsentOwnedUnit", 1)));
    assert!(source.owned_script_condition_for_test(destroyed("AbsentOwnedPlayer")));
}
