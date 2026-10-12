//! Explicit-owner PhysicsUpdate.cpp:603–915 translation and angular integration.
//! Unsupported landing/collision callbacks are rejected before canonical mutation.
use super::*;

pub(crate) struct OwnedPhysicsStep {
    pub(crate) transform: Option<Mat4>,
    pub(crate) height_above_terrain: Option<Real>,
    pub(crate) enable_freefall: bool,
}

impl PhysicsBehaviorUpdate {
    #[cfg(test)]
    pub(in crate::object) fn set_immune_freefall_for_test(&mut self) {
        self.physics_handle
            .state
            .set_flag(FLAG_IMMUNE_TO_FALLING_DAMAGE, true);
        self.physics_handle
            .state
            .set_flag(FLAG_IS_IN_FREEFALL, true);
    }

    pub(in crate::object) fn integrate_owned_at(
        &mut self,
        obj: &mut GameObject,
        frame: u32,
        gravity: Real,
        start_ground: Real,
        mut ground_height: impl FnMut(Real, Real) -> Real,
    ) -> Result<OwnedPhysicsStep, String> {
        let state = &self.physics_handle.state;
        let airborne_at_start =
            GameObject::is_above_terrain_height(obj.get_position().z, start_ground);
        let immune_landing = state.has_flag(FLAG_IMMUNE_TO_FALLING_DAMAGE);
        if self.object_id != obj.get_id()
            || obj.get_contain().is_some()
            || obj.get_contained_by().is_some()
            || obj.is_kind_of(KindOf::Projectile)
            || obj.get_layer() != crate::common::PathfindLayerEnum::Ground
            || obj.test_status(ObjectStatusTypes::DeckHeightOffset)
            || self.module_data.kill_when_resting_on_ground
            || state.has_flag(FLAG_ALLOW_BOUNCE)
            || state.has_flag(FLAG_IS_IN_UPDATE)
            || state.current_overlap != crate::common::INVALID_ID
            || state.previous_overlap != crate::common::INVALID_ID
            || (!immune_landing
                && (airborne_at_start
                    || state.has_flag(FLAG_IS_IN_FREEFALL)
                    || state.has_flag(FLAG_WAS_AIRBORNE_LAST_FRAME)))
        {
            return Err(
                "owned Physics requires explicit cargo, bounce and nonimmune landing continuations"
                    .into(),
            );
        }
        if state.has_flag(FLAG_IS_IN_FREEFALL)
            || obj.is_disabled_by_type(DisabledType::DisabledFreefall)
        {
            obj.validate_owned_leaf_held()?;
            if obj.is_kind_of(KindOf::Dozer) || obj.is_kind_of(KindOf::SpawnsAreTheWeapons) {
                return Err(
                    "owned freefall requires supplied dozer/spawn disabled continuations".into(),
                );
            }
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
            || !self.module_data.aerodynamic_friction.is_finite()
            || !self.module_data.pitch_roll_yaw_factor.is_finite()
            || !self.module_data.center_of_mass_offset.is_finite()
            || !state.pitch_rate.is_finite()
            || !state.roll_rate.is_finite()
            || !state.yaw_rate.is_finite()
        {
            return Err("nonfinite ordinary Physics input".into());
        }
        let held = obj.is_disabled_by_type(DisabledType::Held);
        let mut matrix = obj.get_transform_matrix();
        let mut pos = *obj.get_position();
        let mut velocity = state.vel;
        let mut acceleration = state.accel;
        let mut ground = start_ground;
        let mut stun_relief = false;
        let significantly_above =
            crate::object::die::eject_pilot_die::is_significantly_above_terrain_height(
                obj.get_position().z - start_ground,
                gravity,
            );
        let use_ground_friction =
            state.has_flag(FLAG_APPLY_FRICTION2D_WHEN_AIRBORNE) || !significantly_above;
        let damping = if use_ground_friction {
            1.0 - DEFAULT_LATERAL_FRICTION
        } else {
            1.0 - Self::get_aerodynamic_friction(&self.module_data, state)
        };
        if !held {
            acceleration.z += gravity;
            if use_ground_friction {
                acceleration +=
                    Self::ground_friction_acceleration(&self.module_data, obj, state, frame, 0.0);
            } else {
                acceleration +=
                    velocity * -Self::get_aerodynamic_friction(&self.module_data, state);
            }
            (velocity, pos) = Self::integrate_translation(
                acceleration,
                velocity,
                pos,
                obj.test_status(ObjectStatusTypes::Braking),
                false,
            );
            stun_relief = (velocity.x.abs() < STUN_RELIEF_EPSILON
                && velocity.y.abs() < STUN_RELIEF_EPSILON
                && velocity.z.abs() < STUN_RELIEF_EPSILON)
                || !significantly_above;
            ground = ground_height(pos.x, pos.y);
            if !pos.is_finite() || !ground.is_finite() {
                return Err("ordinary Physics movement requires finite destination terrain".into());
            }
            if pos.z <= ground {
                Self::clamp_ground(&mut velocity, &mut pos, ground);
            } else if !state.has_flag(FLAG_IS_IN_FREEFALL)
                && state.has_flag(FLAG_STICK_TO_GROUND)
                && !state.has_flag(FLAG_ALLOW_TO_FALL)
            {
                pos.z = ground;
            } else if !immune_landing {
                return Err(
                    "owned Physics flight requires a supplied nonimmune landing continuation"
                        .into(),
                );
            }
            // CPP PhysicsUpdate.cpp:689–742 rotates the actual basis, never
            // reconstructs Euler angles. Friction damps rates before rotation.
            if state.has_flag(FLAG_HAS_PITCHROLLYAW) {
                matrix = Self::integrate_basis(
                    matrix,
                    state.roll_rate * damping,
                    state.pitch_rate * damping,
                    state.yaw_rate * damping,
                    self.module_data.pitch_roll_yaw_factor,
                    self.module_data.center_of_mass_offset,
                );
            }
            matrix.w_axis = pos.extend(1.0);
        }
        // No canonical field changes until all terrain/phase validation passes.
        let state = &mut self.physics_handle.state;
        state.set_flag(FLAG_IS_IN_UPDATE, true);
        if !state.has_flag(FLAG_UPDATE_EVER_RUN) {
            state.set_flag(FLAG_WAS_AIRBORNE_LAST_FRAME, airborne_at_start);
        }
        state.prev_accel = state.accel;
        if !held {
            apply_ypr_damping(state, damping);
            state.accel = acceleration;
            state.vel = velocity;
            state.vel_mag = INVALID_VEL_MAG;
            if !state.has_flag(FLAG_HAS_PITCHROLLYAW) {
                state.yaw_angle = obj.get_orientation();
            }
            if pos.z <= ground {
                state.set_flag(FLAG_ALLOW_TO_FALL, false);
            }
            if state.has_flag(FLAG_IS_STUNNED) && stun_relief {
                state.set_flag(FLAG_IS_STUNNED, false);
                obj.clear_model_condition_state(MODELCONDITION_STUNNED);
            }
            if pos.z <= ground && state.has_flag(FLAG_IS_STUNNED) {
                obj.clear_model_condition_state(MODELCONDITION_STUNNED_FLAILING);
                obj.set_model_condition_state(MODELCONDITION_STUNNED);
            }
        }
        Ok(OwnedPhysicsStep {
            transform: (!held).then_some(matrix),
            height_above_terrain: (!held).then_some(pos.z - ground),
            enable_freefall: !held && pos.z > ground && state.has_flag(FLAG_IS_IN_FREEFALL),
        })
    }
    /// Matrix3D Rotate_X/Y/Z post-multiply the original basis in this order.
    fn integrate_basis(
        matrix: Mat4,
        roll_rate: Real,
        pitch_rate: Real,
        yaw_rate: Real,
        factor: Real,
        offset: Real,
    ) -> Mat4 {
        let mut pitch = pitch_rate * factor;
        if offset != 0.0 {
            let x = matrix.x_axis;
            let angle = x.z.atan2((x.x * x.x + x.y * x.y).sqrt());
            let remaining = if offset > 0.0 {
                crate::common::PI / 2.0 - angle
            } else {
                -crate::common::PI / 2.0 + angle
            };
            pitch *= remaining.sin();
        }
        matrix
            * Mat4::from_rotation_x(roll_rate * factor)
            * Mat4::from_rotation_y(pitch)
            * Mat4::from_rotation_z(yaw_rate * factor)
    }

    pub(in crate::object) fn prepare_freefall_cleanup_at(&mut self) {
        self.physics_handle
            .state
            .set_flag(FLAG_IS_IN_FREEFALL, false);
    }

    pub(in crate::object) fn finish_owned_at(
        &mut self,
        obj: &mut GameObject,
        frame: u32,
        airborne_at_end: bool,
    ) -> UpdateSleepTime {
        let state = &mut self.physics_handle.state;
        state.accel = Coord3D::ZERO;
        state.previous_overlap = state.current_overlap;
        state.current_overlap = crate::common::INVALID_ID;
        state.set_flag(FLAG_UPDATE_EVER_RUN, true);
        state.set_flag(FLAG_WAS_AIRBORNE_LAST_FRAME, airborne_at_end);
        state.set_flag(FLAG_IS_IN_UPDATE, false);
        if !airborne_at_end {
            state.set_flag(FLAG_IS_IN_FREEFALL, false);
        }
        if !airborne_at_end
            && obj.get_layer() == crate::common::PathfindLayerEnum::Ground
            && Self::is_zero3d(state.vel)
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

    fn immune_flight(z: Real, velocity: Coord3D) -> (GameObject, PhysicsBehaviorUpdate) {
        let mut owner = GameObject::new_test(93524, 100.0);
        // Unpublished fixture initial pose; runtime poses use supplied services.
        owner.geometry_info.position = Coord3D::new(0.0, 0.0, z);
        owner.transform_matrix.w_axis = owner.geometry_info.position.extend(1.0);
        owner.set_height_above_terrain(z);
        let data = PhysicsBehaviorModuleData {
            aerodynamic_friction: 0.2,
            ..Default::default()
        };
        let mut physics =
            PhysicsBehaviorUpdate::new_for_owner_id(owner.get_id(), Arc::new(data)).unwrap();
        let state = &mut physics.physics_handle.state;
        state.mass = 1.0;
        state.vel = velocity;
        state.set_flag(FLAG_IMMUNE_TO_FALLING_DAMAGE, true);
        state.set_flag(FLAG_IS_IN_FREEFALL, true);
        (owner, physics)
    }

    #[test]
    fn cpp_immune_freefall_uses_aerodynamics_and_remains_awake() {
        let (mut owner, mut physics) = immune_flight(30.0, Coord3D::new(2.0, 0.0, -2.0));
        let step = physics
            .integrate_owned_at(&mut owner, 5, -1.0, 0.0, |_, _| 0.0)
            .unwrap();
        assert!((step.transform.unwrap().w_axis.z - 27.4).abs() < 0.00001);
        assert!(
            physics
                .physics_handle
                .state
                .vel
                .abs_diff_eq(Coord3D::new(1.6, 0.0, -2.6), 0.00001)
        );
        assert!(
            physics
                .physics_handle
                .state
                .has_flag(FLAG_WAS_AIRBORNE_LAST_FRAME)
        );
        assert!(step.enable_freefall);
        assert_eq!(
            physics.finish_owned_at(&mut owner, 5, true),
            UPDATE_SLEEP_NONE
        );
        assert!(physics.physics_handle.state.has_flag(FLAG_IS_IN_FREEFALL));
        assert!(!physics.physics_handle.state.has_flag(FLAG_IS_IN_UPDATE));
    }

    #[test]
    fn cpp_immune_landing_clears_freefall_but_tests_stun_before_ground_clamp() {
        let (mut owner, mut physics) = immune_flight(30.0, Coord3D::new(0.0, 0.0, -50.0));
        physics.physics_handle.state.set_flag(FLAG_IS_STUNNED, true);
        physics
            .physics_handle
            .state
            .set_flag(FLAG_ALLOW_TO_FALL, true);
        owner.set_disabled(DisabledType::DisabledFreefall);
        let step = physics
            .integrate_owned_at(&mut owner, 5, -1.0, 0.0, |_, _| 0.0)
            .unwrap();
        assert_eq!(step.transform.unwrap().w_axis.z, 0.0);
        assert!(physics.physics_handle.state.has_flag(FLAG_IS_STUNNED));
        assert!(!physics.physics_handle.state.has_flag(FLAG_ALLOW_TO_FALL));
        physics.prepare_freefall_cleanup_at();
        // Adapter executes the validated default disabled edge outside its module loan.
        owner.disabled_mask.clear(DisabledType::DisabledFreefall);
        physics.finish_owned_at(&mut owner, 5, false);
        assert!(!owner.is_disabled_by_type(DisabledType::DisabledFreefall));
        assert!(!physics.physics_handle.state.has_flag(FLAG_IS_IN_FREEFALL));
        assert!(
            !physics
                .physics_handle
                .state
                .has_flag(FLAG_WAS_AIRBORNE_LAST_FRAME)
        );
    }

    #[test]
    fn cpp_held_airborne_owner_skips_pose_and_never_sleeps_forever() {
        let (mut owner, mut physics) = immune_flight(30.0, Coord3D::ZERO);
        owner.set_disabled(DisabledType::Held);
        let step = physics
            .integrate_owned_at(&mut owner, 5, -1.0, 0.0, |_, _| {
                panic!("held must not sample destination")
            })
            .unwrap();
        assert!(step.transform.is_none());
        assert!(step.height_above_terrain.is_none());
        assert_eq!(
            physics.finish_owned_at(&mut owner, 5, true),
            UPDATE_SLEEP_NONE
        );
        assert!(physics.physics_handle.state.has_flag(FLAG_IS_IN_FREEFALL));
    }

    #[test]
    fn supplied_angular_owner_is_isolated_and_resumes_saved_rates() {
        use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
        use std::io::Cursor;
        let owner = Arc::new(RwLock::new(GameObject::new_test(93523, 100.0)));
        let peer = Arc::new(RwLock::new(GameObject::new_test(93523, 100.0)));
        let data: Arc<dyn ModuleData> = Arc::new(PhysicsBehaviorModuleData::default());
        let mut physics = PhysicsBehaviorUpdate::new(owner.clone(), data.clone()).unwrap();
        let mut other = PhysicsBehaviorUpdate::new(peer.clone(), data).unwrap();
        physics.physics_handle.state.mass = 1.0;
        other.physics_handle.state.mass = 1.0;
        physics.physics_handle.state.roll_rate = 0.2;
        physics.physics_handle.state.pitch_rate = 0.3;
        physics.physics_handle.state.yaw_rate = 0.4;
        physics
            .physics_handle
            .state
            .set_flag(FLAG_HAS_PITCHROLLYAW, true);
        let mut saved = Vec::new();
        physics
            .xfer(&mut XferSave::new(Cursor::new(&mut saved), 1))
            .unwrap();
        other
            .xfer(&mut XferLoad::new(Cursor::new(&saved), 1))
            .unwrap();
        let mut peer_before = Vec::new();
        other
            .xfer(&mut XferSave::new(Cursor::new(&mut peer_before), 1))
            .unwrap();
        let step = physics
            .integrate_owned_at(&mut owner.write().unwrap(), 5, -1.0, 0.0, |_, _| 0.0)
            .unwrap();
        let mut peer_after = Vec::new();
        other
            .xfer(&mut XferSave::new(Cursor::new(&mut peer_after), 1))
            .unwrap();
        assert_eq!(peer_before, peer_after);
        let restored = other
            .integrate_owned_at(&mut peer.write().unwrap(), 5, -1.0, 0.0, |_, _| 0.0)
            .unwrap();
        assert!(
            step.transform
                .unwrap()
                .abs_diff_eq(restored.transform.unwrap(), 0.000001)
        );
        assert_eq!(
            physics.finish_owned_at(&mut owner.write().unwrap(), 5, false),
            UPDATE_SLEEP_NONE
        );
        assert_eq!(
            other.finish_owned_at(&mut peer.write().unwrap(), 5, false),
            UPDATE_SLEEP_NONE
        );
    }

    #[test]
    fn cpp_angular_step_preserves_translation_and_rotates_existing_basis() {
        let original =
            Mat4::from_translation(Coord3D::new(3.0, 4.0, 5.0)) * Mat4::from_rotation_y(0.7);
        let result = PhysicsBehaviorUpdate::integrate_basis(original, 0.2, 0.3, 0.4, 2.0, 0.0);
        let expected = original
            * Mat4::from_rotation_x(0.4)
            * Mat4::from_rotation_y(0.6)
            * Mat4::from_rotation_z(0.8);
        assert!(result.abs_diff_eq(expected, 0.000001));
        assert_eq!(result.w_axis, original.w_axis);
    }

    #[test]
    fn cpp_center_of_mass_uses_current_basis_pitch_and_offset_sign() {
        let original = Mat4::from_rotation_y(-0.4);
        for offset in [-1.0, 1.0] {
            let remaining = if offset > 0.0 {
                crate::common::PI / 2.0 - 0.4
            } else {
                -crate::common::PI / 2.0 + 0.4
            };
            let expected = original * Mat4::from_rotation_y(0.3 * remaining.sin());
            let result =
                PhysicsBehaviorUpdate::integrate_basis(original, 0.0, 0.3, 0.0, 1.0, offset);
            assert!(result.abs_diff_eq(expected, 0.000001));
        }
    }

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
