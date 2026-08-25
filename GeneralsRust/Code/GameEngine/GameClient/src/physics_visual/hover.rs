//! C++ `Drawable::calcPhysicsXformHoverOrWings` (`Drawable.cpp:1525-1633`).

use super::PhysicsVisualXform;
use super::loco_state::PhysicsVisualLocoState;
use super::spring::{
    apply_motive_accel_kick, clamp_accel_pitch_roll, integrate_accel_axis, integrate_chassis_axis,
};
use super::types::{LocomotorVisualParams, PhysicsVisualBody};

/// Hover / wings spring + motive vel/accel + rudder/elevator modulators.
pub fn calc_hover_or_wings(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    body: &PhysicsVisualBody,
    info: &mut PhysicsVisualXform,
) {
    // C++ Drawable.cpp:1546-1557 — early-out leaves info at zeros.
    if !body.has_object || !body.has_ai || !body.has_physics {
        return;
    }

    integrate_chassis_axis(
        &mut loco.pitch,
        &mut loco.pitch_rate,
        0.0,
        params.pitch_stiffness,
        params.pitch_damping,
        params.uniform_axial_damping,
    );
    integrate_chassis_axis(
        &mut loco.roll,
        &mut loco.roll_rate,
        0.0,
        params.roll_stiffness,
        params.roll_damping,
        params.uniform_axial_damping,
    );

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

    info.total_pitch = loco.pitch + loco.acceleration_pitch;
    info.total_roll = loco.roll + loco.acceleration_roll;

    if body.is_motive {
        if params.pitch_by_z_vel_coef != 0.0 && body.vel_z.abs() > 0.001 {
            let horiz = (body.vel_x * body.vel_x + body.vel_y * body.vel_y).sqrt();
            let pitch = body.vel_z.atan2(horiz);
            loco.pitch -= params.pitch_by_z_vel_coef * pitch;
        }
        let forward_vel = body.dir_x * body.vel_x + body.dir_y * body.vel_y;
        loco.pitch += -(params.forward_vel_coef * forward_vel);
        let lateral_vel = -body.dir_y * body.vel_x + body.dir_x * body.vel_y;
        loco.roll += -(params.lateral_vel_coef * lateral_vel);
        apply_motive_accel_kick(loco, body, params);
    }

    clamp_accel_pitch_roll(loco, params);

    loco.yaw_modulator += params.rudder_correction_rate;
    loco.pitch_modulator += params.elevator_correction_rate;
    info.total_yaw = params.rudder_correction_degree * loco.yaw_modulator.sin();
    info.total_pitch += params.elevator_correction_degree * loco.pitch_modulator.cos();
    info.total_z = 0.0;
}
