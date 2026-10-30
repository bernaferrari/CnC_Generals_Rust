use game_engine::common::dict::Dict;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkOutput;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};

fn map_file(root: &std::path::Path, destination: [f32; 3]) -> std::path::PathBuf {
    map_file_sized(root, destination, 41)
}

fn map_file_sized(root: &std::path::Path, destination: [f32; 3], size: i32) -> std::path::PathBuf {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [size, size, 1, size * size] {
        out.write_int(value);
    }
    for _ in 0..size * size {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.open_data_chunk("Object", 3);
    for value in [destination[0], destination[2], destination[1], 0.0] {
        out.write_real(value);
    }
    out.write_int(0);
    out.write_ascii_string("Waypoint");
    let mut properties = Dict::new();
    properties.set_int(NameKeyGenerator::name_to_key("waypointID"), 701);
    properties.set_ascii_string(NameKeyGenerator::name_to_key("waypointName"), "OwnedMove");
    out.write_dict(&properties);
    out.close_data_chunk();
    out.close_data_chunk();

    let path = root.join("TeamMove.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors.apply_source("Locomotor GroupGround\n Surfaces = GROUND\n Speed = 20\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n").unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world
        .admit_object_ini_sources([(
            "TeamMove.ini".into(),
            "Object OwnedTeamMoveUnit\n KindOf = SELECTABLE INFANTRY\n Locomotor = SET_NORMAL GroupGround\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Owner", true).with_script_identity("Owner"));
    let unit = world
        .create_object_for_player("OwnedTeamMoveUnit", 1, glam::Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    (world, unit)
}

#[test]
fn contact_attack_uses_reachable_nearby_position_in_its_own_world() {
    use generals_main::command_executor::CommandExecutor;
    use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
    use glam::Vec3;
    use std::time::UNIX_EPOCH;

    fn contact_world(map: &std::path::Path) -> (GameLogic, generals_main::game_logic::ObjectId) {
        let mut game = GameLogic::new();
        let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
        locomotors.apply_source("Locomotor GroupGround\n Surfaces = GROUND\n Speed = 20\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n").unwrap();
        game.admit_locomotor_definitions(locomotors).unwrap();
        game.admit_object_ini_sources([("Contact.ini".into(), "Object ContactUnit\n KindOf = SELECTABLE INFANTRY\n Locomotor = SET_NORMAL GroupGround\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\n WeaponSet\n Conditions = NONE\n Weapon = PRIMARY ContactFixtureWeapon\n End\nEnd\n".into())]).unwrap();
        let mut weapon = gamelogic::weapon::WeaponTemplate::new("ContactFixtureWeapon".into());
        weapon.primary_damage = 10.0;
        weapon.attack_range = 5.0;
        game.admit_receiving_weapon_snapshot(vec![("ContactFixtureWeapon".into(), weapon)])
            .unwrap();
        game.start_new_game(GameMode::SinglePlayer);
        assert!(game.load_map(map.to_str().unwrap()));
        game.add_player(Player::new(1, Team::USA, "Owner", true).with_script_identity("Owner"));
        let id = game
            .create_object_for_player("ContactUnit", 1, Vec3::new(100.0, 0.0, 100.0))
            .unwrap();
        (game, id)
    }
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path(), [205.0, 0.0, 205.0]);
    let (mut blocked, attacker) = contact_world(&map);
    let (mut peer, peer_attacker) = contact_world(&map);
    let (mut sealed, sealed_attacker) = contact_world(&map);
    assert_eq!(attacker, peer_attacker);
    assert_eq!(attacker, sealed_attacker);
    blocked.set_pathfinding_static_block(20, 20, true);
    for x in 10..=30 {
        for z in 10..=30 {
            sealed.set_pathfinding_static_block(x, z, true);
        }
    }
    let destination = Vec3::new(205.0, 0.0, 205.0);
    for game in [&mut blocked, &mut peer, &mut sealed] {
        CommandExecutor::new(game, 1)
            .execute_command(GameCommand {
                command_type: CommandType::AttackPosition {
                    location: Some(destination),
                    max_shots: 3,
                },
                player_id: 1,
                command_id: 1,
                timestamp: UNIX_EPOCH,
                selected_units: vec![attacker],
                modifier_keys: ModifierKeys::default(),
            })
            .unwrap();
    }
    let actual = blocked
        .host_object(attacker)
        .unwrap()
        .target_location
        .unwrap();
    assert_ne!(actual, destination, "blocked contact goal must be adjusted");
    assert!(actual.distance(destination) <= 100.0);
    assert!(
        !blocked.is_pathfinding_static_blocked((actual.x / 10.0) as i32, (actual.z / 10.0) as i32)
    );
    assert_eq!(
        peer.host_object(attacker).unwrap().target_location,
        Some(destination)
    );
    assert_eq!(
        sealed.host_object(attacker).unwrap().target_location,
        Some(destination),
        "an exhausted search preserves the requested position"
    );
    assert_eq!(blocked.host_object(attacker).unwrap().max_shots_to_fire, 3);
}

fn move_script(team: &str, waypoint: &str) -> ScriptEngine {
    let mut action = ScriptAction::new(ScriptActionType::MoveTeamTo);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Waypoint,
            waypoint.into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnedTeamMove".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    engine
}

#[test]
fn live_team_move_uses_receiving_world_when_object_ids_collide() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = map_file(owner_root.path(), [180.0, 0.0, 210.0]);
    let peer_map = map_file(peer_root.path(), [360.0, 0.0, 390.0]);
    let (mut owner, owner_unit) = world(&owner_map);
    let (peer, peer_unit) = world(&peer_map);
    assert_eq!(owner_unit, peer_unit);

    owner
        .update_local_script_engine_with_owner(&move_script("teamOwner", "OwnedMove"))
        .unwrap();

    assert_eq!(
        owner.host_object(owner_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(180.0, 0.0, 210.0))
    );
    assert_eq!(
        owner.host_object(owner_unit).unwrap().last_command_source,
        1 // C++ CMD_FROM_SCRIPT
    );
    assert_eq!(
        owner.ai_group_members_of(owner_unit),
        Some(vec![owner_unit])
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        None
    );
}

#[test]
fn live_team_move_admits_roster_before_missing_waypoint_returns() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path(), [180.0, 0.0, 210.0]);
    let (mut world, unit) = world(&map);

    world
        .update_local_script_engine_with_owner(&move_script("teamOwner", "MissingWaypoint"))
        .unwrap();

    assert_eq!(world.ai_group_members_of(unit), Some(vec![unit]));
    assert_eq!(world.host_object(unit).unwrap().requested_destination, None);
}

