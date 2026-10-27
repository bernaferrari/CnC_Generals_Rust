use game_engine::common::ini::ini_locomotor::{
    LocomotorStore, LocomotorSurfaceTypeMask, get_locomotor_store_mut,
};
use game_engine::common::system::DataChunkOutput;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};
use generals_main::save_load::SnapshotBuilder;

fn base_catalog(speed: u32, pitch: u32) -> LocomotorStore {
    let mut catalog = LocomotorStore::new();
    catalog
        .apply_source(&format!(
            "Locomotor SharedOwnerLoco\n Surfaces = GROUND\n Speed = {speed}\n Acceleration = 60\n Braking = 80\n TurnRate = 120\n Appearance = TWO_LEGS\n PitchStiffness = {pitch}\n WanderWidthFactor = 2\n StickToGround = Yes\nEnd\n"
        ))
        .unwrap();
    catalog
}

fn world(map: &std::path::Path, base_speed: u32, pitch: u32) -> GameLogic {
    let mut world = GameLogic::new();
    world.admit_locomotor_definitions(base_catalog(base_speed, pitch));
    world
        .admit_object_ini_sources([(
            "Owner.ini".into(),
            "Object OwnerMover\n KindOf = SELECTABLE VEHICLE\n Locomotor = SET_NORMAL SharedOwnerLoco\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(42, Team::USA, "Owner", true));
    world
}

#[test]
fn locomotors_follow_the_receiving_world_through_map_reset_and_restore() {
    // A mixed map source contains an Object field named Locomotor; it is not
    // itself a Locomotor definition. Overrides retain unspecified base fields.
    let mut parser = base_catalog(20, 3);
    let mixed = "Object Decoy\n Locomotor = SET_NORMAL Wrong\n Behavior = AIUpdateInterface Brain\n End\nEnd\nLocomotor SharedOwnerLoco\n Speed = 40\n Surfaces = +RUBBLE\nEnd\n";
    assert_eq!(parser.apply_source(mixed).unwrap(), 1);
    let row = parser.find_template("SharedOwnerLoco").unwrap();
    assert_eq!(row.pitch_stiffness, 3.0);
    assert_eq!(row.max_speed * 30.0, 40.0);
    assert_eq!(
        row.surfaces.0,
        LocomotorSurfaceTypeMask::GROUND | LocomotorSurfaceTypeMask::RUBBLE
    );
    let expected_surfaces = row.surfaces.0;
    assert!(parser.find_template("=").is_none());
    assert!(
        parser
            .apply_source("Locomotor SharedOwnerLoco\n Surfaces = +AIR GROUND\nEnd\n")
            .is_err()
    );
    assert_eq!(
        parser.find_template("SharedOwnerLoco").unwrap().surfaces.0,
        expected_surfaces
    );

    // A same-name foreign process catalog must never select either world’s data.
    {
        let mut foreign = get_locomotor_store_mut();
        foreign.apply_source("Locomotor SharedOwnerLoco\n Surfaces = GROUND\n Speed = 999\n PitchStiffness = 99\nEnd\n").unwrap();
    }
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
    map.close_data_chunk();
    let bytes = map.into_ckmp_bytes();
    let root = tempfile::tempdir().unwrap();
    let make_map = |name: &str, speed: u32| {
        let directory = root.path().join(name);
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("Owner.map");
        std::fs::write(&path, &bytes).unwrap();
        std::fs::write(
            directory.join("Map.ini"),
            format!("Locomotor SharedOwnerLoco\n Speed = {speed}\nEnd\n"),
        )
        .unwrap();
        path
    };
    let first_map = make_map("first", 40);
    let peer_map = make_map("peer", 75);
    let mut first = world(&first_map, 20, 3);
    let mut peer = world(&peer_map, 60, 7);
    let first_id = first
        .create_object_for_player("OwnerMover", 42, glam::Vec3::ZERO)
        .unwrap();
    let peer_id = peer
        .create_object_for_player("OwnerMover", 42, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(first_id, peer_id);
    assert_eq!(
        first.host_object(first_id).unwrap().movement.max_speed,
        40.0
    );
    assert!((peer.host_object(peer_id).unwrap().movement.max_speed - 75.0).abs() < 0.001);
    assert_eq!(
        first.host_object(first_id).unwrap().wander_width_factor,
        2.0
    );
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    peer.reset();
    let mut restored = world(&first_map, 20, 3);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let restored = restored.host_object(first_id).unwrap();
    assert_eq!(restored.movement.max_speed, 40.0);
    assert_eq!(restored.wander_width_factor, 2.0);
    assert!(restored.stick_to_ground);
    let mut missing = world(&first_map, 20, 3);
    missing.admit_locomotor_definitions(LocomotorStore::new());
    assert!(builder.restore_from_snapshot(&snapshot, &mut missing).is_err());
    assert_eq!(
        first.host_object(first_id).unwrap().movement.max_speed,
        40.0
    );
}
