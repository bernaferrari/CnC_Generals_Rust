//! Borrowed scalar camera movements at the original admitted View boundary.
//! W3DView.cpp:2407-2537, 2951-3071. No second camera or runtime clock lives here.

const FRAME_SECONDS: f32 = 0.033;

#[derive(Debug, Clone, Copy)]
pub(super) struct MotionAdmission {
    pub update_views: bool,
    pub fast_camera_only: bool,
    pub paused: bool,
    pub debug_frozen: bool,
    pub disable_camera_movement: bool,
}

impl MotionAdmission {
    fn advances(self) -> bool {
        self.fast_camera_only || (self.update_views && !self.paused && !self.debug_frozen)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum AxisKind {
    ZoomPitch,
    Yaw,
}

/// These references point to the existing engine fields. Elapsed seconds encode
/// whole source frames; requests initialize them to zero. Camera timers are not
/// part of the current native/persist-v18 save payload.
pub(super) struct AxisMotion<'a> {
    pub value: &'a mut f32,
    pub target: &'a mut Option<f32>,
    pub elapsed_seconds: &'a mut f32,
    pub start: f32,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AxisStep {
    pub was_active: bool,
    pub changed: bool,
    pub completed: bool,
}

/// C++ receives integer milliseconds, clamps them to at least one, then uses
/// integer division by 33. A short or zero-duration request still takes one
/// admitted movement step, rather than changing a pose during command intake.
fn frame_count(duration_seconds: f32) -> u32 {
    let milliseconds = (duration_seconds * 1000.0) as i32;
    (milliseconds.max(1) / 33).max(1) as u32
}

pub(super) fn step_axis(
    axis: AxisMotion<'_>,
    kind: AxisKind,
    admission: MotionAdmission,
) -> AxisStep {
    let Some(target) = *axis.target else {
        return AxisStep {
            was_active: false,
            changed: false,
            completed: false,
        };
    };
    let mut result = AxisStep {
        was_active: true,
        changed: false,
        completed: false,
    };
    if !admission.advances() {
        return result;
    }
    // Reconstruct only the integral frame already encoded by this helper, not
    // a wall-clock delta. Rounding avoids .033 representation drift on step N.
    let frame = (*axis.elapsed_seconds / FRAME_SECONDS).round().max(0.0) as u32;
    let frame = frame.saturating_add(1);
    *axis.elapsed_seconds = frame as f32 * FRAME_SECONDS;
    let count = frame_count(axis.duration_seconds);
    let before = *axis.value;
    if !admission.disable_camera_movement {
        if frame <= count {
            let duration_ms = ((axis.duration_seconds * 1000.0) as i32).max(1) as f32;
            let eased = super::CnCGameEngine::parabolic_ease(
                frame as f32 / count as f32,
                axis.ease_in_seconds * 1000.0 / duration_ms,
                axis.ease_out_seconds * 1000.0 / duration_ms,
            );
            *axis.value = axis.start + (target - axis.start) * eased;
            if matches!(kind, AxisKind::Yaw) {
                normalize_yaw(axis.value);
            }
        }
        if frame >= count {
            // Source rotateCameraOneFrame assigns the exact final angle after
            // its intermediate normalization, including a full-turn endpoint.
            *axis.value = target;
        }
    }
    if frame >= count {
        *axis.target = None;
        result.completed = true;
    }
    result.changed = before.to_bits() != axis.value.to_bits();
    result
}

fn normalize_yaw(angle: &mut f32) {
    let pi = std::f32::consts::PI;
    if *angle < -10.0 * pi || *angle > 10.0 * pi {
        *angle = 0.0;
    }
    while *angle < -pi {
        *angle += 2.0 * pi;
    }
    while *angle > pi {
        *angle -= 2.0 * pi;
    }
}

#[cfg(test)]
#[path = "camera_motion_tests.rs"]
mod tests;
