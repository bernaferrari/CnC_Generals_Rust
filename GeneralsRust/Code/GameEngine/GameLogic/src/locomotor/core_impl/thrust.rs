fn nearly_zero(v: Real) -> bool {
    v.abs() <= 0.001
}

fn unit_or_x(v: Coord3D) -> Coord3D {
    let len = v.length();
    if len > 1.0e-8 {
        v / len
    } else {
        Coord3D::new(1.0, 0.0, 0.0)
    }
}

fn try_to_rotate_vector3d(max_angle: Real, start: Coord3D, end: Coord3D) -> (Coord3D, Real) {
    let start_len = start.length();
    let end_len = end.length();
    if start_len < 1.0e-6 || end_len < 1.0e-6 {
        return (end, 0.0);
    }
    let start_n = start / start_len;
    let end_n = end / end_len;
    let dot = start_n.dot(end_n).clamp(-1.0, 1.0);
    let angle = dot.acos();
    if max_angle <= 0.0 {
        return (start_n, angle);
    }
    if angle <= max_angle {
        return (end_n, angle);
    }
    let axis = start_n.cross(end_n);
    let axis_len = axis.length();
    if axis_len < 1.0e-6 {
        return (end_n, angle);
    }
    let axis_n = axis / axis_len;
    let (sin_a, cos_a) = max_angle.sin_cos();
    let rotated = start_n * cos_a + axis_n.cross(start_n) * sin_a + axis_n * axis_n.dot(start_n) * (1.0 - cos_a);
    let rot_len = rotated.length();
    if rot_len < 1.0e-6 {
        (end_n, angle)
    } else {
        (rotated / rot_len, angle)
    }
}

impl Locomotor {
    /// C++ `calcDirectionToApplyThrust` (Locomotor.cpp:175-250).
    fn calc_direction_to_apply_thrust(
        &self,
        obj_pos: Coord3D,
        cur_vel: Coord3D,
        goal_pos: Coord3D,
        max_accel: Real,
        forward: Coord3D,
    ) -> Coord3D {
        let vec_to_goal = goal_pos - obj_pos;
        if nearly_zero(vec_to_goal.length_squared()) {
            return unit_or_x(forward);
        }

        let mut cur_vel = cur_vel;
        cur_vel.z += loco_gravity();

        let dist_to_goal = vec_to_goal.length();
        let cur_vel_mag_sqr = cur_vel.length_squared();
        let cur_vel_mag = cur_vel_mag_sqr.sqrt();
        let max_accel_sqr = max_accel * max_accel;
        let denom = cur_vel_mag_sqr - max_accel_sqr;

        if !nearly_zero(denom) {
            let t = (dist_to_goal * (cur_vel_mag + max_accel)) / denom;
            let t2 = (dist_to_goal * (cur_vel_mag - max_accel)) / denom;
            if t >= 0.0 || t2 >= 0.0 {
                let t = if t < 0.0 || (t2 >= 0.0 && t2 < t) {
                    t2
                } else {
                    t
                };
                if !nearly_zero(t) {
                    let mut dir = Coord3D::new(
                        vec_to_goal.x / t - cur_vel.x,
                        vec_to_goal.y / t - cur_vel.y,
                        vec_to_goal.z / t - cur_vel.z,
                    );
                    let len = dir.length();
                    if len > 1.0e-8 {
                        dir /= len;
                        return dir;
                    }
                }
            }
        }

        let len = vec_to_goal.length();
        if len > 1.0e-8 {
            vec_to_goal / len
        } else {
            Coord3D::new(1.0, 0.0, 0.0)
        }
    }

    /// Move towards position - Thrust locomotor (helicopters / jets).
    /// Matches C++ Locomotor.cpp:1891-2004 moveTowardsPositionThrust.
    pub fn move_towards_position_thrust_physics(
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
        let mut desired_speed = desired_speed.clamp(self.template.min_speed, max_speed);
        let braking = self.get_braking();

        if braking > 0.0 {
            let slow_down_dist =
                Self::calc_slow_down_dist(current_speed, self.template.min_speed, braking);
            if on_path_dist_to_goal < slow_down_dist && !self.no_slow_down_approaching_dest() {
                desired_speed = self.template.min_speed;
            }
        }

        let mut local_goal_pos = goal_pos;
        if self.preferred_height != 0.0 && !self.uses_precise_z_pos() {
            let surface_ht = self.get_surface_ht_at_pt(current_pos.x, current_pos.y);
            local_goal_pos.z = self.preferred_height + surface_ht;
            let delta = local_goal_pos.z - current_pos.z;
            local_goal_pos.z = current_pos.z + delta * self.preferred_height_damping;
        }

        let forward = Coord3D::new(current_angle.cos(), current_angle.sin(), 0.0);
        let heading_vel = Coord3D::new(
            current_angle.cos() * current_speed,
            current_angle.sin() * current_speed,
            0.0,
        );

        let speed_delta = desired_speed - current_speed;
        let max_accel = if speed_delta > 0.0 || braking == 0.0 {
            self.get_max_acceleration(condition)
        } else {
            -braking
        };
        let mut max_turn_rate = self.get_max_turn_rate(condition);

        let desired_thrust_dir = self.calc_direction_to_apply_thrust(
            current_pos,
            heading_vel,
            local_goal_pos,
            max_accel,
            forward,
        );
        let max_thrust_angle = if max_turn_rate > 0.0 {
            self.template.max_thrust_angle
        } else {
            0.0
        };
        let (thrust_dir, thrust_angle) =
            try_to_rotate_vector3d(max_thrust_angle, forward, desired_thrust_dir);

        let mut desired_angle = current_angle;
        if current_speed.abs() > 1.0e-4 || thrust_dir.length_squared() > 1.0e-8 {
            let mut orient = if self.is_braking() {
                max_turn_rate *= 3.0;
                local_goal_pos - current_pos
            } else {
                heading_vel
            };
            if orient.length_squared() < 1.0e-8 {
                orient = thrust_dir;
            }
            desired_angle = orient.y.atan2(orient.x);
        }

        let mut max_forward_speed = max_speed;
        if max_forward_speed <= 0.0 {
            max_forward_speed = 0.01;
        }
        let damping = (max_accel / max_forward_speed).clamp(0.0, 1.0);
        let accel_vec = thrust_dir * max_accel - heading_vel * damping;
        self.last_motive_accel = accel_vec;

        let acceleration = if speed_delta != 0.0 || thrust_angle != 0.0 {
            accel_vec.x * current_angle.cos() + accel_vec.y * current_angle.sin()
        } else {
            0.0
        };

        (current_pos, desired_angle, acceleration)
    }
}
