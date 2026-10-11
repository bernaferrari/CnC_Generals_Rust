//! CPP ScriptActions1336–1359 / 1767–1865 against the actual driving owner.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::ScriptEngine;

fn roster(owner: &GameLogic, unit: ObjectId) -> u32 {
    let mut factory = owner.team_factory.lock().unwrap();
    factory
        .init_team("FollowTeam".into(), "NamedOwner".into(), false, None)
        .unwrap();
    let team = factory.create_team("FollowTeam").unwrap();
    let mut team = team.write().unwrap();
    team.set_controlling_player_id_for_owned_members(Some(1));
    team.add_member(unit.0);
    team.get_id()
}
fn issue(owner: &mut GameLogic, team: u32, action: &ScriptAction) {
    let engine = ScriptEngine::new().unwrap();
    engine.friend_execute_action_with_driver(
        action,
        Some(team),
        gamelogic::scripting::executor::ScriptContext::at_frame(owner.frame),
        &mut HostScriptExecutionDriver::new(owner),
    );
}
fn action(kind: ScriptActionType, first: &str, second: &str) -> ScriptAction {
    let mut action = ScriptAction::new(kind);
    for token in [first, second] {
        action
            .add_parameter(Parameter::with_string(ParameterType::Team, token.into()))
            .unwrap();
    }
    action
}
#[test]
fn my_inner_perimeter_qualification_uses_owned_player_start_and_polygon() {
    let (mut owner, unit, target) = world();
    let (peer, peer_unit, _) = world();
    let team = roster(&owner, unit);
    owner.players.get_mut(&1).unwrap().start_position = 0;
    owner
        .host_trigger_world
        .lock()
        .unwrap()
        .set_trigger_areas(&[gamelogic::polygon_trigger::PolygonTrigger::new(
            9301,
            "InnerPerimeter1".into(),
            vec![
                gamelogic::common::ICoord3D::new(60, 60, 0),
                gamelogic::common::ICoord3D::new(90, 60, 0),
                gamelogic::common::ICoord3D::new(90, 90, 0),
                gamelogic::common::ICoord3D::new(60, 90, 0),
            ],
        )]);
    let order = action(
        ScriptActionType::NamedAttackArea,
        "NamedUnit",
        "[Skirmish]MyInnerPerimeter",
    );
    issue(&mut owner, team, &order);
    assert_eq!(owner.host_object(unit).unwrap().target, Some(target));
    assert_eq!(owner.host_object(unit).unwrap().formation_id, 0);
    assert_eq!(
        owner.host_object(unit).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(peer.host_object(peer_unit).unwrap().target, None);
    assert_eq!(peer.host_object(peer_unit).unwrap().formation_id, 19);
}
#[test]
fn team_waypoint_orders_choose_owned_path_and_preserve_or_clear_formation() {
    for (kind, flag) in [
        (ScriptActionType::TeamFollowWaypoints, 0),
        (ScriptActionType::TeamFollowWaypoints, 1),
        (ScriptActionType::TeamFollowWaypointsExact, 1),
    ] {
        let (mut owner, unit, _) = world();
        let (peer, peer_unit, _) = world();
        let team = roster(&owner, unit);
        let mut map = gamelogic::system::map_loader::MapData::new();
        map.width = 64;
        map.height = 64;
        map.heightmap = vec![0; 4096];
        map.boundaries = vec![gamelogic::common::ICoord2D::new(64, 64)];
        map.waypoints
            .push(gamelogic::system::map_loader::MapWaypoint {
                id: 9401,
                name: "TeamPoint".into(),
                location: gamelogic::common::Coord3D::new(80.0, 90.0, 0.0),
                path_label1: "TeamPath".into(),
                path_label2: String::new(),
                path_label3: String::new(),
                bi_directional: false,
            });
        let mut foreign_map = map.clone();
        foreign_map.waypoints[0].location = gamelogic::common::Coord3D::new(400.0, 400.0, 0.0);
        peer.world_services
            .terrain()
            .write()
            .unwrap()
            .load_map_data(foreign_map);
        let mut order = action(kind, "FollowTeam", "TeamPath");
        order
            .add_parameter(Parameter::with_int(ParameterType::Boolean, flag))
            .unwrap();
        issue(&mut owner, team, &order);
        assert_eq!(
            owner.host_object(unit).unwrap().requested_destination,
            None,
            "missing owned path never borrows a matching foreign path"
        );
        assert_eq!(owner.host_object(unit).unwrap().formation_id, 19);
        owner
            .world_services
            .terrain()
            .write()
            .unwrap()
            .load_map_data(map);
        issue(&mut owner, team, &order);
        let object = owner.host_object(unit).unwrap();
        assert_eq!(object.formation_id, if flag == 0 { 0 } else { 19 });
        let destination = if flag == 0 {
            Vec3::new(80.0, 0.0, 90.0)
        } else {
            Vec3::new(84.0, 0.0, 96.0)
        };
        if kind == ScriptActionType::TeamFollowWaypointsExact {
            // Exact installs the authored path immediately; ordinary Follow
            // queues its pathfinding request at the original request phase.
            assert!(object.is_exact_path);
            assert_eq!(object.movement.path.last().copied(), Some(destination));
            assert_eq!(object.movement.target_position, Some(destination));
        } else {
            assert_eq!(object.requested_destination, Some(destination));
        }
        assert_eq!(
            object.last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
        );
        assert_eq!(
            peer.host_object(peer_unit).unwrap().requested_destination,
            None
        );
        assert_eq!(peer.host_object(peer_unit).unwrap().formation_id, 19);
    }
}
