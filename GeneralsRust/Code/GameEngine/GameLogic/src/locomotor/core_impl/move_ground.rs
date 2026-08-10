impl Locomotor {
    /// Move towards position - Treads locomotor (tanks) with full physics
    /// Matches C++ Locomotor.cpp:1144-1255 moveTowardsPositionTreads
    pub fn move_towards_position_treads_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        let max_speed = self.get_max_speed_for_condition(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        if self.is_naval_blocked_at(current_pos) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_downhill_only(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_jump_slowdown(desired_speed, current_pos, goal_pos);
        let max_acceleration = self.get_max_acceleration(condition);

        // Calculate relative angle to goal (with turn pivot offset)
        // C++ uses rotateTowardsPosition which also sets physics->setTurning
        let desired_angle =
            self.desired_angle_with_pivot(current_pos, current_angle, goal_pos, self.is_braking());
        let rel_angle = Self::std_angle_diff(desired_angle, current_angle);

        // Modulate speed according to turning
        // C++ Locomotor.cpp:1170-1173
        const QUARTER_PI: Real = std::f32::consts::PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUARTER_PI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;
        goal_speed = self.apply_naval_turn_limit(goal_speed, current_angle, desired_angle);

        // Check if close to target and turning - slow down for precision
        // C++ Locomotor.cpp:1190-1192
        let dx = current_pos.x - goal_pos.x;
        let dy = current_pos.y - goal_pos.y;
        if (dx * dx + dy * dy) < (2.0 * PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F)
            && angle_coeff > 0.05
        {
            goal_speed = current_speed * 0.6;
        }

        // Braking logic - matches C++ Locomotor.cpp:1187-1221
        // C++ uses actualSpeed / getBraking() for time and actualSpeed/1.50f * time for dist
        let braking = self.get_braking();
        let slow_down_time = if braking > 0.0 {
            current_speed / braking
        } else {
            0.0
        };
        let slow_down_dist = (current_speed / 1.5) * slow_down_time;

        // Start braking if close enough and not already braking
        // C++ Locomotor.cpp:1194-1198
        if on_path_dist_to_goal < slow_down_dist
            && !self.is_braking()
            && !self.no_slow_down_approaching_dest()
        {
            self.set_flag(FLAG_IS_BRAKING, true);
            self.braking_factor = 1.1;
        }

        // Stop braking if far enough from goal
        // C++ Locomotor.cpp:1200-1203
        if on_path_dist_to_goal > PATHFIND_CELL_SIZE_F
            && on_path_dist_to_goal > 2.0 * slow_down_dist
        {
            self.set_flag(FLAG_IS_BRAKING, false);
        }

        // Apply braking factor and reduce speed
        // C++ Locomotor.cpp:1205-1221
        if self.is_braking() {
            if on_path_dist_to_goal > 0.0 {
                self.braking_factor = slow_down_dist / on_path_dist_to_goal;
            }
            self.braking_factor *= self.braking_factor;
            if self.braking_factor > MAX_BRAKING_FACTOR {
                self.braking_factor = MAX_BRAKING_FACTOR;
            }

            if slow_down_dist > on_path_dist_to_goal {
                goal_speed = current_speed - braking;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else if slow_down_dist > on_path_dist_to_goal * 0.75 {
                goal_speed = current_speed - braking / 2.0;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else {
                goal_speed = current_speed;
            }
        }

        // Calculate acceleration force - matches C++ Locomotor.cpp:1230-1254
        // C++ uses mass * acceleration and clamps accelForce <= mass * speedDelta
        // We return the acceleration directly; the caller applies mass.
        let speed_delta = goal_speed - current_speed;
        let acceleration = if speed_delta > 0.0 {
            max_acceleration
        } else {
            -self.braking_factor * braking
        };

        (current_pos, desired_angle, acceleration)
    }

    /// Move towards position - Wheels locomotor (trucks, vehicles) with full physics
    /// Matches C++ Locomotor.cpp:1258-1498 moveTowardsPositionWheels
    pub fn move_towards_position_wheels_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
        major_radius: Real,
        current_frame: u32,
    ) -> (Coord3D, Real, Real, bool) {
        let max_speed = self.get_max_speed_for_condition(condition);
        let max_turn_rate = self.get_max_turn_rate(condition);
        let max_acceleration = self.get_max_acceleration(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        if self.is_naval_blocked_at(current_pos) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_downhill_only(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current_pos, goal_pos);

        let mut turn_speed = self.template.min_turn_speed;
        let mut desired_angle =
            self.desired_angle_with_pivot(current_pos, current_angle, goal_pos, false);
        let mut rel_angle = Self::std_angle_diff(desired_angle, current_angle);

        let mut move_backwards = false;

        // Wheeled vehicles can only turn while moving, so make sure the turn speed is reasonable.
        // C++ Locomotor.cpp:1283-1286
        if turn_speed < max_speed / 4.0 {
            turn_speed = max_speed / 4.0;
        }

        let mut actual_speed = current_speed;
        let mut _do3point_turn = false;

        // 3-point turn logic - C++ Locomotor.cpp:1292-1313
        if actual_speed == 0.0 {
            self.set_flag(FLAG_MOVING_BACKWARDS, false);
            if self.template.can_move_backward && rel_angle.abs() > std::f32::consts::PI / 2.0 {
                self.set_flag(FLAG_MOVING_BACKWARDS, true);
                self.set_flag(
                    FLAG_DOING_THREE_POINT_TURN,
                    on_path_dist_to_goal > 5.0 * major_radius,
                );
            }
        }
        if self.is_moving_backwards() {
            if rel_angle.abs() < std::f32::consts::PI / 2.0 {
                move_backwards = false;
                self.set_flag(FLAG_MOVING_BACKWARDS, false);
            } else {
                move_backwards = true;
                self.set_flag(
                    FLAG_DOING_THREE_POINT_TURN,
                    on_path_dist_to_goal > 5.0 * major_radius,
                );
                _do3point_turn = self.get_flag(FLAG_DOING_THREE_POINT_TURN);
                if !_do3point_turn {
                    desired_angle = Self::normalize_angle(desired_angle + std::f32::consts::PI);
                    rel_angle = Self::std_angle_diff(desired_angle, current_angle);
                }
            }
        }

        // Reduce speed when turning sharply - C++ Locomotor.cpp:1316-1323
        const SMALL_TURN: Real = std::f32::consts::PI / 20.0;
        if rel_angle.abs() > SMALL_TURN && desired_speed > turn_speed {
            desired_speed = turn_speed;
        }

        let mut goal_speed = desired_speed;
        if move_backwards {
            actual_speed = -actual_speed;
        }
        goal_speed = self.apply_naval_turn_limit(goal_speed, current_angle, desired_angle);

        // Braking distance calculation - C++ Locomotor.cpp:1332-1337
        let braking = self.get_braking();
        let slow_down_time = if braking > 0.0 {
            actual_speed / braking + 1.0
        } else {
            0.0
        };
        let slow_down_dist = (actual_speed / 1.5) * slow_down_time + actual_speed;
        let mut effective_slow_down_dist = slow_down_dist;
        if effective_slow_down_dist < 1.0 * PATHFIND_CELL_SIZE_F {
            effective_slow_down_dist = 1.0 * PATHFIND_CELL_SIZE_F;
        }

        // Start braking if close enough - C++ Locomotor.cpp:1393-1403
        if on_path_dist_to_goal < effective_slow_down_dist
            && !self.is_braking()
            && !self.no_slow_down_approaching_dest()
        {
            self.set_flag(FLAG_IS_BRAKING, true);
            self.braking_factor = 1.1;
        }

        if on_path_dist_to_goal > PATHFIND_CELL_SIZE_F
            && on_path_dist_to_goal > 2.0 * slow_down_dist
        {
            self.set_flag(FLAG_IS_BRAKING, false);
        }

        // Donut timer - stop near destination for precise positioning
        // C++ Locomotor.cpp:1405-1411
        if on_path_dist_to_goal > DONUT_DISTANCE {
            self.donut_timer =
                current_frame + (DONUT_TIME_DELAY_SECONDS * LOGICFRAMES_PER_SECOND as Real) as u32;
        } else if current_frame >= self.donut_timer {
            self.set_flag(FLAG_IS_BRAKING, true);
        }

        // Apply braking factor - C++ Locomotor.cpp:1413-1430
        if self.is_braking() {
            if on_path_dist_to_goal > 0.0 {
                self.braking_factor = slow_down_dist / on_path_dist_to_goal;
            }
            self.braking_factor *= self.braking_factor;
            if self.braking_factor > MAX_BRAKING_FACTOR {
                self.braking_factor = MAX_BRAKING_FACTOR;
            }
            // C++ sets m_brakingFactor = 1.0f after the clamp above (line 1420)
            // This means the braking factor calculation is effectively unused for wheels
            // and the code below uses the raw braking values.
            self.braking_factor = 1.0;

            if slow_down_dist > on_path_dist_to_goal {
                goal_speed = actual_speed - braking;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else if slow_down_dist > on_path_dist_to_goal * 0.75 {
                goal_speed = actual_speed - braking / 2.0;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else {
                goal_speed = actual_speed;
            }
        }

        // Turn rate based on speed - C++ Locomotor.cpp:1438-1444
        // (Turn factor is used for rotateObjAroundLocoPivot; we incorporate it into desired_angle)
        let turn_factor = if turn_speed > 0.0 {
            (actual_speed / turn_speed).abs().min(1.0)
        } else {
            0.0
        };
        let _turn_amount = turn_factor * max_turn_rate;

        // Acceleration force - C++ Locomotor.cpp:1458-1496
        let mut speed_delta = goal_speed - actual_speed;
        if move_backwards {
            speed_delta = -goal_speed + actual_speed;
        }
        let acceleration = if speed_delta == 0.0 {
            0.0
        } else if move_backwards {
            if speed_delta < 0.0 {
                -max_acceleration
            } else {
                self.braking_factor * braking
            }
        } else {
            if speed_delta > 0.0 {
                max_acceleration
            } else {
                -self.braking_factor * braking
            }
        };

        (current_pos, desired_angle, acceleration, move_backwards)
    }

    /// Move towards position - Legs locomotor (infantry) with full physics
    /// Matches C++ Locomotor.cpp:1594-1687 moveTowardsPositionLegs
    pub fn move_towards_position_legs_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        // C++ Locomotor.cpp:1596-1598 - downhill only check for legs
        if self.template.downhill_only && current_pos.z < goal_pos.z {
            return (current_pos, current_angle, 0.0);
        }

        let max_speed = self.get_max_speed_for_condition(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        if self.is_naval_blocked_at(current_pos) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_jump_slowdown(desired_speed, current_pos, goal_pos);
        let max_acceleration = self.get_max_acceleration(condition);

        let mut desired_angle =
            self.desired_angle_with_pivot(current_pos, current_angle, goal_pos, false);

        // Wander logic for infantry - C++ Locomotor.cpp:1618-1633
        if self.template.wander_width_factor != 0.0 {
            let angle_limit = std::f32::consts::PI / 8.0 * self.template.wander_width_factor;
            if self.is_offset_increasing() {
                self.angle_offset += self.offset_increment * current_speed;
                if self.angle_offset > angle_limit {
                    self.set_flag(FLAG_OFFSET_INCREASING, false);
                }
            } else {
                self.angle_offset -= self.offset_increment * current_speed;
                if self.angle_offset < -angle_limit {
                    self.set_flag(FLAG_OFFSET_INCREASING, true);
                }
            }
            desired_angle = Self::normalize_angle(desired_angle + self.angle_offset);
        }

        let rel_angle = Self::std_angle_diff(desired_angle, current_angle);

        // Modulate speed according to turning - C++ Locomotor.cpp:1641-1646
        const QUARTER_PI: Real = std::f32::consts::PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUARTER_PI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;
        goal_speed = self.apply_naval_turn_limit(goal_speed, current_angle, desired_angle);

        // Slow down as approaching destination - C++ Locomotor.cpp:1649-1653
        let braking = self.get_braking();
        let slow_down_dist =
            Self::calc_slow_down_dist(current_speed, self.template.min_speed, braking);
        if on_path_dist_to_goal < slow_down_dist && !self.no_slow_down_approaching_dest() {
            goal_speed = self.template.min_speed;
        }

        // Calculate acceleration - C++ Locomotor.cpp:1660-1686
        // C++ applies mass * acceleration as force, clamped to mass * speedDelta
        // We return the acceleration directly.
        let speed_delta = goal_speed - current_speed;
        let acceleration = if speed_delta > 0.0 {
            max_acceleration
        } else {
            -braking
        };

        (current_pos, desired_angle, acceleration)
    }

    /// Move towards position - Hover locomotor with full physics
    /// Matches C++ Locomotor.cpp:1863-1888 moveTowardsPositionHover
    pub fn move_towards_position_hover_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        // Hover uses the "Other" movement logic for 2D
        self.move_towards_position_other_physics(
            current_pos,
            current_angle,
            goal_pos,
            on_path_dist_to_goal,
            desired_speed,
            current_speed,
            condition,
        )
    }

    /// Move towards position - Other/generic locomotor with full physics
    /// Matches C++ Locomotor.cpp:2326-2404 moveTowardsPositionOther
    ///
    /// Returns (current_pos, desired_angle, acceleration).
    /// When ULTRA_ACCURATE is set and close enough, desired_angle is overridden
    /// to point directly at the goal (C++ slides without rotating the model).
    pub fn move_towards_position_other_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        let max_speed = self.get_max_speed_for_condition(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        if self.is_naval_blocked_at(current_pos) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_downhill_only(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_jump_slowdown(desired_speed, current_pos, goal_pos);
        if self.template.appearance == LocomotorAppearance::Wings
            && desired_speed < self.template.min_turn_speed
        {
            desired_speed = self.template.min_turn_speed;
        }
        let max_acceleration = self.get_max_acceleration(condition);

        // C++ Locomotor.cpp:2344-2366: ULTRA_ACCURATE slide-into-place logic
        // When close enough, don't turn -- just slide in the right direction.
        // C++ uses dirToApplyForce directly toward goal instead of unit direction vector.
        let mut _goal_speed = desired_speed;
        let mut desired_angle = if matches!(
            self.template.appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) {
            self.apply_air_corrections(
                current_angle,
                self.apply_wings_circling(
                    current_pos,
                    goal_pos,
                    (goal_pos.y - current_pos.y).atan2(goal_pos.x - current_pos.x),
                ),
            )
        } else {
            self.desired_angle_with_pivot(current_pos, current_angle, goal_pos, self.is_braking())
        };

        let mut sliding_into_place = false;
        self.set_flag(FLAG_SLIDING_INTO_PLACE, false);
        if self.is_ultra_accurate() {
            let slide_threshold = desired_speed * self.template.ultra_accurate_slide_factor;
            if (goal_pos.x - current_pos.x).abs() <= slide_threshold
                && (goal_pos.y - current_pos.y).abs() <= slide_threshold
            {
                // C++ Locomotor.cpp:2356-2360: override force direction toward goal,
                // don't turn (TURN_NONE). We return desired_angle pointing at goal
                // so the caller advances toward it, and set sliding flag so
                // step_angle skips rotation.
                let dx = goal_pos.x - current_pos.x;
                let dy = goal_pos.y - current_pos.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.001 {
                    desired_angle = dy.atan2(dx);
                }
                sliding_into_place = true;
                self.set_flag(FLAG_SLIDING_INTO_PLACE, true);
            }
        }

        // C++ Locomotor.cpp:2363-2366: rotateTowardsPosition only if not sliding
        // (handled by step_angle in the caller via sliding_into_place concept;
        // we encode it by returning the angle diff = 0 for ultra_accurate slides)

        let rel_angle = if sliding_into_place {
            // When sliding into place, angle_coeff stays 0 so we don't slow down.
            0.0
        } else {
            Self::std_angle_diff(desired_angle, current_angle)
        };

        const QUARTER_PI: Real = std::f32::consts::PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUARTER_PI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        _goal_speed = (1.0 - angle_coeff) * desired_speed;
        _goal_speed = self.apply_naval_turn_limit(_goal_speed, current_angle, desired_angle);

        // C++ Locomotor.cpp:2368-2374: uses minSpeed, not 0.0
        if !self.no_slow_down_approaching_dest() {
            let slow_down_dist = Self::calc_slow_down_dist(
                current_speed,
                self.template.min_speed,
                self.get_braking(),
            );
            if on_path_dist_to_goal < slow_down_dist {
                _goal_speed = self.template.min_speed;
            }
        }

        // C++ Locomotor.cpp:2380-2401: maintain goal speed
        // C++ clamps accelForce to mass * speedDelta to avoid overshooting.
        let speed_delta = _goal_speed - current_speed;
        let acceleration = if speed_delta == 0.0 {
            0.0
        } else if speed_delta > 0.0 {
            max_acceleration
        } else {
            -self.get_braking()
        };

        (current_pos, desired_angle, acceleration)
    }

}
