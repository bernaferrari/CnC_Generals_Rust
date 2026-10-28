use game_engine::common::dict::Dict;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_faction, key_player_is_human, key_player_name, key_team_generic_script_hook,
    key_team_is_singleton, key_team_name, key_team_owner,
};
use gamelogic::scripting::chunk_codec::ScriptTemplateLookup;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_PLAYER,
};
use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::sides_list::SidesList;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId};

const TEAM: &str = "ReceivingGenericTeam";
const SCRIPT: &str = "ReceivingGenericGive";
const UNIT: &str = "ReceivingGenericUnit";

fn map_file(root: &std::path::Path, amount: i32) -> std::path::PathBuf {
    let mut sides = SidesList::new();
    let mut player = Dict::new();
    player.set_ascii_string(key_player_name(), "Owner");
    player.set_ascii_string(key_player_faction(), "FactionAmerica");
    player.set_bool(key_player_is_human(), true);
    sides.add_side(&player);

    let mut team = Dict::new();
    team.set_ascii_string(key_team_name(), TEAM);
    team.set_ascii_string(key_team_owner(), "Owner");
    team.set_bool(key_team_is_singleton(), true);
    let generic_hook = NameKeyGenerator::name_to_key(&format!(
        "{}0",
        NameKeyGenerator::key_to_name(key_team_generic_script_hook()).unwrap()
    ));
    team.set_ascii_string(generic_hook, SCRIPT);
    sides.add_team(&team);

    let mut action = ScriptAction::new(ScriptActionType::PlayerGiveMoney);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, amount))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = SCRIPT.into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(script));
    sides
        .get_side_info_mut(0)
        .expect("Owner side")
        .set_script_list(Some(Box::new(script_list)));
    let engine = ScriptEngine::new().expect("script templates");

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
    for value in [100.0, 100.0, 0.0, 0.0] {
        out.write_real(value);
    }
    out.write_int(0);
    out.write_ascii_string(UNIT);
    let mut properties = Dict::new();
    properties.set_ascii_string(NameKeyGenerator::name_to_key("objectName"), "GenericMember");
    properties.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), TEAM);
    out.write_dict(&properties);
    out.close_data_chunk();
    out.close_data_chunk();
    sides.write_sides_data_chunk(&mut out, &ScriptTemplateLookup::from_engine(&engine));

    let path = root.join("ReceivingGeneric.map");
    std::fs::write(&path, out.into_ckmp_bytes()).expect("write map");
    std::fs::write(root.join("Map.ini"), "").expect("write empty Map.ini");
    path
}

fn world(map: &std::path::Path) -> (GameLogic, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "ReceivingGeneric.ini".into(),
        format!(
            "Object {UNIT}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n"
        ),
    )])
    .expect("admit generic hook object");
    game.start_new_game(GameMode::SinglePlayer);
    assert!(game.load_map(map.to_str().expect("UTF-8 map path")));
    assert!(game.get_player(1).is_some(), "map admits its Owner side");
    let member = game
        .host_objects()
        .registered_values()
        .find(|object| object.name == "GenericMember")
        .map(|object| object.id)
        .expect("map admits the Team member through originalOwner");
    (game, member)
}

#[test]
fn map_authored_generic_team_hook_runs_in_each_receiving_game_logic_tick() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = map_file(owner_root.path(), 17);
    let peer_map = map_file(peer_root.path(), 29);
    let (mut owner, owner_member) = world(&owner_map);
    let (mut peer, peer_member) = world(&peer_map);
    assert_eq!(owner_member, peer_member, "worlds reuse the same Object ID");
    assert_eq!(
        owner.get_player(1).unwrap().name,
        peer.get_player(1).unwrap().name
    );

    let owner_before = owner.get_player(1).unwrap().resources.supplies;
    let peer_before = peer.get_player(1).unwrap().resources.supplies;
    owner.update();
    owner.update();
    assert_eq!(
        owner.get_player(1).unwrap().resources.supplies,
        owner_before + 17,
        "the admitted generic hook fires once at the player's AI phase"
    );
    assert_eq!(
        peer.get_player(1).unwrap().resources.supplies,
        peer_before,
        "updating one same-ID world cannot run the peer's generic script"
    );

    peer.update();
    assert_eq!(
        peer.get_player(1).unwrap().resources.supplies,
        peer_before + 29,
        "the peer uses its own map-authored action"
    );
    assert_eq!(
        owner.get_player(1).unwrap().resources.supplies,
        owner_before + 17
    );
}
