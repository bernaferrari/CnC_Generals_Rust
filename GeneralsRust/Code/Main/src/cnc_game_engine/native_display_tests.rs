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
fn all_public_updates_complete_display_after_update_and_render_never_rebuilds_camera() {
    let input = include_str!("input.rs");
    assert!(input.contains("fn finish_display_after_update("));
    assert_eq!(
        input.matches("self.finish_display_after_update();").count(),
        4
    );
    assert!(!input.contains("if self.native_display.has_pending_work()"));
    let catchup = &input[input
        .find("while self.menu_loading_tick_accumulator")
        .unwrap()..input.find("pub fn update(&mut self").unwrap()];
    assert!(
        catchup.find("self.update_internal(dt);").unwrap()
            < catchup.find("self.finish_display_after_update();").unwrap()
    );
    let render = &input[input.find("pub fn render(").unwrap()..];
    assert!(!render.contains("self.apply_camera_orbit_transform();"));
    assert!(!render.contains("self.rebuild_tactical_projection();"));
    let run = include_str!("run_loop.rs");
    assert!(!run.contains("advance_ww3d_visual_sync"));
    assert!(!run.contains("WW3D::sync("));
    assert!(!run.contains("engine.complete_native_display_step()"));
}

#[test]
fn actual_no_draw_and_failed_prefix_return_before_display_clock_and_device_poll() {
    let source = include_str!("native_display.rs");
    let body = &source[source
        .find("pub(super) fn complete_native_display_step(")
        .unwrap()..];
    let clock = body
        .find("self.native_display.admit(DisplayInputs")
        .unwrap();
    let no_draw = body.find("!pending.step.display_requested()").unwrap();
    let failed = body
        .find("self.native_display.check_shell_before_display()?")
        .unwrap();
    let poll = body.find(".poll(wgpu::PollType::Poll)").unwrap();
    assert!(failed < no_draw && no_draw < poll && poll < clock);
    assert!(body[..clock].contains("return self"));
}
