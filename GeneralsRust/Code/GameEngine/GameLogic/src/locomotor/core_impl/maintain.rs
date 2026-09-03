/// Result of C++ `Locomotor::fixInvalidPosition` (Locomotor.cpp:1528-1560).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidPositionFix {
    pub correction: Coord3D,
    pub extra_push: Option<Coord3D>,
}

/// Result of C++ `doLocomotor` NONE-goal settle + maintain (AIUpdate.cpp:2234-2265).
#[derive(Debug, Clone, Copy)]
pub struct GoalNoneUpdate {
    pub pos: Coord3D,
    pub angle: Real,
    pub speed: Real,
    pub requires_constant: bool,
    pub do_final_position: bool,
}

impl Locomotor {
    // ========================================================================
    // MISSING METHODS — Ported from C++ Locomotor.cpp
    // ========================================================================

    /// Set physics options on leftover PhysicsState (signature kept for callers).
    /// Live path uses [`Self::apply_physics_options`].
    pub fn set_physics_options(&self, physics: &mut PhysicsState) {
        let extra = self.ultra_accurate_extra_friction();
        physics.friction = extra;
        physics.allow_motive_force_while_airborne = self.template.allow_motive_force_while_airborne;
        let _ = physics;
    }

    /// Matches C++ Locomotor::setPhysicsOptions (Locomotor.cpp:911-926).
    pub fn apply_physics_options(&self, physics: &mut dyn crate::modules::PhysicsBehavior) {
        physics.set_extra_friction(self.ultra_accurate_extra_friction());
        physics.set_allow_airborne_friction(self.template.apply_2d_friction_when_airborne);
        physics.set_stick_to_ground(self.template.stick_to_ground);
    }

    fn ultra_accurate_extra_friction(&self) -> Real {
        const EXTRA_FRIC: Real = 0.5;
        if self.is_ultra_accurate() {
            self.template.extra_2d_friction + EXTRA_FRIC
        } else {
            self.template.extra_2d_friction
        }
    }

    /// Rotate unit towards a desired angle without translating.
    /// Matches C++ Locomotor::locoUpdate_moveTowardsAngle (Locomotor.cpp:847-898)
    ///
    /// Returns `(desired_angle, accel_z)` where `desired_angle` is the new heading
    /// and `accel_z` is the Z-axis force to apply (for handleBehaviorZ parity).
    pub fn loco_update_move_towards_angle(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_angle: Real,
        current_speed: Real,
        condition: BodyDamageType,
        delta_time: Real,
    ) -> (Coord3D, Real, Real) {
        self.set_flag(FLAG_MAINTAIN_POS_VALID, false);
        // C++ rotateTowardsPosition uses full maxTurnRate when minSpeed==0 (Locomotor.cpp:887-895).
        self.wheeled_turn_factor = 1.0;


        let min_speed = self.template.min_speed;
        if min_speed > 0.0 {
            // Can't stay still — move at min_speed toward goal_angle
            let dist = min_speed * 2.0;
            let desired_pos = Coord3D::new(
                current_pos.x + goal_angle.cos() * dist,
                current_pos.y + goal_angle.sin() * dist,
                current_pos.z,
            );
            // Delegate to movement toward the projected point
            let _on_path_dist = 99999.0;
            return self.move_towards(
                current_pos,
                current_angle,
                current_speed,
                desired_pos,
                min_speed,
                condition,
                delta_time,
            );
        }

        // Just rotate towards angle
        let desired_angle = goal_angle;
        let new_angle = self.step_angle(current_angle, desired_angle, condition, delta_time);
        let z_accel = self.compute_z_force(current_pos, current_pos, condition);
        (current_pos, new_angle, z_accel)
    }

