use game_engine::common::system::DataChunkOutput;
use gamelogic::ai::pathfind_complete::SURFACE_GROUND;
use generals_main::game_logic::pathfinding::PendingHostPath;
use generals_main::game_logic::{
    GameLogic, GameMode, HostGeometryType, KindOf, ObjectId, Player, Team, ThingTemplate,
};
use glam::Vec3;

fn world(map: &std::path::Path, human: bool) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.clear_all_players();
    world.add_player(Player::new(0, Team::USA, "Mover owner", human));
    world.add_player(Player::new(1, Team::GLA, "Other player", !human));
    assert!(world.select_local_player(Some(if human { 1 } else { 0 })));
    for (name, immobile) in [("Mover", false), ("Threat", true)] {
        let mut template = ThingTemplate::new(name);
        template.add_kind_of(KindOf::Vehicle).set_health(100.0);
        if immobile {
            template.add_kind_of(KindOf::Immobile);
        }
        template.geometry_info.geom_type = HostGeometryType::Sphere;
        template.geometry_info.major_radius = 2.0;
        template.geometry_info.authored = true;
        world.templates.insert(name.into(), template);
    }
    let from = Vec3::new(150.0, 0.0, 150.0);
    let mover = world.create_object_for_player("Mover", 0, from).unwrap();
    let threat = world
        .create_object_for_player("Threat", 1, Vec3::new(170.0, 0.0, 150.0))
        .unwrap();
    let unit = world.host_object_mut(mover).unwrap();
    unit.is_approach_path = true;
    unit.waiting_for_path = true;
    unit.requested_victim_id = Some(threat);
    unit.vision_range = 60.0;
    unit.locomotor_surfaces = SURFACE_GROUND;
    unit.cur_locomotor_name = Some("GroundLocomotor".into());
    world.restore_pending_host_paths([PendingHostPath {
        unit_id: mover,
        start: from,
        destination: Vec3::new(210.0, 0.0, 150.0),
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

fn assert_live_closest_controller() {
    let root = tempfile::tempdir().unwrap();
    let map = root.path().join("SafeController.map");
    let mut data = DataChunkOutput::new();
    data.open_data_chunk("HeightMapData", 4);
    // The 41-cell physical map contains a smaller 10-cell human boundary.
    // Both movers start outside that boundary but inside the physical grid.
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
    computer.update_with_dt(1.0 / 30.0);
    human.update_with_dt(1.0 / 30.0);
    no_controller.update_with_dt(1.0 / 30.0);
    let c = computer.host_object(computer_id).unwrap();
    let h = human.host_object(human_id).unwrap();
    let missing = no_controller.host_object(no_controller_id).unwrap();
    assert!(
        c.movement.path.len() > 1,
        "computer should search outside the human boundary: {:?}",
        c.movement.path
    );
    assert_eq!(
        h.movement.path,
        vec![Vec3::new(150.0, 0.0, 150.0)],
        "human search remains at its starting cell"
    );
    assert_eq!(
        missing.movement.path, h.movement.path,
        "missing controller defaults to human"
    );
}

#[test]
fn queued_closest_path_uses_controller_type() {
    assert_live_closest_controller();
}
