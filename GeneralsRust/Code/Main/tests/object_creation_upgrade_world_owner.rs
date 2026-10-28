use game_engine::common::{
    dict::Dict,
    name_key_generator::NameKeyGenerator,
    system::DataChunkOutput,
    well_known_keys::{
        key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
        key_team_is_singleton, key_team_name, key_team_owner,
    },
};
use gamelogic::{
    scripting::{
        chunk_codec::ScriptTemplateLookup,
        core::{
            Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
            ScriptActionType, ScriptList,
        },
        engine::ScriptEngine,
    },
    sides_list::SidesList,
};
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Team};

fn upgrade_script(master: &str, label: &str) -> Script {
    let mut action = ScriptAction::new(ScriptActionType::NamedReceiveUpgrade);
    action
        .add_parameter(Parameter::with_string(ParameterType::Unit, master.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            "Upgrade_Veterancy_VETERAN".into(),
        ))
        .unwrap();
    let mut second_action = ScriptAction::new(ScriptActionType::NamedReceiveUpgrade);
    second_action
        .add_parameter(Parameter::with_string(ParameterType::Unit, master.into()))
        .unwrap();
    second_action
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            "Upgrade_Veterancy_ELITE".into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = format!("{label}UpgradeScript");
    script.is_one_shot = false;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    script.append_action(Box::new(second_action));
    let mut execution_marker = ScriptAction::new(ScriptActionType::PlayerGiveMoney);
    execution_marker
        .add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
        .unwrap();
    execution_marker
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    script.append_action(Box::new(execution_marker));
    script
}

fn world(root: &std::path::Path, label: &str) -> (GameLogic, ObjectId, String) {
    let master = format!("{label}UpgradeMaster");
    let child = format!("{label}UpgradeChild");
    let ocl = format!("OCL_{label}UpgradeChild");
    let team_name = format!("{label}UpgradeTeam");

    let mut game = GameLogic::new();
    game.admit_object_creation_list_sources([format!(
        "ObjectCreationList {ocl}\n CreateObject\n  ObjectNames = {child}\n  Count = 1\n End\nEnd\n"
    )])
    .unwrap();
    game.admit_object_ini_sources([(
        format!("{label}Upgrade.ini"),
        format!(
            "Object {master}\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = ObjectCreationUpgrade CreateChild\n  TriggeredBy = Upgrade_Veterancy_VETERAN Upgrade_Veterancy_ELITE\n  UpgradeObject = {ocl}\n End\nEnd\nObject {child}\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 50\n InitialHealth = 50\n End\nEnd\n"
        ),
    )])
    .unwrap();

    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(upgrade_script(&master, label)));

    let mut sides = SidesList::new();
    let mut owner = Dict::new();
    owner.set_ascii_string(key_player_name(), "Owner");
    owner.set_ascii_string(key_player_faction(), "FactionAmerica");
    owner.set_unicode_string(key_player_display_name(), "Owner");
    owner.set_bool(key_player_is_human(), true);
    sides.add_side(&owner);
    let mut team = Dict::new();
    team.set_ascii_string(key_team_name(), team_name.as_str());
    team.set_ascii_string(key_team_owner(), "Owner");
    team.set_bool(key_team_is_singleton(), true);
    sides.add_team(&team);
    sides
        .get_side_info_mut(0)
        .expect("owner side")
        .set_script_list(Some(Box::new(script_list)));
    let script_engine = ScriptEngine::new().expect("script templates");

    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        map.write_int(value);
    }
    for _ in 0..1681 {
        map.write_byte(0);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.open_data_chunk("Object", 3);
    for value in [50.0, 50.0, 0.0, 0.0] {
        map.write_real(value);
    }
    map.write_int(0);
    map.write_ascii_string(master.as_str());
    let mut properties = Dict::new();
    properties.set_ascii_string(NameKeyGenerator::name_to_key("objectName"), master.as_str());
    properties.set_ascii_string(
        NameKeyGenerator::name_to_key("originalOwner"),
        team_name.as_str(),
    );
    map.write_dict(&properties);
    map.close_data_chunk();
    map.close_data_chunk();
    sides.write_sides_data_chunk(&mut map, &ScriptTemplateLookup::from_engine(&script_engine));

    let path = root.join(format!("{label}Upgrade.map"));
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    game.start_new_game(GameMode::SinglePlayer);
    assert!(game.load_map(path.to_str().expect("UTF-8 map path")));
    let master_id = game
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == master)
        .expect("map admits the authored master")
        .id;
    assert_eq!(
        game.host_object(master_id).unwrap().owner_player_id,
        Some(1)
    );
    assert_eq!(game.get_player(1).expect("map owner").team, Team::USA);
    (game, master_id, child)
}

