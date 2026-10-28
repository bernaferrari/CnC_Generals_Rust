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
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
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
    world
        .admit_object_ini_sources([(
            "TeamMove.ini".into(),
            "Object OwnedTeamMoveUnit\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into(),
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

fn move_script(team: &str) -> ScriptEngine {
    let mut action = ScriptAction::new(ScriptActionType::MoveTeamTo);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Waypoint,
            "OwnedMove".into(),
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
        .update_local_script_engine_with_owner(&move_script("teamOwner"))
        .unwrap();

    assert_eq!(
        owner.host_object(owner_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(180.0, 0.0, 210.0))
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        None
    );
}
