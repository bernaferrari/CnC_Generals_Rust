use game_engine::common::ini::ini_locomotor::LocomotorStore;
use game_engine::common::system::DataChunkOutput;
use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
use generals_main::game_logic::pathfinding::PendingHostPath;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use glam::Vec3;

fn world(map: &std::path::Path, human: bool) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut locomotors = LocomotorStore::new_owned();
    locomotors
        .apply_source(
            "\
Locomotor TestWalk
  Surfaces = GROUND
  Speed = 30
  Acceleration = 60
  TurnRate = 360
  Appearance = TWO_LEGS
End
",
        )
        .unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world
        .admit_object_ini_sources([(
            "TestMover.ini".into(),
            "\
Object Mover
  KindOf = SELECTABLE INFANTRY
  Locomotor = SET_NORMAL TestWalk
  Body = ActiveBody Body
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = AIUpdateInterface Brain
  End
End
"
            .into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.clear_all_players();
    world.add_player(Player::new(0, Team::USA, "Mover owner", human));
    world.add_player(Player::new(1, Team::GLA, "Other player", !human));
    assert!(world.select_local_player(Some(if human { 1 } else { 0 })));
    let from = Vec3::new(25.0, 0.0, 55.0);
    let mover = world.create_object_for_player("Mover", 0, from).unwrap();
    // Let normal admission register the idle object's pathfinding footprint.
    world.update_with_dt(1.0 / 30.0);
    world.update_with_dt(1.0 / 30.0);
    let unit = world.host_object_mut(mover).unwrap();
    unit.is_approach_path = false;
    unit.is_final_goal = true;
    unit.requested_destination = Some(Vec3::new(145.0, 0.0, 55.0));
    unit.waiting_for_path = true;
    unit.locomotor_surfaces = SURFACE_GROUND;
    world.restore_pending_host_paths([PendingHostPath {
        unit_id: mover,
        start: from,
        destination: Vec3::new(145.0, 0.0, 55.0),
        waypoints: Vec::new(),
        follow_path_appends: Vec::new(),
        aircraft: false,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        ignore_obstacle: None,
        adjust_destinations: false,
    }]);
    (world, mover)
}

#[test]
fn queued_ordinary_path_uses_controller_type_and_survives_save_load() {
    let root = tempfile::tempdir().unwrap();
    let map = root.path().join("OrdinaryController.map");
    let mut data = DataChunkOutput::new();
    data.open_data_chunk("HeightMapData", 4);
    // The physical map is larger than the human-controlled area.
    for value in [41, 41, 1, 1, 10, 10, 1681] {
        data.write_int(value);
    }
    for _ in 0..1681 {
        data.write_byte(0);
    }
    data.close_data_chunk();
    data.open_data_chunk("ObjectsList", 3);
    data.close_data_chunk();
    std::fs::write(&map, data.into_ckmp_bytes()).unwrap();
    let (mut computer, computer_id) = world(&map, false);
    let (mut human, human_id) = world(&map, true);
    let (mut no_controller, no_controller_id) = world(&map, false);
    no_controller.clear_all_players();
    assert_eq!(computer_id, human_id);
    assert_eq!(computer_id, no_controller_id);

    // Save before the queued search executes and resume it in another session.
    let builder = generals_main::save_load::SnapshotBuilder::new();
    let mut saved = builder.create_world_snapshot(&no_controller).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    use generals_main::save_load::{Snapshot, XferLoad, XferSave};
    saved.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut saved = generals_main::save_load::WorldSnapshot::default();
    saved
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let (mut restored, restored_id) = world(&map, true);
    assert_eq!(no_controller_id, restored_id);
    builder
        .restore_from_snapshot(&saved, &mut restored)
        .unwrap();
    assert_eq!(restored.snapshot_pending_host_paths().len(), 1);
    assert_ne!(
        no_controller
            .host_object(no_controller_id)
            .unwrap()
            .pathfind_cur_cell,
        (-1, -1),
        "the original mover must be admitted to the pathfinding grid before saving"
    );
    assert_eq!(
        restored.host_object(restored_id).unwrap().pathfind_cur_cell,
        no_controller
            .host_object(no_controller_id)
            .unwrap()
            .pathfind_cur_cell
    );

    computer.update_with_dt(1.0 / 30.0);
    human.update_with_dt(1.0 / 30.0);
    no_controller.update_with_dt(1.0 / 30.0);
    restored.update_with_dt(1.0 / 30.0);
    let c = computer.host_object(computer_id).unwrap();
    let h = human.host_object(human_id).unwrap();
    let missing = no_controller.host_object(no_controller_id).unwrap();
    let restored_unit = restored.host_object(restored_id).unwrap();
    let destination = Vec3::new(145.0, 0.0, 55.0);
    assert_eq!(
        c.movement.path.last().copied(),
        Some(destination),
        "computer must reach beyond the human extent"
    );
    assert!(
        h.movement.path.last().is_some_and(|end| end.x < 100.0),
        "human search must remain in the allowed extent"
    );
    assert_eq!(
        missing.movement.path, h.movement.path,
        "a missing controller must use human policy"
    );
    assert_eq!(
        restored_unit.movement.path, missing.movement.path,
        "queued search must continue with the same controller policy after save/load"
    );
    assert!(!restored_unit.waiting_for_path);
    assert!(restored.snapshot_pending_host_paths().is_empty());
}
