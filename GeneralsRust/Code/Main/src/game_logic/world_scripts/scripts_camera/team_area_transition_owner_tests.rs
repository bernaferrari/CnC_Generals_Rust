//! Team transition markers belong to the resolved team until the script update clears them.
use super::super::named_command_test_support::world;
use super::*;
use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};

fn condition(team: &str, area: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::TeamEnteredAreaPartially);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            area.into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::SurfacesAllowed, 1))
        .unwrap();
    condition
}

#[test]
fn team_event_marker_survives_roster_removal_until_script_state_phase() {
    let (mut world, unit, _) = world();
    let team_name = "TransitionRoster";
    let team_id = {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team(team_name.into(), "".into(), false, None)
            .unwrap();
        let team = factory.create_inactive_team(team_name).unwrap();
        let mut team = team.write().unwrap();
        team.add_member(unit.0);
        team.set_controlling_player_id(Some(1));
        team.get_id()
    };
    {
        let object = world.host_object_mut(unit).unwrap();
        object.team_instance_name = team_name.into();
        object.team_instance_id = Some(team_id);
    }
    world.frame = 20;
    let area = gamelogic::polygon_trigger::PolygonTrigger::new(
        1902,
        "RosterEdgeArea".into(),
        vec![
            gamelogic::common::ICoord3D::new(0, 0, 0),
            gamelogic::common::ICoord3D::new(20, 0, 0),
            gamelogic::common::ICoord3D::new(20, 20, 0),
            gamelogic::common::ICoord3D::new(0, 20, 0),
        ],
    );
    {
        let mut triggers = world.host_trigger_world.lock().unwrap();
        triggers.set_trigger_areas(&[area]);
        triggers.set_current_frame(20);
    }
    world.inject_host_script_query_snapshot();
    world.set_fixture_object_position(unit, Vec3::new(2.0, 0.0, 2.0));
    world.inject_host_script_query_snapshot();
    assert!(world
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(team_id)
        .unwrap()
        .read()
        .unwrap()
        .did_enter_or_exit());

    // Transfer/removal occurs after the event callback but before the script
    // condition observes it. C++ retains Team::m_enteredOrExited through this
    // walk even though the current roster no longer contains its event source.
    world
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(team_id)
        .unwrap()
        .write()
        .unwrap()
        .remove_member(unit.0);
    assert!(world
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(team_id)
        .unwrap()
        .read()
        .unwrap()
        .did_enter_or_exit());
    assert!(!world.owned_script_condition_for_test(condition(team_name, "RosterEdgeArea")));
    assert!(
        !world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(team_id)
            .unwrap()
            .read()
            .unwrap()
            .did_enter_or_exit(),
        "the owning ScriptEngine team-state phase clears the marker after observation"
    );
}
