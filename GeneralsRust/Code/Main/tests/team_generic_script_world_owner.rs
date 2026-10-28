use game_engine::common::dict::Dict;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_faction, key_player_is_human, key_player_name, key_team_generic_script_hook,
    key_team_is_singleton, key_team_name, key_team_owner, key_team_production_priority,
    key_team_unit_max_count1, key_team_unit_min_count1, key_team_unit_type1,
};
use gamelogic::scripting::chunk_codec::ScriptTemplateLookup;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_PLAYER,
};
use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::sides_list::SidesList;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId};
use generals_main::save_load::SaveFileManager;

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
    // Team generic hooks evaluate conditions directly, even on inactive root
    // scripts. Keep the ordinary root walk from executing this hook as well.
    script.is_active = false;
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
    first_script.is_active = false;
    first_script.condition = Some(always_true());
    first_script.action = Some(Box::new(create));
    let mut later_script = Script::new();
    later_script.script_name = "RunLaterTeam".into();
    later_script.is_active = false;
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

fn map_with_team_transfer(root: &std::path::Path) -> std::path::PathBuf {
    map_with_transfer_options(root, false)
}

fn map_with_transfer_options(root: &std::path::Path, siblings: bool) -> std::path::PathBuf {
    let mut sides = SidesList::new();
    for name in ["Owner", "Receiver"] {
        let mut player = Dict::new();
        player.set_ascii_string(key_player_name(), name);
        player.set_ascii_string(key_player_faction(), "FactionAmerica");
        player.set_bool(key_player_is_human(), true);
        sides.add_side(&player);
    }

    let hook_key = |slot: usize| {
        NameKeyGenerator::name_to_key(&format!(
            "{}{}",
            NameKeyGenerator::key_to_name(key_team_generic_script_hook()).unwrap(),
            slot
        ))
    };
    let mut team = Dict::new();
    team.set_ascii_string(key_team_name(), "TransferPrototype");
    team.set_ascii_string(key_team_owner(), "Owner");
    team.set_bool(key_team_is_singleton(), !siblings);
    if siblings {
        team.set_ascii_string(key_team_unit_type1(), UNIT);
        team.set_int(key_team_unit_min_count1(), 1);
        team.set_int(key_team_unit_max_count1(), 1);
        team.set_int(key_team_production_priority(), 100);
    }
    team.set_ascii_string(hook_key(0), "AwardCurrentOwner");
    sides.add_team(&team);

    let mut award = ScriptAction::new(ScriptActionType::PlayerGiveMoney);
    award
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.into(),
        ))
        .unwrap();
    award
        .add_parameter(Parameter::with_int(ParameterType::Int, 43))
        .unwrap();

    let mut award_script = Script::new();
    award_script.script_name = "AwardCurrentOwner".into();
    award_script.is_active = false;
    award_script.is_one_shot = false;
    award_script.condition = Some(always_true());
    award_script.action = Some(Box::new(award));
    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(award_script));
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
    properties.set_ascii_string(
        NameKeyGenerator::name_to_key("objectName"),
        "TransferMember",
    );
    properties.set_ascii_string(
        NameKeyGenerator::name_to_key("originalOwner"),
        "TransferPrototype",
    );
    out.write_dict(&properties);
    out.close_data_chunk();
    if siblings {
        out.open_data_chunk("Object", 3);
        for value in [150.0, 150.0, 0.0, 0.0] {
            out.write_real(value);
        }
        out.write_int(0);
        out.write_ascii_string("Waypoint");
        let mut waypoint = Dict::new();
        waypoint.set_int(NameKeyGenerator::name_to_key("waypointID"), 701);
        waypoint.set_ascii_string(
            NameKeyGenerator::name_to_key("waypointName"),
            "TransferSpawn",
        );
        out.write_dict(&waypoint);
        out.close_data_chunk();
    }
    out.close_data_chunk();
    sides.write_sides_data_chunk(&mut out, &ScriptTemplateLookup::from_engine(&engine));

    let path = root.join("GenericTransfersPrototype.map");
    std::fs::write(&path, out.into_ckmp_bytes()).expect("write map");
    std::fs::write(root.join("Map.ini"), "").expect("write empty Map.ini");
    path
}

fn transfer_engine() -> ScriptEngine {
    let mut action = ScriptAction::new(ScriptActionType::TeamTransferToPlayer);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "TransferPrototype".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "Receiver".into(),
        ))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "TransferPrototypeOutsidePlayerCursor".into();
    script.is_one_shot = true;
    script.condition = Some(always_true());
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().expect("script templates");
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .expect("admit transfer script");
    engine
}

