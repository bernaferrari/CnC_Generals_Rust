use game_engine::common::dict::Dict;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId};
use generals_main::save_load::{SnapshotBuilder, WorldSnapshot};
use glam::Vec3;

fn map_file(root: &std::path::Path) -> std::path::PathBuf {
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
    for value in [50.0, 50.0, 0.0, 0.0] {
        out.write_real(value);
    }
    out.write_int(0);
    out.write_ascii_string("Waypoint");
    let mut properties = Dict::new();
    properties.set_int(NameKeyGenerator::name_to_key("waypointID"), 701);
    properties.set_ascii_string(
        NameKeyGenerator::name_to_key("waypointName"),
        "GarrisonOwner",
    );
    out.write_dict(&properties);
    out.close_data_chunk();
    out.close_data_chunk();
    let path = root.join("GarrisonOwner.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}
fn no_team_world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "NoTeamGarrison.ini".into(),
            "Object NoTeamGarrison\n\
 KindOf = STRUCTURE SELECTABLE\n\
 Body = ActiveBody BodyTag\n\
  MaxHealth = 500\n\
  InitialHealth = 500\n\
 End\n\
 Behavior = GarrisonContain GarrisonTag\n\
  ContainMax = 4\n\
 End\n\
End\n\
Object NoTeamRider\n\
 KindOf = INFANTRY SELECTABLE\n\
 TransportSlotCount = 1\n\
 Body = ActiveBody BodyTag\n\
  MaxHealth = 100\n\
  InitialHealth = 100\n\
 End\n\
End\n"
                .into(),
        )])
        .expect("admit no-Team Garrison fixture");
    world.start_new_game(GameMode::SinglePlayer);
    let map_root = tempfile::tempdir().unwrap();
    let map = map_file(map_root.path());
    assert!(world.load_map(map.to_str().unwrap()));
    let host = world
        .create_object(
            "NoTeamGarrison",
            generals_main::game_logic::Team::Neutral,
            Vec3::ZERO,
        )
        .expect("create unowned Garrison");
    let rider = world
        .create_object(
            "NoTeamRider",
            generals_main::game_logic::Team::Neutral,
            Vec3::ZERO,
        )
        .expect("create unowned rider");
    assert_eq!(
        world.host_object(host).unwrap().team_instance_identity(),
        None
    );
    assert_eq!(
        world.host_object(rider).unwrap().team_instance_identity(),
        None
    );
    (world, host, rider)
}

#[test]
fn null_team_garrison_recalculation_and_save_keep_null_original_identity() {
    let (mut source, host, rider) = no_team_world();
    assert!(source.host_object_mut(host).unwrap().add_occupant(rider));
    assert!(source.unit_command_remove_occupant(host, rider));
    let building = source
        .host_object(host)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap();
    assert_eq!(
        source.host_object(host).unwrap().team_instance_identity(),
        None
    );
    assert_eq!(building.original_team, None);
    assert_eq!(building.original_garrison_player_id, None);
    assert_eq!(building.original_team_instance_id, None);
    assert_eq!(building.original_team_instance_name, None);

    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .expect("save null-Team empty Garrison");
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
    let (mut receiving, _, _) = no_team_world();
    SnapshotBuilder::new()
        .restore_from_snapshot(&decoded, &mut receiving)
        .expect("restore null-Team Garrison");
    let restored = receiving.host_object(host).unwrap();
    assert_eq!(restored.team_instance_identity(), None);
    let building = restored.building_data.as_ref().unwrap();
    assert_eq!(building.original_team, None);
    assert_eq!(building.original_team_instance_id, None);
    assert_eq!(building.original_team_instance_name, None);
}
