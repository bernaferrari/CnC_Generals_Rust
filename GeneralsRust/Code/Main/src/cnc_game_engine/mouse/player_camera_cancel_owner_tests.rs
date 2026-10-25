use crate::game_logic::{GameLogic, Object, ObjectId, Team, ThingTemplate};
use crate::presentation_frame::waypoint::{Point, WaypointMotion};
fn owner() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_object(Object::new(
        ThingTemplate::new("CameraTarget"),
        ObjectId(42),
        Team::USA,
    ));
    world.session_view.camera.install_zoom(2., 3., 0., 0.);
    world.session_view.camera.install_pitch(0.7, 4., 0., 0.);
    world.session_view.camera.install_rotate(1., 5., 0., 0.);
    world.session_view.scripted.motion = WaypointMotion::move_to(
        Point {
            x: 100.,
            y: 200.,
            z: 0.,
        },
        Point {
            x: 0.,
            y: 0.,
            z: 0.,
        },
        0.,
        0.,
        0.,
        1,
        3000,
        false,
        0.,
        0.,
    );
    world.session_view.scripted.script_camera_freeze_time = true;
    world
}
#[test]
#[cfg(feature = "game_client")]
fn receiving_player_cancel_scopes_preserve_peer_and_foreign_core() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_player_cancel_scopes_preserve_peer_and_foreign_core",
        || {
            use game_client::display::view::{with_tactical_view, with_tactical_view_ref};
            with_tactical_view(|view| {
                view.rotate_camera(1., 3000, 0., 0.);
                view.zoom_camera(2., 4000, 0., 0.);
                view.set_camera_lock(Some(812));
                view.set_camera_lock_drawable(Some(913));
            });
            let core = || {
                with_tactical_view_ref(|view| {
                    (
                        view.is_camera_movement_finished(),
                        view.camera_lock_id(),
                        view.camera_lock_drawable_id(),
                    )
                })
            };
            let foreign = core();
            assert_eq!(foreign, (false, Some(812), Some(913)));
            let peer = owner();
            let peer_before = peer.capture_camera_continuation().unwrap();
            let mut scrolling = owner();
            let path = serde_json::to_value(&scrolling.session_view.scripted.motion).unwrap();
            let zoom = scrolling.session_view.camera.camera_zoom_target;
            let pitch = scrolling.session_view.camera.camera_pitch_target;
            scrolling.cancel_scripted_camera_from_player_scroll();
            assert!(!scrolling.session_view.camera.rotation_registers.active());
            assert_eq!(
                scrolling.session_view.camera.rotation.duration_seconds(),
                5.
            );
            assert_eq!(scrolling.session_view.camera.camera_zoom_target, zoom);
            assert_eq!(scrolling.session_view.camera.camera_pitch_target, pitch);
            assert_eq!(
                serde_json::to_value(&scrolling.session_view.scripted.motion).unwrap(),
                path
            );
            assert!(scrolling.session_view.scripted.script_camera_freeze_time);
            let mut look = owner();
            look.session_view.scripted.motion.active = false;
            look.session_view.scripted.path_arrived = true;
            look.cancel_scripted_camera_from_player_look_at();
            assert!(!look.session_view.scripted.path_arrived);
            assert!(!look.session_view.camera.rotation_registers.active());
            assert!(look.session_view.camera.camera_zoom_target.is_some());
            assert!(look.session_view.camera.camera_pitch_target.is_some());
            assert!(look.session_view.scripted.script_camera_freeze_time);
            let mut setting = owner();
            setting.cancel_scripted_camera_from_player_set();
            assert!(!setting.session_view.scripted.motion.active);
            assert!(!setting.session_view.camera.rotation_registers.active());
            assert!(setting.session_view.camera.camera_pitch_target.is_none());
            assert!(setting.session_view.camera.camera_zoom_target.is_none());
            assert_eq!(setting.session_view.camera.rotation.duration_seconds(), 5.);
            assert_eq!(setting.session_view.camera.camera_pitch_duration, 4.);
            assert_eq!(setting.session_view.camera.camera_zoom_duration, 3.);
            assert!(setting.session_view.scripted.script_camera_freeze_time);
            // setAngle preserves stored freeze even though cancellation makes
            // the current movement complete. It must govern later motion after load.
            assert!(!setting.is_script_camera_time_frozen());
            let saved = setting.capture_camera_continuation().unwrap();
            let mut restored = GameLogic::new();
            restored
                .admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap())
                .unwrap();
            assert_eq!(restored.capture_camera_continuation().unwrap(), saved);
            assert!(restored.session_view.scripted.script_camera_freeze_time);
            restored.session_view.camera.install_zoom(1.5, 3., 0., 0.);
            assert!(restored.is_script_camera_time_frozen());
            restored.advance_camera_view(
                crate::cnc_game_engine::camera_motion::MotionAdmission {
                    update_views: true,
                    fast_camera_only: false,
                    paused: false,
                    debug_frozen: false,
                    disable_camera_movement: false,
                },
                33,
            );
            assert!(restored.is_script_camera_time_frozen());
            let mut follow = owner();
            follow.set_camera_tether_object(ObjectId(42), false, 14.);
            follow.set_camera_follow_object(None);
            assert!(follow.camera_follow_object_id().is_none());
            assert!(follow.peek_camera_tether_play().is_none());
            assert!(
                follow.session_view.scripted.motion.active,
                "setCameraLock(INVALID) does not cancel waypoint path"
            );
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            assert_eq!(core(), foreign);
        },
    );
}
