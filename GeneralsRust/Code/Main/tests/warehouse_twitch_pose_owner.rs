use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{
    AIState, DockKind, GameLogic, GameMode, HostGeometryInfo, HostGeometryType, ObjectId, Player,
    SupplyTruckMetadata, SupplyTruckState, Team, host_supply_gather,
};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;
use std::path::Path;

fn map_file(root: &Path, area: (f32, f32), area_name: &str) -> std::path::PathBuf {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();

    let (x, z) = area;
    let lo_x = (x.floor() as i32) - 1;
    let hi_x = (x.ceil() as i32) + 1;
    let lo_z = (z.floor() as i32) - 1;
    let hi_z = (z.ceil() as i32) + 1;
    out.open_data_chunk("PolygonTriggers", 4);
    out.write_int(1);
    out.write_ascii_string(area_name);
    out.write_ascii_string("WarehouseTwitchLayer");
    out.write_int(2901);
    out.write_byte(1);
    out.write_byte(0);
    out.write_int(0);
    out.write_int(4);
    for (px, pz) in [(lo_x, lo_z), (hi_x, lo_z), (hi_x, hi_z), (lo_x, hi_z)] {
        out.write_int(px);
        out.write_int(pz);
        out.write_int(0);
    }
    out.close_data_chunk();

    out.open_data_chunk("ObjectsList", 3);
    out.close_data_chunk();
    let path = root.join(format!("{area_name}.map"));
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &Path) -> GameLogic {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(
        Player::new(1, Team::USA, "Warehouse Owner", true).with_script_identity("Warehouse Owner"),
    );

    let geometry = HostGeometryInfo {
        geom_type: HostGeometryType::Sphere,
        is_small: false,
        height: 1.0,
        major_radius: 0.1,
        minor_radius: 0.1,
        authored: true,
    };
    // The live AI state must come from an admitted source so the actual
    // world snapshot can validate and preserve its continuation.
    world
        .admit_object_ini_sources([(
            "WarehouseTwitch.ini".into(),
            "Object TwitchTruck\n KindOf = VEHICLE HARVESTER\n Body = ActiveBody BodyTag\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = AIUpdateInterface BaseAI\n End\nEnd\nObject TwitchWarehouse\n KindOf = STRUCTURE SUPPLY_SOURCE\n Body = ActiveBody BodyTag\n MaxHealth = 1000\n InitialHealth = 1000\n End\nEnd\n".into(),
        )])
        .expect("receiving truck AI source");
    let truck = world
        .templates
        .get_mut("TwitchTruck")
        .expect("truck template");
    truck.geometry_info = geometry;
    truck.supply_truck_metadata = Some(SupplyTruckMetadata {
        max_boxes: 4,
        warehouse_scan_distance: 100.0,
        warehouse_delay_frames: 0,
        center_delay_frames: 0,
        upgraded_supply_boost: 0,
    });
    let warehouse = world
        .templates
        .get_mut("TwitchWarehouse")
        .expect("warehouse template");
    warehouse.geometry_info = geometry;
    warehouse.dock_kind = DockKind::SupplyWarehouse;

    world
}

fn prepare_world(mut world: GameLogic, idle_frames: u32) -> (GameLogic, ObjectId) {
    for _ in 0..idle_frames {
        assert_eq!(world.update_with_dt(1.0 / 30.0).steps_run, 1);
    }
    let warehouse = world
        .create_object(
            "TwitchWarehouse",
            Team::Neutral,
            Vec3::new(100.0, 0.0, 100.0),
        )
        .unwrap();
    let truck = world
        .create_object_for_player("TwitchTruck", 1, Vec3::new(100.7, 0.0, 100.0))
        .unwrap();
    let blocker = world
        .create_object_for_player("TwitchTruck", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    assert_eq!(
        (warehouse, truck, blocker),
        (ObjectId(1), ObjectId(2), ObjectId(3))
    );
    world
        .host_object_mut(warehouse)
        .unwrap()
        .set_stored_supplies(400);

    world
        .unit_command_set_ai_state(blocker, AIState::Gathering)
        .unwrap();
    world
        .unit_command_set_ai_state(truck, AIState::Gathering)
        .unwrap();
    let frame = world.get_frame();
    {
        let object = world.host_object_mut(truck).unwrap();
        object.target = Some(warehouse);
        object.supply_truck_state = SupplyTruckState::DockingWarehouse;
        object.supply_truck_next_dock_action_frame = frame;
    }
    {
        let object = world.host_object_mut(blocker).unwrap();
        object.target = Some(warehouse);
        object.supply_truck_state = SupplyTruckState::DockingWarehouse;
        object.supply_truck_next_dock_action_frame = frame;
    }
    world.host_object_mut(warehouse).unwrap().dock_active_docker = Some(blocker);
    (world, truck)
}

#[test]
fn live_warehouse_twitch_updates_only_the_receiving_world_trigger() {
    let twitch_frame = (0..128)
        .find(|frame| {
            let seed = host_supply_gather::twitch_seed(ObjectId(2), *frame);
            let (dx, dz) = host_supply_gather::warehouse_twitch_delta(seed, 1);
            dx.abs() > 1.5 && dz.abs() > 1.5
        })
        .expect("find deterministic twitch outside the initial pose");
    let (dx, dz) = host_supply_gather::warehouse_twitch_delta(
        host_supply_gather::twitch_seed(ObjectId(2), twitch_frame),
        1,
    );
    let expected = (100.7 + dx, 100.0 + dz);
    let root = tempfile::tempdir().unwrap();
    let owner_map = map_file(root.path(), expected, "OwnerWarehouseTwitchArea");
    let peer_map = map_file(
        root.path(),
        (expected.0 + 70.0, expected.1 + 70.0),
        "PeerWarehouseTwitchArea",
    );
    let mut owner = prepare_world(world(&owner_map), twitch_frame);
    let mut peer = prepare_world(world(&peer_map), twitch_frame);
    assert_eq!(owner.1, peer.1);
    assert_eq!(owner.0.get_frame(), twitch_frame);
    assert_eq!(peer.0.get_frame(), twitch_frame);

    for (logic, truck) in [(&mut owner.0, owner.1), (&mut peer.0, peer.1)] {
        assert_eq!(logic.update_with_dt(1.0 / 30.0).steps_run, 1);
        assert_eq!(
            logic.host_object(ObjectId(1)).unwrap().dock_active_docker,
            Some(ObjectId(3)),
            "the competing docker must retain its valid warehouse claim"
        );
        let position = logic.host_object(truck).unwrap().get_position();
        assert!(
            (position.x - expected.0).abs() < 0.001,
            "{position:?} {expected:?}"
        );
        assert!(
            (position.z - expected.1).abs() < 0.001,
            "{position:?} {expected:?}"
        );
    }

    let owner_snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&owner.0)
        .unwrap();
    let owner_trigger = owner_snapshot
        .object_triggers
        .iter()
        .find(|entry| entry.object_id == owner.1)
        .unwrap();
    assert!(owner_trigger.slots.iter().any(|slot| {
        slot.trigger_name == "OwnerWarehouseTwitchArea" && slot.entered && slot.is_inside
    }));

    let peer_snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&peer.0)
        .unwrap();
    let peer_trigger = peer_snapshot
        .object_triggers
        .iter()
        .find(|entry| entry.object_id == peer.1)
        .unwrap();
    assert!(
        !peer_trigger
            .slots
            .iter()
            .any(|slot| slot.trigger_name == "PeerWarehouseTwitchArea" && slot.is_inside)
    );
}