    /// Maintain current position — dispatches to appearance-specific methods.
    /// Returns `true` if constant per-frame calling is required (hovering, circling).
    /// Matches C++ Locomotor::locoUpdate_maintainCurrentPosition (Locomotor.cpp:2412-2477)
    pub fn loco_update_maintain_current_position(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        current_speed: Real,
        condition: BodyDamageType,
        delta_time: Real,
    ) -> (Coord3D, Real, Real, bool) {
        // Reset donut timer and braking
        self.donut_timer = TheGameLogic::get_frame()
            + (DONUT_TIME_DELAY_SECONDS * LOGICFRAMES_PER_SECOND as Real) as u32;
        self.set_flag(FLAG_IS_BRAKING, false);

        if !self.get_flag(FLAG_MAINTAIN_POS_VALID) {
            self.maintain_pos = current_pos;
            self.set_flag(FLAG_MAINTAIN_POS_VALID, true);
        }

        let _requires_constant = match self.template.appearance {
            LocomotorAppearance::Thrust => {
                let (pos, angle, speed) = self.maintain_current_position_thrust(
                    current_pos,
                    current_angle,
                    current_speed,
                    condition,
                    delta_time,
                );
                return (pos, angle, speed, true);
            }
            LocomotorAppearance::TwoLegs | LocomotorAppearance::Climber => {
                let (pos, angle, speed) = self.maintain_current_position_other(
                    current_pos,
                    current_angle,
                    current_speed,
                    condition,
                    delta_time,
                );
                return (pos, angle, speed, false);
            }
            LocomotorAppearance::FourWheels | LocomotorAppearance::Motorcycle => {
                let (pos, angle, speed) = self.maintain_current_position_other(
                    current_pos,
                    current_angle,
                    current_speed,
                    condition,
                    delta_time,
                );
                return (pos, angle, speed, false);
            }
            LocomotorAppearance::Treads => {
                let (pos, angle, speed) = self.maintain_current_position_other(
                    current_pos,
                    current_angle,
                    current_speed,
                    condition,
                    delta_time,
                );
                return (pos, angle, speed, false);
            }
            LocomotorAppearance::Hover => {
                let (pos, angle, speed) = self.maintain_current_position_hover(
                    current_pos,
                    current_angle,
                    current_speed,
                    condition,
                    delta_time,
                );
                return (pos, angle, speed, true);
            }
            LocomotorAppearance::Wings => {
                let (pos, angle, speed) = self.maintain_current_position_wings(
                    current_pos,
                    current_angle,
                    current_speed,
                    condition,
                    delta_time,
                );
                return (pos, angle, speed, true);
            }
            _ => {
                let (pos, angle, speed) = self.maintain_current_position_other(
                    current_pos,
                    current_angle,
                    current_speed,
                    condition,
                    delta_time,
                );
                return (pos, angle, speed, true);
            }
        };
    }

    /// Helicopter hover maintenance — keeps thrust locomotor at position using min_speed.
    /// Matches C++ Locomotor::maintainCurrentPositionThrust (Locomotor.cpp:2480-2485)
    fn maintain_current_position_thrust(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        current_speed: Real,
        condition: BodyDamageType,
        _delta_time: Real,
    ) -> (Coord3D, Real, Real) {
        // C++ delegates to moveTowardsPositionThrust at minSpeed
        self.move_towards_position_thrust_physics(
            current_pos,
            current_angle,
            current_pos, // goal = current position
            0.0,
            self.template.min_speed,
            current_speed,
            condition,
        )
    }