#[test]
fn live_team_move_preserves_a_formed_multi_unit_roster_and_speed() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path(), [260.0, 0.0, 300.0]);
    let (mut world, first) = world(&map);
    let second = world
        .create_object_for_player("OwnedTeamMoveUnit", 1, glam::Vec3::new(115.0, 0.0, 100.0))
        .unwrap();
    for (id, speed, offset) in [(first, 10.0, -20.0), (second, 20.0, 20.0)] {
        let unit = world.host_object_mut(id).unwrap();
        unit.movement.max_speed = speed;
        unit.set_formation(41, glam::Vec2::new(offset, 0.0));
    }
    let (width, height, _) = world.snapshot_pathfinding_passability();
    assert!(width > 30 && height > 30);
    // A static pathfinding wall through the direct line; leave both map edges
    // open so the group-ground query must route around it.
    for x in 15..26 {
        world.set_pathfinding_static_block(x, (height / 2) as i32, true);
    }
    assert!(world.is_pathfinding_static_blocked(20, (height / 2) as i32));

    world
        .update_local_script_engine_with_owner(&move_script("teamOwner", "OwnedMove"))
        .unwrap();

    // Object::setTeam prepends each member; getTeamAsAIGroup appends in that order.
    assert_eq!(world.ai_group_members_of(first), Some(vec![second, first]));
    for _ in 0..10 {
        if [first, second]
            .iter()
            .all(|&id| !world.host_object(id).unwrap().movement.path.is_empty())
        {
            break;
        }
        world.update_with_dt(1.0 / 30.0);
    }
    for id in [first, second] {
        let unit = world.host_object(id).unwrap();
        assert!(
            !unit.movement.path.is_empty(),
            "{id:?} receives a group route: can_move={} destination={:?} state={:?} moving={} formation={} position={:?}",
            unit.can_move(),
            unit.requested_destination,
            unit.ai_state(),
            unit.status.moving,
            unit.formation_id,
            unit.get_position()
        );
        assert_eq!(unit.formation_id, 41);
        assert_eq!(
            unit.last_command_source,
            1 // C++ CMD_FROM_SCRIPT
        );
    }
    let first = world.host_object(first).unwrap();
    let second = world.host_object(second).unwrap();
    assert!(
        (first.group_speed_factor - 1.0).abs() < 1e-5
            && (second.group_speed_factor - 0.5).abs() < 1e-5,
        "formed group uses slowest member speed: {}, {}",
        first.group_speed_factor,
        second.group_speed_factor
    );
    assert_ne!(
        first.movement.path.last(),
        second.movement.path.last(),
        "members retain their distinct formation endpoints"
    );
    for (unit, start) in [
        (first, glam::Vec3::new(100.0, 0.0, 100.0)),
        (second, glam::Vec3::new(115.0, 0.0, 100.0)),
    ] {
        let traveled: f32 = unit
            .movement
            .path
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).length())
            .sum::<f32>()
            + unit
                .movement
                .path
                .first()
                .map(|first| (*first - start).length())
                .unwrap_or(0.0);
        assert!(
            traveled > (unit.requested_destination.unwrap() - start).length() * 1.1,
            "formed route should detour around the static wall: {traveled}"
        );
    }
}

