//! C++ `Drawable::calcPhysicsXformWheels` (`Drawable.cpp:1895-2186`).

use super::PhysicsVisualXform;
use super::loco_state::PhysicsVisualLocoState;
use super::rng::ClientVisualRng;
use super::spring::{finish_accel_totals, ground_pitch_roll};
use super::types::{CPP_PI, LocomotorVisualParams, PhysicsVisualBody};
use super::wheels_suspension::{apply_airborne_rear_extension, apply_grounded_suspension};

/// Wheels: airborne early-return, bounce, ground spring, suspension, Z divisor.
pub fn calc_wheels(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
    rng: &mut impl ClientVisualRng,
    info: &mut PhysicsVisualXform,
) {
    if !body.has_object || !body.has_ai || !body.has_physics {
        return;
    }

    let (ground_pitch, ground_roll) = ground_pitch_roll(body);
    let airborne = body.significantly_above_terrain;

    if airborne {
        if params.has_suspension {
            apply_airborne_rear_extension(loco, params, body);
        }
        let pitch_height =
            body.major_radius * (loco.pitch + loco.acceleration_pitch - ground_pitch).sin();
        let roll_height =
            body.minor_radius * (loco.roll + loco.acceleration_roll - ground_roll).sin();
        info.total_z = pitch_height.abs() / 4.0 + roll_height.abs() / 4.0;
        // C++ Drawable.cpp:1978 — orientation frozen; pitch/roll/yaw stay 0.
        return;
    }

    apply_bounce(loco, params, body, rng);

    loco.pitch_rate += (-params.pitch_stiffness * (loco.pitch - ground_pitch))
        + (-params.pitch_damping * loco.pitch_rate);
    if loco.pitch_rate > 0.0 {
        loco.pitch_rate *= 0.5;
    }
    loco.roll_rate += (-params.roll_stiffness * (loco.roll - ground_roll))
        + (-params.roll_damping * loco.roll_rate);
    loco.pitch += loco.pitch_rate * params.uniform_axial_damping;
    loco.roll += loco.roll_rate * params.uniform_axial_damping;

    finish_accel_totals(
        loco,
        body,
        params,
        &mut info.total_pitch,
        &mut info.total_roll,
    );
    info.total_z = 0.0;

    let pitch_height = body.major_radius * (info.total_pitch - ground_pitch).sin();
    let roll_height = body.minor_radius * (info.total_roll - ground_roll).sin();
    if params.has_suspension {
        apply_grounded_suspension(loco, params, body, pitch_height, roll_height);
    }
    apply_grounded_z(info, ground_pitch, pitch_height, roll_height);
}

fn apply_bounce(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
    rng: &mut impl ClientVisualRng,
) {
    let max_speed = body.cur_locomotor_speed;
    if body.velocity_magnitude <= max_speed / 10.0 {
        return;
    }
    let factor = if max_speed != 0.0 {
        body.velocity_magnitude / max_speed
    } else {
        0.0
    };
    let kick = params.bounce_kick;
    if loco.pitch_rate.abs() >= factor * kick / 4.0 || loco.roll_rate.abs() >= factor * kick / 8.0 {
        return;
    }
    // C++ GameClientRandomValue(0, 3) inclusive.
    match rng.random_int(0, 3) {
        0 => {
            loco.pitch_rate -= kick * factor;
            loco.roll_rate -= kick * factor / 2.0;
        }
        1 => {
            loco.pitch_rate += kick * factor;
            loco.roll_rate -= kick * factor / 2.0;
        }
        2 => {
            loco.pitch_rate -= kick * factor;
            loco.roll_rate += kick * factor / 2.0;
        }
        _ => {
            loco.pitch_rate += kick * factor;
            loco.roll_rate += kick * factor / 2.0;
        }
    }
}

pub(crate) fn apply_grounded_z(
    info: &mut PhysicsVisualXform,
    ground_pitch: f32,
    pitch_height: f32,
    roll_height: f32,
) {
    let mut divisor = 4.0;
    let pitch = (info.total_pitch - ground_pitch).abs();
    if pitch > CPP_PI / 8.0 {
        divisor = ((4.0 * CPP_PI / 8.0) + (pitch - CPP_PI / 8.0)) / pitch;
    }
    info.total_z += pitch_height.abs() / divisor;
    info.total_z += roll_height.abs() / divisor;
}
