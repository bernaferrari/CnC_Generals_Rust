use game_engine::common::ini::ini_locomotor::LocomotorStore;
use game_engine::common::system::DataChunkOutput;
use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
use generals_main::game_logic::pathfinding::PendingHostPath;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use glam::Vec3;

fn world(map: &std::path::Path, human: bool) -> (GameLogic, ObjectId) {
    let (world, mover, _) = world_with_occupant(map, human, None);
    (world, mover)
}

fn world_with_occupant(
    map: &std::path::Path,
    human: bool,
    occupant: Option<Vec3>,
) -> (GameLogic, ObjectId, Option<ObjectId>) {
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
        .admit_object_ini_sources([
            (
                "TestMover.ini".into(),
                "\
Object Mover
  KindOf = SELECTABLE INFANTRY
  CrusherLevel = 2
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
            ),
            ("PathOccupant.ini".into(), path_occupant_ini()),
        ])
        .unwrap();
    assert!(
        world.templates.contains_key("PathOccupant"),
        "admitted templates: {:?}",
        world.templates.keys().collect::<Vec<_>>()
    );
    world.start_new_game(GameMode::SinglePlayer).unwrap();
    assert!(world.load_map(map.to_str().unwrap()));
    world.clear_all_players();
    world.add_player(Player::new(0, Team::USA, "Mover owner", human));
    world.add_player(Player::new(1, Team::GLA, "Other player", !human));
    assert!(world.select_local_player(Some(if human { 1 } else { 0 })));
    let from = Vec3::new(25.0, 0.0, 55.0);
    let mover = world.create_object_for_player("Mover", 0, from).unwrap();
    let occupant = occupant.map(|position| {
        world
            .create_object_for_player("PathOccupant", 1, position)
            .unwrap()
    });
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
    (world, mover, occupant)
}

fn write_map(root: &std::path::Path) -> std::path::PathBuf {
    let map = root.join("OrdinaryController.map");
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
    map
}

#[test]
fn queued_ordinary_path_uses_controller_type_and_survives_save_load() {
    let root = tempfile::tempdir().unwrap();
    let map = write_map(root.path());
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

#[test]
fn appended_waypoint_search_uses_the_movers_controller() {
    let root = tempfile::tempdir().unwrap();
    let map = write_map(root.path());
    let (mut computer, computer_id) = world(&map, false);
    let (mut human, human_id) = world(&map, true);
    assert_eq!(computer_id, human_id);
    let first = Vec3::new(65.0, 0.0, 55.0);
    let next = Vec3::new(145.0, 0.0, 55.0);
    for (w, id) in [(&mut computer, computer_id), (&mut human, human_id)] {
        w.host_object_mut(id).unwrap().requested_destination = Some(first);
        w.update_with_dt(1.0 / 30.0);
        assert_eq!(
            w.host_object(id).unwrap().movement.path.last().copied(),
            Some(first)
        );
        assert!(w.append_unit_waypoint(id, next));
    }
    let c = &computer.host_object(computer_id).unwrap().movement.path;
    let h = &human.host_object(human_id).unwrap().movement.path;
    // This checks the remaining eager search bridge. C++ stores future goals
    // instead; the bridge and this assertion must go when that migration lands.
    assert_eq!(c.last().copied(), Some(next));
    assert_eq!(h.last().copied(), Some(next));
    assert_ne!(
        c, h,
        "the existing bridge must not discard the mover's controller"
    );
}

fn path_occupant_ini() -> String {
    "\
Object PathOccupant
  KindOf = SELECTABLE VEHICLE
  CrushableLevel = 1
  Geometry = SPHERE
  GeometryMajorRadius = 2.0
  GeometryMinorRadius = 2.0
  GeometryHeight = 4.0
  Locomotor = SET_NORMAL TestWalk
  Body = ActiveBody Body
    MaxHealth = 100
    InitialHealth = 100
  End
  Behavior = AIUpdateInterface Brain
  End
End
"
    .into()
}

fn start_occupant_move_before_queued_search(
    world: &mut GameLogic,
    mover: ObjectId,
    occupant: ObjectId,
    destination: Vec3,
) {
    let pending = world.snapshot_pending_host_paths();
    world.restore_pending_host_paths(std::iter::empty::<PendingHostPath>());
    world.host_object_mut(mover).unwrap().waiting_for_path = false;
    world.host_object_mut(occupant).unwrap().is_final_goal = true;
    assert!(world.assign_unit_path(occupant, destination, &[]));
    world.update_with_dt(1.0 / 30.0);
    world.restore_pending_host_paths(pending);
    world.host_object_mut(mover).unwrap().waiting_for_path = true;
}

#[test]
fn queued_ordinary_path_with_moving_occupant_survives_save_and_peer_interleaving() {
    use generals_main::game_logic::pathfinding::GridPos;
    use generals_main::save_load::{Snapshot, XferLoad, XferSave};
    let root = tempfile::tempdir().unwrap();
    let map = write_map(root.path());
    let (mut owner, mover, occupant) =
        world_with_occupant(&map, false, Some(Vec3::new(75.0, 0.0, 55.0)));
    let (mut peer, peer_mover, peer_occupant) =
        world_with_occupant(&map, false, Some(Vec3::new(95.0, 0.0, 55.0)));
    assert_eq!(mover, peer_mover);
    let occupant = occupant.unwrap();
    let peer_occupant = peer_occupant.unwrap();
    assert_eq!(occupant, peer_occupant);
    start_occupant_move_before_queued_search(
        &mut owner,
        mover,
        occupant,
        Vec3::new(75.0, 0.0, 155.0),
    );
    start_occupant_move_before_queued_search(
        &mut peer,
        peer_mover,
        peer_occupant,
        Vec3::new(95.0, 0.0, 155.0),
    );
    let occupied = GridPos::new(7, 5);
    assert!(owner.host_object(occupant).unwrap().host_ai_is_moving());
    assert_eq!(owner.pathfinding_ground_position(occupied), Some(occupant));
    assert_ne!(
        owner.pathfinding_ground_goal(occupied),
        Some(occupant),
        "the occupant is registered as moving"
    );
    assert_eq!(owner.snapshot_pending_host_paths().len(), 1);
    owner.set_pathfinding_static_block(3, 4, true);
    peer.set_pathfinding_static_block(3, 4, true);

    let builder = generals_main::save_load::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&owner).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = generals_main::save_load::WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let (mut restored, restored_mover) = world(&map, false);
    assert_eq!(mover, restored_mover);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    // AIUpdateInterface::loadPostProcess re-registers a moving unit's goal,
    // but does not call updatePos for that unit. Its saved cell remains on
    // the actor while the transient position grid is initially empty.
    assert!(restored.host_object(occupant).unwrap().host_ai_is_moving());
    assert_eq!(restored.pathfinding_ground_position(occupied), None);
    assert_eq!(
        restored.host_object(occupant).unwrap().pathfind_cur_cell,
        owner.host_object(occupant).unwrap().pathfind_cur_cell,
    );
    assert_ne!(restored.pathfinding_ground_goal(occupied), Some(occupant));
    assert_eq!(restored.snapshot_pending_host_paths().len(), 1);

    peer.update_with_dt(1.0 / 30.0);
    owner.update_with_dt(1.0 / 30.0);
    assert!(peer.set_registered_object_position(peer_occupant, Vec3::new(115.0, 0.0, 55.0)));
    restored.update_with_dt(1.0 / 30.0);
    let original = &owner.host_object(mover).unwrap().movement.path;
    let loaded = &restored.host_object(mover).unwrap().movement.path;
    assert!(
        !original.is_empty(),
        "the queued search must install a route"
    );
    assert_eq!(
        loaded, original,
        "moving occupancy must not change the queued route across restore"
    );
    assert!(owner.snapshot_pending_host_paths().is_empty());
    assert!(restored.snapshot_pending_host_paths().is_empty());
    assert_eq!(
        owner.pathfinding_ground_position(occupied),
        Some(occupant),
        "peer movement must not alter the owner's occupancy"
    );
}