    /// Fixed-wing circling maintenance — orbits the maintain position.
    /// Matches C++ Locomotor::maintainCurrentPositionWings (Locomotor.cpp:2488-2524)
    fn maintain_current_position_wings(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        current_speed: Real,
        condition: BodyDamageType,
        _delta_time: Real,
    ) -> (Coord3D, Real, Real) {
        if current_speed == 0.0 {
            return (current_pos, current_angle, 0.0);
        }

        // C++ computes a circle target offset from maintain pos
        let mut turn_radius = self.template.circling_radius;
        if turn_radius == 0.0 {
            turn_radius = self.calc_min_turn_radius(condition);
        }

        // C++ Locomotor.cpp:2502-2521 — orbit relative to m_maintainPos, not current pos.
        let dx = self.maintain_pos.x - current_pos.x;
        let dy = self.maintain_pos.y - current_pos.y;
        let angle_toward = if dx.abs() < 0.001 && dy.abs() < 0.001 {
            current_angle
        } else {
            dy.atan2(dx)
        };

        let mut aim_dir = std::f32::consts::PI - std::f32::consts::FRAC_PI_8;
        if turn_radius < 0.0 {
            turn_radius = -turn_radius;
            aim_dir = -aim_dir;
        }
        let circle_angle = angle_toward + aim_dir;

        let circle_target = Coord3D::new(
            self.maintain_pos.x + circle_angle.cos() * turn_radius,
            self.maintain_pos.y + circle_angle.sin() * turn_radius,
            self.maintain_pos.z,
        );

        self.move_towards_position_other_physics(
            current_pos,
            current_angle,
            circle_target,
            0.0,
            self.template.min_speed,
            current_speed,
            condition,
        )
    }

    /// Hover vehicle position hold — applies braking to zero velocity.
    /// Matches C++ Locomotor::maintainCurrentPositionHover (Locomotor.cpp:2527-2576)
    fn maintain_current_position_hover(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        current_speed: Real,
        condition: BodyDamageType,
        _delta_time: Real,
    ) -> (Coord3D, Real, Real) {
        if current_speed == 0.0 {
            return (current_pos, current_angle, 0.0);
        }

        let max_acceleration = self.get_max_acceleration(condition);
        let braking = self.get_braking();
        let min_speed = self.template.min_speed.max(1.0e-10);
        let speed_delta = min_speed - current_speed;

        let acceleration = if speed_delta.abs() > min_speed {
            if speed_delta > 0.0 {
                max_acceleration
            } else {
                -braking
            }
        } else {
            0.0
        };

        (current_pos, current_angle, acceleration)
    }

    /// Generic position hold — scrub velocity to zero.
    /// Matches C++ Locomotor::maintainCurrentPositionOther (Locomotor.cpp:2579-2588)
    fn maintain_current_position_other(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        _current_speed: Real,
        _condition: BodyDamageType,
        _delta_time: Real,
    ) -> (Coord3D, Real, Real) {
        if _current_speed == 0.0 {
            return (current_pos, current_angle, 0.0);
        }
        let braking = self.get_braking();
        (current_pos, current_angle, -braking)
    }

    /// Infantry wander variant of moveTowardsPosition.
    /// Matches C++ Locomotor::moveTowardsPositionLegsWander (Locomotor.cpp lines ~1594-1687
    /// with wander logic inlined).
    ///
    /// This is the same as `move_towards_position_legs_physics` but with explicit
    /// position/speed parameters for the wander use case.
    pub fn move_towards_position_legs_wander(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        // Same as regular legs physics — wander is already integrated into
        // move_towards_position_legs_physics via the wander_width_factor check.
        self.move_towards_position_legs_physics(
            current_pos,
            current_angle,
            goal_pos,
            on_path_dist_to_goal,
            desired_speed,
            current_speed,
            condition,
        )
    }

    /// Fix units stuck in invalid terrain by applying a correction force.
    /// Returns the motive correction (and optional extra push) if applied.
    /// Matches C++ Locomotor::fixInvalidPosition (Locomotor.cpp:1500-1564).
    pub fn fix_invalid_position(
        &self,
        current_pos: Coord3D,
        velocity: Coord3D,
        mass: Real,
        is_dozer: bool,
        layer: crate::common::PathfindLayerEnum,
    ) -> Option<InvalidPositionFix> {
        self.fix_invalid_position_with(is_dozer, current_pos, velocity, mass, |pos| {
            self.valid_movement_terrain_at(layer, pos)
        })
    }

