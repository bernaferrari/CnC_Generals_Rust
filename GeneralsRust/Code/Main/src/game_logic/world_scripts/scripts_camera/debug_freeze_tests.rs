use super::*;

fn moving_world(debug: bool, script: bool) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(debug);
    world.set_script_time_frozen_for_test(script);
    let camera = &mut world.session_view.camera;
    camera.camera_zoom_start = 1.;
    camera.camera_zoom_target = Some(2.);
    camera.camera_zoom_duration = 1.;
    camera.camera_zoom_frame = 0;
    world
}

#[test]
fn receiving_debug_gate_preserves_all_four_script_debug_combinations() {
    // W3DView.cpp1263: script time freeze is intentionally commented out.
    for debug in [false, true] {
        for script in [false, true] {
            let mut world = moving_world(debug, script);
            let mut peer = moving_world(!debug, !script);
            let peer_before = peer.capture_camera_continuation().unwrap();
            world
                .advance_receiving_camera_view(true, false, false, false, 33, None)
                .unwrap();
            assert_eq!(
                world.session_view.camera.camera_zoom_frame,
                u32::from(!debug)
            );
            assert_eq!(world.session_view.camera.camera_zoom_target, Some(2.));
            assert_eq!(world.is_script_time_frozen(), script);
            assert_eq!(
                world.simulation_frozen_with_script_engine(None).unwrap(),
                debug || script
            );
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            peer.advance_receiving_camera_view(true, false, false, false, 33, None)
                .unwrap();
            assert_eq!(peer.session_view.camera.camera_zoom_frame, u32::from(debug));
        }
    }
}

#[test]
fn receiving_debug_operation_uses_explicit_interpreter_loan_without_publishing() {
    let mut world = moving_world(true, false);
    let before = world.capture_camera_continuation().unwrap();
    world
        .with_owned_script_engine(|engine, world| {
            assert!(world.owned_script_engine.is_none());
            assert!(world.script_debug_frozen(None).is_err());
            assert!(
                world
                    .advance_receiving_camera_view(true, false, false, false, 33, None)
                    .is_err()
            );
            assert_eq!(world.capture_camera_continuation().unwrap(), before);
            assert!(world.script_debug_frozen(Some(engine)).unwrap());
            world
                .advance_receiving_camera_view(true, false, false, false, 33, Some(engine))
                .unwrap();
            assert_eq!(world.session_view.camera.camera_zoom_frame, 0);
            world.force_script_debug_continue(Some(engine)).unwrap();
            assert!(!world.script_debug_frozen(Some(engine)).unwrap());
            world
                .advance_receiving_camera_view(true, false, false, false, 33, Some(engine))
                .unwrap();
            assert_eq!(world.session_view.camera.camera_zoom_frame, 1);
        })
        .unwrap();
    assert!(!world.script_debug_frozen(None).unwrap());
}

#[test]
fn rejected_and_fast_camera_offers_do_not_query_an_unavailable_debug_owner() {
    let mut world = moving_world(true, false);
    let _receiving_engine = world.owned_script_engine.take().unwrap();
    let before = world.capture_camera_continuation().unwrap();
    assert!(
        !world
            .advance_receiving_camera_view(false, false, false, false, 33, None)
            .unwrap()
    );
    assert_eq!(world.capture_camera_continuation().unwrap(), before);
    // W3DDisplay.cpp1745: the fast-only movement call bypasses full View gate.
    world
        .advance_receiving_camera_view(false, true, true, false, 33, None)
        .unwrap();
    assert_eq!(world.session_view.camera.camera_zoom_frame, 1);
    assert!(
        world
            .advance_receiving_camera_view(true, false, true, false, 33, None)
            .is_err()
    );
    assert_eq!(world.session_view.camera.camera_zoom_frame, 1);
}

#[test]
fn clear_game_debug_continue_preserves_script_and_camera_freeze_and_peer() {
    let mut world = moving_world(true, true);
    let peer = moving_world(true, true);
    world.session_view.scripted.script_camera_freeze_time = true;
    world.owned_script_engine.as_ref().unwrap().do_freeze_time();
    let before = world.capture_camera_continuation().unwrap();
    let peer_before = peer.capture_camera_continuation().unwrap();
    world.force_script_debug_continue(None).unwrap();
    assert!(!world.script_debug_frozen(None).unwrap());
    assert!(
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .is_time_frozen_script()
    );
    assert!(world.is_script_time_frozen());
    assert!(world.is_script_camera_time_frozen());
    assert_eq!(world.capture_camera_continuation().unwrap(), before);
    assert!(peer.script_debug_frozen(None).unwrap());
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    // A fresh interpreter is inert; clearing another receiver does not seed it.
    assert!(!GameLogic::new().script_debug_frozen(None).unwrap());
}

