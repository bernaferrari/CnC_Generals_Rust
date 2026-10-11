//! CPP ScriptConditions228/340/912 and Team1705/1751: owner-held presence.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, THIS_TEAM,
};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver, ScriptOwnerQuery};
use gamelogic::team::{TeamStateObservation, TeamStateStep};

fn roster(owner: &GameLogic, member: Option<ObjectId>) -> u32 {
    let mut factory = owner.team_factory.lock().unwrap();
    if factory.find_team_prototype("PresenceTeam").is_none() {
        factory
            .init_team("PresenceTeam".into(), "NamedOwner".into(), false, None)
            .unwrap();
    }
    let team = factory.create_inactive_team("PresenceTeam").unwrap();
    let mut team = team.write().unwrap();
    team.set_active();
    if let Some(id) = member {
        team.add_member(id.0);
    }
    team.get_id()
}
fn evaluate(owner: &mut GameLogic, kind: ConditionType, name: &str, context: Option<u32>) -> bool {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Team, name.into()))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut script = Script::new();
    script.condition = Some(Box::new(branch));
    let engine = ScriptEngine::new().unwrap();
    engine.evaluate_conditions_with_driver(
        &mut script,
        context,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(owner.frame),
        &mut HostScriptExecutionDriver::with_interpreter(owner, &engine),
    )
}
#[test]
fn team_presence_reads_exact_owned_roster_creation_phase_and_inert_unit_difference() {
    let (mut owner, unit, _) = world();
    let (mut peer, peer_unit, _) = world();
    assert_eq!(unit, peer_unit);
    let id = roster(&owner, Some(unit));
    let peer_id = roster(&peer, None);
    assert_eq!(id, peer_id);
    assert!(!evaluate(
        &mut owner,
        ConditionType::TeamDestroyed,
        "PresenceTeam",
        None
    ));
    assert!(evaluate(
        &mut peer,
        ConditionType::TeamDestroyed,
        "PresenceTeam",
        None
    ));
    assert!(evaluate(
        &mut owner,
        ConditionType::TeamCreated,
        "PresenceTeam",
        None
    ));
    // Actual owned phase clears Created before OnCreate runs; no fielded latch substitute.
    let team = owner
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(id)
        .unwrap();
    team.write()
        .unwrap()
        .advance_owned_state(&mut TeamStateStep::Start, TeamStateObservation::default());
    assert!(!evaluate(
        &mut owner,
        ConditionType::TeamCreated,
        "PresenceTeam",
        None
    ));
    assert!(evaluate(
        &mut peer,
        ConditionType::TeamCreated,
        "PresenceTeam",
        None
    ));
    owner
        .host_object_mut(unit)
        .unwrap()
        .template_mut()
        .add_kind_of(KindOf::Inert);
    assert!(evaluate(
        &mut owner,
        ConditionType::TeamDestroyed,
        "PresenceTeam",
        None
    ));
    assert!(evaluate(
        &mut owner,
        ConditionType::TeamHasUnits,
        "PresenceTeam",
        None
    ));
    owner
        .host_object_mut(unit)
        .unwrap()
        .template_mut()
        .add_kind_of(KindOf::Structure);
    assert!(!evaluate(
        &mut owner,
        ConditionType::TeamHasUnits,
        "PresenceTeam",
        None
    ));
    owner.host_object_mut(unit).unwrap().status.destroyed = true;
    assert!(!evaluate(
        &mut owner,
        ConditionType::TeamHasUnits,
        "PresenceTeam",
        None
    ));
    for kind in [
        ConditionType::TeamCreated,
        ConditionType::TeamDestroyed,
        ConditionType::TeamHasUnits,
    ] {
        assert!(!evaluate(&mut owner, kind, "AbsentPresence", None));
    }
}
#[test]
fn team_has_units_preserves_contextual_instance_before_prototype_aggregation() {
    let (mut owner, unit, _) = world();
    let empty = roster(&owner, None);
    let populated = roster(&owner, Some(unit));
    // Without a context, every instance is considered. With a same-name
    // calling team, its exact roster wins even when another instance has units.
    assert!(evaluate(
        &mut owner,
        ConditionType::TeamHasUnits,
        "PresenceTeam",
        None
    ));
    assert!(!evaluate(
        &mut owner,
        ConditionType::TeamHasUnits,
        "PresenceTeam",
        Some(empty)
    ));
    assert!(!evaluate(
        &mut owner,
        ConditionType::TeamHasUnits,
        THIS_TEAM,
        Some(empty)
    ));
    assert!(evaluate(
        &mut owner,
        ConditionType::TeamHasUnits,
        THIS_TEAM,
        Some(populated)
    ));
    assert!(!evaluate(
        &mut owner,
        ConditionType::TeamHasUnits,
        THIS_TEAM,
        None
    ));
    let driver = HostScriptExecutionDriver::new(&mut owner);
    assert_eq!(
        driver.team_has_units("PresenceTeam", Some(empty), Some(populated)),
        ScriptOwnerQuery::Present(false)
    );
}
