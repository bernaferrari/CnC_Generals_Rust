//! Explicit-owner ordinary grounded PhysicsUpdate.cpp:603–915 integration.
//! Unsupported landing/collision callbacks are rejected before canonical mutation.
use super::*;

pub(crate) struct OwnedPhysicsStep {
    pub(crate) transform: Option<Mat4>,
    pub(crate) height_above_terrain: Option<Real>,
}

impl PhysicsBehaviorUpdate {
    pub(in crate::object) fn integrate_ground_at(
        &mut self,
        obj: &mut GameObject,
        frame: u32,
        gravity: Real,
        start_ground: Real,
        mut ground_height: impl FnMut(Real, Real) -> Real,
    ) -> Result<OwnedPhysicsStep, String> {
        let state = &self.physics_handle.state;
        if self.object_id != obj.get_id()
            || obj.get_contain().is_some()
            || obj.get_contained_by().is_some()
            || obj.is_kind_of(KindOf::Projectile)
            || obj.get_layer() != crate::common::PathfindLayerEnum::Ground
            || obj.test_status(ObjectStatusTypes::DeckHeightOffset)
            || self.module_data.kill_when_resting_on_ground
            || state.has_flag(FLAG_ALLOW_BOUNCE)
            || state.has_flag(FLAG_IS_IN_FREEFALL)
            || state.has_flag(FLAG_HAS_PITCHROLLYAW)
            || state.has_flag(FLAG_IS_IN_UPDATE)
            || state.has_flag(FLAG_WAS_AIRBORNE_LAST_FRAME)
            || state.current_overlap != crate::common::INVALID_ID
            || state.previous_overlap != crate::common::INVALID_ID
            || obj.get_position().z > start_ground
        {
            return Err("ordinary grounded Physics cannot admit cargo, flight, bounce, angular motion or collision continuation".into());
        }
        if !gravity.is_finite()
            || !start_ground.is_finite()
            || !obj.get_position().is_finite()
            || !state.vel.is_finite()
            || !state.accel.is_finite()
            || !state.mass.is_finite()
            || state.mass <= 0.0
            || !state.extra_friction.is_finite()
            || !self.module_data.forward_friction.is_finite()
            || !self.module_data.lateral_friction.is_finite()
        {
            return Err("nonfinite ordinary Physics input".into());
        }
        let held = obj.is_disabled_by_type(DisabledType::Held);
        let mut matrix = obj.get_transform_matrix();
        let mut pos = *obj.get_position();
        let mut velocity = state.vel;
        let mut acceleration = state.accel;
        let mut ground = start_ground;
        if !held {
            acceleration.z += gravity;
            acceleration +=
                Self::ground_friction_acceleration(&self.module_data, obj, state, frame, 0.0);
            (velocity, pos) = Self::integrate_translation(
                acceleration,
                velocity,
                pos,
                obj.test_status(ObjectStatusTypes::Braking),
                false,
            );
            ground = ground_height(pos.x, pos.y);
            if !pos.is_finite() || !ground.is_finite() {
                return Err("ordinary Physics movement requires finite destination terrain".into());
            }
            if pos.z <= ground {
                Self::clamp_ground(&mut velocity, &mut pos, ground);
            } else if state.has_flag(FLAG_STICK_TO_GROUND) && !state.has_flag(FLAG_ALLOW_TO_FALL) {
                pos.z = ground;
            } else {
                return Err(
                    "ordinary Physics movement would enter an unsupported airborne phase".into(),
                );
            }
            // C++ preserves the existing basis when angular rates are absent.
            matrix.w_axis = pos.extend(1.0);
        }
        // No canonical field changes until all terrain/phase validation passes.
        let state = &mut self.physics_handle.state;
        state.set_flag(FLAG_IS_IN_UPDATE, true);
        state.prev_accel = state.accel;
        if !held {
            apply_ypr_damping(state, 1.0 - DEFAULT_LATERAL_FRICTION);
            state.accel = acceleration;
            state.vel = velocity;
            state.vel_mag = INVALID_VEL_MAG;
            state.yaw_angle = obj.get_orientation();
            if pos.z <= ground {
                state.set_flag(FLAG_ALLOW_TO_FALL, false);
            }
            if state.has_flag(FLAG_IS_STUNNED) {
                state.set_flag(FLAG_IS_STUNNED, false);
                obj.clear_model_condition_state(MODELCONDITION_STUNNED);
            }
        }
        Ok(OwnedPhysicsStep {
            transform: (!held).then_some(matrix),
            height_above_terrain: (!held).then_some(pos.z - ground),
        })
    }
    pub(in crate::object) fn finish_ground_at(
        &mut self,
        obj: &mut GameObject,
        frame: u32,
    ) -> UpdateSleepTime {
        let state = &mut self.physics_handle.state;
        state.accel = Coord3D::ZERO;
        state.previous_overlap = state.current_overlap;
        state.current_overlap = crate::common::INVALID_ID;
        state.set_flag(FLAG_UPDATE_EVER_RUN, true);
        state.set_flag(FLAG_WAS_AIRBORNE_LAST_FRAME, false);
        state.set_flag(FLAG_IS_IN_UPDATE, false);
        obj.clear_disabled(DisabledType::DisabledFreefall);
        obj.clear_model_condition_state(MODELCONDITION_FREEFALL);
        if Self::is_zero3d(state.vel)
            && Self::is_zero3d(state.accel)
            && !state.has_flag(FLAG_HAS_PITCHROLLYAW)
            && state.motive_force_expires <= frame
            && state.current_overlap == crate::common::INVALID_ID
            && state.previous_overlap == crate::common::INVALID_ID
        {
            UPDATE_SLEEP_FOREVER
        } else {
            UPDATE_SLEEP_NONE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpp_euler_uses_updated_velocity_and_clamps_tiny_components() {
        let (velocity, position) = PhysicsBehaviorUpdate::integrate_translation(
            Coord3D::new(2.0, -0.9995, -1.0),
            Coord3D::new(1.0, 1.0, 1.0),
            Coord3D::new(10.0, 20.0, 30.0),
            false,
            false,
        );
        assert_eq!(velocity, Coord3D::new(3.0, 0.0, 0.0));
        assert_eq!(position, Coord3D::new(13.0, 20.0, 30.0));
    }

    #[test]
    fn cpp_braking_integrates_only_z_except_projectiles() {
        for (projectile, expected_z) in [(false, 33.0), (true, 30.0)] {
            let (velocity, position) = PhysicsBehaviorUpdate::integrate_translation(
                Coord3D::ZERO,
                Coord3D::new(1.0, 2.0, 3.0),
                Coord3D::new(10.0, 20.0, 30.0),
                true,
                projectile,
            );
            assert_eq!(velocity, Coord3D::new(1.0, 2.0, 3.0));
            assert_eq!(position, Coord3D::new(10.0, 20.0, expected_z));
        }
    }

    #[test]
    fn cpp_ground_slam_removes_excess_but_preserves_down_slope_velocity() {
        let mut velocity = Coord3D::new(1.0, 2.0, -3.0);
        let mut position = Coord3D::new(10.0, 20.0, -1.0);
        PhysicsBehaviorUpdate::clamp_ground(&mut velocity, &mut position, 0.0);
        assert_eq!(velocity, Coord3D::new(1.0, 2.0, -2.0));
        assert_eq!(position, Coord3D::new(10.0, 20.0, 0.0));
        PhysicsBehaviorUpdate::clamp_ground(&mut velocity, &mut position, 4.0);
        assert_eq!(velocity.z, 0.0);
        assert_eq!(position.z, 4.0);
    }
}
