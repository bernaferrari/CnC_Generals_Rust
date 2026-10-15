use super::*;

fn admitted() -> MotionAdmission {
    MotionAdmission {
        update_views: true,
        fast_camera_only: false,
        paused: false,
        debug_frozen: false,
        disable_camera_movement: false,
    }
}
struct Fields {
    value: f32,
    target: Option<f32>,
    frame: u32,
}
impl Fields {
    fn new() -> Self {
        Self {
            value: 0.0,
            target: Some(1.0),
            frame: 0,
        }
    }
    fn step(&mut self, duration: f32, kind: AxisKind, mode: MotionAdmission) -> AxisStep {
        step_axis(
            AxisMotion {
                value: &mut self.value,
                target: &mut self.target,
                frame: &mut self.frame,
                start: 0.0,
                duration_seconds: duration,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            },
            kind,
            mode,
        )
    }
}

#[test]
fn rejected_view_offers_never_advance_actual_borrowed_fields() {
    let mut fields = Fields::new();
    let rejected = MotionAdmission {
        update_views: false,
        ..admitted()
    };
    for _ in 0..8 {
        let result = fields.step(1.0, AxisKind::ZoomPitch, rejected);
        assert!(result.was_active && !result.changed && !result.completed);
        assert_eq!(
            (fields.value, fields.frame, fields.target),
            (0.0, 0, Some(1.0))
        );
    }
    fields.step(1.0, AxisKind::ZoomPitch, admitted());
    assert_eq!(fields.frame, 1);
    assert_eq!(fields.value, 1.0 / 30.0);
}

#[test]
fn paused_and_debug_freeze_block_full_view_but_script_freeze_does_not() {
    for mode in [
        MotionAdmission {
            paused: true,
            ..admitted()
        },
        MotionAdmission {
            debug_frozen: true,
            ..admitted()
        },
    ] {
        let mut fields = Fields::new();
        fields.step(1.0, AxisKind::ZoomPitch, mode);
        assert_eq!(fields.frame, 0);
        assert_eq!(fields.value, 0.0);
    }
    // Script freeze is intentionally not a movement gate; an admitted frozen
    // View still performs one source updateCameraMovements step.
    let mut fields = Fields::new();
    fields.step(1.0, AxisKind::ZoomPitch, admitted());
    assert_eq!(fields.frame, 1);
}

#[test]
fn fast_camera_subset_steps_once_without_a_full_view() {
    let mut fields = Fields::new();
    fields.step(
        1.0,
        AxisKind::ZoomPitch,
        MotionAdmission {
            update_views: false,
            fast_camera_only: true,
            ..admitted()
        },
    );
    assert_eq!((fields.frame, fields.value), (1, 1.0 / 30.0));
}

#[test]
fn integer_millisecond_division_preserves_short_and_completion_boundaries() {
    assert_eq!(frame_count(0.0), 1);
    assert_eq!(frame_count(0.032), 1);
    assert_eq!(frame_count(0.065), 1);
    assert_eq!(frame_count(0.066), 2);
    assert_eq!(frame_count(1.0), 30);
    let mut fields = Fields::new();
    for _ in 0..29 {
        assert!(!fields.step(1.0, AxisKind::ZoomPitch, admitted()).completed);
    }
    assert!(fields.target.is_some());
    assert!(fields.step(1.0, AxisKind::ZoomPitch, admitted()).completed);
    assert_eq!((fields.value, fields.target), (1.0, None));
    assert_eq!(fields.frame, 30);
    let after = fields.frame;
    assert!(!fields.step(1.0, AxisKind::ZoomPitch, admitted()).was_active);
    assert_eq!(fields.frame, after);
}

