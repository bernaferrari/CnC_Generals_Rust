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

fn map_with_later_prototype_creation(root: &std::path::Path) -> std::path::PathBuf {
    let mut sides = SidesList::new();
    let mut player = Dict::new();
    player.set_ascii_string(key_player_name(), "Owner");
    player.set_ascii_string(key_player_faction(), "FactionAmerica");
    player.set_bool(key_player_is_human(), true);
    sides.add_side(&player);

    let hook_key = |slot: usize| {
        NameKeyGenerator::name_to_key(&format!(
            "{}{}",
            NameKeyGenerator::key_to_name(key_team_generic_script_hook()).unwrap(),
            slot
        ))
    };
    let mut first = Dict::new();
    first.set_ascii_string(key_team_name(), "FirstPrototype");
    first.set_ascii_string(key_team_owner(), "Owner");
    first.set_bool(key_team_is_singleton(), true);
    first.set_ascii_string(hook_key(0), "CreateLaterTeam");
    sides.add_team(&first);

    let mut later = Dict::new();
    later.set_ascii_string(key_team_name(), "LaterPrototype");
    later.set_ascii_string(key_team_owner(), "Owner");
    later.set_bool(key_team_is_singleton(), false);
    later.set_ascii_string(hook_key(0), "RunLaterTeam");
    sides.add_team(&later);

    let mut create = ScriptAction::new(ScriptActionType::CreateObject);
    for parameter in [
        Parameter::with_string(ParameterType::ObjectType, UNIT.into()),
        Parameter::with_string(ParameterType::Team, "LaterPrototype".into()),
        Parameter::with_coord(
            ParameterType::Coord3D,
            gamelogic::common::Coord3D::new(120.0, 0.0, 120.0),
        ),
        Parameter::with_real(ParameterType::Angle, 0.0),
    ] {
        create.add_parameter(parameter).unwrap();
    }

    let mut give_money = ScriptAction::new(ScriptActionType::PlayerGiveMoney);
    give_money
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.into(),
        ))
        .unwrap();
    give_money
        .add_parameter(Parameter::with_int(ParameterType::Int, 43))
        .unwrap();

    let mut first_script = Script::new();
    first_script.script_name = "CreateLaterTeam".into();
    first_script.condition = Some(always_true());
    first_script.action = Some(Box::new(create));
    let mut later_script = Script::new();
    later_script.script_name = "RunLaterTeam".into();
    later_script.condition = Some(always_true());
    later_script.action = Some(Box::new(give_money));
    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(first_script));
    script_list.append_script(Box::new(later_script));
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
    properties.set_ascii_string(NameKeyGenerator::name_to_key("objectName"), "InitialMember");
    properties.set_ascii_string(
        NameKeyGenerator::name_to_key("originalOwner"),
        "FirstPrototype",
    );
    out.write_dict(&properties);
    out.close_data_chunk();
    out.close_data_chunk();
    sides.write_sides_data_chunk(&mut out, &ScriptTemplateLookup::from_engine(&engine));

    let path = root.join("GenericCreatesLaterPrototype.map");
    std::fs::write(&path, out.into_ckmp_bytes()).expect("write map");
    std::fs::write(root.join("Map.ini"), "").expect("write empty Map.ini");
    path
}

fn always_true() -> Box<OrCondition> {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    Box::new(branch)
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
        .find(|object| matches!(object.name.as_str(), "GenericMember" | "InitialMember"))
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

#[test]
fn generic_hook_sees_instance_created_for_a_later_prototype_in_same_player_phase() {
    let root = tempfile::tempdir().unwrap();
    let map = map_with_later_prototype_creation(root.path());
    let (mut game, _) = world(&map);
    let before = game.get_player(1).unwrap().resources.supplies;

    game.update();

    assert_eq!(
        game.get_player(1).unwrap().resources.supplies,
        before + 43,
        "C++ Player::update reaches LaterPrototype after the earlier hook creates its instance"
    );
}
