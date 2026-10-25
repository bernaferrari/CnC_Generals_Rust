use super::*;

fn input(frame: u32) -> DisplayInputs {
    DisplayInputs {
        client_frame: frame,
        frozen: false,
        time_fast: false,
        multiplier: 1,
        device_ready: true,
        tivo_replay: false,
        replay_frame: frame,
    }
}
#[test]
fn actual_display_owner_keeps_tivo_cpu_phase_and_gpu_query_inert() {
    let mut display = NativeDisplayState::new();
    let offer = display.admit(DisplayInputs {
        tivo_replay: true,
        replay_frame: 2,
        ..input(1)
    });
    assert!(!display.should_present());
    let cpu = NativeDisplayState::cpu_phase(offer);
    assert!(
        cpu.draw_drawables && cpu.update_particles,
        "TiVO skips GPU, never CPU view callbacks or particles"
    );
    for _ in 0..4 {
        assert!(!display.should_present());
    }
    let next = display.admit(input(2));
    assert_eq!(
        next.frame_time_ms, 33,
        "GPU queries must not offer the clock again"
    );
    assert!(display.should_present());
}
#[test]
fn guardband_is_owned_persistent_after_request_drain_and_reset_does_not_reset_display_clock() {
    let mut world = GameLogic::new();
    let mut peer = GameLogic::new();
    world.queue_pending_view_guardband(4., 9.);
    assert!(world.take_view_guardband_request().is_some());
    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut world, 0);
    assert_eq!(
        frame.view_guardband,
        Some((4., 9.)),
        "drained command must not erase persistent view configuration"
    );
    let mut display = NativeDisplayState::new();
    display
        .receive_guardband(1, world.native_view_guardband())
        .unwrap();
    display.admit(input(1));
    peer.queue_pending_view_guardband(-2., -3.);
    peer.take_view_guardband_request();
    assert_eq!(display.guardband, [4., 9.]);
    assert_eq!(world.native_view_guardband(), [4., 9.]);
    display.reset_world(2);
    assert_eq!(display.guardband, [0.; 2]);
    assert_eq!(display.admit(input(2)).sync_time_ms, 66);
    assert!(display.receive_guardband(1, [99.; 2]).is_err());
    assert_eq!(display.guardband, [0.; 2]);
}
#[test]
fn failed_expected_shell_has_no_implicit_fallback_or_second_admission() {
    let mut display = NativeDisplayState::new();
    display.fail_shell("actual prefix rejected");
    assert!(display.require_shell_slot().is_err());
    assert_eq!(
        display.check_shell_before_display().unwrap_err(),
        "actual prefix rejected"
    );
    assert!(!display.should_present());
    assert!(display.require_shell_slot().is_ok());
    assert_eq!(
        display.admit(input(2)).sync_time_ms,
        33,
        "failed prefix never offered a Display clock"
    );
}

#[test]
fn receiving_debug_freeze_precedes_actual_fast_display_admission() {
    let mut owner = GameLogic::new();
    let peer = GameLogic::new();
    let mut debug = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    debug.set_time_frozen_debug(true);
    owner.install_owned_script_engine(debug);
    for world in [&mut owner] {
        world.session_view.camera.camera_zoom_start = 1.;
        world.session_view.camera.camera_zoom_target = Some(2.);
        world.session_view.camera.camera_zoom_duration = 1.;
    }
    let mut display = NativeDisplayState::new();
    for frame in [17, 17, 18] {
        let decision = display
            .admit_receiving(&owner, false, |frozen| DisplayInputs {
                frozen,
                time_fast: true,
                ..input(frame)
            })
            .unwrap();
        assert!(decision.update_views);
        assert!(!decision.fast_camera_only);
        owner
            .advance_receiving_camera_view(
                decision.update_views,
                decision.fast_camera_only,
                false,
                false,
                33,
                None,
            )
            .unwrap();
        assert_eq!(owner.session_view.camera.camera_zoom_frame, 0);
    }
    let mut peer_display = NativeDisplayState::new();
    let decision = peer_display
        .admit_receiving(&peer, false, |frozen| DisplayInputs {
            frozen,
            time_fast: true,
            ..input(17)
        })
        .unwrap();
    assert!(decision.fast_camera_only && !decision.update_views);
}

#[test]
fn unavailable_receiving_debug_owner_does_not_consume_actual_display_clock() {
    let mut world = GameLogic::new();
    world
        .with_owned_script_engine(|_, world| {
            let mut display = NativeDisplayState::new();
            let before = serde_json::to_value(display.capture_continuation(71).unwrap()).unwrap();
            let mut constructed = false;
            assert!(
                display
                    .admit_receiving(world, false, |frozen| {
                        constructed = true;
                        DisplayInputs {
                            frozen,
                            ..input(71)
                        }
                    })
                    .is_err()
            );
            assert!(!constructed);
            assert_eq!(
                serde_json::to_value(display.capture_continuation(71).unwrap()).unwrap(),
                before
            );
            world.session_view.scripted.script_camera_freeze_time = true;
            world.session_view.camera.camera_zoom_target = Some(2.);
            let decision = display
                .admit_receiving(world, false, |frozen| DisplayInputs {
                    frozen,
                    time_fast: true,
                    ..input(71)
                })
                .unwrap();
            assert!(!decision.fast_camera_only && decision.update_views);
        })
        .unwrap();
}
