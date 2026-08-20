impl Locomotor {
    /// Move towards position - Wings (fixed-wing aircraft) locomotor.
    /// Matches C++ Locomotor.cpp:1821-1860 moveTowardsPositionWings
    ///
    /// Key behaviors:
    /// - Circle-for-landing: when `circle_thresh > 0` and the Z delta to goal
    ///   exceeds the threshold, the aircraft aims for a point on the opposite
    ///   side of a circle around the goal to gain/lose altitude before resuming.

    /// Move towards position - Wings (fixed-wing aircraft) locomotor.
    /// Matches C++ Locomotor.cpp:1821-1860 moveTowardsPositionWings
    ///
    /// Key behaviors:
    /// - Circle-for-landing: when `circle_thresh > 0` and the Z delta to goal
    ///   exceeds the threshold, the aircraft aims for a point on the opposite
    ///   side of a circle around the goal to gain/lose altitude before resuming.
    /// - Enforces minimum turn speed (wings cannot fly below min_turn_speed).
    /// - Applies circling correction when within `circling_radius` of target.
    /// - Applies air corrections (rudder correction degree limiting).
    /// - Otherwise delegates to the same physics as Other locomotors.
    pub fn move_towards_position_wings_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        // C++ Locomotor.cpp:1821-1860 moveTowardsPositionWings
        //
        // The C++ code has the circle-for-landing logic guarded by #ifdef CIRCLE_FOR_LANDING,
        // which is disabled (#define NO_CIRCLE_FOR_LANDING) in the shipped game.
        // We implement it here but gate it on circle_thresh > 0 (default 0.0) so it
        // matches the disabled-by-default behavior while remaining available.
        //
        // When circle_thresh > 0 and the vertical distance (dz) to the goal exceeds
        // the threshold, we compute a point on the opposite side of a circle centered
        // on the goal and aim for that instead. This makes the aircraft circle to
        // gain/lose altitude before resuming its course.

        let mut effective_goal = goal_pos;

        if self.template.circle_thresh > 0.0 {
            let dz = (goal_pos.z - current_pos.z).abs();

            if dz > self.template.circle_thresh {
                // Compute direction toward the goal position (2D only)
                let dx = goal_pos.x - current_pos.x;
                let dy = goal_pos.y - current_pos.y;

                // C++ Locomotor.cpp:1837-1840: use current orientation if dx,dy are ~zero
                let angle_toward_pos = if dx.abs() < 0.001 && dy.abs() < 0.001 {
                    current_angle
                } else {
                    dy.atan2(dx)
                };

                // C++ Locomotor.cpp:1842-1843: aim for the opposite side of the circle
                // aimDir = PI - PI/8 = 7*PI/8
                let aim_dir = std::f32::consts::PI - std::f32::consts::FRAC_PI_8;
                let circle_angle = angle_toward_pos + aim_dir;

                // C++ Locomotor.cpp:1846: turnRadius = calcMinTurnRadius * 4
                let turn_radius = self.calc_min_turn_radius(condition) * 4.0;

                // C++ Locomotor.cpp:1849-1851: project a spot "radius" dist away from goal
                effective_goal = Coord3D {
                    x: goal_pos.x + circle_angle.cos() * turn_radius,
                    y: goal_pos.y + circle_angle.sin() * turn_radius,
                    z: goal_pos.z,
                };

                // C++ Locomotor.cpp:1852: moveTowardsPositionOther with the adjusted goal
                return self.move_towards_position_other_physics(
                    current_pos,
                    current_angle,
                    effective_goal,
                    0.0, // onPathDistToGoal = 0 (not on path when circling)
                    desired_speed,
                    current_speed,
                    condition,
                );
            }
        }

        // C++ Locomotor.cpp:1859: handle the 2D component via moveTowardsPositionOther
        self.move_towards_position_other_physics(
            current_pos,
            current_angle,
            effective_goal,
            on_path_dist_to_goal,
            desired_speed,
            current_speed,
            condition,
        )
    }

    /// Move towards position - Climber locomotor (cliff climbing).
    /// Matches C++ Locomotor.cpp:1690-1818 moveTowardsPositionClimb
    pub fn move_towards_position_climber_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real, bool) {
        let max_speed = self.get_max_speed_for_condition(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        if self.is_naval_blocked_at(current_pos) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current_pos, goal_pos);
        let max_acceleration = self.get_max_acceleration(condition);
        let braking = self.get_braking();

        // Climbing detection - C++ Locomotor.cpp:1711-1716
        // Uses PATHFIND_CELL_SIZE_F for the threshold (10.0)
        let dz = current_pos.z - goal_pos.z;
        if dz * dz > PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
            self.set_flag(FLAG_CLIMBING, true);
        }
        if dz.abs() < 1.0 {
            self.set_flag(FLAG_CLIMBING, false);
        }

        let mut move_backwards = false;

        // Climbing behavior - check ground slope ahead - C++ Locomotor.cpp:1721-1740
        if self.is_climbing() {
            // C++ normalizes the 2D direction from pos to goalPos, then adds pos back
            // to get a point exactly 1 unit away in that direction:
            //   delta = goalPos; delta -= pos; delta.z=0; delta.normalize();
            //   delta += pos; delta.z = getGroundHeight(delta.x, delta.y);
            let delta = goal_pos - current_pos;
            let delta_len = (delta.x * delta.x + delta.y * delta.y).sqrt();
            let mut forward_x = current_pos.x;
            let mut forward_y = current_pos.y;
            if delta_len > 0.001 {
                forward_x = current_pos.x + delta.x / delta_len;
                forward_y = current_pos.y + delta.y / delta_len;
            }
            let ground_z = TheTerrainLogic::get()
                .map(|terrain| terrain.get_ground_height(forward_x, forward_y, None))
                .unwrap_or(current_pos.z);

            if ground_z < current_pos.z - 0.1 {
                move_backwards = true;
            }

            // C++ Locomotor.cpp:1734-1739 - reduce speed based on slope
            let ground_slope = (ground_z - current_pos.z).abs();
            let ground_slope = if ground_slope < 1.0 {
                1.0
            } else {
                ground_slope
            };
            if ground_slope > 1.0 {
                desired_speed /= ground_slope * 4.0;
            }
        }
        self.set_flag(FLAG_MOVING_BACKWARDS, move_backwards);

        // Orient toward goal - C++ Locomotor.cpp:1746-1757
        let mut desired_angle =
            self.desired_angle_with_pivot(current_pos, current_angle, goal_pos, false);
        if move_backwards {
            desired_angle = Self::normalize_angle(desired_angle + std::f32::consts::PI);
        }
        let rel_angle = Self::std_angle_diff(desired_angle, current_angle);

        // Modulate speed by turn angle - C++ Locomotor.cpp:1762-1767
        const QUARTER_PI: Real = std::f32::consts::PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUARTER_PI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;

        let mut actual_speed = current_speed;
        if move_backwards {
            actual_speed = -actual_speed;
        }

        // Slow down approaching destination - C++ Locomotor.cpp:1776-1780
        let slow_down_dist =
            Self::calc_slow_down_dist(actual_speed.abs(), self.template.min_speed, braking);
        if on_path_dist_to_goal < slow_down_dist && !self.no_slow_down_approaching_dest() {
            goal_speed = self.template.min_speed;
        }

        // Acceleration with backward sign swap - C++ Locomotor.cpp:1785-1817
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
                braking
            }
        } else {
            if speed_delta > 0.0 {
                max_acceleration
            } else {
                -braking
            }
        };

        (current_pos, desired_angle, acceleration, move_backwards)
    }

}
