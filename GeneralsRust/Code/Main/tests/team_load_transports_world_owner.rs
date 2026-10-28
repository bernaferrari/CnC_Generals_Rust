use game_engine::common::{
    dict::Dict,
    system::DataChunkOutput,
    well_known_keys::{
        key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
        key_team_is_singleton, key_team_name, key_team_owner,
    },
};
use gamelogic::scripting::{
    core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    },
    engine::ScriptEngine,
};
use generals_main::game_logic::{AIState, GameLogic, GameMode, ObjectId};
use glam::Vec3;

fn map_file(root: &std::path::Path) -> std::path::PathBuf {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("SidesList", 2);
    out.write_int(2);
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "FactionCivilian");
    let mut human = Dict::new();
    human.set_ascii_string(key_player_name(), "Owner");
    human.set_ascii_string(key_player_faction(), "FactionAmerica");
    human.set_unicode_string(key_player_display_name(), "Owner");
    human.set_bool(key_player_is_human(), true);
    for side in [neutral, human] {
        out.write_dict(&side);
        out.write_int(0);
    }
    out.write_int(1);
    let mut team = Dict::new();
    team.set_ascii_string(key_team_name(), "LoadRoster");
    team.set_ascii_string(key_team_owner(), "Owner");
    team.set_bool(key_team_is_singleton(), false);
    out.write_dict(&team);
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.close_data_chunk();
    let path = root.join("LoadTransports.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path, is_transport: bool) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let transport_kind = if is_transport { "TRANSPORT" } else { "" };
    world
        .admit_object_ini_sources([(
            "LoadTransports.ini".into(),
            format!(
                "Object LoadRanger\n KindOf = SELECTABLE INFANTRY\n TransportSlotCount = 1\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\nObject LoadVehicle\n KindOf = SELECTABLE VEHICLE {transport_kind}\n Body = ActiveBody BodyTag\n MaxHealth = 300\n InitialHealth = 300\n End\n Behavior = TransportContain Cargo\n Slots = 2\n End\nEnd\n"
            ),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    let ranger = world
        .create_object_for_player("LoadRanger", 1, Vec3::new(10.0, 0.0, 50.0))
        .unwrap();
    let vehicle = world
        .create_object_for_player("LoadVehicle", 1, Vec3::new(80.0, 0.0, 50.0))
        .unwrap();
    for id in [ranger, vehicle] {
        world.host_object_mut(id).unwrap().team_instance_name = "LoadRoster".into();
        world.activate_leftover_team_for_host_object(id);
    }
    (world, ranger, vehicle)
}

fn script_engine(team: &str) -> ScriptEngine {
    let mut action = ScriptAction::new(ScriptActionType::TeamLoadTransports);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "LoadTransports".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

#[test]
fn authored_load_orders_are_synchronous_and_belong_to_the_receiving_world() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let (mut owner, owner_unit, owner_transport) = world(&map, true);
    let (mut peer, peer_unit, peer_vehicle) = world(&map, false);
    assert_eq!((owner_unit, owner_transport), (peer_unit, peer_vehicle));
    assert_eq!(owner.host_object(owner_transport).unwrap().max_transport, 2);

    owner
        .update_local_script_engine_with_owner(&script_engine("LoadRoster"))
        .unwrap();
    let unit = owner.host_object(owner_unit).unwrap();
    assert_eq!(unit.target, Some(owner_transport));
    assert_eq!(unit.ai_state(), AIState::Entering);
    assert_eq!(peer.host_object(peer_unit).unwrap().target, None);

    peer.update_local_script_engine_with_owner(&script_engine("LoadRoster"))
        .unwrap();
    assert_eq!(peer.host_object(peer_unit).unwrap().target, None);
    assert_eq!(
        owner.host_object(owner_unit).unwrap().target,
        Some(owner_transport)
    );

    owner
        .update_local_script_engine_with_owner(&script_engine("MissingRoster"))
        .unwrap();
    assert_eq!(
        owner.host_object(owner_unit).unwrap().target,
        Some(owner_transport)
    );
}