    /// C++ `fixInvalidPosition` core (Locomotor.cpp:1502-1562) with injected cell validity.
    pub fn fix_invalid_position_with(
        &self,
        is_dozer: bool,
        current_pos: Coord3D,
        velocity: Coord3D,
        mass: Real,
        is_valid: impl Fn(Coord3D) -> bool,
    ) -> Option<InvalidPositionFix> {
        // C++ Locomotor.cpp:1502-1504 — KINDOF_DOZER is never shoved.
        if is_dozer {
            return None;
        }

        let mut dx_acc: Real = 0.0;
        let mut dy_acc: Real = 0.0;
        for j in -1i32..=1 {
            for i in -1i32..=1 {
                let check = Coord3D::new(
                    current_pos.x + (i as Real) * PATHFIND_CELL_SIZE_F,
                    current_pos.y + (j as Real) * PATHFIND_CELL_SIZE_F,
                    current_pos.z,
                );
                if !is_valid(check) {
                    if i < 0 {
                        dx_acc += 1.0;
                    }
                    if i > 0 {
                        dx_acc -= 1.0;
                    }
                    if j < 0 {
                        dy_acc += 1.0;
                    }
                    if j > 0 {
                        dy_acc -= 1.0;
                    }
                }
            }
        }
        if dx_acc == 0.0 && dy_acc == 0.0 {
            return None;
        }

        let correction = Coord3D::new(dx_acc * mass / 5.0, dy_acc * mass / 5.0, 0.0);
        let mut correction_normalized = correction;
        let len = (correction_normalized.x * correction_normalized.x
            + correction_normalized.y * correction_normalized.y)
            .sqrt();
        if len > 0.0001 {
            correction_normalized.x /= len;
            correction_normalized.y /= len;
        }
        let dot = velocity.x * correction_normalized.x + velocity.y * correction_normalized.y;
        // C++ Locomotor.cpp:1542-1544 — already leaving the invalid cell.
        if dot > 0.25 {
            return None;
        }
        let extra_push = if dot < 0.0 {
            let mag = (-dot).sqrt();
            Some(Coord3D::new(
                correction_normalized.x * mag * mass,
                correction_normalized.y * mag * mass,
                0.0,
            ))
        } else {
            None
        };
        Some(InvalidPositionFix {
            correction,
            extra_push,
        })
    }

