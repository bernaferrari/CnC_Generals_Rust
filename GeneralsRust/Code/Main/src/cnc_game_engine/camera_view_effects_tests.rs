use super::*;
use crate::cnc_game_engine::camera_motion::AxisStep;
use crate::cnc_game_engine::display_clock::{DisplayInputs, VisualClock};
use crate::cnc_game_engine::native_display::NativeDisplayState;

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

struct LocalView {
    clock: VisualClock,
    value: f32,
    target: Option<f32>,
    axis_frame: u32,
    view: crate::presentation_frame::SessionView,
}
impl LocalView {
    fn new() -> Self {
        Self {
            clock: VisualClock::new(),
            value: 0.,
            target: Some(1.),
            axis_frame: 0,
            view: crate::presentation_frame::SessionView {
                target: Vec3::ZERO,
                impulse: crate::presentation_frame::SessionViewImpulse {
                    intensity: 4.,
                    cosine: 1.,
                    sine: -0.5,
                    offset: Vec3::ZERO,
                },
                ..Default::default()
            },
        }
    }
    fn offer(
        &mut self,
        input: DisplayInputs,
        paused: bool,
        duration: f32,
    ) -> (DisplayAdmission, AxisStep) {
        let decision = self.clock.admit(input);
        // The production camera operation borrows these same scalar fields
        // and forwards the clock's two distinct CPU-view admission flags.
        let axis = step_axis(
            AxisMotion {
                value: &mut self.value,
                target: &mut self.target,
                frame: &mut self.axis_frame,
                start: 0.,
                duration_seconds: duration,
                ease_in_seconds: 0.,
                ease_out_seconds: 0.,
            },
            AxisKind::ZoomPitch,
            MotionAdmission {
                update_views: decision.update_views,
                fast_camera_only: decision.fast_camera_only,
                paused,
                debug_frozen: false,
                disable_camera_movement: false,
            },
        );
        if decision.update_views {
            self.view.advance_impulse();
        }
        (decision, axis)
    }
    fn snapshot(&self) -> (f32, Option<f32>, u32, f32, f32, f32, Vec3) {
        (
            self.value,
            self.target,
            self.axis_frame,
            self.view.impulse.intensity,
            self.view.impulse.cosine,
            self.view.impulse.sine,
            self.view.impulse.offset,
        )
    }
}

#[test]
fn clock_rejection_and_multiplier_do_not_advance_borrowed_axes_or_impulse() {
    let mut view = LocalView::new();
    view.offer(
        DisplayInputs {
            multiplier: 4,
            ..input(1)
        },
        false,
        1.,
    );
    assert_eq!(view.axis_frame, 1);
    assert_eq!(view.value, 1. / 30.);
    assert_eq!(view.view.impulse.offset, Vec3::new(4., 0., -2.));
    assert_eq!(view.view.impulse.intensity, 3.);
    let completed = view.snapshot();
    for frame in 2..=3 {
        let (decision, axis) = view.offer(
            DisplayInputs {
                multiplier: 4,
                ..input(frame)
            },
            false,
            1.,
        );
        assert!(!decision.update_views && !decision.fast_camera_only);
        assert!(axis.was_active && !axis.changed && !axis.completed);
        assert_eq!(view.snapshot(), completed);
    }
    view.offer(
        DisplayInputs {
            multiplier: 4,
            ..input(4)
        },
        false,
        1.,
    );
    assert_eq!(view.axis_frame, 2);
    assert_eq!(view.value, 2. / 30.);
    assert_eq!(view.view.impulse.offset, Vec3::new(-3., 0., 1.5));
    assert_eq!(view.view.impulse.intensity, 2.25);
    let completed = view.snapshot();
    let (decision, _) = view.offer(
        DisplayInputs {
            device_ready: false,
            ..input(5)
        },
        false,
        1.,
    );
    assert!(!decision.update_views);
    assert_eq!(view.snapshot(), completed);
}

#[test]
fn fast_camera_subset_advances_one_source_axis_frame_without_decaying_spring() {
    let mut view = LocalView::new();
    let original = view.snapshot();
    let (decision, axis) = view.offer(
        DisplayInputs {
            time_fast: true,
            ..input(1)
        },
        false,
        1.,
    );
    assert!(decision.fast_camera_only && !decision.update_views && !decision.sync);
    assert!(axis.changed && axis.was_active);
    assert_eq!((view.axis_frame, view.value), (1, 1. / 30.));
    assert_eq!(
        (
            view.view.impulse.intensity,
            view.view.impulse.cosine,
            view.view.impulse.sine,
            view.view.impulse.offset
        ),
        (original.3, original.4, original.5, original.6)
    );
    let (decision, _) = view.offer(
        DisplayInputs {
            frozen: true,
            ..input(2)
        },
        false,
        1.,
    );
    assert!(decision.update_views && !decision.fast_camera_only);
    assert_eq!(decision.frame_time_ms, 33);
    assert_eq!(view.axis_frame, 2);
    assert_eq!(view.view.impulse.intensity, 3.);
    assert_eq!(view.view.impulse.offset, Vec3::new(4., 0., -2.));
}

