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
            "Locomotor SharedOwnerLoco\n Surfaces = GROUND\n Speed = {speed}\n Acceleration = 60\n Braking = 80\n TurnRate = 120\n Appearance = TWO_LEGS\n PitchStiffness = {pitch}\n WanderWidthFactor = 2\n StickToGround = Yes\nEnd\n\
             Locomotor CliffOwnerLoco\n Surfaces = CLIFF\n Speed = {speed}\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n\
             Locomotor GroundOwnerLoco\n Surfaces = GROUND\n Speed = {}\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n\
             Locomotor PanicOwnerLoco\n Surfaces = GROUND\n Speed = {}\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n\
             Locomotor AirOwnerLoco\n Surfaces = AIR\n Speed = {}\n Acceleration = 60\n TurnRate = 120\n Appearance = WINGS\n MinSpeed = 10\nEnd\n\
             Locomotor TaxiOwnerLoco\n Surfaces = GROUND\n Speed = {}\n Acceleration = 60\n TurnRate = 120\n Appearance = FOUR_WHEELS\nEnd\n\
             Locomotor ThresholdOwnerLoco\n Surfaces = GROUND\n Speed = 20\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n",
            speed + 10,
            speed + 5,
            speed + 100,
            speed + 4,
        ))
        .unwrap();
    catalog
}

fn world(map: &std::path::Path, base_speed: u32, pitch: u32) -> GameLogic {
    world_with_catalog(map, base_catalog(base_speed, pitch))
}