    /// C++ `Pathfinder::validMovementTerrain` for this locomotor's surfaces.
    /// Missing pathfinder fail-opens (treat as valid) so tests without a map still move.
    pub fn valid_movement_terrain_at(
        &self,
        layer: crate::common::PathfindLayerEnum,
        pos: Coord3D,
    ) -> bool {
        let ai_store = crate::ai::the_ai();let Some(ai) = ai_store.read().ok() else {
            return true;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return true;
        };
        let Ok(pf) = pathfinder.read() else {
            return true;
        };
        let set = LocomotorSet::from_surfaces(self.template.surfaces);
        let astar_layer = crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32);
        pf.valid_movement_terrain(astar_layer, &set, &pos)
    }

    /// Start a new move — reset only the donut timer.
    /// Matches C++ Locomotor::startMove (Locomotor.cpp:761-765).
    pub fn start_move(&mut self) {
        self.donut_timer = TheGameLogic::get_frame()
            + (DONUT_TIME_DELAY_SECONDS * LOGICFRAMES_PER_SECOND as Real) as u32;
    }

    /// C++ `AIUpdate::doLocomotor` NONE-goal final-position settle (AIUpdate.cpp:2236-2262).
    /// Returns `(new_pos, still_do_final_position)`.
    pub fn settle_final_position(
        current_pos: Coord3D,
        final_position: Coord3D,
        on_ground: bool,
    ) -> (Coord3D, bool) {
        const DARN_CLOSE: Real = 0.25;
        let dx = final_position.x - current_pos.x;
        let dy = final_position.y - current_pos.y;
        let d_sqr = dx * dx + dy * dy;
        if d_sqr < DARN_CLOSE {
            let mut pos = final_position;
            if on_ground {
                pos.z = TheTerrainLogic::get()
                    .map(|terrain| terrain.get_ground_height(final_position.x, final_position.y, None))
                    .unwrap_or(final_position.z);
            } else {
                pos.z = current_pos.z;
            }
            (pos, false)
        } else {
            let mut dist = d_sqr.sqrt();
            if dist < 1.0 {
                dist = 1.0;
            }
            let frames = LOGICFRAMES_PER_SECOND as Real;
            let mut pos = current_pos;
            pos.x += 2.0 * PATHFIND_CELL_SIZE_F * dx / (dist * frames);
            pos.y += 2.0 * PATHFIND_CELL_SIZE_F * dy / (dist * frames);
            if on_ground {
                pos.z = TheTerrainLogic::get()
                    .map(|terrain| terrain.get_ground_height(pos.x, pos.y, None))
                    .unwrap_or(pos.z);
            }
            (pos, true)
        }
    }

    /// C++ NONE-goal update: optional final-position settle, then `locoUpdate_maintainCurrentPosition`.
    /// AIUpdate.cpp:2234-2265.
    pub fn loco_update_when_goal_none(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        current_speed: Real,
        condition: BodyDamageType,
        delta_time: Real,
        do_final_position: bool,
        final_position: Coord3D,
        on_ground: bool,
    ) -> GoalNoneUpdate {
        let (pos, still_do_final) = if do_final_position {
            Self::settle_final_position(current_pos, final_position, on_ground)
        } else {
            (current_pos, false)
        };
        let (pos, angle, speed, requires_constant) = self.loco_update_maintain_current_position(
            pos,
            current_angle,
            current_speed,
            condition,
            delta_time,
        );
        GoalNoneUpdate {
            pos,
            angle,
            speed,
            requires_constant,
            do_final_position: still_do_final,
        }
    }

    /// Get terrain/water surface height at a 2D point.
    /// Matches C++ Locomotor::getSurfaceHtAtPt (Locomotor.cpp:2007-2019)
    pub fn get_surface_ht_at_pt(&self, x: Real, y: Real) -> Real {
        TheTerrainLogic::get()
            .map(|terrain| {
                let mut water_z = 0.0;
                let mut terrain_z = 0.0;
                if terrain.is_underwater(x, y, Some(&mut water_z), Some(&mut terrain_z)) {
                    water_z
                } else {
                    terrain_z
                }
            })
            .unwrap_or(0.0)
    }

    /// Calculate lift force to use at a given point for aircraft.
    /// Matches C++ Locomotor::calcLiftToUseAtPt (Locomotor.cpp:2022-2110)
    pub fn calc_lift_to_use_at_pt(
        &self,
        cur_z: Real,
        _surface_at_pt: Real,
        preferred_height: Real,
        vel_z: Real,
        condition: BodyDamageType,
        gravity: Real,
    ) -> Real {
        let max_gross_lift = self.get_max_lift(condition);
        let max_net_lift = (max_gross_lift + gravity).max(0.0);

        let max_accel = if self.is_ultra_accurate() {
            if vel_z < 0.0 {
                2.0 * max_net_lift
            } else {
                -2.0 * max_net_lift
            }
        } else if vel_z < 0.0 {
            max_net_lift
        } else {
            gravity
        };

        const TINY_ACCEL: Real = 0.001;
        let desired_accel = if max_accel.abs() > TINY_ACCEL {
            let delta_z = preferred_height - cur_z;
            let brake_dist = vel_z * vel_z / max_accel.abs();

            if brake_dist.abs() > delta_z.abs() {
                max_accel
            } else if vel_z.abs() > self.template.speed_limit_z {
                self.template.speed_limit_z - vel_z
            } else {
                2.0 * (delta_z - vel_z)
            }
        } else {
            0.0
        };

        let mut lift_to_use = desired_accel - gravity;

        if self.is_ultra_accurate() {
            const UP_FACTOR: Real = 3.0;
            if lift_to_use > UP_FACTOR * max_gross_lift {
                lift_to_use = UP_FACTOR * max_gross_lift;
            } else if lift_to_use < -max_gross_lift {
                lift_to_use = -max_gross_lift;
            }
        } else {
            if lift_to_use > max_gross_lift {
                lift_to_use = max_gross_lift;
            } else if lift_to_use < 0.0 {
                lift_to_use = 0.0;
            }
        }

        lift_to_use
    }

    /// Calculate minimum turn radius at a given speed.
    /// Matches C++ Locomotor::calcMinTurnRadius (Locomotor.cpp:1567-1590)
    /// C++ version: minTurnRadius = minSpeed / maxTurnRate
    pub fn calc_min_turn_radius_at_speed(&self, speed: Real, condition: BodyDamageType) -> Real {
        let max_turn_rate = self.get_max_turn_rate(condition);
        if max_turn_rate > 0.0 {
            speed / max_turn_rate
        } else {
            f32::INFINITY
        }
    }

    /// Rotate to face a target position — returns the relative angle turned.
    /// Matches C++ Locomotor::rotateTowardsPosition (Locomotor.cpp:901-908)
    pub fn rotate_towards_position(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        condition: BodyDamageType,
    ) -> Real {
        let turn_rate = self.get_max_turn_rate(condition);
        self.rotate_obj_around_loco_pivot(current_pos, current_angle, goal_pos, turn_rate)
    }

    /// Rotate object around its locomotor pivot point.
    /// Returns the relative angle difference.
    /// Matches C++ Locomotor::rotateObjAroundLocoPivot (Locomotor.cpp:2113-2189)
    pub fn rotate_obj_around_loco_pivot(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        max_turn_rate: Real,
    ) -> Real {
        let mut offset = self.template.turn_pivot_offset;
        if self.is_braking() {
            offset = 0.0;
        }

        if offset.abs() > 0.0001 {
            let radius = self.close_enough_dist.max(1.0);
            let turn_point_offset = offset * radius;
            let dir_x = current_angle.cos();
            let dir_y = current_angle.sin();
            let turn_x = current_pos.x + dir_x * turn_point_offset;
            let turn_y = current_pos.y + dir_y * turn_point_offset;
            let dx = goal_pos.x - turn_x;
            let dy = goal_pos.y - turn_y;

            if dx.abs() < 0.1 && dy.abs() < 0.1 {
                return 0.0;
            }

            let desired_angle = dy.atan2(dx);
            let amount = Self::std_angle_diff(desired_angle, current_angle);
            let clamped = amount.clamp(-max_turn_rate, max_turn_rate);
            clamped
        } else {
            let desired_angle = (goal_pos.y - current_pos.y).atan2(goal_pos.x - current_pos.x);
            let amount = Self::std_angle_diff(desired_angle, current_angle);
            amount.clamp(-max_turn_rate, max_turn_rate)
        }
    }

    /// Compute Z-axis force for a movement step using compute_z_target.
    /// Helper for move_towards_angle and other methods.

    /// Compute Z-axis force for a movement step using compute_z_target.
    /// Helper for move_towards_angle and other methods.
    fn compute_z_force(
        &self,
        current: Coord3D,
        target: Coord3D,
        _condition: BodyDamageType,
    ) -> Real {
        if let Some(z_target) = self.compute_z_target(current, target) {
            let delta = z_target - current.z;
            let max_z_speed = self.template.speed_limit_z.max(0.0);
            if max_z_speed > 0.0 {
                delta.signum() * max_z_speed.min(delta.abs())
            } else {
                delta
            }
        } else {
            0.0
        }
    }

}
