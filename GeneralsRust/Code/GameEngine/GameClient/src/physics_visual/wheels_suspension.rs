//! Wheel-offset integration shared by wheels and motorcycle.

use super::loco_state::PhysicsVisualLocoState;
use super::types::{LocomotorVisualParams, PhysicsVisualBody};
use crate::drawable::WheelInfo;

const WHEEL_SMOOTHNESS: f32 = 10.0;
const SPRING_FACTOR: f32 = 0.9;

/// Airborne rear-only extension (`Drawable.cpp:1956-1970`).
pub fn apply_airborne_rear_extension(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
) {
    let wheel = &mut loco.wheel_info;
    wheel.frames_airborne = 0;
    wheel.frames_airborne_counter += 1;
    let target = if body.pos_z - body.terrain_height > -params.max_wheel_extension {
        params.max_wheel_extension
    } else {
        0.0
    };
    wheel.rear_left_height_offset += (target - wheel.rear_left_height_offset) / 2.0;
    wheel.rear_right_height_offset += (target - wheel.rear_right_height_offset) / 2.0;
}

/// Motorcycle airborne: extend rear left, then mirror to right.
pub fn apply_airborne_rear_extension_mirrored(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
) {
    let wheel = &mut loco.wheel_info;
    wheel.frames_airborne = 0;
    wheel.frames_airborne_counter += 1;
    let target = if body.pos_z - body.terrain_height > -params.max_wheel_extension {
        params.max_wheel_extension
    } else {
        0.0
    };
    wheel.rear_left_height_offset += (target - wheel.rear_left_height_offset) / 2.0;
    wheel.rear_right_height_offset = wheel.rear_left_height_offset;
}

/// Steer smoothing + pitch/roll height offsets (`Drawable.cpp:2071-2175`).
pub fn apply_grounded_suspension(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
    pitch_height: f32,
    roll_height: f32,
) {
    let wheel = &mut loco.wheel_info;
    wheel.frames_airborne = wheel.frames_airborne_counter;
    wheel.frames_airborne_counter = 0;
    smooth_wheel_angle(wheel, params, body);

    let (fl, fr, rl, rr) = pitch_roll_offsets(pitch_height, roll_height);
    damp_or_snap(&mut wheel.front_left_height_offset, fl);
    damp_or_snap(&mut wheel.front_right_height_offset, fr);
    damp_or_snap(&mut wheel.rear_left_height_offset, rl);
    damp_or_snap(&mut wheel.rear_right_height_offset, rr);
    clamp_extension(wheel, params.max_wheel_extension);
}

/// Motorcycle: pitch offsets on left, then copy to right. No roll-to-wheel.
pub fn apply_motorcycle_suspension(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
    pitch_height: f32,
) {
    let wheel = &mut loco.wheel_info;
    wheel.frames_airborne = wheel.frames_airborne_counter;
    wheel.frames_airborne_counter = 0;
    smooth_wheel_angle(wheel, params, body);

    let (fl, _, rl, _) = pitch_roll_offsets(pitch_height, 0.0);
    if fl < wheel.front_left_height_offset {
        wheel.front_left_height_offset += (fl - wheel.front_left_height_offset) / 2.0;
    } else {
        wheel.front_left_height_offset = fl;
    }
    wheel.front_right_height_offset = wheel.front_left_height_offset;
    if rl < wheel.rear_left_height_offset {
        wheel.rear_left_height_offset += (rl - wheel.rear_left_height_offset) / 2.0;
    } else {
        wheel.rear_left_height_offset = rl;
    }
    wheel.rear_right_height_offset = wheel.rear_left_height_offset;

    if wheel.front_left_height_offset < params.max_wheel_extension {
        wheel.front_left_height_offset = params.max_wheel_extension;
        wheel.front_right_height_offset = params.max_wheel_extension;
    }
    if wheel.rear_left_height_offset < params.max_wheel_extension {
        wheel.rear_left_height_offset = params.max_wheel_extension;
        wheel.rear_right_height_offset = params.max_wheel_extension;
    }
}

fn smooth_wheel_angle(
    wheel: &mut WheelInfo,
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
    wheel.wheel_angle += (new_angle - wheel.wheel_angle) / WHEEL_SMOOTHNESS;
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

fn clamp_extension(wheel: &mut WheelInfo, max_extension: f32) {
    if wheel.front_left_height_offset < max_extension {
        wheel.front_left_height_offset = max_extension;
    }
    if wheel.front_right_height_offset < max_extension {
        wheel.front_right_height_offset = max_extension;
    }
    if wheel.rear_left_height_offset < max_extension {
        wheel.rear_left_height_offset = max_extension;
    }
    if wheel.rear_right_height_offset < max_extension {
        wheel.rear_right_height_offset = max_extension;
    }
}
