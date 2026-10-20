//! Observations from the actual receiving SessionView, not a second scalar algorithm.
//! Source input cases match the extracted original-C++ component driver.
use super::camera::motion::MotionAdmission;
use super::*;
use serde_json::{Value, json};

fn admitted() -> MotionAdmission {
    MotionAdmission {
        update_views: true,
        fast_camera_only: false,
        paused: false,
        debug_frozen: false,
        disable_camera_movement: false,
    }
}
fn observe(rows: &mut Vec<Value>, scenario: &str, phase: &str, view: &SessionView, speed: f32) {
    rows.push(json!({
        "scenario":scenario,"phase":phase,
        "cur_frame":view.camera.camera_yaw_frame,
        "num_frames":view.camera.rotation.turn_frames,
        "hold_frames":view.camera.rotation_registers.hold_frames(),
        "track_object":u8::from(view.camera.rotation_registers.is_tracking()),
        "start_multiplier":view.camera.rotation.time.start,
        "end_multiplier":view.camera.rotation.time.end,
        "multiplier":speed as i32,
        "angle_bits":view.camera.camera_yaw_radians.to_bits(),
        "active":u8::from(view.camera.rotation_registers.active()),
    }));
}
fn offer(view: &mut SessionView, speed: &mut f32, mode: MotionAdmission) {
    view.advance_camera_movements(mode, 33, speed);
}

#[test]
fn actual_receiving_scalar_rows_for_original_cpp_comparison() {
    let mut rows = Vec::new();
    {
        let mut view = SessionView::default();
        let mut speed = 1.;
        view.install_rotate(0.25, 0.132, 0., 0., speed);
        // Future endpoint fixture, not expected current-speed output: the
        // production movement owner performs every interpolation/rounding step.
        view.camera.rotation.time.end = 5;
        observe(&mut rows, "ordinary", "admit", &view, speed);
        for _ in 0..4 {
            offer(&mut view, &mut speed, admitted());
            observe(&mut rows, "ordinary", "normal", &view, speed);
        }
        assert_eq!(speed, 5.);
        assert!(!view.camera.rotation_registers.active());
    }
    {
        let mut view = SessionView::default();
        let mut speed = 5.;
        view.install_rotate(0.25, 0.066, 0., 0., speed);
        view.camera.rotation.time.end = 2;
        offer(&mut view, &mut speed, admitted());
        observe(&mut rows, "decreasing_half", "normal", &view, speed);
        assert_eq!(
            speed, 3.,
            "source NEAR fast_float_floor sign-subtract/trunc boundary"
        );
        offer(&mut view, &mut speed, admitted());
        observe(&mut rows, "decreasing_half", "normal", &view, speed);
    }
    {
        let mut view = SessionView::default();
        let mut speed = 1.;
        view.install_rotate(0.25, 0.132, 0.033, 0.033, speed);
        view.camera.rotation.time.end = 5;
        offer(&mut view, &mut speed, admitted());
        observe(&mut rows, "near_zero_eased", "before", &view, speed);
        let same = view.target;
        assert!(!view.install_look_toward(same, same, 0.99, 0.999, 0.999, false, speed));
        observe(&mut rows, "near_zero_eased", "after_admit", &view, speed);
        assert_eq!(view.camera.rotation.turn_frames, 30);
        assert_eq!(view.camera.camera_yaw_frame, 1);
        assert_eq!(view.camera.rotation.time.end, 5);
        for _ in 0..3 {
            offer(&mut view, &mut speed, admitted());
            observe(&mut rows, "near_zero_eased", "normal", &view, speed);
        }
        assert_eq!(
            speed, 1.,
            "old .25/.25 ease retained independently of new count"
        );
    }
    {
        let mut view = SessionView::default();
        let mut speed = 1.;
        view.install_rotate(0.25, 0.132, 0., 0., speed);
        view.camera.rotation.time.end = 5;
        offer(
            &mut view,
            &mut speed,
            MotionAdmission {
                debug_frozen: true,
                ..admitted()
            },
        );
        observe(&mut rows, "admission", "debug_rejected", &view, speed);
        offer(
            &mut view,
            &mut speed,
            MotionAdmission {
                paused: true,
                ..admitted()
            },
        );
        observe(&mut rows, "admission", "pause_rejected", &view, speed);
        assert_eq!(view.camera.camera_yaw_frame, 0);
        // This is the already-admitted View phase: script freeze does not
        // suppress ordinary scripted movement (the actual C++ gate comments it out).
        offer(&mut view, &mut speed, admitted());
        observe(&mut rows, "admission", "script_freeze_normal", &view, speed);
        offer(
            &mut view,
            &mut speed,
            MotionAdmission {
                fast_camera_only: true,
                update_views: false,
                ..admitted()
            },
        );
        observe(&mut rows, "admission", "admitted_fast", &view, speed);
        for phase in ["disabled", "disabled_expiry"] {
            offer(
                &mut view,
                &mut speed,
                MotionAdmission {
                    disable_camera_movement: true,
                    ..admitted()
                },
            );
            observe(&mut rows, "admission", phase, &view, speed);
        }
        offer(&mut view, &mut speed, admitted());
        observe(
            &mut rows,
            "admission",
            "inactive_after_disabled_expiry",
            &view,
            speed,
        );
        assert_eq!(
            speed, 3.,
            "disabled final offer cannot publish a late final multiplier"
        );
    }
    for factor in [0x3effffff, 0x3f000000, 0x3f000001] {
        let mut view = SessionView::default();
        view.camera.rotation.time = super::camera::rotation::RotationTime { start: 2, end: 3 };
        let speed = view.camera.rotation.time.sample(f32::from_bits(factor));
        observe(
            &mut rows,
            "round_boundary",
            &factor.to_string(),
            &view,
            speed,
        );
    }
    assert_eq!(
        rows.len(),
        22,
        "nonempty exact selected source case coverage"
    );
    if let Some(path) = std::env::var_os("GENERALS_SCALAR_CPP_TRACE_PATH") {
        let path = std::path::PathBuf::from(path);
        assert!(
            path.is_absolute(),
            "trace output must be an explicit absolute evidence path"
        );
        let mut bytes=serde_json::to_vec(&json!({"schema":"rust-session-scalar-component-v1","rounding":"rust-platform-default"})).unwrap();
        bytes.push(b'\n');
        for row in rows {
            serde_json::to_writer(&mut bytes, &row).unwrap();
            bytes.push(b'\n');
        }
        std::fs::write(path, bytes).unwrap();
    }
}