fn single_action_engine(name: &str, action: ScriptAction) -> ScriptEngine {
    let mut script = Script::new();
    script.script_name = name.into();
    script.is_one_shot = true;
    script.condition = Some(always_true());
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().expect("script templates");
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .expect("admit script");
    engine
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
        .find(|object| {
            matches!(
                object.name.as_str(),
                "GenericMember" | "InitialMember" | "TransferMember"
            )
        })
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

#[test]
fn transferred_prototype_updates_siblings_and_recruits_for_its_current_owner() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = map_with_transfer_options(owner_root.path(), true);
    let peer_map = map_with_transfer_options(peer_root.path(), true);
    let (mut owner, first) = world(&owner_map);
    let (peer, peer_first) = world(&peer_map);
    assert_eq!(first, peer_first);

    let mut spawn = ScriptAction::new(ScriptActionType::CreateReinforcementTeam);
    spawn
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "TransferPrototype".into(),
        ))
        .unwrap();
    spawn
        .add_parameter(Parameter::with_string(
            ParameterType::Waypoint,
            "TransferSpawn".into(),
        ))
        .unwrap();
    owner
        .update_local_script_engine_with_owner(&single_action_engine("SpawnSibling", spawn))
        .expect("create a second instance of the authored prototype");
    let sibling = owner
        .host_objects()
        .registered_values()
        .find(|object| object.id != first && object.template_name == UNIT)
        .map(|object| object.id)
        .expect("reinforcement member");
    let owner_before = owner.get_player(1).unwrap().resources.supplies;
    owner.update();
    assert_eq!(
        owner.get_player(1).unwrap().resources.supplies,
        owner_before + 86,
        "both instances run the authored prototype hook"
    );

    owner
        .update_local_script_engine_with_owner(&transfer_engine())
        .expect("transfer the shared prototype");
    for id in [first, sibling] {
        assert_eq!(owner.host_object(id).unwrap().owner_player_id, Some(2));
    }
    assert_eq!(
        peer.host_object(peer_first).unwrap().owner_player_id,
        Some(1)
    );

    owner.add_ai_opponent(
        2,
        generals_main::game_logic::Team::USA,
        generals_main::ai::AIDifficulty::Medium,
    );
    let candidate = owner
        .create_object_for_player(UNIT, 2, glam::Vec3::new(180.0, 0.0, 180.0))
        .expect("receiver's recruitable unit");
    owner.host_object_mut(candidate).unwrap().is_recruitable = true;
    let mut recruit = ScriptAction::new(ScriptActionType::RecruitTeam);
    recruit
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "TransferPrototype".into(),
        ))
        .unwrap();
    recruit
        .add_parameter(Parameter::with_real(ParameterType::Real, 999.0))
        .unwrap();
    owner
        .update_local_script_engine_with_owner(&single_action_engine("RecruitTransferred", recruit))
        .expect("recruit through the transferred prototype");
    let recruited = owner.host_object(candidate).unwrap();
    assert_eq!(recruited.owner_player_id, Some(2));
    assert_eq!(recruited.team_instance_name, "TransferPrototype");
    assert_eq!(
        peer.host_object(peer_first).unwrap().owner_player_id,
        Some(1)
    );
}

#[test]
fn transfer_moves_prototype_owner_and_two_worlds_continue_independently() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = map_with_team_transfer(owner_root.path());
    let peer_map = map_with_team_transfer(peer_root.path());
    let (mut owner, owner_member) = world(&owner_map);
    let (peer, peer_member) = world(&peer_map);
    assert_eq!(owner_member, peer_member, "worlds reuse the same Object ID");

    let owner_before = owner.get_player(1).unwrap().resources.supplies;
    let receiver_before = owner.get_player(2).unwrap().resources.supplies;
    let peer_receiver_before = peer.get_player(2).unwrap().resources.supplies;
    owner
        .update_local_script_engine_with_owner(&transfer_engine())
        .expect("transfer prototype through a receiving script action");
    assert_eq!(
        owner.host_object(owner_member).unwrap().owner_player_id,
        Some(2)
    );

    owner.update();

    assert_eq!(
        owner.get_player(1).unwrap().resources.supplies,
        owner_before,
        "the transferred prototype no longer visits the former owner's generic phase"
    );
    assert_eq!(
        owner.get_player(2).unwrap().resources.supplies,
        receiver_before + 43,
        "the receiving Player runs the prototype's generic hook in its own phase"
    );
    assert_eq!(
        peer.host_object(peer_member).unwrap().owner_player_id,
        Some(1)
    );
    assert_eq!(
        peer.get_player(2).unwrap().resources.supplies,
        peer_receiver_before,
        "the same-ID peer retains its original prototype owner until its own update"
    );

    let save_root = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(save_root.path());
    saves.init().unwrap();
    saves
        .quick_save(&owner)
        .expect("encode transferred prototype owner");
    let decoded = saves
        .load_game_snapshot("quicksave")
        .expect("decode transferred prototype owner");
    let (mut restored, _) = world(&owner_map);
    saves
        .restore_game_snapshot(&decoded.snapshot, &mut restored)
        .expect("restore transferred prototype owner");
    let owner_after_save = restored.get_player(1).unwrap().resources.supplies;
    let receiver_after_save = restored.get_player(2).unwrap().resources.supplies;
    restored.update();
    assert_eq!(
        restored.get_player(1).unwrap().resources.supplies,
        owner_after_save,
        "restoration must not return the transferred prototype to the map owner"
    );
    assert_eq!(
        restored.get_player(2).unwrap().resources.supplies,
        receiver_after_save + 43,
        "the receiving owner continues its generic hook after encoded restore"
    );

    assert!(owner.load_map(owner_map.to_str().unwrap()));
    let readmitted = owner
        .host_objects()
        .registered_values()
        .find(|object| object.name == "TransferMember")
        .expect("map re-admits the named member");
    assert_eq!(
        readmitted.owner_player_id,
        Some(1),
        "map re-admission resets the prototype to its authored owner"
    );
    assert_eq!(
        peer.host_object(peer_member).unwrap().owner_player_id,
        Some(1),
        "resetting the receiving world leaves the peer's same-ID team intact"
    );
}
