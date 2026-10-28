use game_engine::common::{
    dict::Dict,
    name_key_generator::NameKeyGenerator,
    system::DataChunkOutput,
    well_known_keys::{
        key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
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

fn map_file(
    root: &std::path::Path,
    goal: Option<(&str, [f32; 3])>,
    action_goal: &str,
) -> std::path::PathBuf {
    let mut action = ScriptAction::new(ScriptActionType::MoveNamedUnitTo);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedUnit".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Waypoint,
            action_goal.into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnedNamedMove".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));

    let mut sides = SidesList::new();
    let mut owner = Dict::new();
    owner.set_ascii_string(key_player_name(), "Owner");
    owner.set_ascii_string(key_player_faction(), "FactionAmerica");
    owner.set_unicode_string(key_player_display_name(), "Owner");
    owner.set_bool(key_player_is_human(), true);
    sides.add_side(&owner);
    sides
        .get_side_info_mut(0)
        .expect("owner side")
        .set_script_list(Some(Box::new(scripts)));
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
    if let Some((name, location)) = goal {
        map.open_data_chunk("Object", 3);
        for value in [location[0], location[2], location[1], 0.0] {
            map.write_real(value);
        }
        map.write_int(0);
        map.write_ascii_string("Waypoint");
        let mut properties = Dict::new();
        properties.set_int(NameKeyGenerator::name_to_key("waypointID"), 701);
        properties.set_ascii_string(NameKeyGenerator::name_to_key("waypointName"), name);
        map.write_dict(&properties);
        map.close_data_chunk();
    }
    map.open_data_chunk("Object", 3);
    for value in [50.0, 50.0, 0.0, 0.0] {
        map.write_real(value);
    }
    map.write_int(0);
    map.write_ascii_string("OwnedNamedMoveUnit");
    let mut properties = Dict::new();
    properties.set_ascii_string(NameKeyGenerator::name_to_key("objectName"), "NamedUnit");
    properties.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), "Owner");
    map.write_dict(&properties);
    map.close_data_chunk();
    map.close_data_chunk();
    sides.write_sides_data_chunk(&mut map, &ScriptTemplateLookup::from_engine(&script_engine));

    let path = root.join(format!("NamedMoveOwner-{action_goal}.map"));
    std::fs::write(&path, map.into_ckmp_bytes()).expect("write map fixture");
    path
}

fn world(path: &std::path::Path) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "NamedMoveOwner.ini".into(),
            "Object OwnedNamedMoveUnit\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(path.to_str().expect("UTF-8 map path")));
    let unit = world
        .host_objects()
        .registered_values()
        .find(|object| object.name == "NamedUnit")
        .expect("map admits the named unit")
        .id;
    (world, unit)
}

#[test]
fn map_authored_named_move_uses_each_receiving_world_and_missing_waypoint_is_noop() {
    let root = tempfile::tempdir().unwrap();
    let owner_map = map_file(
        root.path(),
        Some(("OwnerGoal", [180.0, 0.0, 210.0])),
        "OwnerGoal",
    );
    let peer_map = map_file(
        root.path(),
        Some(("PeerGoal", [160.0, 0.0, 170.0])),
        "PeerGoal",
    );
    let missing_map = map_file(root.path(), None, "MissingGoal");
    let (mut owner, owner_unit) = world(&owner_map);
    let (mut peer, peer_unit) = world(&peer_map);
    let (mut missing, missing_unit) = world(&missing_map);
    assert_eq!(owner_unit, peer_unit);
    assert_eq!(owner_unit, missing_unit);
    owner
        .host_object_mut(owner_unit)
        .unwrap()
        .set_formation(17, glam::Vec2::ZERO);
    peer.host_object_mut(peer_unit)
        .unwrap()
        .set_formation(29, glam::Vec2::ZERO);

    // Main's ordinary update executes each map-authored ScriptEngine through
    // its receiving HostScriptExecutionDriver.
    owner.update();
    assert_eq!(
        owner.host_object(owner_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(180.0, 0.0, 210.0))
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        None
    );
    assert_eq!(owner.host_object(owner_unit).unwrap().formation_id, 0);
    assert_eq!(peer.host_object(peer_unit).unwrap().formation_id, 29);

    peer.update();
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(160.0, 0.0, 170.0))
    );
    assert_eq!(
        owner.host_object(owner_unit).unwrap().requested_destination,
        Some(glam::Vec3::new(180.0, 0.0, 210.0))
    );
    assert_eq!(peer.host_object(peer_unit).unwrap().formation_id, 0);

    missing
        .host_object_mut(missing_unit)
        .unwrap()
        .set_formation(31, glam::Vec2::ZERO);
    missing.update();
    let failed = missing.host_object(missing_unit).unwrap();
    assert_eq!(failed.formation_id, 31);
    assert_eq!(failed.requested_destination, None);
}
