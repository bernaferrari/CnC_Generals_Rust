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
    action_kind: ScriptActionType,
    sound_name: &str,
) -> std::path::PathBuf {
    let mut action = ScriptAction::new(action_kind);
    if action_kind == ScriptActionType::SoundPlayNamed {
        action
            .add_parameter(Parameter::with_string(
                ParameterType::TextString,
                sound_name.into(),
            ))
            .unwrap();
    }
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedSoundUnit".into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "ObjectSoundOwner".into();
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
    map.open_data_chunk("Object", 3);
    for value in [50.0, 50.0, 0.0, 0.0] {
        map.write_real(value);
    }
    map.write_int(0);
    map.write_ascii_string("OwnedObjectSoundUnit");
    let mut properties = Dict::new();
    properties.set_ascii_string(
        NameKeyGenerator::name_to_key("objectName"),
        "NamedSoundUnit",
    );
    properties.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), "Owner");
    map.write_dict(&properties);
    map.close_data_chunk();
    map.close_data_chunk();
    sides.write_sides_data_chunk(&mut map, &ScriptTemplateLookup::from_engine(&script_engine));

    let path = root.join(format!("ObjectSoundOwner-{action_kind:?}.map"));
    std::fs::write(&path, map.into_ckmp_bytes()).expect("write map fixture");
    path
}

fn world(path: &std::path::Path) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "ObjectSoundOwner.ini".into(),
            "Object OwnedObjectSoundUnit\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(path.to_str().expect("UTF-8 map path")));
    let unit = world
        .host_objects()
        .registered_values()
        .find(|object| object.name == "NamedSoundUnit")
        .expect("map admits the named sound object")
        .id;
    (world, unit)
}

#[test]
fn map_authored_object_sound_actions_are_synchronous_and_world_owned() {
    let root = tempfile::tempdir().unwrap();
    let enabled_map = map_file(root.path(), ScriptActionType::EnableObjectSound, "unused");
    let disabled_map = map_file(root.path(), ScriptActionType::DisableObjectSound, "unused");
    let sound_map = map_file(root.path(), ScriptActionType::SoundPlayNamed, "UnitCheer");
    let (mut owner, owner_id) = world(&enabled_map);
    let (mut peer, peer_id) = world(&enabled_map);
    let (mut disabling, disabling_id) = world(&disabled_map);
    let (mut sound, sound_id) = world(&sound_map);
    assert_eq!(owner_id, peer_id);
    assert_eq!(owner_id, disabling_id);
    assert_eq!(owner_id, sound_id);
    owner
        .host_object_mut(owner_id)
        .unwrap()
        .ambient_sound_enabled_from_script = false;
    peer.host_object_mut(peer_id)
        .unwrap()
        .ambient_sound_enabled_from_script = false;
    disabling
        .host_object_mut(disabling_id)
        .unwrap()
        .ambient_sound_enabled_from_script = true;

    owner.update();
    assert!(
        owner
            .host_object(owner_id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );
    assert!(
        !peer
            .host_object(peer_id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );

    peer.update();
    assert!(
        peer.host_object(peer_id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );
    assert!(
        owner
            .host_object(owner_id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );

    disabling.update();
    assert!(
        !disabling
            .host_object(disabling_id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );
    assert!(
        owner
            .host_object(owner_id)
            .unwrap()
            .ambient_sound_enabled_from_script
    );

    // SOUND_PLAY_NAMED traverses the same receiving object-effect callback;
    // this headless fixture has no presentation-audio output to inspect.
    sound.update();
}
