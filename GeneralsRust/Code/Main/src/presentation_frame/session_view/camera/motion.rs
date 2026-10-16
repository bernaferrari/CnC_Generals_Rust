//! Borrowed scalar camera movements at the original admitted View boundary.
//! W3DView.cpp:2407-2537, 2951-3071. No second camera or runtime clock lives here.

#[derive(Debug, Clone, Copy)]
pub(crate) struct MotionAdmission {
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
pub(crate) enum AxisKind {
    ZoomPitch,
    Yaw,
}

/// These references point to the existing engine fields. The canonical counter
/// is an integer, matching source curFrame; requests initialize it to zero.
/// Positional persist-v18 omits these timers; the versioned CAMC camera
/// companion retains them without changing that historical layout.
pub(crate) struct AxisMotion<'a> {
    pub value: &'a mut f32,
    pub target: &'a mut Option<f32>,
    pub frame: &'a mut u32,
    pub start: f32,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AxisStep {
    pub was_active: bool,
    pub changed: bool,
    pub completed: bool,
}

/// Original View::xfer restores the angle through W3DView::setAngle, which
/// cancels all three scalar requests (W3DView.cpp:1836-1855). The positional
/// native save predates CAMC and has no requests to restore either. Borrow only
/// the receiving camera's active flags: inactive timing metadata, application
/// input, bookmarks and other sessions are untouched.
pub(crate) fn cancel_scalar_requests(targets: [&mut Option<f32>; 3]) {
    for target in targets {
        *target = None;
    }
}

/// C++ receives integer milliseconds, clamps them to at least one, then uses
/// integer division by 33. A short or zero-duration request still takes one
/// admitted movement step, rather than changing a pose during command intake.
fn frame_count(duration_seconds: f32) -> u32 {
    let milliseconds = (duration_seconds * 1000.0) as i32;
    (milliseconds.max(1) / 33).max(1) as u32
}

pub(crate) fn step_axis(
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
    *axis.frame = axis.frame.saturating_add(1);
    let frame = *axis.frame;
    let count = frame_count(axis.duration_seconds);
    let before = *axis.value;
    if !admission.disable_camera_movement {
        if frame <= count {
            let duration_ms = ((axis.duration_seconds * 1000.0) as i32).max(1) as f32;
            let eased = parabolic_ease(
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

pub(crate) fn parabolic_ease(param: f32, ease_in_time: f32, ease_out_time: f32) -> f32 {
    let param = param.clamp(0.0, 1.0);
    let mut in_t = ease_in_time.clamp(0.0, 1.0);
    let out_t = 1.0 - ease_out_time.clamp(0.0, 1.0);
    if in_t > out_t {
        in_t = out_t;
    }
    let v0 = 1.0 + out_t - in_t;
    if param < in_t {
        if in_t <= 0.0 {
            0.0
        } else {
            param * param / (v0 * in_t)
        }
    } else if param <= out_t {
        // ParabolicEase.cpp uses double literal 2.0 here. Preserve its
        // float subexpressions before promoting the surrounding arithmetic.
        ((f64::from(in_t) + 2.0 * f64::from(param - in_t)) / f64::from(v0)) as f32
    } else {
        let numerator = f64::from(in_t)
            + 2.0 * f64::from(out_t - in_t)
            + (2.0 * f64::from(param - out_t) + f64::from(out_t * out_t)
                - f64::from(param * param))
                / f64::from(1.0 - out_t);
        (numerator / f64::from(v0)) as f32
    }
}

#[cfg(test)]
#[path = "../../../cnc_game_engine/camera_motion_tests.rs"]
mod tests;