#[test]
fn live_team_move_uses_snapped_spine_endpoint_when_formation_trim_is_empty() {
    let root = tempfile::tempdir().unwrap();
    let click = [212.3, 0.0, 151.7];
    let map = map_file(root.path(), click);
    let (mut world, first) = world(&map);
    let second = world
        .create_object_for_player("OwnedTeamMoveUnit", 1, glam::Vec3::new(115.0, 0.0, 100.0))
        .unwrap();
    for (id, offset) in [(first, -20.0), (second, 20.0)] {
        world
            .host_object_mut(id)
            .unwrap()
            .set_formation(42, glam::Vec2::new(offset, 0.0));
    }

    world
        .update_local_script_engine_with_owner(&move_script("teamOwner", "OwnedMove"))
        .unwrap();

    let click = glam::Vec3::from_array(click);
    let first_goal = world
        .host_object(first)
        .unwrap()
        .requested_destination
        .unwrap();
    let second_goal = world
        .host_object(second)
        .unwrap()
        .requested_destination
        .unwrap();
    let first_center = first_goal - glam::Vec3::new(-20.0, 0.0, 0.0);
    let second_center = second_goal - glam::Vec3::new(20.0, 0.0, 0.0);
    assert!(
        first_center.distance(second_center) < 20.0,
        "fallback keeps both members on the snapped spine endpoint: {first_center:?} {second_center:?}"
    );
    assert!(
        first_center.distance(click) > 1.0,
        "empty trim window must fall back to the retained path endpoint, not the click: {first_center:?}"
    );
}

#[test]
fn live_formed_team_move_queues_a_shared_spine_in_receiving_world() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file_sized(root.path(), [1000.0, 0.0, 1200.0], 161);
    let (mut peer, peer_first) = world(&map);
    let (mut world, first) = world(&map);
    let second = world
        .create_object_for_player("OwnedTeamMoveUnit", 1, glam::Vec3::new(115.0, 0.0, 100.0))
        .unwrap();
    let peer_second = peer
        .create_object_for_player("OwnedTeamMoveUnit", 1, glam::Vec3::new(115.0, 0.0, 100.0))
        .unwrap();
    assert_eq!((first, second), (peer_first, peer_second));
    for (id, offset) in [(first, -20.0), (second, 20.0)] {
        world
            .host_object_mut(id)
            .unwrap()
            .set_formation(43, glam::Vec2::new(offset, 0.0));
    }
    let (width, height, _) = world.snapshot_pathfinding_passability();
    for x in 15..(width as i32 / 2) {
        world.set_pathfinding_static_block(x, (height / 2) as i32, true);
    }
    world
        .update_local_script_engine_with_owner(&move_script("teamOwner", "OwnedMove"))
        .unwrap();
    assert!(peer.snapshot_pending_host_paths().is_empty());
    assert!(
        peer.host_object(peer_first)
            .unwrap()
            .requested_destination
            .is_none()
    );
    assert!(
        peer.host_object(peer_second)
            .unwrap()
            .requested_destination
            .is_none()
    );
    let pending = world.snapshot_pending_host_paths();
    let first_path = pending
        .iter()
        .find(|req| req.unit_id == first)
        .expect("first request");
    let second_path = pending
        .iter()
        .find(|req| req.unit_id == second)
        .expect("second request");
    assert!(
        !first_path.waypoints.is_empty(),
        "shared-spine follow must queue intermediate waypoints: {first_path:?} {second_path:?}"
    );
    assert_eq!(first_path.waypoints.len(), second_path.waypoints.len());
    for (a, b) in first_path.waypoints.iter().zip(&second_path.waypoints) {
        assert!(
            (*a + glam::Vec3::new(40.0, 0.0, 0.0)).distance(*b) < 0.01,
            "shared spine offsets: {a:?} {b:?}"
        );
    }
}
