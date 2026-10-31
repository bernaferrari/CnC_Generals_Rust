use game_engine::common::dict::Dict;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkOutput;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};

fn map_file(root: &std::path::Path, destination: [f32; 3], area_start: i32) -> std::path::PathBuf {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("PolygonTriggers", 4);
    out.write_int(1);
    out.write_ascii_string("OwnedArea");
    out.write_ascii_string("OwnerLayer");
    out.write_int(1901);
    out.write_byte(1);
    out.write_byte(0);
    out.write_int(0);
    out.write_int(4);
    for (x, y) in [(0, 0), (100, 0), (100, 100), (0, 100)] {
        out.write_int(area_start + x);
        out.write_int(area_start + y);
        out.write_int(0);
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
    out.open_data_chunk("Object", 3);
    for value in [260.0, 285.0, 0.0, 0.0] {
        out.write_real(value);
    }
    out.write_int(0);
    out.write_ascii_string("Waypoint");
    let mut properties = Dict::new();
    properties.set_int(NameKeyGenerator::name_to_key("waypointID"), 702);
    properties.set_ascii_string(NameKeyGenerator::name_to_key("waypointName"), "OtherMove");
    out.write_dict(&properties);
    out.close_data_chunk();
    out.close_data_chunk();

    let path = root.join("TeamState.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "TeamState.ini".into(),
            "Object OwnedTeamStateUnit\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Owner", true).with_script_identity("Owner"));
    let unit = world
        .create_object_for_player("OwnedTeamStateUnit", 1, glam::Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    (world, unit)
}

fn script_engine(name: &str, condition: Condition, action: ScriptAction) -> ScriptEngine {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut script = Script::new();
    script.script_name = name.into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    engine
}

fn true_condition() -> Condition {
    Condition::new(ConditionType::ConditionTrue)
}

#[test]
fn nearest_object_script_uses_receiving_off_map_status() {
    use glam::Vec3;

    fn nearest_map(root: &std::path::Path, boundary_cells: i32) -> std::path::PathBuf {
        let mut out = DataChunkOutput::new();
        out.open_data_chunk("HeightMapData", 4);
        for value in [41, 41, 1, 1, boundary_cells, boundary_cells, 1681] {
            out.write_int(value);
        }
        for _ in 0..1681 {
            out.write_byte(0);
        }
        out.close_data_chunk();
        out.open_data_chunk("PolygonTriggers", 4);
        out.write_int(1);
        out.write_ascii_string("OwnedArea");
        out.write_ascii_string("OwnerLayer");
        out.write_int(1901);
        out.write_byte(1);
        out.write_byte(0);
        out.write_int(0);
        out.write_int(4);
        for (x, y) in [(0, 0), (110, 0), (110, 110), (0, 110)] {
            out.write_int(x);
            out.write_int(y);
            out.write_int(0);
        }
        out.close_data_chunk();
        out.open_data_chunk("ObjectsList", 3);
        out.close_data_chunk();
        let path = root.join(format!("Nearest{boundary_cells}.map"));
        std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
        path
    }

    fn nearest_world(path: &std::path::Path) -> (GameLogic, generals_main::game_logic::ObjectId) {
        let mut world = GameLogic::new();
        world
            .admit_object_ini_sources([(
                "NearestOwner.ini".into(),
                r#"
Object OwnedNearestMover
 KindOf = SELECTABLE INFANTRY
 Locomotor = SET_NORMAL BasicHumanLocomotor
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = AIUpdateInterface Brain
 End
End
Object OwnedNearestTarget
 KindOf = SELECTABLE INFANTRY
 Body = ActiveBody BodyTag
  MaxHealth = 100
  InitialHealth = 100
 End
End
"#
                .into(),
            )])
            .unwrap();
        world.start_new_game(GameMode::SinglePlayer);
        assert!(world.load_map(path.to_str().unwrap()));
        world.add_player(Player::new(1, Team::USA, "Owner", true).with_script_identity("Owner"));
        let mover = world
            .create_object_for_player("OwnedNearestMover", 1, Vec3::new(100.0, 0.0, 100.0))
            .unwrap();
        world.host_object_mut(mover).unwrap().name = "NearestMover".into();
        world
            .create_object_for_player("OwnedNearestTarget", 1, Vec3::new(80.0, 0.0, 80.0))
            .unwrap();
        (world, mover)
    }

    fn execute(world: &mut GameLogic) {
        let mut action = ScriptAction::new(ScriptActionType::UnitMoveTowardsNearestObjectType);
        for (kind, name) in [
            (ParameterType::Unit, "NearestMover"),
            (ParameterType::ObjectType, "OwnedNearestTarget"),
            (ParameterType::TriggerArea, "OwnedArea"),
        ] {
            action
                .add_parameter(Parameter::with_string(kind, name.into()))
                .unwrap();
        }
        let mut counter = ScriptAction::new(ScriptActionType::IncrementCounter);
        counter
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        counter
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "ObservedNearest".into(),
            ))
            .unwrap();
        action.next_action = Some(Box::new(counter));
        let engine = script_engine("NearestOffMap", true_condition(), action);
        engine.set_counter("ObservedNearest", 0).unwrap();
        world
            .update_local_script_engine_with_owner(&engine)
            .unwrap();
        assert_eq!(engine.get_counter("ObservedNearest").unwrap().value, 1);
    }

    let root = tempfile::tempdir().unwrap();
    let (mut small, mover) = nearest_world(&nearest_map(root.path(), 10));
    let (mut large, peer_mover) = nearest_world(&nearest_map(root.path(), 20));
    let (mut both_off, both_mover) = nearest_world(&nearest_map(root.path(), 8));
    assert_eq!((mover, mover), (peer_mover, both_mover));
    execute(&mut small);
    execute(&mut large);
    assert_eq!(
        small.host_object(mover).unwrap().requested_destination,
        None
    );
    assert_eq!(
        large.host_object(mover).unwrap().requested_destination,
        Some(Vec3::new(80.0, 0.0, 80.0))
    );

    // Both actors are off-map at the strict 80-unit edge, so they match.
    execute(&mut both_off);
    assert_eq!(
        both_off.host_object(mover).unwrap().requested_destination,
        Some(Vec3::new(80.0, 0.0, 80.0))
    );
}