#[test]
fn disabled_movement_consumes_frames_without_snapping_pose() {
    let mut fields = Fields::new();
    fields.value = 0.375;
    let disabled = MotionAdmission {
        disable_camera_movement: true,
        ..admitted()
    };
    assert!(!fields.step(0.066, AxisKind::ZoomPitch, disabled).completed);
    let result = fields.step(0.066, AxisKind::ZoomPitch, disabled);
    assert!(result.completed && !result.changed);
    assert_eq!((fields.value, fields.target), (0.375, None));
}

#[test]
fn yaw_normalizes_intermediate_pose_but_preserves_exact_final_endpoint() {
    let mut fields = Fields::new();
    fields.target = Some(4.0 * std::f32::consts::PI);
    fields.step(0.099, AxisKind::Yaw, admitted());
    assert!(fields.value.abs() <= std::f32::consts::PI);
    fields.step(0.099, AxisKind::Yaw, admitted());
    fields.step(0.099, AxisKind::Yaw, admitted());
    assert_eq!(fields.value, 4.0 * std::f32::consts::PI);
    assert_eq!(fields.target, None);
}

#[test]
fn easing_uses_source_frame_ratio_and_receiving_fields_are_independent() {
    let mut first = Fields::new();
    let mut peer = Fields::new();
    let result = step_axis(
        AxisMotion {
            value: &mut first.value,
            target: &mut first.target,
            frame: &mut first.frame,
            start: 0.0,
            duration_seconds: 0.132,
            ease_in_seconds: 0.066,
            ease_out_seconds: 0.0,
        },
        AxisKind::ZoomPitch,
        admitted(),
    );
    assert!(result.changed);
    assert_eq!(
        first.value,
        super::super::CnCGameEngine::parabolic_ease(0.25, 0.5, 0.0)
    );
    assert_eq!((peer.value, peer.frame, peer.target), (0.0, 0, Some(1.0)));
    peer.step(0.066, AxisKind::ZoomPitch, admitted());
    assert_eq!(first.frame, 1);
}

#[test]
fn integer_counter_advances_beyond_float_precision_and_preserves_completion() {
    let mut fields = Fields::new();
    fields.frame = 1 << 24;
    // Still a valid original i32 millisecond duration: 33,000,000 frames.
    for expected in [(1 << 24) + 1, (1 << 24) + 2, (1 << 24) + 3] {
        let result = fields.step(1_089_000.0, AxisKind::ZoomPitch, admitted());
        assert_eq!(
            fields.frame, expected,
            "source integer counter never stalls at f32 precision limit"
        );
        assert!(result.was_active && !result.completed);
        assert!(fields.target.is_some());
    }
    let mut ordinary = Fields::new();
    for frame in 1..=1000 {
        let result = ordinary.step(33.0, AxisKind::ZoomPitch, admitted());
        assert_eq!(ordinary.frame, frame);
        assert_eq!(result.completed, frame == 1000);
        assert!(
            result.was_active,
            "completion frame remains scripted for HAG"
        );
    }
    assert_eq!(ordinary.value, 1.0);
}

#[test]
fn pose_only_admission_stops_receiving_axes_without_touching_another_camera() {
    let mut receiving = [Fields::new(), Fields::new(), Fields::new()];
    let mut foreign = Fields::new();
    for axis in &mut receiving {
        axis.step(1.0, AxisKind::ZoomPitch, admitted());
    }
    let poses = receiving.each_ref().map(|axis| (axis.value, axis.frame));
    let [zoom, pitch, yaw] = &mut receiving;
    cancel_scalar_requests([&mut zoom.target, &mut pitch.target, &mut yaw.target]);
    for (axis, expected) in receiving.iter_mut().zip(poses) {
        for _ in 0..40 {
            let result = axis.step(1.0, AxisKind::ZoomPitch, admitted());
            assert!(!result.was_active && !result.changed);
        }
        assert_eq!((axis.value, axis.frame), expected);
        assert_eq!(axis.target, None);
    }
    assert!(
        foreign
            .step(1.0, AxisKind::ZoomPitch, admitted())
            .was_active
    );
    assert_eq!(foreign.target, Some(1.0));
}
