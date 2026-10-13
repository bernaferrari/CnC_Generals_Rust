//! Canonical synchronous PhysicsUpdate.cpp bounce continuations.
use super::*;

/// A synchronous bounce continuation. The first transform is observable before
/// stun destruction; ground clamping and the outer transform follow callbacks.
pub(crate) struct OwnedBounce {
    pub(crate) first_transform: Option<Mat4>,
    pub(crate) force: Option<Coord3D>,
    pub(super) ground: Real,
    pub(super) movement_z: Real,
}

impl PhysicsBehaviorUpdate {
    pub(in crate::object) fn complete_owned_bounce_at(
        &mut self,
        owner: &mut GameObject,
        step: &mut OwnedPhysicsStep,
    ) -> Result<(), String> {
        let Some(bounce) = &step.bounce else {
            return Ok(());
        };
        let state = &mut self.physics_handle.state;
        step.active_velocity_z = state.vel.z;
        let mut position = step
            .transform
            .ok_or("bounce outer transform absent")?
            .w_axis
            .truncate();
        position.z = bounce.movement_z;
        Self::clamp_ground(&mut state.vel, &mut position, bounce.ground);
        state.vel_mag = INVALID_VEL_MAG;
        state.set_flag(FLAG_ALLOW_TO_FALL, false);
        if state.has_flag(FLAG_IS_STUNNED) {
            owner.clear_model_condition_state(MODELCONDITION_STUNNED_FLAILING);
            owner.set_model_condition_state(MODELCONDITION_STUNNED);
        }
        let mut transform = step.transform.unwrap();
        if bounce.force.is_some() {
            transform = owner.get_transform_matrix();
        }
        transform.w_axis = position.extend(1.0);
        step.transform = Some(transform);
        Ok(())
    }

    pub(in crate::object) fn bounce_stun_destruction_active(&self) -> bool {
        self.physics_handle.state.has_flag(FLAG_IS_STUNNED)
    }

    pub(in crate::object) fn apply_owned_bounce_force_at(&mut self, force: &Coord3D, mass: Real) {
        if self.physics_handle.state.has_flag(FLAG_ALLOW_BOUNCE) {
            let mass = if mass.abs() < 0.0001 { 0.0001 } else { mass };
            self.physics_handle.state.accel += *force / mass;
        }
    }
}
