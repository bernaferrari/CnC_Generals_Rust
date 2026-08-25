//! C++ PhysicsBehavior::handleBounce, doBounceSound, testStunnedUnitForDestruction.
//! Sibling of `physics_update.rs` — keep that file from absorbing more collide/bounce logic.

use super::{FLAG_ALLOW_BOUNCE, FLAG_IS_STUNNED, PhysicsBehaviorState, apply_ypr_damping};
use crate::common::{Coord3D, Real};
use crate::helpers::{TheAudio, TheTerrainLogic};
use crate::object::Object as GameObject;
use crate::path::{SURFACE_CLIFF, SURFACE_WATER};
use game_engine::common::global_data;

const MIN_STIFF: Real = 0.01;
const MAX_STIFF: Real = 0.99;
const YPR_DAMPING: Real = 0.7;
const NORMAL_VEL_Z: Real = 0.25;
const NORMAL_MASS: Real = 50.0;

/// C++ PhysicsBehavior::handleBounce (PhysicsUpdate.cpp:481-531).
/// On vz<0, rights yaw and sets roll to 0 or PI from the current up vector.
/// On bounceForce.z > 0, runs testStunnedUnitForDestruction.
pub(super) fn handle_bounce(
    state: &mut PhysicsBehaviorState,
    obj: &mut GameObject,
    mass: Real,
    old_z: Real,
    new_z: Real,
    ground_z: Real,
) -> Option<Coord3D> {
    if !(state.has_flag(FLAG_ALLOW_BOUNCE) && new_z <= ground_z) {
        return None;
    }

    let stiffness = global_data::read_safe()
        .map(|data| data.ground_stiffness)
        .unwrap_or(0.8)
        .clamp(MIN_STIFF, MAX_STIFF);

    let vz = state.vel.z;
    let mut desired_accel_z = 0.0;
    if old_z > ground_z && vz < 0.0 {
        desired_accel_z = vz.abs() * stiffness;
    }

    let bounce_force = Coord3D::new(0.0, 0.0, mass * desired_accel_z);
    apply_ypr_damping(state, YPR_DAMPING);

    if vz < 0.0 {
        // C++: zvec.Z > 0 → roll 0, else PI. Pitch forced 0. Don't flip both.
        let up_z = state.roll_angle.cos() * state.pitch_angle.cos();
        let roll_angle = if up_z > 0.0 {
            0.0
        } else {
            std::f32::consts::PI
        };
        state.yaw_angle = obj.get_orientation();
        state.pitch_angle = 0.0;
        state.roll_angle = roll_angle;
    }

    if bounce_force.z > 0.0 {
        test_stunned_unit_for_destruction(state, obj);
        return Some(bounce_force);
    }

    state.set_flag(FLAG_ALLOW_BOUNCE, state.original_allow_bounce);
    None
}

/// C++ PhysicsBehavior::testStunnedUnitForDestruction (PhysicsUpdate.cpp:1753-1794).
pub(super) fn test_stunned_unit_for_destruction(
    state: &PhysicsBehaviorState,
    obj: &mut GameObject,
) {
    if !state.has_flag(FLAG_IS_STUNNED) {
        return;
    }

    // Upside-down after the bounce setAngles (roll ≈ PI → up_z < 0).
    let up_z = state.roll_angle.cos() * state.pitch_angle.cos();
    if up_z < 0.0 {
        obj.kill(None, None);
        return;
    }

    if obj.is_off_map() {
        obj.kill(None, None);
        return;
    }

    let Some(ai) = obj.get_ai() else {
        return;
    };
    let Ok(ai) = ai.try_lock() else {
        return;
    };
    let pos = *obj.get_position();
    let Some(terrain) = TheTerrainLogic::get() else {
        return;
    };
    if terrain.is_cliff_cell(pos.x, pos.y) && !ai.has_locomotor_for_surface(SURFACE_CLIFF) {
        obj.kill(None, None);
        return;
    }
    if terrain.is_underwater(pos.x, pos.y, None, None)
        && !ai.has_locomotor_for_surface(SURFACE_WATER)
    {
        obj.kill(None, None);
    }
}

/// C++ PhysicsBehavior::doBounceSound (PhysicsUpdate.cpp:1089-1128).
/// Volume shaping under FIX_AUDIO is not compiled in retail C++; we still play the event.
pub(super) fn do_bounce_sound(
    obj: &GameObject,
    bounce_sound: Option<&crate::common::audio::AudioEventRts>,
    prev_pos: Coord3D,
    mass: Real,
) {
    let Some(sound) = bounce_sound else {
        return;
    };
    let _ = (prev_pos, mass, NORMAL_VEL_Z, NORMAL_MASS);
    let mut event = sound.clone();
    event.set_object_id(obj.get_id() as u32);
    let _ = TheAudio.add_audio_event(&event);
}
