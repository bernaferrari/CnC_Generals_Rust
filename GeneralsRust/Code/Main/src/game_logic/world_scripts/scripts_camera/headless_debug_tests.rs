use super::*;

fn moving_world() -> GameLogic {
    let mut world = GameLogic::new();
    let camera = &mut world.session_view.camera;
    camera.camera_zoom_start = 1.0;
    camera.camera_zoom_target = Some(2.0);
    camera.camera_zoom_duration = 1.0;
    camera.camera_zoom_frame = 0;
    world
}

#[test]
fn checked_headless_offer_observes_actual_receiving_debug_and_preserves_script_freeze() {
    for debug in [false, true] {
        for script in [false, true] {
            let mut owner = moving_world();
            let mut peer = moving_world();
            owner
                .owned_script_engine
                .as_mut()
                .unwrap()
                .set_time_frozen_debug(debug);
            peer.owned_script_engine
                .as_mut()
                .unwrap()
                .set_time_frozen_debug(!debug);
            owner.set_script_time_frozen_for_test(script);
            let peer_before = peer.capture_camera_continuation().unwrap();
            owner.headless_camera_view_offer(33).unwrap();
            assert_eq!(
                owner.session_view.camera.camera_zoom_frame,
                u32::from(!debug)
            );
            assert_eq!(owner.is_script_time_frozen(), script);
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            peer.headless_camera_view_offer(33).unwrap();
            assert_eq!(peer.session_view.camera.camera_zoom_frame, u32::from(debug));
        }
    }
}

#[test]
fn checked_headless_offer_requires_actual_owner_or_explicit_interpreter_loan() {
    let mut world = moving_world();
    world
        .owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(true);
    world
        .with_owned_script_engine(|engine, world| {
            let before = world.capture_camera_continuation().unwrap();
            assert!(world.headless_camera_view_offer(33).is_err());
            assert_eq!(world.capture_camera_continuation().unwrap(), before);
            world
                .headless_camera_view_offer_with_script_engine(33, Some(engine))
                .unwrap();
            assert_eq!(world.session_view.camera.camera_zoom_frame, 0);
            world.force_script_debug_continue(Some(engine)).unwrap();
            world
                .headless_camera_view_offer_with_script_engine(33, Some(engine))
                .unwrap();
            assert_eq!(world.session_view.camera.camera_zoom_frame, 1);
        })
        .unwrap();
}

#[test]
fn checked_headless_pause_and_native_fast_offer_preserve_distinct_source_gates() {
    let mut world = moving_world();
    world.is_paused = true; // Test owns this session; avoid the audio compatibility adapter.
    world.headless_camera_view_offer(33).unwrap();
    assert_eq!(world.session_view.camera.camera_zoom_frame, 0);
    world
        .owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(true);
    // W3DDisplay's direct fast-only branch bypasses the full View pause/debug
    // gate; the headless full View offer above deliberately does not select it.
    world
        .advance_receiving_camera_view(false, true, true, false, 33, None)
        .unwrap();
    assert_eq!(world.session_view.camera.camera_zoom_frame, 1);
}

#[test]
fn checked_headless_debug_only_freeze_does_not_advance() {
    let mut owner = moving_world();
    owner
        .owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(true);
    assert!(!owner.is_script_time_frozen());
    owner.headless_camera_view_offer(33).unwrap();
    assert_eq!(
        owner.session_view.camera.camera_zoom_frame, 0,
        "receiving debug freeze must block full headless View movement"
    );
}
