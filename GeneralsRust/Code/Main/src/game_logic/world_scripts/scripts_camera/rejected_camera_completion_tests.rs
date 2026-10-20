//! Rejected admission refreshes a derived hook, never the canonical motion.
use super::*;

fn request() -> CameraMoveToRequest {
    CameraMoveToRequest {
        position: Vec3::new(20., 0., 30.),
        seconds: 1.,
        camera_stutter_seconds: 0.,
        ease_in_seconds: 0.,
        ease_out_seconds: 0.,
    }
}
fn owner(active: bool) -> GameLogic {
    let mut world = GameLogic::new();
    if active {
        world.session_view.install_rotate(0.7, 2., 0., 0., 1.);
    }
    world.mark_script_camera_movement_maybe_finished();
    world
}
fn reject_and_compare(world: &mut GameLogic, peer: &GameLogic, bad: CameraMoveToRequest) {
    let before = world.capture_camera_continuation().unwrap();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let expected = world.session_view.movement_finished();
    // Actual ingress marks false before the driving world consumes the request.
    world.mission_scripts.push_camera_move_to(bad);
    assert!(!world.mission_scripts.is_camera_movement_finished());
    let pending = world.mission_scripts.drain_camera_move_to();
    assert_eq!(pending.len(), 1);
    world.start_camera_move_to(pending.into_iter().next().unwrap());
    compare_rejection(world, peer, before, peer_before, expected);
}
fn compare_rejection(
    world: &mut GameLogic,
    peer: &GameLogic,
    before: Vec<u8>,
    peer_before: Vec<u8>,
    expected: bool,
) {
    assert_eq!(
        world.mission_scripts.is_camera_movement_finished(),
        expected
    );
    assert_eq!(world.capture_camera_continuation().unwrap(), before);
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    let mut restored = GameLogic::new();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&before).unwrap())
        .unwrap();
    assert_eq!(restored.capture_camera_continuation().unwrap(), before);
    assert_eq!(restored.session_view.movement_finished(), expected);
    assert_eq!(
        restored.mission_scripts.is_camera_movement_finished(),
        expected
    );
    for _ in 0..3 {
        let admission = MotionAdmission {
            update_views: true,
            fast_camera_only: false,
            paused: false,
            debug_frozen: false,
            disable_camera_movement: false,
        };
        world.session_view.advance_camera_movements(
            admission,
            33,
            &mut world.visual_speed_multiplier,
        );
        restored.session_view.advance_camera_movements(
            admission,
            33,
            &mut restored.visual_speed_multiplier,
        );
        assert_eq!(
            world.capture_camera_continuation().unwrap(),
            restored.capture_camera_continuation().unwrap()
        );
    }
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}
#[test]
fn rejected_nonfinite_move_preserves_idle_active_save_and_peer_completion() {
    for active in [false, true] {
        let mut world = owner(active);
        let peer = owner(!active);
        let mut bad = request();
        bad.position.x = f32::NAN;
        reject_and_compare(&mut world, &peer, bad);
    }
}
#[test]
fn rejected_nonfinite_ease_preserves_idle_active_save_and_peer_completion() {
    for active in [false, true] {
        let mut world = owner(active);
        let peer = owner(!active);
        let mut bad = request();
        bad.ease_out_seconds = f32::NAN;
        reject_and_compare(&mut world, &peer, bad);
    }
}
#[test]
fn rejected_poisoned_receiving_terrain_repairs_hook_without_touching_peer() {
    for active in [false, true] {
        let mut world = owner(active);
        let peer = owner(!active);
        // Poison only the world's existing terrain lock; no new registry,
        // synchronization, thread or process mutation is introduced.
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = world.world_services.terrain().write().unwrap();
            panic!("intentional receiving terrain poison");
        }));
        assert!(caught.is_err());
        reject_and_compare(&mut world, &peer, request());
    }
}

#[test]
fn rejected_nonfinite_named_path_preserves_idle_active_save_and_peer_completion() {
    use gamelogic::system::map_loader::{MapData, MapWaypoint};
    use gamelogic::terrain::TerrainLogic;
    for active in [false, true] {
        let mut world = owner(active);
        let peer = owner(!active);
        let mut map = MapData::new();
        map.width = 64;
        map.height = 64;
        map.heightmap = vec![60; 64 * 64];
        for (id, name, x) in [
            (1, "RejectedOwnedPath", 111.),
            (2, "RejectedOwnedEnd", 211.),
        ] {
            map.waypoints.push(MapWaypoint {
                id,
                name: name.into(),
                location: gamelogic::common::Coord3D::new(x, x + 20., 0.),
                path_label1: String::new(),
                path_label2: String::new(),
                path_label3: String::new(),
                bi_directional: false,
            });
        }
        map.waypoint_links.push((1, 2));
        let mut terrain = TerrainLogic::new();
        terrain.load_map_data(map);
        *world.world_services.terrain().write().unwrap() = terrain;
        let chain = super::super::path_admission::admit_camera_chain(
            &world,
            "RejectedOwnedPath",
            world.session_view.target,
        )
        .unwrap()
        .expect("real receiving named path must admit before motion validation");
        assert_eq!(chain.points.len(), 2);
        let bad = CameraPathRequest {
            waypoint: "RejectedOwnedPath".into(),
            seconds: 1.,
            camera_stutter_seconds: 0.,
            ease_in_seconds: 0.,
            ease_out_seconds: f32::NAN,
        };
        let probe = WaypointMotion::along_chain(
            &chain.points,
            Point::from_world(world.session_view.target),
            world.session_view.camera.camera_yaw_radians,
            world
                .session_view
                .camera
                .native_camera
                .basis()
                .ground_level(),
            chain.final_ground,
            world.visual_speed_multiplier as i32,
            1000,
            0,
            CAMERA_FRAME_MS,
            true,
            0.,
            f32::NAN,
            gamelogic::common::MAP_XY_FACTOR,
        );
        assert!(
            probe.validate().is_err(),
            "nonfinite ease must reach invalid-motion branch"
        );
        let before = world.capture_camera_continuation().unwrap();
        let peer_before = peer.capture_camera_continuation().unwrap();
        let expected = world.session_view.movement_finished();
        world.mission_scripts.push_camera_path_move(bad);
        assert!(!world.mission_scripts.is_camera_movement_finished());
        let pending = world.mission_scripts.drain_camera_path_moves();
        assert_eq!(pending.len(), 1);
        world.start_camera_path_move(pending.into_iter().next().unwrap());
        compare_rejection(&mut world, &peer, before, peer_before, expected);
    }
}
