//! C++ `Drawable::calcPhysicsXformThrust` (`Drawable.cpp:1446-1521`).

use super::PhysicsVisualXform;
use super::loco_state::PhysicsVisualLocoState;
use super::types::LocomotorVisualParams;

/// Thrust wobble / roll. No object, physics, or terrain input.
pub fn calc_thrust(
    loco: &mut PhysicsVisualLocoState,
    params: &LocomotorVisualParams,
    info: &mut PhysicsVisualXform,
) {
    let wobble_rate = params.wobble_rate;
    if wobble_rate != 0.0 {
        if loco.wobble >= 1.0 {
            if loco.pitch < params.max_wobble - wobble_rate * 2.0 {
                loco.pitch += wobble_rate;
                loco.yaw += wobble_rate;
            } else {
                loco.pitch += wobble_rate / 2.0;
                loco.yaw += wobble_rate / 2.0;
            }
            if loco.pitch >= params.max_wobble {
                loco.wobble = -1.0;
            }
        } else {
            if loco.pitch >= params.min_wobble + wobble_rate * 2.0 {
                loco.pitch -= wobble_rate;
                loco.yaw -= wobble_rate;
            } else {
                loco.pitch -= wobble_rate / 2.0;
                loco.yaw -= wobble_rate / 2.0;
            }
            if loco.pitch <= params.min_wobble {
                loco.wobble = 1.0;
            }
        }
        info.total_pitch = loco.pitch;
        info.total_yaw = loco.yaw;
    }

    if params.thrust_roll != 0.0 {
        loco.roll += params.thrust_roll;
        info.total_roll = loco.roll;
    }
}
