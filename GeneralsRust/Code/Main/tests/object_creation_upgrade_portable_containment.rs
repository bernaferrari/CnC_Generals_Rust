//! Public integration coverage for authored ObjectCreationUpgrade OCL containment.
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
use generals_main::game_logic::{GameLogic, GameMode, ObjectId};
use generals_main::save_load::{SnapshotBuilder, WorldSnapshot};

const TRIGGER: &str = "Upgrade_Veterancy_VETERAN";
const HELIX_NAME: &str = "ArbitraryAuthoredHelixHost";
const OBJECT_NAME: &str = "PortableUpgradeHelix";

fn receive_upgrade_script() -> Script {
    let mut action = ScriptAction::new(ScriptActionType::NamedReceiveUpgrade);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            OBJECT_NAME.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            TRIGGER.into(),
        ))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "ReceivePortableUpgrade".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    script
}

fn world(
    root: &std::path::Path,
    label: &str,
    host_template: &str,
    has_helix_contain: bool,
) -> (GameLogic, ObjectId, [String; 3]) {
    let first = format!("{label}ArbitraryPortableOne");
    let second = format!("{label}ArbitraryPortableTwo");
    let deceptive = format!("{label}HelixGattlingPortablePretender");
    let mut game = GameLogic::new();
    game.admit_object_creation_list_sources([
        format!(
            "ObjectCreationList OCL_PortableOne\nCreateObject\nObjectNames = {first}\nCount = 1\nContainInsideSourceObject = Yes\nEnd\nEnd\n"
        ),
        format!(
            "ObjectCreationList OCL_PortableTwo\nCreateObject\nObjectNames = {second}\nCount = 1\nContainInsideSourceObject = Yes\nEnd\nEnd\n"
        ),
        format!(
            "ObjectCreationList OCL_DeceptivePortable\nCreateObject\nObjectNames = {deceptive}\nCount = 1\nContainInsideSourceObject = Yes\nEnd\nEnd\n"
        ),
    ])
    .unwrap();
    let contain = if has_helix_contain {
        "Behavior = HelixContain HelixHold\n Slots = 5\nEnd\n"
    } else {
        ""
    };
    game.admit_object_ini_sources([(
        format!("{label}Portable.ini"),
        format!(
            "Object {host_template}\nKindOf = VEHICLE AIRCRAFT SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\n{contain}Behavior = ObjectCreationUpgrade FirstPortable\n TriggeredBy = {TRIGGER}\n UpgradeObject = OCL_PortableOne\nEnd\nBehavior = ObjectCreationUpgrade SecondPortable\n TriggeredBy = {TRIGGER}\n UpgradeObject = OCL_PortableTwo\nEnd\nBehavior = ObjectCreationUpgrade DeceptivePortable\n TriggeredBy = {TRIGGER}\n UpgradeObject = OCL_DeceptivePortable\nEnd\nEnd\nObject {first}\nKindOf = VEHICLE PORTABLE_STRUCTURE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\nObject {second}\nKindOf = VEHICLE PORTABLE_STRUCTURE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\nObject {deceptive}\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n"
        ),
    )])
    .unwrap();

    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(receive_upgrade_script()));
    let mut sides = SidesList::new();
    let mut owner = Dict::new();
    owner.set_ascii_string(key_player_name(), "Owner");
    owner.set_ascii_string(key_player_faction(), "FactionChina");
    owner.set_unicode_string(key_player_display_name(), "Owner");
    owner.set_bool(key_player_is_human(), true);
    sides.add_side(&owner);
    let mut team = Dict::new();
    team.set_ascii_string(key_team_name(), "HelixTeam");
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
    map.write_ascii_string(host_template);
    let mut properties = Dict::new();
    properties.set_ascii_string(NameKeyGenerator::name_to_key("objectName"), OBJECT_NAME);
    properties.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), "HelixTeam");
    map.write_dict(&properties);
    map.close_data_chunk();
    map.close_data_chunk();
    sides.write_sides_data_chunk(&mut map, &ScriptTemplateLookup::from_engine(&script_engine));

    let path = root.join(format!("{label}Portable.map"));
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    game.start_new_game(GameMode::SinglePlayer);
    assert!(game.load_map(path.to_str().expect("UTF-8 map path")));
    let host = game
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == host_template)
        .expect("map-created Helix")
        .id;
    assert_eq!(game.host_object(host).unwrap().name, OBJECT_NAME);
    assert_eq!(
        game.host_object(host).unwrap().is_helix_transport,
        has_helix_contain
    );
    (game, host, [first, second, deceptive])
}

