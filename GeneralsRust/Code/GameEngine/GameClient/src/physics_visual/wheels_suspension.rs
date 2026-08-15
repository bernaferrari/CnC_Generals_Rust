//! Wheel-offset integration shared by wheels and motorcycle.

use super::loco_state::PhysicsVisualLocoState;
use super::types::{LocomotorVisualParams, PhysicsVisualBody};

const WHEEL_SMOOTHNESS: f32 = 10.0;
const SPRING_FACTOR: f32 = 0.9;

/// Airborne rear-only extension (`Drawable.cpp:1956-1970`).
pub fn apply_airborne_rear_extension(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
) {
    loco.frames_airborne = 0;
    loco.frames_airborne_counter += 1;
    let target = if body.pos_z - body.terrain_height > -params.max_wheel_extension {
        params.max_wheel_extension
    } else {
        0.0
    };
    loco.rear_left_height_offset += (target - loco.rear_left_height_offset) / 2.0;
    loco.rear_right_height_offset += (target - loco.rear_right_height_offset) / 2.0;
}

/// Motorcycle airborne: extend rear left, then mirror to right.
pub fn apply_airborne_rear_extension_mirrored(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
) {
    loco.frames_airborne = 0;
    loco.frames_airborne_counter += 1;
    let target = if body.pos_z - body.terrain_height > -params.max_wheel_extension {
        params.max_wheel_extension
    } else {
        0.0
    };
    loco.rear_left_height_offset += (target - loco.rear_left_height_offset) / 2.0;
    loco.rear_right_height_offset = loco.rear_left_height_offset;
}

/// Steer smoothing + pitch/roll height offsets (`Drawable.cpp:2071-2175`).
pub fn apply_grounded_suspension(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
    pitch_height: f32,
    roll_height: f32,
) {
    loco.frames_airborne = loco.frames_airborne_counter;
    loco.frames_airborne_counter = 0;
    smooth_wheel_angle(loco, params, body);

    let (fl, fr, rl, rr) = pitch_roll_offsets(pitch_height, roll_height);
    damp_or_snap(&mut loco.front_left_height_offset, fl);
    damp_or_snap(&mut loco.front_right_height_offset, fr);
    damp_or_snap(&mut loco.rear_left_height_offset, rl);
    damp_or_snap(&mut loco.rear_right_height_offset, rr);
    clamp_extension(loco, params.max_wheel_extension);
}

/// Motorcycle: pitch offsets on left, then copy to right. No roll-to-wheel.
pub fn apply_motorcycle_suspension(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
    pitch_height: f32,
) {
    loco.frames_airborne = loco.frames_airborne_counter;
    loco.frames_airborne_counter = 0;
    smooth_wheel_angle(loco, params, body);

    let (fl, _, rl, _) = pitch_roll_offsets(pitch_height, 0.0);
    if fl < loco.front_left_height_offset {
        loco.front_left_height_offset += (fl - loco.front_left_height_offset) / 2.0;
    } else {
        loco.front_left_height_offset = fl;
    }
    loco.front_right_height_offset = loco.front_left_height_offset;
    if rl < loco.rear_left_height_offset {
        loco.rear_left_height_offset += (rl - loco.rear_left_height_offset) / 2.0;
    } else {
        loco.rear_left_height_offset = rl;
    }
    loco.rear_right_height_offset = loco.rear_left_height_offset;

    if loco.front_left_height_offset < params.max_wheel_extension {
        loco.front_left_height_offset = params.max_wheel_extension;
        loco.front_right_height_offset = params.max_wheel_extension;
    }
    if loco.rear_left_height_offset < params.max_wheel_extension {
        loco.rear_left_height_offset = params.max_wheel_extension;
        loco.rear_right_height_offset = params.max_wheel_extension;
    }
}

fn smooth_wheel_angle(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
) {
    let mut new_angle = if body.turning < 0 {
        -params.wheel_turn_angle
    } else if body.turning > 0 {
        params.wheel_turn_angle
    } else {
        0.0
    };
    if body.forward_speed_2d < 0.0 {
        new_angle = -new_angle;
    }
    loco.wheel_angle += (new_angle - loco.wheel_angle) / WHEEL_SMOOTHNESS;
}

fn pitch_roll_offsets(pitch_height: f32, roll_height: f32) -> (f32, f32, f32, f32) {
    let (mut fl, mut fr, mut rl, mut rr) = if pitch_height < 0.0 {
        let front = SPRING_FACTOR * (pitch_height / 3.0 + pitch_height / 2.0);
        let rear = -pitch_height / 2.0 + pitch_height / 4.0;
        (front, front, rear, rear)
    } else {
        let front = -pitch_height / 4.0 + pitch_height / 2.0;
        let rear = SPRING_FACTOR * (-pitch_height / 2.0 + -pitch_height / 3.0);
        (front, front, rear, rear)
    };
    if roll_height > 0.0 {
        let right = -SPRING_FACTOR * (roll_height / 3.0 + roll_height / 2.0);
        let left = roll_height / 2.0 - roll_height / 4.0;
        fr += right;
        rr += right;
        rl += left;
        fl += left;
    } else {
        let right = -roll_height / 2.0 + roll_height / 4.0;
        let left = SPRING_FACTOR * (roll_height / 3.0 + roll_height / 2.0);
        fr += right;
        rr += right;
        rl += left;
        fl += left;
    }
    (fl, fr, rl, rr)
}

fn damp_or_snap(current: &mut f32, new_value: f32) {
    if new_value < *current {
        *current += (new_value - *current) / 2.0;
    } else {
        *current = new_value;
    }
}

fn clamp_extension(loco: &mut PhysicsVisualLocoState, max_extension: f32) {
    if loco.front_left_height_offset < max_extension {
        loco.front_left_height_offset = max_extension;
    }
    if loco.front_right_height_offset < max_extension {
        loco.front_right_height_offset = max_extension;
    }
    if loco.rear_left_height_offset < max_extension {
        loco.rear_left_height_offset = max_extension;
    }
    if loco.rear_right_height_offset < max_extension {
        loco.rear_right_height_offset = max_extension;
    }
}