#[test]
fn repeated_paused_and_tivo_offers_keep_distinct_axis_spring_and_gpu_semantics() {
    let mut view = LocalView::new();
    view.offer(input(1), false, 1.);
    let (repeated, _) = view.offer(input(1), false, 1.);
    assert_eq!(repeated.frame_time_ms, 0);
    assert!(repeated.update_views && !repeated.draw_drawables);
    assert_eq!(
        view.axis_frame, 2,
        "full View camera movements still step on repeated Client frames"
    );
    assert_eq!(view.view.impulse.intensity, 2.25);
    let before = (view.value, view.axis_frame);
    let (paused, axis) = view.offer(
        DisplayInputs {
            frozen: true,
            ..input(2)
        },
        true,
        1.,
    );
    assert!(paused.update_views);
    assert!(axis.was_active && !axis.changed);
    assert_eq!((view.value, view.axis_frame), before);
    assert_eq!(
        view.view.impulse.intensity,
        2.25 * 0.75,
        "the full-view spring is independent of the scalar pause gate"
    );
    let (tivo, axis) = view.offer(
        DisplayInputs {
            tivo_replay: true,
            replay_frame: 2,
            ..input(3)
        },
        false,
        1.,
    );
    assert!(!tivo.draw_gpu && tivo.update_views);
    assert!(axis.changed);
    assert_eq!(view.axis_frame, 3);
    assert_eq!(view.view.impulse.intensity, 4. * 0.75_f32.powi(4));
}

#[test]
fn completion_frame_remains_active_and_equal_state_view_owners_are_independent() {
    let mut first = LocalView::new();
    let mut baseline = LocalView::new();
    let mut peer = LocalView::new();
    for frame in 1..=2 {
        let (_, actual) = first.offer(input(frame), false, 0.066);
        let (_, expected) = baseline.offer(input(frame), false, 0.066);
        assert_eq!(actual, expected);
        assert!(actual.was_active);
        assert_eq!(actual.completed, frame == 2);
        peer.offer(
            DisplayInputs {
                time_fast: true,
                ..input(frame)
            },
            false,
            1.,
        );
        peer.offer(
            DisplayInputs {
                device_ready: false,
                ..input(frame + 10)
            },
            false,
            1.,
        );
        assert_eq!(first.snapshot(), baseline.snapshot());
    }
    assert_eq!(first.target, None);
    assert_eq!(first.value, 1.);
    let (_, completed) = first.offer(input(3), false, 0.066);
    assert!(!completed.was_active && !completed.changed && !completed.completed);
    assert_ne!(first.snapshot(), peer.snapshot());
}

#[test]
fn display_owner_scroll_is_one_owned_operation_and_reset_clears_it() {
    let mut first = NativeDisplayState::new();
    let mut peer = NativeDisplayState::new();
    first.record_camera_scroll(4.);
    peer.record_camera_scroll(9.);
    assert_eq!(first.take_camera_scroll(), 4.);
    assert_eq!(first.take_camera_scroll(), 0.);
    assert_eq!(peer.take_camera_scroll(), 9.);
    first.record_camera_scroll(6.);
    first.reset_world(2);
    assert_eq!(first.take_camera_scroll(), 0.);
    peer.record_camera_scroll(3.);
    assert_eq!(peer.take_camera_scroll(), 3.);
}

#[test]
fn production_camera_phase_precedes_native_region_and_fast_subset_never_commits_matrix() {
    let input = include_str!("input.rs");
    assert!(!input.contains("step_axis("));
    assert!(!input.contains("self.update_script_camera_shake("));
    let display = include_str!("native_display.rs");
    let phase = &display[display
        .find("fn finish_native_display_after_admission(")
        .unwrap()..];
    assert!(
        phase
            .find("self.advance_native_camera_view(decision, camera_scroll)")
            .unwrap()
            < phase.find("view::native_view_from_camera(").unwrap()
    );
    assert!(phase.contains("if decision.update_views && !self.script_time_fast()"));
    let live = include_str!("camera_view_effects.rs");
    assert!(live.contains("if decision.update_views"));
    assert!(
        live.find("self.update_script_camera_shake()").unwrap()
            < live.find("self.ease_camera_height_above_ground(").unwrap()
    );
    assert!(
        live.find("self.ease_camera_height_above_ground(").unwrap()
            < live.find("self.update_camera_transform_shakers()").unwrap()
    );
    assert!(live.contains("if !self.script_time_fast()"));
    assert!(live.contains("advance_camera_view("));
    let owner = include_str!("../presentation_frame/session_view/camera.rs");
    assert!(owner.contains("AxisKind::ZoomPitch") && owner.contains("AxisKind::Yaw"));
}

