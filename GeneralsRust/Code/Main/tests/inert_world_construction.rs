use gamelogic::system::engine_stores::{
    active_services, install_services, new_world_services, uninstall_services,
};
use generals_main::game_logic::GameLogic;
use std::sync::Arc;

#[test]
fn world_construction_and_landmark_geometry_are_instance_owned() {
    let live = new_world_services();
    install_services(Arc::clone(&live));
    let first = GameLogic::initialize();
    assert!(Arc::ptr_eq(&active_services(), &live));
    let peer = GameLogic::initialize();
    assert!(Arc::ptr_eq(&active_services(), &live));
    drop(peer);
    drop(first);
    assert!(Arc::ptr_eq(&active_services(), &live));
    assert!(uninstall_services(&live));
    same_named_landmark_bridges_use_the_receiving_objects_geometry();
}

fn same_named_landmark_bridges_use_the_receiving_objects_geometry() {
    use game_engine::common::dict::Dict;
    use game_engine::common::system::DataChunkOutput;
    use generals_main::game_logic::{HostGeometryType, KindOf, ThingTemplate};

    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [21, 21, 1, 441] {
        map.write_int(value);
    }
    for _ in 0..441 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.open_data_chunk("Object", 3);
    for value in [40.0, 40.0, 10.0, 0.0] {
        map.write_real(value);
    }
    map.write_int(0);
    map.write_ascii_string("SharedLandmarkBridge");
    map.write_dict(&Dict::new());
    map.close_data_chunk();
    map.close_data_chunk();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Bridge.map");
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    let placements =
        generals_main::game_logic::script_loader::parse_object_placements(path.to_str().unwrap())
            .unwrap();
    assert_eq!(placements.len(), 1, "fixture must decode the bridge object");
    {
        // The road catalog is still an ambient dependency; this fixture holds
        // it constant while the two object-template owners intentionally differ.
        let mut roads = game_engine::common::ini::get_terrain_roads_mut();
        let bridge = roads.new_bridge("SharedLandmarkBridge".into());
        bridge.tower_object_name[0] = "SharedTower".into();
    }

    let create_world = |major: f32, minor: f32, tower_radius: f32| {
        let mut world = GameLogic::new();
        let mut template = ThingTemplate::new("SharedLandmarkBridge");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::LandmarkBridge)
            .add_kind_of(KindOf::Bridge)
            .set_health(400.0);
        template.geometry_info.geom_type = HostGeometryType::Box;
        template.geometry_info.major_radius = major;
        template.geometry_info.minor_radius = minor;
        template.geometry_info.authored = false;
        world.templates.insert(template.name.clone(), template);
        let mut tower = ThingTemplate::new("SharedTower");
        tower.add_kind_of(KindOf::Structure).set_health(100.0);
        tower.geometry_info.major_radius = tower_radius;
        world.templates.insert(tower.name.clone(), tower);
        assert!(world.load_map(path.to_str().unwrap()));
        assert_eq!(
            world.objects.registered_len(),
            2,
            "map admission must create the bridge and its authored tower"
        );
        world
    };
    let first = create_world(6.0, 2.0, 3.0);
    let peer = create_world(12.0, 4.0, 7.0);
    fn find<'a>(world: &'a GameLogic, name: &str) -> &'a generals_main::game_logic::Object {
        world
            .objects
            .registered_values()
            .find(|object| object.template_name == name)
            .unwrap()
    }
    let first_id = find(&first, "SharedLandmarkBridge").id;
    let peer_id = find(&peer, "SharedLandmarkBridge").id;
    assert_eq!(first_id, peer_id);
    assert!((find(&first, "SharedTower").get_position().z - 45.0).abs() < 0.01);
    assert!((find(&peer, "SharedTower").get_position().z - 51.0).abs() < 0.01);
    let query = glam::Vec3::new(40.0, 0.0, 43.0);
    let first_height = first.terrain_height_at(query).unwrap();
    let peer_height = peer.terrain_height_at(query).unwrap();
    let first_ground = first
        .terrain_height_at(glam::Vec3::new(40.0, 0.0, 48.0))
        .unwrap();
    let first_deck = first
        .terrain_height_at(glam::Vec3::new(40.0, 0.0, 40.0))
        .unwrap();
    let peer_object_height = peer.objects.get(&peer_id).unwrap().get_position().y;
    assert!((first_height - first_ground).abs() < 0.01);
    assert!(
        (peer_height - first_deck).abs() < 0.01,
        "peer height={peer_height}, first deck={first_deck}, first height={first_height}, ground={first_ground}",
    );
    assert!(
        (peer_height - peer_object_height).abs() < 0.01,
        "grounding must not lift the bridge Object onto its own deck"
    );
    assert!(peer_height > first_height + 9.0);
    drop(peer);
    assert_eq!(first.terrain_height_at(query), Some(first_height));
}