fn team_state_action(team: &str, state: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::TeamSetState);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TeamState,
            state.into(),
        ))
        .unwrap();
    action
}

fn team_state_condition(kind: ConditionType, team: &str, state: &str) -> Condition {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TeamState,
            state.into(),
        ))
        .unwrap();
    condition
}

fn move_team_action(team: &str, waypoint: &str) -> ScriptAction {
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
    action
}

#[test]
fn team_state_scripts_are_owned_by_the_receiving_world_and_missing_team_is_false() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = map_file(owner_root.path(), [180.0, 0.0, 210.0], 50);
    let peer_map = map_file(peer_root.path(), [360.0, 0.0, 390.0], 150);
    let (mut owner, owner_unit) = world(&owner_map);
    let (mut peer, peer_unit) = world(&peer_map);
    assert_eq!(owner_unit, peer_unit);

    owner
        .update_local_script_engine_with_owner(&script_engine(
            "SetOwnerTeamState",
            true_condition(),
            team_state_action("teamOwner", "OwnerState"),
        ))
        .unwrap();
    peer.update_local_script_engine_with_owner(&script_engine(
        "SetPeerTeamState",
        true_condition(),
        team_state_action("teamOwner", "PeerState"),
    ))
    .unwrap();

    owner
        .update_local_script_engine_with_owner(&script_engine(
            "MoveWhenOwnerState",
            team_state_condition(ConditionType::TeamStateIs, "teamOwner", "OwnerState"),
            move_team_action("teamOwner", "OwnedMove"),
        ))
        .unwrap();
    peer.update_local_script_engine_with_owner(&script_engine(
        "MoveWhenOwnerState",
        team_state_condition(ConditionType::TeamStateIs, "teamOwner", "OwnerState"),
        move_team_action("teamOwner", "OwnedMove"),
    ))
    .unwrap();
    assert_eq!(
        owner.host_object(owner_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(180.0, 0.0, 210.0))
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        None
    );

    owner
        .update_local_script_engine_with_owner(&script_engine(
            "MissingTeamIsNot",
            team_state_condition(ConditionType::TeamStateIsNot, "MissingTeam", "Anything"),
            move_team_action("teamOwner", "OtherMove"),
        ))
        .unwrap();
    assert_eq!(
        owner.host_object(owner_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(180.0, 0.0, 210.0)),
        "C++ TeamStateIsNot also returns false when the named team is missing"
    );
    owner
        .update_local_script_engine_with_owner(&script_engine(
            "MissingTeamIs",
            team_state_condition(ConditionType::TeamStateIs, "MissingTeam", "Anything"),
            move_team_action("teamOwner", "OtherMove"),
        ))
        .unwrap();
    assert_eq!(
        owner.host_object(owner_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(180.0, 0.0, 210.0)),
        "C++ TeamStateIs returns false when the named team is missing"
    );
}

#[test]
fn team_area_conditions_observe_receiving_tracked_slots() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = map_file(owner_root.path(), [180.0, 0.0, 210.0], 50);
    let peer_map = map_file(peer_root.path(), [300.0, 0.0, 320.0], 150);
    let (mut owner, owner_unit) = world(&owner_map);
    let (mut peer, peer_unit) = world(&peer_map);
    assert_eq!(owner_unit, peer_unit);
    for (logic, id) in [(&mut owner, owner_unit), (&mut peer, peer_unit)] {
        logic.host_object_mut(id).unwrap().locomotor_surfaces = gamelogic::path::SURFACE_GROUND;
        // The production script phase projects the receiving trigger slots.
        logic.update();
    }

    let area_condition = |kind, team: &str, area: &str| {
        let mut condition = Condition::new(kind);
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
    };
    let inside = || {
        script_engine(
            "InsideOwnedArea",
            area_condition(
                ConditionType::TeamInsideAreaPartially,
                "teamOwner",
                "OwnedArea",
            ),
            move_team_action("teamOwner", "OwnedMove"),
        )
    };
    owner
        .update_local_script_engine_with_owner(&inside())
        .unwrap();
    peer.update_local_script_engine_with_owner(&inside())
        .unwrap();
    assert_eq!(
        owner.host_object(owner_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(180.0, 0.0, 210.0))
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        None
    );

    let outside = script_engine(
        "OutsideOwnedArea",
        area_condition(
            ConditionType::TeamOutsideAreaEntirely,
            "teamOwner",
            "OwnedArea",
        ),
        move_team_action("teamOwner", "OwnedMove"),
    );
    peer.update_local_script_engine_with_owner(&outside)
        .unwrap();
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(300.0, 0.0, 320.0))
    );

    owner
        .host_object_mut(owner_unit)
        .unwrap()
        .locomotor_surfaces = 0;
    let zero_surface = script_engine(
        "ZeroSurfaceInside",
        area_condition(
            ConditionType::TeamInsideAreaEntirely,
            "teamOwner",
            "OwnedArea",
        ),
        move_team_action("teamOwner", "OtherMove"),
    );
    owner
        .update_local_script_engine_with_owner(&zero_surface)
        .unwrap();
    assert_eq!(
        owner.host_object(owner_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(180.0, 0.0, 210.0))
    );
}