#[test]
fn impulse_threshold_clears_only_intensity_and_preserves_phase_until_next_enqueue() {
    let (mut intensity, mut cosine, mut sine) = (0.01, -0.5, 0.75);
    assert_eq!(
        step_view_impulse(&mut intensity, &mut cosine, &mut sine, false),
        None
    );
    assert_eq!((intensity, cosine, sine), (0.01, -0.5, 0.75));
    assert_eq!(
        step_view_impulse(&mut intensity, &mut cosine, &mut sine, true),
        Some(Vec3::ZERO)
    );
    assert_eq!((intensity, cosine, sine), (0., -0.5, 0.75));
}

#[test]
fn frozen_fast_view_damps_impulse_but_retains_transform_shaker_time() {
    let mut shakers = vec![ScriptCameraShaker::new(Vec3::ZERO, 100., 1., 4.)];
    let mut peer = vec![ScriptCameraShaker::new(Vec3::ZERO, 100., 1., 4.)];
    let mut clock = VisualClock::new();
    let offer = DisplayInputs {
        frozen: true,
        time_fast: true,
        ..input(1)
    };
    let decision = clock.admit(offer);
    assert!(decision.update_views && !decision.fast_camera_only);
    let (mut intensity, mut cosine, mut sine) = (4., 1., 0.);
    step_view_impulse(
        &mut intensity,
        &mut cosine,
        &mut sine,
        decision.update_views,
    );
    step_transform_shakers(&mut shakers, decision.update_views && !offer.time_fast);
    assert_eq!(intensity, 3.);
    assert_eq!(shakers[0].elapsed_seconds, 0.);
    step_transform_shakers(&mut shakers, true);
    assert_eq!(shakers[0].elapsed_seconds, 1. / 30.);
    assert_eq!(peer[0].elapsed_seconds, 0.);
    step_transform_shakers(&mut peer, false);
    assert_eq!(peer[0].elapsed_seconds, 0.);
}

#[test]
fn owning_logic_pause_is_distinct_from_script_freeze_and_keeps_full_view_shake() {
    let mut logic = GameLogic::new();
    logic.set_paused(true);
    assert!(!logic.is_time_frozen_for_simulation());
    let mut view = LocalView::new();
    let (decision, axis) = view.offer(
        DisplayInputs {
            frozen: logic.is_paused(),
            tivo_replay: !logic.is_paused(),
            ..input(1)
        },
        logic.is_paused(),
        1.,
    );
    assert!(!axis.changed && axis.was_active);
    assert_eq!(view.axis_frame, 0);
    assert!(!decision.draw_drawables && decision.update_views && decision.draw_gpu);
    assert_eq!(view.view.impulse.intensity, 3.);
    let source = include_str!("native_display.rs");
    let start = source.find("fn native_view_is_paused(").unwrap();
    let end = source[start..].find("/// Invoked once").unwrap() + start;
    assert!(source[start..end].contains("self.game_logic.is_paused()"));
    assert!(source.contains("!self.native_view_is_paused()"));
    assert!(
        include_str!("camera_view_effects.rs").contains("paused: self.native_view_is_paused()")
    );
}

#[test]
fn admitted_zoom_matches_executed_cpp_mixed_precision_ease_trace() {
    // Original W3DView::zoomCameraOneFrame + ParabolicEase.cpp, compiled with
    // float Real and contraction disabled; these are original-execution bits.
    let golden = [
        (7, 1_053_460_034),
        (8, 1_055_100_473),
        (16, 1_062_594_296),
        (23, 1_066_753_389),
    ];
    let (mut value, mut target, mut elapsed) = (0.2, Some(1.3), 0);
    for frame in 1..=30 {
        step_axis(
            AxisMotion {
                value: &mut value,
                target: &mut target,
                frame: &mut elapsed,
                start: 0.2,
                duration_seconds: 1.,
                ease_in_seconds: 0.2,
                ease_out_seconds: 0.3,
            },
            AxisKind::ZoomPitch,
            MotionAdmission {
                update_views: true,
                fast_camera_only: false,
                paused: false,
                debug_frozen: false,
                disable_camera_movement: false,
            },
        );
        if let Some((_, bits)) = golden.iter().find(|(step, _)| *step == frame) {
            assert_eq!(value.to_bits(), *bits, "original CPP zoom frame {frame}");
        }
    }
    assert_eq!(value, 1.3);
    assert!(target.is_none());
}
