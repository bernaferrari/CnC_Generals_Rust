//! C++ `Drawable::calcPhysicsXformMotorcycle` (`Drawable.cpp:2190-2482`).

use super::PhysicsVisualXform;
use super::loco_state::PhysicsVisualLocoState;
use super::rng::ClientVisualRng;
use super::spring::{finish_accel_totals, ground_pitch_roll};
use super::types::{LocomotorVisualParams, PhysicsVisualBody};
use super::wheels::apply_grounded_z;
use super::wheels_suspension::{
    apply_airborne_rear_extension_mirrored, apply_motorcycle_suspension,
};

/// Motorcycle: no airborne return; totalRoll always 0 (impossible predicate).
pub fn calc_motorcycle(
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
            apply_airborne_rear_extension_mirrored(loco, params, body);
        }
        let pitch_height =
            body.major_radius * (loco.pitch + loco.acceleration_pitch - ground_pitch).sin();
        info.total_z = pitch_height.abs() / 4.0;
        // C++ Drawable.cpp:2273 — `return` is commented out; fall through.
    }

    apply_motorcycle_bounce(loco, params, body, rng, airborne);

    if !airborne {
        loco.pitch_rate += (-params.pitch_stiffness * (loco.pitch - ground_pitch))
            + (-params.pitch_damping * loco.pitch_rate);
        loco.roll_rate += (-params.roll_stiffness * (loco.roll - ground_roll))
            + (-params.roll_damping * loco.roll_rate);
    } else {
        // Autolevel toward 0, not ground.
        loco.pitch_rate +=
            (-params.pitch_stiffness * loco.pitch) + (-params.pitch_damping * loco.pitch_rate);
        loco.roll_rate +=
            (-params.roll_stiffness * loco.roll) + (-params.roll_damping * loco.roll_rate);
    }
    loco.pitch += loco.pitch_rate * params.uniform_axial_damping;
    loco.roll += loco.roll_rate * params.uniform_axial_damping;

    finish_accel_totals(
        loco,
        body,
        params,
        &mut info.total_pitch,
        &mut info.total_roll,
    );

    // C++ Drawable.cpp:2342-2343 — `> 0.5 && < -0.5` is impossible.
    let unclamped_roll = loco.roll + loco.acceleration_roll;
    info.total_roll = if unclamped_roll > 0.5 && unclamped_roll < -0.5 {
        unclamped_roll
    } else {
        0.0
    };

    info.total_z = 0.0;
    let pitch_height = body.major_radius * (info.total_pitch - ground_pitch).sin();
    let roll_height = body.minor_radius * (info.total_roll - ground_roll).sin();
    if params.has_suspension {
        apply_motorcycle_suspension(loco, params, body, pitch_height);
    }
    if !airborne {
        apply_grounded_z(info, ground_pitch, pitch_height, roll_height);
    }
}

fn apply_motorcycle_bounce(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
    rng: &mut impl ClientVisualRng,
    airborne: bool,
) {
    if airborne {
        return;
    }
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