fn world_with_catalog(map: &std::path::Path, catalog: LocomotorStore) -> GameLogic {
    let mut world = GameLogic::new();
    world.admit_locomotor_definitions(catalog).unwrap();
    world
        .admit_object_ini_sources([(
            "Owner.ini".into(),
            "Object OwnerMover\n KindOf = SELECTABLE VEHICLE\n Locomotor = SET_NORMAL SharedOwnerLoco\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n\
             Object OwnerSwitcher\n KindOf = SELECTABLE VEHICLE\n Locomotor = SET_NORMAL CliffOwnerLoco GroundOwnerLoco\n Locomotor = SET_PANIC PanicOwnerLoco\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n\
             Object OwnerBike\n KindOf = SELECTABLE VEHICLE\n Locomotor = SET_NORMAL CliffOwnerLoco GroundOwnerLoco\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = RiderChangeContain RiderTag\n Slots = 1\n Rider1 = OwnerRider RIDER1 WEAPON_RIDER1 STATUS_RIDER1 BikeSet SET_NORMAL\n End\nEnd\n\
             Object OwnerRider\n KindOf = SELECTABLE INFANTRY\n TransportSlotCount = 1\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n\
             Object OwnerJet\n KindOf = SELECTABLE AIRCRAFT\n Locomotor = SET_NORMAL AirOwnerLoco\n Locomotor = SET_TAXIING TaxiOwnerLoco\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = JetAIUpdate JetBrain\n End\nEnd\n\
             Object OwnerTargetProbe\n KindOf = SELECTABLE VEHICLE\n Locomotor = SET_NORMAL ThresholdOwnerLoco\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
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
        foreign.apply_source("Locomotor SharedOwnerLoco\n Surfaces = GROUND\n Speed = 999\n PitchStiffness = 99\n AirborneTargetingHeight = 0\nEnd\n\
            Locomotor CliffOwnerLoco\n Surfaces = GROUND\n Speed = 999\nEnd\n\
            Locomotor GroundOwnerLoco\n Surfaces = GROUND\n Speed = 999\nEnd\n\
            Locomotor PanicOwnerLoco\n Surfaces = GROUND\n Speed = 999\nEnd\n\
            Locomotor AirOwnerLoco\n Surfaces = AIR\n Speed = 999\nEnd\n\
            Locomotor TaxiOwnerLoco\n Surfaces = GROUND\n Speed = 999\nEnd\n\
            Locomotor ThresholdOwnerLoco\n Surfaces = GROUND\n Speed = 999\n AirborneTargetingHeight = 0\nEnd\n").unwrap();
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
    assert!(
        first
            .admit_locomotor_definitions(LocomotorStore::new())
            .is_err()
    );
    assert!(first.admit_locomotor_companion_sources(&[]).is_err());
    assert_eq!(
        first.host_object(first_id).unwrap().movement.max_speed,
        40.0
    );
    assert!((peer.host_object(peer_id).unwrap().movement.max_speed - 75.0).abs() < 0.001);
    assert_eq!(
        first.host_object(first_id).unwrap().wander_width_factor,
        2.0
    );
    let first_switcher = first
        .create_object_for_player("OwnerSwitcher", 42, glam::Vec3::new(50., 0., 50.))
        .unwrap();
    let peer_switcher = peer
        .create_object_for_player("OwnerSwitcher", 42, glam::Vec3::new(50., 0., 50.))
        .unwrap();
    assert_eq!(first_switcher, peer_switcher);
    assert!(first.move_object_to(first_switcher, glam::Vec3::new(100., 0., 50.)));
    assert!(peer.move_object_to(peer_switcher, glam::Vec3::new(100., 0., 50.)));
    let first_tick = first.update_with_dt(1.0 / 30.0);
    let peer_tick = peer.update_with_dt(1.0 / 30.0);
    assert_eq!((first_tick.steps_run, peer_tick.steps_run), (1, 1));
    // The source movement update first wakes at logic frame 1; the first
    // step above advances frame 0 to that wake boundary.
    assert_eq!(first.update_with_dt(1.0 / 30.0).steps_run, 1);
    assert_eq!(peer.update_with_dt(1.0 / 30.0).steps_run, 1);
    for (world, id, expected) in [(&first, first_switcher, 30.0), (&peer, peer_switcher, 70.0)] {
        let object = world.host_object(id).unwrap();
        assert_eq!(
            object.cur_locomotor_name.as_deref(),
            Some("GroundOwnerLoco"),
            "frame {}: position {:?}, goal {:?}, path {:?}, waiting {}, moving {}",
            world.get_frame(),
            object.get_position(),
            object.movement.target_position,
            object.movement.path,
            object.waiting_for_path,
            object.status.moving
        );
        assert!((object.movement.max_speed - expected).abs() < 0.001);
    }
    assert!(first.apply_unit_locomotor_set(first_switcher, "panic"));
    assert!(peer.apply_unit_locomotor_set(peer_switcher, "panic"));
    for (world, id, expected) in [(&first, first_switcher, 25.0), (&peer, peer_switcher, 65.0)] {
        let object = world.host_object(id).unwrap();
        assert_eq!(object.cur_locomotor_name.as_deref(), Some("PanicOwnerLoco"));
        assert!((object.movement.max_speed - expected).abs() < 0.001);
    }
    assert!(first.apply_unit_locomotor_set(first_switcher, "normal"));
    assert!(peer.apply_unit_locomotor_set(peer_switcher, "normal"));
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    let first_jet = first
        .create_object_for_player("OwnerJet", 42, glam::Vec3::new(80., 0., 120.))
        .unwrap();
    let peer_jet = peer
        .create_object_for_player("OwnerJet", 42, glam::Vec3::new(80., 0., 120.))
        .unwrap();
    assert_eq!(first_jet, peer_jet);
    let first_probe = first
        .create_object_for_player("OwnerTargetProbe", 42, glam::Vec3::new(0., 100., 0.))
        .unwrap();
    let peer_probe = peer
        .create_object_for_player("OwnerTargetProbe", 42, glam::Vec3::new(0., 100., 0.))
        .unwrap();
    assert_eq!(first_probe, peer_probe);
    for (world, id) in [(&mut first, first_probe), (&mut peer, peer_probe)] {
        let unit = world.host_object_mut(id).unwrap();
        assert!(unit.get_position().y > 10.0);
        unit.ground_height = 0.0;
        unit.stamp_airborne_target_from_locomotor();
        assert!(!unit.status.airborne_target);
    }
    assert_eq!(first.update_with_dt(1.0 / 30.0).steps_run, 1);
    assert_eq!(peer.update_with_dt(1.0 / 30.0).steps_run, 1);
    assert_eq!(first.update_with_dt(1.0 / 30.0).steps_run, 1);
    assert_eq!(peer.update_with_dt(1.0 / 30.0).steps_run, 1);
    for (world, id, expected) in [(&first, first_jet, 24.0), (&peer, peer_jet, 64.0)] {
        let jet = world.host_object(id).unwrap();
        assert_eq!(jet.cur_locomotor_name.as_deref(), Some("TaxiOwnerLoco"));
        assert!((jet.movement.max_speed - expected).abs() < 0.001);
    }
    let aircraft_snapshot = builder.create_world_snapshot(&first).unwrap();
    let mut aircraft_restored = world(&first_map, 20, 3);
    builder
        .restore_from_snapshot(&aircraft_snapshot, &mut aircraft_restored)
        .unwrap();
    let restored_jet = aircraft_restored.host_object(first_jet).unwrap();
    assert_eq!(
        restored_jet.cur_locomotor_name.as_deref(),
        Some("TaxiOwnerLoco")
    );
    assert!((restored_jet.movement.max_speed - 24.0).abs() < 0.001);
    let first_bike = first
        .create_object_for_player("OwnerBike", 42, glam::Vec3::new(80., 0., 80.))
        .unwrap();
    let peer_bike = peer
        .create_object_for_player("OwnerBike", 42, glam::Vec3::new(80., 0., 80.))
        .unwrap();
    assert_eq!(first_bike, peer_bike);
    for (world, id) in [(&first, first_bike), (&peer, peer_bike)] {
        let bike = world.host_object(id).unwrap();
        assert!(bike.supports_authored_rider_change_normal_enter());
        let rider = bike
            .authored_rider_change_rider_for_template("OwnerRider")
            .unwrap();
        assert_eq!(
            rider.active_locomotor_names,
            ["CliffOwnerLoco", "GroundOwnerLoco"]
        );
        assert_eq!(
            rider.active_locomotor_surfaces,
            generals_main::game_logic::object::LOCO_SURFACE_CLIFF
                | generals_main::game_logic::object::LOCO_SURFACE_GROUND
        );
    }
    let first_rider = first
        .create_object_for_player("OwnerRider", 42, glam::Vec3::new(81., 0., 80.))
        .unwrap();
    let peer_rider = peer
        .create_object_for_player("OwnerRider", 42, glam::Vec3::new(81., 0., 80.))
        .unwrap();
    assert_eq!(first_rider, peer_rider);
    for (world, bike, rider, expected_speed) in [
        (&mut first, first_bike, first_rider, 30.0),
        (&mut peer, peer_bike, peer_rider, 70.0),
    ] {
        use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
        assert!(world.can_unit_enter_normal_target(rider, bike));
        world.queue_command(GameCommand {
            command_type: CommandType::Enter { target_id: bike },
            player_id: 42,
            command_id: 1,
            timestamp: std::time::UNIX_EPOCH,
            selected_units: vec![rider],
            modifier_keys: ModifierKeys::default(),
        });
        world.process_commands();
        for _ in 0..4 {
            world.update_with_dt(1.0 / 30.0);
        }
        assert_eq!(world.host_object(rider).unwrap().contained_by, Some(bike));
        let bike = world.host_object(bike).unwrap();
        assert_eq!(bike.cur_locomotor_name.as_deref(), Some("GroundOwnerLoco"));
        assert!((bike.movement.max_speed - expected_speed).abs() < 0.001);
    }
    peer.reset();
    let mut restored = world(&first_map, 20, 3);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let restored_switcher = restored.host_object(first_switcher).unwrap();
    assert_eq!(
        restored_switcher.cur_locomotor_name.as_deref(),
        Some("GroundOwnerLoco")
    );
    assert!((restored_switcher.movement.max_speed - 30.0).abs() < 0.001);
    let restored = restored.host_object(first_id).unwrap();
    assert_eq!(restored.movement.max_speed, 40.0);
    assert_eq!(restored.wander_width_factor, 2.0);
    assert!(restored.stick_to_ground);
    let mut missing = world_with_catalog(&first_map, LocomotorStore::new());
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut missing)
            .is_err()
    );
    assert_eq!(
        first.host_object(first_id).unwrap().movement.max_speed,
        40.0
    );
}