#[test]
fn queued_ordinary_path_with_crushable_fixed_occupant_survives_save_and_peer_interleaving() {
    use generals_main::game_logic::pathfinding::GridPos;
    use generals_main::save_load::{Snapshot, XferLoad, XferSave};
    let root = tempfile::tempdir().unwrap();
    let map = write_map(root.path());
    let (mut owner, mover, occupant) =
        world_with_occupant(&map, false, Some(Vec3::new(75.0, 0.0, 55.0)));
    let (mut peer, peer_mover, peer_occupant) =
        world_with_occupant(&map, false, Some(Vec3::new(75.0, 0.0, 55.0)));
    let occupant = occupant.unwrap();
    let peer_occupant = peer_occupant.unwrap();
    assert_eq!((mover, occupant), (peer_mover, peer_occupant));
    let occupied = GridPos::new(7, 5);
    assert!(!owner.host_object(occupant).unwrap().host_ai_is_moving());
    assert_eq!(owner.pathfinding_ground_position(occupied), Some(occupant));
    assert_eq!(owner.pathfinding_ground_goal(occupied), Some(occupant));
    assert!(owner
        .host_object(mover)
        .unwrap()
        .can_crush_or_squish(owner.host_object(occupant).unwrap(), false));
    owner.set_pathfinding_static_block(3, 4, true);
    peer.set_pathfinding_static_block(3, 4, true);
    peer.host_object_mut(peer_occupant).unwrap().crushable_level = 255;
    assert!(!peer
        .host_object(peer_mover)
        .unwrap()
        .can_crush_or_squish(peer.host_object(peer_occupant).unwrap(), false));

    let builder = generals_main::save_load::SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&owner).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = generals_main::save_load::WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let (mut restored, restored_mover) = world(&map, false);
    assert_eq!(mover, restored_mover);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        restored.pathfinding_ground_position(occupied),
        Some(occupant)
    );
    assert_eq!(restored.pathfinding_ground_goal(occupied), Some(occupant));
    assert!(restored
        .host_object(mover)
        .unwrap()
        .can_crush_or_squish(restored.host_object(occupant).unwrap(), false));
    assert_eq!(restored.snapshot_pending_host_paths().len(), 1);

    peer.update_with_dt(1.0 / 30.0);
    owner.update_with_dt(1.0 / 30.0);
    restored.update_with_dt(1.0 / 30.0);
    let original = &owner.host_object(mover).unwrap().movement.path;
    let loaded = &restored.host_object(mover).unwrap().movement.path;
    let peer_path = &peer.host_object(peer_mover).unwrap().movement.path;
    assert!(
        !original.is_empty(),
        "the queued search must install a route"
    );
    assert_eq!(
        loaded, original,
        "restored fixed-occupant search must match its owner"
    );
    assert_ne!(
        peer_path, original,
        "the peer's uncrushable occupant must remain independent"
    );
    assert!(owner.snapshot_pending_host_paths().is_empty());
    assert!(restored.snapshot_pending_host_paths().is_empty());
}
