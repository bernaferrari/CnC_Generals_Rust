impl Locomotor {
    /// Set active path from pathfinding result
    /// Matches C++ Locomotor path integration
    pub fn set_path(&mut self, path: crate::ai::pathfinding_system::Path, current_frame: u32) {
        let waypoints: Vec<Coord3D> = path.waypoints.iter().map(|wp| wp.position).collect();
        let layers: Vec<PathfindLayerEnum> = path.waypoints.iter().map(|wp| wp.layer).collect();

        if !waypoints.is_empty() {
            self.active_path = Some(ActivePath::new_with_layers(
                waypoints,
                layers,
                current_frame,
            ));
        }
    }

    /// Clear active path
    pub fn clear_path(&mut self) {
        self.active_path = None;
    }

    /// Update path following - main locomotor update for path-based movement
    /// Matches C++ Locomotor::Move and path following logic
    pub fn update_path_following(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        current_speed: Real,
        condition: BodyDamageType,
        desired_speed: Real,
        current_frame: u32,
        delta_time: Real,
    ) -> Option<(Coord3D, Real, Real)> {
        let use_3d_close_enough = self.is_close_enough_dist_3d();
        let close_enough_dist = self.close_enough_dist;
        let path = self.active_path.as_mut()?;

        // Get current target waypoint
        let target = path.current_target()?;

        // Check if we've reached current waypoint
        let delta_to_target = target - current_pos;
        let distance_to_target = if use_3d_close_enough {
            delta_to_target.length()
        } else {
            (delta_to_target.x * delta_to_target.x + delta_to_target.y * delta_to_target.y).sqrt()
        };
        if distance_to_target < close_enough_dist {
            // Advance to next waypoint
            if !path.advance_waypoint() {
                // Path complete
                self.active_path = None;
                return None;
            }
        }

        // Update distance to waypoint
        if let Some(path) = self.active_path.as_mut() {
            path.distance_to_waypoint = distance_to_target;
        }

        // Get next target after advancing
        let target = self.active_path.as_ref()?.current_target()?;

        // Calculate distance remaining on path
        let on_path_dist_to_goal = self
            .active_path
            .as_ref()
            .map(|p| p.distance_remaining())
            .unwrap_or(distance_to_target);

        // Desired speed based on path following and AI constraints
        let max_speed = self.get_max_speed_for_condition(condition);
        let desired_speed = desired_speed.min(max_speed);

        // Use locomotor-specific movement based on appearance
        match self.template.appearance {
            LocomotorAppearance::Treads => {
                let (_pos, desired_angle, accel) = self.move_towards_position_treads_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::FourWheels | LocomotorAppearance::Motorcycle => {
                let (_pos, desired_angle, acceleration, move_backwards) = self
                    .move_towards_position_wheels_physics(
                        current_pos,
                        current_angle,
                        target,
                        on_path_dist_to_goal,
                        desired_speed,
                        current_speed,
                        condition,
                        self.close_enough_dist, // major_radius proxy
                        current_frame,
                    );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed =
                    (current_speed + acceleration * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    move_backwards,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::TwoLegs => {
                let (_pos, desired_angle, accel) = self.move_towards_position_legs_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::Hover => {
                let (_pos, desired_angle, accel) = self.move_towards_position_hover_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::Thrust => {
                let (_pos, desired_angle, accel) = self.move_towards_position_thrust_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::Wings => {
                let (_pos, desired_angle, accel) = self.move_towards_position_wings_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::Climber => {
                let (_pos, desired_angle, accel, move_backwards) = self
                    .move_towards_position_climber_physics(
                        current_pos,
                        current_angle,
                        target,
                        on_path_dist_to_goal,
                        desired_speed,
                        current_speed,
                        condition,
                    );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    move_backwards,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::Other => {
                let (_pos, desired_angle, accel) = self.move_towards_position_other_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let mut new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                if (self.template.surfaces & SURFACE_CLIFF) != 0 {
                    new_pos.z = current_pos.z.min(new_pos.z);
                }
                Some((new_pos, new_angle, new_speed))
            }
        }
    }

    /// Check for obstacles and request path replan if needed
    /// Matches C++ obstacle detection and dynamic replanning
    pub fn check_obstacles(
        &mut self,
        current_pos: Coord3D,
        pathfinding: &crate::ai::pathfinding_system::PathfindingSystem,
        current_frame: u32,
        requester: ObjectID,
    ) -> bool {
        // Only check every N frames to avoid performance issues
        const OBSTACLE_CHECK_INTERVAL: u32 = 15; // ~0.5 seconds at 30fps

        if current_frame - self.last_obstacle_check < OBSTACLE_CHECK_INTERVAL {
            return false;
        }

        self.last_obstacle_check = current_frame;

        // Get next waypoint to check line of sight
        let path = match self.active_path.as_ref() {
            Some(p) => p,
            None => return false,
        };
        let next_waypoint = match path.next_waypoint() {
            Some(wp) => wp,
            None => return false,
        };

        // Check if path to next waypoint is blocked
        let capabilities = self.to_movement_capabilities();
        let start_coord =
            crate::ai::pathfinding_system::GridCoord::from_world(&current_pos, capabilities.layer);
        let next_coord = crate::ai::pathfinding_system::GridCoord::from_world(
            &next_waypoint,
            capabilities.layer,
        );

        // Detect newly blocked movement between current position and the next waypoint.
        let line_clear = pathfinding.is_line_clear_between(&current_pos, &next_waypoint);

        let terrain_layer = match capabilities.layer {
            crate::ai::pathfinding_system::PathfindLayerEnum::Ground
            | crate::ai::pathfinding_system::PathfindLayerEnum::Tunnel
            | crate::ai::pathfinding_system::PathfindLayerEnum::Invalid => {
                crate::common::PathfindLayerEnum::Ground
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Air => {
                crate::common::PathfindLayerEnum::Top
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Water => {
                crate::common::PathfindLayerEnum::Water
            }
        };

        let terrain_blocked = pathfinding
            .terrain_at(&next_waypoint, terrain_layer)
            .map(|terrain| {
                matches!(
                    terrain,
                    crate::ai::pathfinding_system::TerrainType::Obstacle
                        | crate::ai::pathfinding_system::TerrainType::Impassable
                )
            })
            .unwrap_or(true);

        let obstacle_detected = !line_clear || terrain_blocked;

        if obstacle_detected {
            log::trace!(
                "Locomotor obstacle detected for object {} from {:?} to {:?}",
                requester,
                start_coord,
                next_coord
            );
        }

        obstacle_detected
    }

    /// Get terrain height at position from pathfinding grid
    /// Matches C++ terrain height queries
    pub fn get_terrain_height(
        &self,
        pos: &Coord3D,
        _pathfinding: &crate::ai::pathfinding_system::PathfindingSystem,
    ) -> Real {
        let capabilities = self.to_movement_capabilities();
        let terrain_layer = match capabilities.layer {
            crate::ai::pathfinding_system::PathfindLayerEnum::Ground => {
                crate::common::PathfindLayerEnum::Ground
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Air => {
                crate::common::PathfindLayerEnum::Top
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Water => {
                crate::common::PathfindLayerEnum::Water
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Tunnel => {
                crate::common::PathfindLayerEnum::Tunnel
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Invalid => {
                crate::common::PathfindLayerEnum::Ground
            }
        };

        // Get terrain height from terrain logic.
        match self.template.appearance {
            LocomotorAppearance::Thrust | LocomotorAppearance::Wings => self.preferred_height,
            _ => TheTerrainLogic::get()
                .map(|terrain| terrain.get_layer_height(pos.x, pos.y, terrain_layer))
                .unwrap_or(pos.z),
        }
    }

    /// Helper: normalize angle to [-PI, PI]
    /// Matches C++ normalizeAngle
    fn normalize_angle(angle: Real) -> Real {
        let mut a = angle;
        while a > std::f32::consts::PI {
            a -= 2.0 * std::f32::consts::PI;
        }
        while a < -std::f32::consts::PI {
            a += 2.0 * std::f32::consts::PI;
        }
        a
    }

    /// Helper: standard angle difference
    /// Matches C++ stdAngleDiff
    fn std_angle_diff(angle1: Real, angle2: Real) -> Real {
        Self::normalize_angle(angle1 - angle2)
    }

    fn compute_z_target(&self, current: Coord3D, target: Coord3D) -> Option<Real> {
        let (_ground_z, highest_z, surface_z) = TheTerrainLogic::get()
            .map(|terrain| {
                let mut ground = terrain.get_ground_height(target.x, target.y, None);
                let mut layer = terrain.get_highest_layer_for_destination(&target);
                let mut highest = terrain.get_layer_height(target.x, target.y, layer);
                let mut water_z = 0.0;
                let mut terrain_z = 0.0;
                let _underwater = terrain.is_underwater(
                    target.x,
                    target.y,
                    Some(&mut water_z),
                    Some(&mut terrain_z),
                );

                if self.template.behavior_z == LocomotorBehaviorZ::SmoothRelativeToHighestLayer {
                    let current_layer = terrain.get_layer_for_destination(&current);
                    if current_layer != crate::common::PathfindLayerEnum::Ground {
                        layer = current_layer;
                    } else {
                        layer = terrain.get_highest_layer_for_destination(&current);
                    }
                    ground = terrain.get_ground_height(current.x, current.y, None);
                    highest = terrain.get_layer_height(current.x, current.y, layer);
                }

                let surface = match self.template.appearance {
                    LocomotorAppearance::Thrust
                    | LocomotorAppearance::Wings
                    | LocomotorAppearance::Hover => highest.max(ground),
                    _ => ground,
                };
                (ground, highest.max(ground), surface)
            })
            .unwrap_or((target.z, target.z, target.z));

        let mut desired_z = match self.template.behavior_z {
            LocomotorBehaviorZ::NoZMotiveForce => {
                if self.is_close_enough_dist_3d() {
                    target.z
                } else {
                    return None;
                }
            }
            LocomotorBehaviorZ::SeaLevel => surface_z,
            LocomotorBehaviorZ::AbsoluteHeight | LocomotorBehaviorZ::FixedAbsoluteHeight => {
                self.preferred_height
            }
            LocomotorBehaviorZ::SurfaceRelativeHeight
            | LocomotorBehaviorZ::FixedSurfaceRelativeHeight => surface_z + self.preferred_height,
            LocomotorBehaviorZ::RelativeToGroundAndBuildings
            | LocomotorBehaviorZ::SmoothRelativeToHighestLayer => highest_z + self.preferred_height,
        };

        if self.uses_precise_z_pos() {
            desired_z = target.z;
        }

        if self.preferred_height_damping > 0.0 && !self.uses_precise_z_pos() {
            let delta = desired_z - current.z;
            desired_z = current.z + delta * self.preferred_height_damping;
        }
        if matches!(
            self.template.appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) && self.template.elevator_correction_degree > 0.0
        {
            let max_delta = self
                .template
                .elevator_correction_degree
                .max(0.0)
                .to_radians();
            let z_delta = (desired_z - current.z).clamp(-max_delta, max_delta);
            desired_z = current.z + z_delta;
        }

        Some(desired_z)
    }

    fn is_naval_blocked_at(&self, pos: Coord3D) -> bool {
        if (self.template.surfaces & SURFACE_WATER) == 0 {
            return false;
        }
        if let Some(terrain) = TheTerrainLogic::get() {
            let mut water_z = 0.0;
            let mut terrain_z = 0.0;
            return !terrain.is_underwater(pos.x, pos.y, Some(&mut water_z), Some(&mut terrain_z));
        }
        false
    }

    fn apply_downhill_only(&self, desired_speed: Real, current: Coord3D, target: Coord3D) -> Real {
        if self.template.downhill_only && target.z > current.z + 0.01 {
            0.0
        } else {
            desired_speed
        }
    }

    fn is_tunnel_too_shallow(&self, current: Coord3D, target: Coord3D) -> bool {
        if (self.template.surfaces & SURFACE_CLIFF) == 0 {
            return false;
        }
        if let Some(terrain) = TheTerrainLogic::get() {
            let surface = terrain.get_ground_height(target.x, target.y, None);
            return target.z > surface - 0.5 || current.z > surface - 0.5;
        }
        false
    }

    fn apply_tunnel_depth_constraint(
        &self,
        desired_speed: Real,
        current: Coord3D,
        target: Coord3D,
    ) -> Real {
        if self.is_tunnel_too_shallow(current, target) {
            0.0
        } else {
            desired_speed
        }
    }

    fn apply_jump_slowdown(&self, desired_speed: Real, current: Coord3D, target: Coord3D) -> Real {
        // Jump slowdown applies to infantry-like appearances
        if !matches!(
            self.template.appearance,
            LocomotorAppearance::TwoLegs | LocomotorAppearance::Climber
        ) {
            return desired_speed;
        }
        let dist = (target - current).length();
        if dist < self.template.wander_about_point_radius.max(1.0) {
            desired_speed * 0.5
        } else {
            desired_speed
        }
    }

    fn apply_naval_turn_limit(
        &self,
        desired_speed: Real,
        current_angle: Real,
        desired_angle: Real,
    ) -> Real {
        if (self.template.surfaces & SURFACE_WATER) == 0 {
            return desired_speed;
        }
        let rel = Self::std_angle_diff(desired_angle, current_angle).abs();
        let limit = std::f32::consts::PI / 6.0;
        if rel > limit {
            desired_speed * 0.6
        } else {
            desired_speed
        }
    }

    fn apply_wings_circling(&self, current: Coord3D, target: Coord3D, desired_angle: Real) -> Real {
        if self.template.appearance != LocomotorAppearance::Wings {
            return desired_angle;
        }
        if self.template.circling_radius <= 0.0 {
            return desired_angle;
        }
        let delta = target - current;
        let dist = delta.length();
        if dist <= self.template.circling_radius {
            let base_angle = (delta.y).atan2(delta.x);
            let dir = if self.template.turn_pivot_offset >= 0.0 {
                1.0
            } else {
                -1.0
            };
            return Self::normalize_angle(base_angle + dir * (std::f32::consts::PI / 2.0));
        }
        desired_angle
    }

    fn apply_air_corrections(&self, current_angle: Real, desired_angle: Real) -> Real {
        if !matches!(
            self.template.appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) {
            return desired_angle;
        }
        let rel = Self::std_angle_diff(desired_angle, current_angle);
        let max_deg = self.template.rudder_correction_degree.max(0.0).to_radians();
        if max_deg <= 0.0 {
            return desired_angle;
        }
        let clamped = rel.clamp(-max_deg, max_deg);
        Self::normalize_angle(current_angle + clamped)
    }

    fn desired_angle_with_pivot(
        &self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        is_braking: bool,
    ) -> Real {
        let mut pivot_offset = self.template.turn_pivot_offset;
        if is_braking {
            pivot_offset = 0.0;
        }
        if pivot_offset.abs() < 0.0001 {
            return (goal_pos.y - current_pos.y).atan2(goal_pos.x - current_pos.x);
        }

        // Approximate bounding radius using close-enough distance as a proxy.
        let offset = pivot_offset * self.close_enough_dist.max(1.0);
        let dir_x = current_angle.cos();
        let dir_y = current_angle.sin();
        let turn_x = current_pos.x + dir_x * offset;
        let turn_y = current_pos.y + dir_y * offset;
        let dx = goal_pos.x - turn_x;
        let dy = goal_pos.y - turn_y;
        if dx.abs() < 0.1 && dy.abs() < 0.1 {
            current_angle
        } else {
            dy.atan2(dx)
        }
    }

    fn step_angle(
        &self,
        current_angle: Real,
        desired_angle: Real,
        condition: BodyDamageType,
        delta_time: Real,
    ) -> Real {
        // C++ Locomotor.cpp:2356: when ULTRA_ACCURATE and sliding into place,
        // TURN_NONE is set so the model does not rotate.
        if self.get_flag(FLAG_SLIDING_INTO_PLACE) {
            return current_angle;
        }

        let mut max_turn = self.get_max_turn_rate(condition) * delta_time.max(0.0);
        if matches!(
            self.template.appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) && self.template.rudder_correction_rate > 0.0
        {
            let rudder_limit = self.template.rudder_correction_rate * delta_time.max(0.0);
            if rudder_limit > 0.0 {
                max_turn = max_turn.min(rudder_limit);
            }
        }
        if max_turn <= 0.0 {
            return current_angle;
        }

        let diff = Self::std_angle_diff(desired_angle, current_angle);
        current_angle + diff.clamp(-max_turn, max_turn)
    }

    fn advance_position(
        &self,
        current: Coord3D,
        target: Coord3D,
        angle: Real,
        speed: Real,
        delta_time: Real,
        move_backwards: bool,
    ) -> Coord3D {
        let mut new_pos = current;
        let step = speed.max(0.0) * delta_time.max(0.0);
        if step > 0.0 {
            let dir_sign = if move_backwards { -1.0 } else { 1.0 };
            new_pos.x += angle.cos() * step * dir_sign;
            new_pos.y += angle.sin() * step * dir_sign;
        }

        if let Some(z_target) = self.compute_z_target(current, target) {
            let z_delta = z_target - current.z;
            if z_delta.abs() > f32::EPSILON {
                let mut z_speed = self.template.speed_limit_z.max(0.0);
                if matches!(
                    self.template.appearance,
                    LocomotorAppearance::Wings | LocomotorAppearance::Thrust
                ) && self.template.elevator_correction_rate > 0.0
                {
                    z_speed = z_speed.min(self.template.elevator_correction_rate).max(0.0);
                }
                let z_step = if z_speed > 0.0 {
                    z_delta.signum() * (z_speed * delta_time.max(0.0)).min(z_delta.abs())
                } else {
                    z_delta
                };
                new_pos.z += z_step;
            }
        }

        new_pos
    }

}
