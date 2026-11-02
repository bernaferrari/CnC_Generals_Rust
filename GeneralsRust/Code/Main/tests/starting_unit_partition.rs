//! Starting builders must not spawn inside their command center.
use game_engine::common::{
    dict::Dict, ini::INILoadType, name_key_generator::NameKeyGenerator,
    rts::player_template::PlayerTemplateStore, system::DataChunkOutput,
};
use generals_main::game_logic::GameLogic;
use generals_main::skirmish_config::{
    SkirmishPlayerTemplateSelection, apply_skirmish_config, golden_skirmish_config,
};

fn map_file(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("StartingUnits.map");
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [101, 101, 1, 10201] {
        map.write_int(value);
    }
    for _ in 0..10201 {
        map.write_byte(0);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    for (id, name, x, z) in [
        (1, "Player_1_Start", 400.0, 400.0),
        (2, "Player_1_Rally", 400.0, 400.0),
    ] {
        let mut properties = Dict::new();
        properties.set_int(NameKeyGenerator::name_to_key("waypointID"), id);
        properties.set_ascii_string(NameKeyGenerator::name_to_key("waypointName"), name);
        map.open_data_chunk("Object", 3);
        for value in [x, z, 0.0, 0.0] {
            map.write_real(value);
        }
        map.write_int(0);
        map.write_ascii_string("Waypoint");
        map.write_dict(&properties);
        map.close_data_chunk();
    }
    map.close_data_chunk();
    map.open_data_chunk("SidesList", 2);
    map.write_int(0);
    map.write_int(0);
    map.close_data_chunk();
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world.admit_object_ini_sources([("start.ini".to_string(), "Object StartYard\n KindOf = STRUCTURE COMMANDCENTER SELECTABLE\n Geometry = BOX\n GeometryMajorRadius = 30\n GeometryMinorRadius = 30\n GeometryHeight = 20\n Body = ActiveBody Body\n MaxHealth = 1000\n InitialHealth = 1000\n End\nEnd\nObject StartDozer\n KindOf = VEHICLE DOZER SELECTABLE\n Geometry = BOX\n GeometryMajorRadius = 5\n GeometryMinorRadius = 5\n GeometryHeight = 5\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".to_string())]).unwrap();
    let mut definitions = PlayerTemplateStore::new_owned();
    definitions.apply_source("PlayerTemplate FactionAmerica\n Side = America\n BaseSide = America\n StartingBuilding = StartYard\n StartingUnit0 = StartDozer\nEnd\n", INILoadType::Overwrite).unwrap();
    let index = definitions.find_template_index("FactionAmerica").unwrap() as i32;
    world
        .admit_player_template_definitions(definitions)
        .unwrap();
    let mut config = golden_skirmish_config(map.to_str().unwrap());
    config.slots.truncate(1);
    config.slots[0].player_template = SkirmishPlayerTemplateSelection::Exact {
        template_name: "FactionAmerica".into(),
        template_index: index,
    };
    apply_skirmish_config(&mut world, &config).unwrap();
    assert!(world.load_map(map.to_str().unwrap()));
    world
}

#[test]
fn starting_dozer_uses_clear_partition_position_and_is_world_owned() {
    let dir = tempfile::tempdir().unwrap();
    let map = map_file(dir.path());
    let first = world(&map);
    let second = world(&map);
    for world in [&first, &second] {
        let objects = world.get_objects();
        let yard = objects
            .registered_values()
            .find(|o| o.template_name == "StartYard")
            .expect("starting yard");
        let dozer = objects
            .registered_values()
            .find(|o| o.template_name == "StartDozer")
            .expect("starting dozer");
        let delta = dozer.get_position() - yard.get_position();
        assert!(
            delta.x.abs() > 30.0 || delta.z.abs() > 30.0,
            "dozer spawned inside the yard: {delta:?}"
        );
        assert_eq!(dozer.owner_player_id, yard.owner_player_id);
    }
}
