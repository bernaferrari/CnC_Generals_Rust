use super::*;

pub(super) fn ms_to_frames(milliseconds: i32) -> i32 {
    let ms = milliseconds.max(1) as f32;
    let frames = (ms / FRAME_LENGTH_MS) as i32;
    frames.max(1)
}

pub(super) fn frames_to_ms(frames: i32) -> i32 {
    ((frames.max(1) as f32) * FRAME_LENGTH_MS) as i32
}

pub(super) fn normalize_angle(mut angle: f32) -> f32 {
    while angle < -PI {
        angle += 2.0 * PI;
    }
    while angle > PI {
        angle -= 2.0 * PI;
    }
    angle
}

pub(super) fn travel_camera_angle(dx: f32, dy: f32, fallback: f32) -> f32 {
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.1 {
        return fallback;
    }
    let mut angle = (dx / len).acos();
    if dy < 0.0 {
        angle = -angle;
    }
    normalize_angle(angle - PI * 0.5)
}

pub(super) fn ease_ratios(milliseconds: i32, ease_in: f32, ease_out: f32) -> (f32, f32) {
    let total = milliseconds.max(1) as f32;
    (ease_in / total, ease_out / total)
}