fn child_templates(game: &GameLogic, master: &str) -> Vec<String> {
    game.host_objects()
        .registered_values()
        .filter(|object| object.template_name != master)
        .map(|object| object.template_name.clone())
        .collect()
}

#[test]
fn authored_named_receive_upgrade_runs_each_worlds_admitted_object_creation_module() {
    let root = tempfile::tempdir().unwrap();
    let (mut owner, owner_id, owner_child) = world(root.path(), "Owner");
    let (mut peer, peer_id, peer_child) = world(root.path(), "Peer");
    assert_eq!(owner_id, peer_id, "fixture must collide on object identity");
    let owner_master = "OwnerUpgradeMaster";
    let peer_master = "PeerUpgradeMaster";

    // Both OR triggers execute in one script; the module must spawn only once.
    owner.update();
    assert_eq!(child_templates(&owner, owner_master), vec![owner_child]);
    assert!(child_templates(&peer, peer_master).is_empty());

    peer.update();
    assert_eq!(child_templates(&peer, peer_master), vec![peer_child]);
    assert_eq!(
        child_templates(&owner, owner_master),
        vec!["OwnerUpgradeChild"]
    );
}

#[test]
fn authored_upgrade_latch_survives_save_and_continuation() {
    use generals_main::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    use std::time::{Duration, UNIX_EPOCH};

    let root = tempfile::tempdir().unwrap();
    let (mut source, id, child) = world(root.path(), "Save");
    source.update();
    assert_eq!(child_templates(&source, "SaveUpgradeMaster"), vec![child]);

    let mut saves = SaveFileManager::with_save_directory(root.path());
    saves.init().unwrap();
    let info = SaveGameInfo {
        pristine_map_name: None,
        filename: "upgrade".into(),
        display_name: "Upgrade continuation".into(),
        description: String::new(),
        map_name: root.path().join("SaveUpgrade.map").to_str().unwrap().into(),
        campaign_side: None,
        mission_number: None,
        save_date: UNIX_EPOCH + Duration::from_secs(1),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    saves.save_game("upgrade", &source, &info).unwrap();
    let (mut restored, restored_id, _) = world(root.path(), "Save");
    assert_eq!(id, restored_id);
    saves.load_game("upgrade", &mut restored).unwrap();

    // Both worlds execute the same grants again. A lost latch creates another
    // child despite the already completed module.
    let source_money = source.get_player(1).unwrap().resources.supplies;
    let restored_money = restored.get_player(1).unwrap().resources.supplies;
    source.update();
    restored.update();
    assert_eq!(
        source.get_player(1).unwrap().resources.supplies,
        source_money + 1
    );
    assert_eq!(
        restored.get_player(1).unwrap().resources.supplies,
        restored_money + 1
    );
    assert_eq!(
        child_templates(&source, "SaveUpgradeMaster"),
        vec!["SaveUpgradeChild"]
    );
    assert_eq!(
        child_templates(&restored, "SaveUpgradeMaster"),
        vec!["SaveUpgradeChild"]
    );
}
