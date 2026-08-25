//! Shared chassis spring / accel helpers from `Drawable.cpp`.

use super::loco_state::PhysicsVisualLocoState;
use super::types::{CPP_PI, LocomotorVisualParams, PhysicsVisualBody};

/// Ground pitch/roll from the terrain (or overlap) normal.
///
/// C++ `Drawable.cpp:1782-1786` / `1946-1950` / `2241-2245`.
#[must_use]
pub fn ground_pitch_roll(body: &PhysicsVisualBody) -> (f32, f32) {
    let perp_x = -body.dir_y;
    let perp_y = body.dir_x;
    let ground_pitch =
        (body.terrain_normal_x * body.dir_x + body.terrain_normal_y * body.dir_y) * (CPP_PI / 2.0);
    let ground_roll =
        (body.terrain_normal_x * perp_x + body.terrain_normal_y * perp_y) * (CPP_PI / 2.0);
    (ground_pitch, ground_roll)
}

/// Chassis spring toward `target`, then integrate with uniform axial damping.
pub fn integrate_chassis_axis(
    value: &mut f32,
    rate: &mut f32,
    target: f32,
    stiffness: f32,
    damping: f32,
    uniform_axial: f32,
) {
    *rate += (-stiffness * (*value - target)) + (-damping * *rate);
    *value += *rate * uniform_axial;
}

/// Accel-pitch/roll spring toward zero (no uniform axial on the integrate).
///
/// C++ `Drawable.cpp:1572-1576` and the matching treads/wheels blocks.
pub fn integrate_accel_axis(value: &mut f32, rate: &mut f32, stiffness: f32, damping: f32) {
    *rate += (-stiffness * *value) + (-damping * *rate);
    *value += *rate;
}

/// Motive forward/lateral accel kick into accel rates.
///
/// C++ `Drawable.cpp:1601-1606` / `1815-1822` / `2042-2049`.
pub fn apply_motive_accel_kick(
    loco: &mut PhysicsVisualLocoState,
    body: &PhysicsVisualBody,
    params: &LocomotorVisualParams,
) {
    if !body.is_motive {
        return;
    }
    let forward_accel = body.dir_x * body.accel_x + body.dir_y * body.accel_y;
    let lateral_accel = -body.dir_y * body.accel_x + body.dir_x * body.accel_y;
    loco.acceleration_pitch_rate += -(params.forward_accel_coef * forward_accel);
    loco.acceleration_roll_rate += -(params.lateral_accel_coef * lateral_accel);
}

/// Clamp accel pitch/roll to the authored accel/decel limits.
///
/// C++ `Drawable.cpp:1611-1619`. Positive = nose-down (decel).
pub fn clamp_accel_pitch_roll(loco: &mut PhysicsVisualLocoState, params: &LocomotorVisualParams) {
    if loco.acceleration_pitch > params.decel_pitch_limit {
        loco.acceleration_pitch = params.decel_pitch_limit;
    } else if loco.acceleration_pitch < -params.accel_pitch_limit {
        loco.acceleration_pitch = -params.accel_pitch_limit;
    }
    if loco.acceleration_roll > params.decel_pitch_limit {
        loco.acceleration_roll = params.decel_pitch_limit;
    } else if loco.acceleration_roll < -params.accel_pitch_limit {
        loco.acceleration_roll = -params.accel_pitch_limit;
    }
}

/// Integrate accel springs, write chassis+accel totals, motive kick, clamp.
pub fn finish_accel_totals(
    loco: &mut PhysicsVisualLocoState,
    body: &PhysicsVisualBody,
    params: &LocomotorVisualParams,
    info_pitch: &mut f32,
    info_roll: &mut f32,
) {
    integrate_accel_axis(
        &mut loco.acceleration_pitch,
        &mut loco.acceleration_pitch_rate,
        params.pitch_stiffness,
        params.pitch_damping,
    );
    integrate_accel_axis(
        &mut loco.acceleration_roll,
        &mut loco.acceleration_roll_rate,
        params.roll_stiffness,
        params.roll_damping,
    );
    *info_pitch = loco.pitch + loco.acceleration_pitch;
    *info_roll = loco.roll + loco.acceleration_roll;
    apply_motive_accel_kick(loco, body, params);
    clamp_accel_pitch_roll(loco, params);
}

#[must_use]
pub fn normalize3(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let len = (x * x + y * y + z * z).sqrt();
    if len > 0.0 {
        (x / len, y / len, z / len)
    } else {
        (x, y, z)
    }
}

#[must_use]
pub fn cross3(ax: f32, ay: f32, az: f32, bx: f32, by: f32, bz: f32) -> (f32, f32, f32) {
    (ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx)
}
