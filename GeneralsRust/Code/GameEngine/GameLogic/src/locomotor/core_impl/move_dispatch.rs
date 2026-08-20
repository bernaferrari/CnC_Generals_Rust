impl Locomotor {
    /// C++ `Locomotor::locoUpdate_moveTowardsPosition` (Locomotor.cpp:929-1141).
    ///
    /// Appearance movers apply motive acceleration; this dispatcher owns stunned /
    /// invalid / blocked / airborne-skip / braking-cheat and handleBehaviorZ.
    pub fn loco_update_move_towards_position(
        &mut self,
        current: Coord3D,
        current_angle: Real,
        current_speed: Real,
        target: Coord3D,
        mut on_path_dist_to_goal: Real,
        desired_speed: Real,
        condition: BodyDamageType,
        delta_time: Real,
        blocked: bool,
        mut physics: Option<&mut dyn crate::modules::PhysicsBehavior>,
        mut object: Option<&mut crate::object::Object>,
    ) -> (Coord3D, Real, Real) {
        self.set_flag(FLAG_MAINTAIN_POS_VALID, false);
        self.wheeled_turn_factor = 1.0;


        let max_speed = self.get_max_speed_for_condition(condition);
        let desired_speed = desired_speed.min(max_speed);
        let braking = self.get_braking();
        if braking > 0.0 {
            let dist_to_stop = (max_speed / braking) * (max_speed) / 2.0;
            if on_path_dist_to_goal > PATHFIND_CELL_SIZE_F && on_path_dist_to_goal > dist_to_stop {
                self.set_flag(FLAG_IS_BRAKING, false);
                self.braking_factor = 1.0;
            }
        }

        let (disabled_held, object_layer) = object_loco_z_context(object.as_deref());
        if object_is_stunned(object.as_deref()) {
            return (current, current_angle, current_speed);
        }

        // C++ Locomotor.cpp:968-977 — non-air invalid terrain runs fixInvalidPosition.
        if (self.template.surfaces & SURFACE_AIR) == 0 && !self.allows_invalid_position() {
            if !self.valid_movement_terrain_at(object_layer, current) {
                let is_dozer = object
                    .as_ref()
                    .map(|obj| obj.is_kind_of(crate::common::KindOf::Dozer))
                    .unwrap_or(false);
                let mass = physics.as_ref().map(|p| p.get_mass()).unwrap_or(1.0);
                let vel = physics
                    .as_ref()
                    .map(|p| p.get_velocity())
                    .unwrap_or(Coord3D::new(0.0, 0.0, 0.0));
                if let Some(fix) = self.fix_invalid_position_with(
                    is_dozer,
                    current,
                    vel,
                    mass,
                    |pos| self.valid_movement_terrain_at(object_layer, pos),
                ) {
                    if let Some(phys) = physics.as_mut() {
                        if let Some(extra) = fix.extra_push {
                            phys.apply_motive_force(&extra);
                        }
                        phys.apply_motive_force(&fix.correction);
                    }
                    return (current, current_angle, current_speed);
                }
            }
        }

        let dx = target.x - current.x;
        let dy = target.y - current.y;
        let dz = target.z - current.z;
        let dist_2d = (dx * dx + dy * dy).sqrt();
        if dist_2d > on_path_dist_to_goal {
            let is_projectile = object
                .as_ref()
                .map(|obj| obj.is_kind_of(crate::common::KindOf::Projectile))
                .unwrap_or(false);
            if !is_projectile && dist_2d > 2.0 * on_path_dist_to_goal {
                self.set_flag(FLAG_IS_BRAKING, true);
            }
            on_path_dist_to_goal = dist_2d;
        }

        if let Some(physics) = physics.as_mut() {
            physics.apply_motive_force(&Coord3D::new(0.0, 0.0, 0.0));
        }

        let current_speed = physics
            .as_ref()
            .map(|p| p.get_forward_speed_2d())
            .unwrap_or(current_speed);

        let mut blocked = blocked;
        if blocked {
            if let Some(physics) = physics.as_ref() {
                if desired_speed > velocity_magnitude(&physics.get_velocity()) {
                    blocked = false;
                }
            }
            if (self.template.surfaces & SURFACE_AIR) != 0 {
                blocked = false;
            }
        }

        if blocked {
            if let Some(physics) = physics.as_mut() {
                physics.scrub_velocity_2d(desired_speed);
            }
            if self.template.wander_width_factor == 0.0 {
                let _ = self.rotate_towards_position(current, current_angle, target, condition);
            }
            let vel_z = physics
                .as_ref()
                .map(|p| p.get_velocity().z)
                .unwrap_or(0.0);
            let z = self.handle_behavior_z_for(
                current,
                target,
                condition,
                loco_gravity(),
                vel_z,
                disabled_held,
                object_layer,
            );
            let mut pos = current;
            if let Some(snapped) = z.snapped_z {
                pos.z = snapped;
            }
            if let (Some(physics), lift) = (physics, z.lift) {
                if lift != 0.0 {
                    let mass = physics.get_mass();
                    physics.apply_motive_force(&Coord3D::new(0.0, 0.0, lift * mass));
                }
            }
            return (pos, current_angle, desired_speed.min(current_speed));
        }

        if self.template.appearance == LocomotorAppearance::Wings {
            self.set_flag(FLAG_IS_BRAKING, false);
        }

        let was_braking = object
            .as_ref()
            .map(|obj| obj.test_status(crate::common::ObjectStatusTypes::Braking))
            .unwrap_or(self.is_braking());

        if let Some(physics) = physics.as_mut() {
            physics.set_turning(0);
            self.apply_physics_options(*physics);
        }

        let treat_as_airborne = object
            .as_ref()
            .map(|obj| obj.get_height_above_terrain() > -(3.0 * 3.0) * loco_gravity())
            .unwrap_or(false);
        let allow_2d = self.template.allow_motive_force_while_airborne || !treat_as_airborne;

        let (mut pos, mut angle, mut speed) = if allow_2d {
            self.dispatch_appearance_move(
                current,
                current_angle,
                current_speed,
                target,
                on_path_dist_to_goal,
                desired_speed,
                condition,
                delta_time,
            )
        } else {
            (current, current_angle, current_speed)
        };

        let vel_z = physics
            .as_ref()
            .map(|p| p.get_velocity().z)
            .unwrap_or(self.last_motive_accel.z * delta_time);
        let z = self.handle_behavior_z_for(
            pos,
            target,
            condition,
            loco_gravity(),
            vel_z,
            disabled_held,
            object_layer,
        );
        if let Some(snapped) = z.snapped_z {
            pos.z = snapped;
        }
        if let Some(physics) = physics.as_mut() {
            if z.lift != 0.0 {
                let mass = physics.get_mass();
                physics.apply_motive_force(&Coord3D::new(0.0, 0.0, z.lift * mass));
            }
            let mass = physics.get_mass().max(0.001);
            physics.apply_motive_force(&Coord3D::new(
                self.last_motive_accel.x * mass,
                self.last_motive_accel.y * mass,
                self.last_motive_accel.z * mass,
            ));
        }

        if let Some(object) = object.as_mut() {
            object.set_status(
                crate::common::ObjectStatusMaskType::from_status(
                    crate::common::ObjectStatusTypes::Braking,
                ),
                self.is_braking(),
            );
            if self.template.appearance == LocomotorAppearance::Hover {
                self.apply_hover_over_water_model_condition(object);
            }
        }

        if was_braking {
            let cheat_speed = physics
                .as_ref()
                .map(|p| p.get_forward_speed_2d())
                .unwrap_or(speed);
            let cheat = self.braking_cheat_step(
                current,
                target,
                dx,
                dy,
                dz,
                dist_2d,
                cheat_speed,
                object
                    .as_ref()
                    .map(|obj| obj.is_kind_of(crate::common::KindOf::Projectile))
                    .unwrap_or(false),
            );
            pos = cheat;
        }

        (pos, angle, speed)
    }

    fn dispatch_appearance_move(
        &mut self,
        current: Coord3D,
        current_angle: Real,
        current_speed: Real,
        target: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        condition: BodyDamageType,
        delta_time: Real,
    ) -> (Coord3D, Real, Real) {
        self.last_motive_accel = Coord3D::new(0.0, 0.0, 0.0);
        let current_frame = TheGameLogic::get_frame();
        let (desired_angle, accel, move_backwards) = match self.template.appearance {
            LocomotorAppearance::Treads => {
                let (_pos, ang, acc) = self.move_towards_position_treads_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                (ang, acc, false)
            }
            LocomotorAppearance::FourWheels | LocomotorAppearance::Motorcycle => {
                let (_pos, ang, acc, back) = self.move_towards_position_wheels_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                    self.close_enough_dist,
                    current_frame,
                );
                (ang, acc, back)
            }
            LocomotorAppearance::TwoLegs => {
                let (_pos, ang, acc) = self.move_towards_position_legs_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                (ang, acc, false)
            }
            LocomotorAppearance::Hover => {
                let (_pos, ang, acc) = self.move_towards_position_hover_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                (ang, acc, false)
            }
            LocomotorAppearance::Thrust => {
                let (_pos, ang, acc) = self.move_towards_position_thrust_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                (ang, acc, false)
            }
            LocomotorAppearance::Wings => {
                let (_pos, ang, acc) = self.move_towards_position_wings_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                (ang, acc, false)
            }
            LocomotorAppearance::Climber => {
                let (_pos, ang, acc, back) = self.move_towards_position_climber_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                (ang, acc, back)
            }
            LocomotorAppearance::Other => {
                let (_pos, ang, acc) = self.move_towards_position_other_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                (ang, acc, false)
            }
        };

        if self.last_motive_accel.length_squared() < 1.0e-12 {
            let dir_sign = if move_backwards { -1.0 } else { 1.0 };
            self.last_motive_accel = Coord3D::new(
                desired_angle.cos() * accel * dir_sign,
                desired_angle.sin() * accel * dir_sign,
                0.0,
            );
        }

        self.integrate_motive(
            current,
            current_angle,
            current_speed,
            desired_angle,
            accel,
            condition,
            delta_time,
            move_backwards,
        )
    }

    /// One Euler step of applyMotiveForce (accel → vel → pos). Not heading-rail teleport.
    fn integrate_motive(
        &self,
        current: Coord3D,
        current_angle: Real,
        current_speed: Real,
        desired_angle: Real,
        accel: Real,
        condition: BodyDamageType,
        delta_time: Real,
        move_backwards: bool,
    ) -> (Coord3D, Real, Real) {
        let dt = delta_time.max(0.0);
        let new_angle = self.step_angle(current_angle, desired_angle, condition, dt);
        let max_speed = self.get_max_speed_for_condition(condition);
        let mut vel = Coord3D::new(
            current_angle.cos() * current_speed,
            current_angle.sin() * current_speed,
            0.0,
        );
        vel += self.last_motive_accel * dt;
        let mut new_pos = current + vel * dt;
        let mut new_speed = (vel.x * vel.x + vel.y * vel.y).sqrt();
        if !move_backwards {
            new_speed = new_speed.clamp(0.0, max_speed);
        }
        let _ = accel;
        (new_pos, new_angle, new_speed)
    }

    fn braking_cheat_step(
        &self,
        current: Coord3D,
        target: Coord3D,
        dx: Real,
        dy: Real,
        dz: Real,
        dist_2d: Real,
        speed: Real,
        projectile: bool,
    ) -> Coord3D {
        const MIN_VEL: Real = PATHFIND_CELL_SIZE_F / LOGICFRAMES_PER_SECOND as Real;
        let mut pos = current;
        if projectile {
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            let mut vel = speed.abs().max(MIN_VEL);
            if vel > dist {
                vel = dist;
            }
            if dist > 0.001 {
                let inv = 1.0 / dist;
                pos.x += dx * inv * vel;
                pos.y += dy * inv * vel;
                pos.z += dz * inv * vel;
            }
        } else if dist_2d > 0.001 {
            let mut vel = speed.abs().max(MIN_VEL);
            if vel > dist_2d {
                vel = dist_2d;
            }
            let inv = 1.0 / dist_2d;
            pos.x += dx * inv * vel;
            pos.y += dy * inv * vel;
        }
        let _ = target;
        pos
    }

    /// Live AI path: appearance dispatch + motive Euler. Frame comes from TheGameLogic.
    pub fn move_towards(
        &mut self,
        current: Coord3D,
        current_angle: Real,
        current_speed: Real,
        target: Coord3D,
        desired_speed: Real,
        condition: BodyDamageType,
        delta_time: Real,
    ) -> (Coord3D, Real, Real) {
        // C++ only clamps desiredSpeed to maxSpeed and applies downhillOnly from the template.
        let desired_speed = self.apply_downhill_only(desired_speed, current, target);
        let on_path = (target - current).length();
        self.loco_update_move_towards_position(
            current,
            current_angle,
            current_speed,
            target,
            on_path,
            desired_speed,
            condition,
            delta_time,
            false,
            None,
            None,
        )
    }
}

/// C++ `PhysicsBehavior::getIsStunned` live signal is MODELCONDITION_STUNNED(_FLAILING).
pub fn model_condition_is_stunned(flags: crate::common::ModelConditionFlags) -> bool {
    flags.contains(crate::common::ModelConditionFlags::STUNNED)
        || flags.contains(crate::common::ModelConditionFlags::STUNNED_FLAILING)
}

fn object_is_stunned(object: Option<&crate::object::Object>) -> bool {
    let Some(obj) = object else {
        return false;
    };
    let Some(drawable) = obj.get_drawable() else {
        return false;
    };
    let Ok(drawable) = drawable.read() else {
        return false;
    };
    model_condition_is_stunned(drawable.get_model_conditions())
}

fn object_loco_z_context(
    object: Option<&crate::object::Object>,
) -> (bool, crate::common::PathfindLayerEnum) {
    match object {
        Some(obj) => (
            obj.is_disabled_by_type(crate::common::DisabledType::Held),
            obj.get_layer(),
        ),
        None => (false, crate::common::PathfindLayerEnum::Ground),
    }
}

fn velocity_magnitude(vel: &Coord3D) -> Real {
    (vel.x * vel.x + vel.y * vel.y + vel.z * vel.z).sqrt()
}
