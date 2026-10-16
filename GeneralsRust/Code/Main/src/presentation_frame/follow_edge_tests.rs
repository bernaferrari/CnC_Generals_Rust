//! Real receiving object freeze -> follow admission -> terrain/viewport build,
//! including pending-save continuation. No injected follow position or bounds.
use super::*;
use crate::game_logic::{
    GameLogic, ObjectId, Player, ScriptPresentationEffects, Team, ThingTemplate,
};
fn owner(position: Vec3, boundary: (i32, i32), raw_height: u8) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 256;
    map.height = 256;
    map.heightmap = vec![raw_height; 256 * 256];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(boundary.0, boundary.1)];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    world.add_player(Player::new(0, Team::USA, "ReceivingFollow", true));
    world
        .templates
        .insert("EdgeFollower".into(), ThingTemplate::new("EdgeFollower"));
    let id = world
        .create_object_for_player("EdgeFollower", 0, position)
        .unwrap();
    world.session_view.camera.viewport =
        crate::presentation_frame::camera::viewport::TacticalViewport::for_display(1280, 720)
            .unwrap();
    world.session_view.camera.rebuild_projection();
    world.session_view.target = Vec3::new(800., 0., 650.);
    let ground = world
        .world_services
        .terrain()
        .read()
        .unwrap()
        .get_ground_height(800., 650., None);
    world
        .session_view
        .camera
        .native_camera
        .initialize_map(ground, 300., 37.5, 13.);
    world.session_view.commit_orbit_pose();
    (world, id)
}
fn consume(world: &mut GameLogic, id: ObjectId, position: Vec3) {
    let facts = freeze(world).unwrap();
    assert_eq!(facts.id, id);
    assert_eq!(facts.position, position);
    assert_eq!(
        world
            .session_view
            .admit_camera_follow(Some(id), Some(facts), 10., true),
        Ok(crate::presentation_frame::follow::FollowAdmission::Followed)
    );
    // Follow itself writes the exact source target before the distinct build clamp.
    assert_eq!(world.session_view.target.x, position.x);
    assert_eq!(world.session_view.target.z, position.z);
    assert!(!world.session_view.camera.camera_follow_snap_immediate);
    world
        .commit_native_camera_with_effects(&mut ScriptPresentationEffects::default())
        .unwrap();
    let bounds = world.session_view.camera.constraint_bounds().unwrap();
    assert!((bounds.x_min + bounds.x_max - 1610.).abs() < 0.001);
    assert!((bounds.z_min + bounds.z_max - 1370.).abs() < 0.001);
    assert_eq!(
        world.session_view.target,
        bounds.constrain(Vec3::new(position.x, 0., position.z))
    );
    assert!(
        world.session_view.target.x >= bounds.x_min && world.session_view.target.x <= bounds.x_max
    );
    assert!(
        world.session_view.target.z >= bounds.z_min && world.session_view.target.z <= bounds.z_max
    );
    assert_ne!(
        world.session_view.target.x, position.x,
        "near/outside source edge must be constrained at build"
    );
    assert_ne!(world.session_view.target.z, position.z);
    assert!(world.session_view.camera.view_matrix.is_finite());
    assert_eq!(
        world
            .session_view
            .camera
            .native_camera
            .basis()
            .ground_level(),
        position.y
    );
}
#[test]
fn actual_follow_near_and_outside_active_edge_commits_and_resumes_pending_save_without_peer() {
    for position in [Vec3::new(1600., 37., 1360.), Vec3::new(1800., 37., 1500.)] {
        for valid_cache in [false, true] {
            let (mut world, id) = owner(position, (161, 137), 12);
            let (mut peer, peer_id) = owner(Vec3::new(900., 91., 800.), (201, 193), 48);
            assert_eq!(id, peer_id);
            peer.session_view.set_camera_tether(peer_id, true, 7.);
            let peer_before = peer.capture_camera_continuation().unwrap();
            if valid_cache {
                world
                    .commit_native_camera_with_effects(&mut ScriptPresentationEffects::default())
                    .unwrap();
            }
            world.session_view.set_camera_tether(id, true, 9.);
            use crate::presentation_frame::session_view::waypoint::{Point, WaypointMotion};
            world.session_view.scripted.motion = WaypointMotion::move_to(
                Point::from_world(Vec3::new(1100., 0., 900.)),
                Point::from_world(world.session_view.target),
                0.,
                world
                    .session_view
                    .camera
                    .native_camera
                    .basis()
                    .ground_level(),
                world
                    .session_view
                    .camera
                    .native_camera
                    .basis()
                    .ground_level(),
                1,
                1000,
                false,
                0.,
                0.,
            );
            assert!(world.session_view.scripted.motion.active);
            world.session_view.scripted.path_arrived = true;
            assert_eq!(
                world.session_view.camera.constraints.is_valid(),
                valid_cache
            );
            let pending = world.capture_camera_continuation().unwrap();
            let (mut restored, restored_id) = owner(position, (161, 137), 12);
            assert_eq!(restored_id, id);
            restored
                .admit_camera_continuation(GameLogic::decode_camera_continuation(&pending).unwrap())
                .unwrap();
            assert_eq!(restored.camera_follow_object_id(), Some(id));
            assert_eq!(restored.peek_camera_tether_play(), Some(9.));
            assert!(restored.session_view.camera.camera_follow_snap_immediate);
            assert!(
                restored.session_view.scripted.motion.active
                    && restored.session_view.scripted.path_arrived
            );
            assert_eq!(
                restored.session_view.camera.constraints.is_valid(),
                valid_cache
            );
            consume(&mut world, id, position);
            assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
            consume(&mut restored, id, position);
            assert_eq!(
                world.capture_camera_continuation().unwrap(),
                restored.capture_camera_continuation().unwrap()
            );
            assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
        }
    }
}