#[test]
fn simulation_camera_short_circuit_does_not_invent_a_missing_debug_owner() {
    let mut world = moving_world(false, true);
    let _receiving_engine = world.owned_script_engine.take().unwrap();
    world.session_view.scripted.script_camera_freeze_time = true;
    assert!(world.simulation_frozen_with_script_engine(None).unwrap());
    world.session_view.scripted.script_camera_freeze_time = false;
    // Script is later than debug in GameLogic.cpp3603, so it cannot hide absence.
    assert!(world.simulation_frozen_with_script_engine(None).is_err());
}

#[test]
fn explicit_interpreter_reset_keeps_the_existing_owned_port_lifetime() {
    let mut interpreter = ScriptEngine::new().unwrap();
    let mut peer = ScriptEngine::new().unwrap();
    interpreter.set_time_frozen_debug(true);
    interpreter.do_freeze_time();
    peer.set_time_frozen_debug(true);
    peer.do_freeze_time();
    // Existing Rust ScriptEngine reset semantics. C++ debug control lives in
    // the debugger DLL, so this is not a claim about that external lifetime.
    interpreter.reset();
    assert!(!interpreter.is_time_frozen_debug());
    assert!(!interpreter.is_time_frozen_script());
    assert!(peer.is_time_frozen_debug());
    assert!(peer.is_time_frozen_script());
}

#[test]
#[cfg(feature = "game_client")]
fn receiving_follow_history_reads_debug_before_script_and_pause_only_for_resolved_lock() {
    use crate::presentation_frame::follow::{FollowAdmission, FollowTargetFacts};
    for debug in [false, true] {
        for script in [false, true] {
            for paused in [false, true] {
                let mut world = moving_world(debug, script);
                let id = ObjectId(23);
                world.session_view.set_camera_follow(Some(id));
                world.session_view.target = Vec3::new(100., 0., 150.);
                let before = world.session_view.target;
                let prior = Vec3::new(-7., 0., -11.);
                world.session_view.camera.native_filter_previous_target = prior;
                let facts = FollowTargetFacts {
                    id,
                    position: Vec3::new(200., 5., 250.),
                    airborne_orientation: None,
                };
                assert_eq!(
                    world
                        .admit_receiving_camera_follow(Some(id), Some(facts), 10., paused, None)
                        .unwrap(),
                    FollowAdmission::Followed
                );
                assert_ne!(
                    world.session_view.target, before,
                    "follow itself is independent from movement freeze"
                );
                assert_eq!(
                    world.session_view.camera.native_filter_previous_target,
                    if debug || script || paused {
                        prior
                    } else {
                        before
                    }
                );
            }
        }
    }
    let mut world = moving_world(false, false);
    world
        .with_owned_script_engine(|_, world| {
            let id = ObjectId(23);
            assert_eq!(
                world
                    .admit_receiving_camera_follow(None, None, 10., false, None)
                    .unwrap(),
                FollowAdmission::Inactive
            );
            world.session_view.set_camera_follow(Some(id));
            assert_eq!(
                world
                    .admit_receiving_camera_follow(None, None, 10., false, None)
                    .unwrap(),
                FollowAdmission::AwaitingFrame
            );
            assert_eq!(
                world
                    .admit_receiving_camera_follow(Some(id), None, 10., false, None)
                    .unwrap(),
                FollowAdmission::TargetGone
            );
            world.session_view.set_camera_follow(Some(id));
            let facts = FollowTargetFacts {
                id,
                position: Vec3::new(200., 5., 250.),
                airborne_orientation: None,
            };
            let before = world.capture_camera_continuation().unwrap();
            assert!(
                world
                    .admit_receiving_camera_follow(Some(id), Some(facts), 10., true, None)
                    .is_err(),
                "debug is observed before pause"
            );
            world.set_script_time_frozen_for_test(true);
            assert!(
                world
                    .admit_receiving_camera_follow(Some(id), Some(facts), 10., false, None)
                    .is_err(),
                "debug is observed before script freeze"
            );
            world.set_script_time_frozen_for_test(false);
            assert_eq!(world.capture_camera_continuation().unwrap(), before);
        })
        .unwrap();
}