fn find_template<'a>(
    game: &'a GameLogic,
    template: &str,
) -> Vec<&'a generals_main::game_logic::Object> {
    game.host_objects()
        .registered_values()
        .filter(|object| object.template_name == template)
        .collect()
}

fn run_authored_upgrade(game: &mut GameLogic) {
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(receive_upgrade_script()));
    let engine = ScriptEngine::new().expect("script templates");
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    game.update_local_script_engine_with_owner(&engine)
        .expect("owned upgrade script");
}

#[test]
fn authored_upgrade_ocl_uses_kindof_and_receiving_world_containment() {
    let root = tempfile::tempdir().unwrap();
    let (mut owner, owner_host, owner_children) = world(root.path(), "Owner", HELIX_NAME, true);
    let (mut peer, peer_host, peer_children) = world(root.path(), "Peer", HELIX_NAME, true);
    assert_eq!(
        owner_host, peer_host,
        "the worlds must reuse the same host ID"
    );

    run_authored_upgrade(&mut owner);
    let owner_host_object = owner.host_object(owner_host).unwrap();
    let first = find_template(&owner, &owner_children[0]);
    let second = find_template(&owner, &owner_children[1]);
    let deceptive = find_template(&owner, &owner_children[2]);
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_eq!(
        owner_host_object.overlord_portable_occupant,
        Some(first[0].id)
    );
    assert_eq!(first[0].contained_by, Some(owner_host));
    assert!(
        second[0].status.destroyed,
        "the second OCU sees an occupied Helix slot"
    );
    assert!(
        deceptive.is_empty(),
        "nonportable candidate is skipped before creation"
    );
    assert_eq!(peer_host, owner_host);
    assert!(find_template(&peer, &peer_children[0]).is_empty());
    assert!(find_template(&peer, &peer_children[1]).is_empty());
    assert!(find_template(&peer, &peer_children[2]).is_empty());

    run_authored_upgrade(&mut peer);
    let peer_host_object = peer.host_object(peer_host).unwrap();
    let peer_first = find_template(&peer, &peer_children[0]);
    let peer_second = find_template(&peer, &peer_children[1]);
    let peer_deceptive = find_template(&peer, &peer_children[2]);
    assert_eq!(peer_first.len(), 1);
    assert_eq!(peer_second.len(), 1);
    assert_eq!(
        peer_host_object.overlord_portable_occupant,
        Some(peer_first[0].id)
    );
    assert_eq!(peer_first[0].contained_by, Some(peer_host));
    assert!(peer_second[0].status.destroyed);
    assert!(peer_deceptive.is_empty());
    assert_eq!(
        owner
            .host_object(owner_host)
            .unwrap()
            .overlord_portable_occupant,
        Some(first[0].id)
    );
    assert_eq!(owner.host_object(owner_host).unwrap().contained_by, None);

    let saved = SnapshotBuilder::new()
        .create_world_snapshot(&owner)
        .expect("snapshot authored HelixContain world");
    let bytes = bincode_legacy::serialize(&saved).expect("serialize positional world snapshot");
    let decoded: WorldSnapshot =
        bincode_legacy::deserialize(&bytes).expect("deserialize positional world snapshot");
    let (mut restored, restored_host, _) = world(root.path(), "Owner", HELIX_NAME, true);
    assert_eq!(restored_host, owner_host);
    SnapshotBuilder::new()
        .restore_from_snapshot(&decoded, &mut restored)
        .expect("restore using receiving HelixContain template");
    let restored_host_object = restored.host_object(restored_host).unwrap();
    assert!(restored_host_object.is_helix_transport);
    assert_eq!(
        restored_host_object.overlord_portable_occupant,
        Some(first[0].id)
    );
}

#[test]
fn helix_name_without_contain_module_does_not_admit_portable_ocl() {
    let root = tempfile::tempdir().unwrap();
    let (mut world, host, children) = world(root.path(), "NameOnly", "ChinaJetHelix", false);
    run_authored_upgrade(&mut world);
    assert!(!world.host_object(host).unwrap().is_helix_transport);
    for child in children {
        assert!(find_template(&world, &child).is_empty());
    }
}
