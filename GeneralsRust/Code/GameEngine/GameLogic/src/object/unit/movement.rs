//! Unit movement, path helpers, facing, and animation state.

#![allow(unused_imports)]

use super::ai_helpers::to_locomotor_body_damage_type;
use super::identity::Unit;
use super::imports::*;
use super::registry::dual_world_registry_unavailable;
use super::types::*;

impl Unit {
    /// Update movement based on current state
    pub(super) fn update_movement(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let prev_movement_state = self.movement_state;
        let mut completed_move = false;

        let should_stop_for_dead_ai = self
            .base_arc()
            .read()
            .ok()
            .and_then(|obj_guard| obj_guard.get_ai_update_interface())
            .and_then(|ai| {
                ai.lock()
                    .ok()
                    .map(|ai_guard| ai_guard.is_ai_in_dead_state())
            })
            .unwrap_or(false)
            && self
                .current_locomotor
                .as_ref()
                .and_then(|locomotor| {
                    locomotor
                        .lock()
                        .ok()
                        .map(|loc_guard| !loc_guard.template.locomotor_works_when_dead)
                })
                .unwrap_or(false);
        if should_stop_for_dead_ai {
            self.stop_movement();
            self.target_position = None;
            self.path_following_state = None;
            self.current_path = None;
            self.current_speed = 0.0;
            return Ok(());
        }

        if self.is_movement_active() {
            if let Some(target) = self.target_position {
                // Get position before entering the mutable borrow scope
                let current_pos = self.get_position();
                let current_angle = self.facing_direction;
                let current_speed = self.current_speed;

                // Track whether we need to handle a waypoint after the borrow ends
                let mut handle_waypoint: Option<Coord3D> = None;

                let (desired_speed, condition, _blocked) = {
                    let mut speed = FAST_AS_POSSIBLE;
                    let mut body_condition = BodyDamageType::Pristine;
                    let mut blocked = false;
                    if let Ok(obj_guard) = self.base_arc().read() {
                        if let Some(ai) = obj_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                speed = ai_guard.get_desired_speed();
                                blocked = ai_guard.get_num_frames_blocked() > 0;
                                speed = ai_guard.apply_bump_speed_limit(speed, blocked);
                            }
                        }
                        if let Some(body) = obj_guard.get_body_module() {
                            if let Ok(body_guard) = body.lock() {
                                body_condition =
                                    to_locomotor_body_damage_type(body_guard.get_damage_state());
                            }
                        }
                    }
                    (speed, body_condition, blocked)
                };

                let unit_object_id = self.object_id;
                let base_arc = self.base_arc();
                let current_loco = self.current_locomotor.clone();
                if let (Some(path_state), Some(locomotor)) =
                    (self.path_following_state.as_mut(), current_loco.as_ref())
                {
                    // Clone the locomotor Arc so we don't keep borrowing self
                    let locomotor_clone = locomotor.clone();

                    let ai_store = the_ai(); if let Ok(ai_guard) = ai_store.read() {
                        if let Some(pathfinding) = ai_guard.pathfinding_system() {
                            if let Ok(mut loc_guard) = locomotor_clone.lock() {
                                let current_frame = TheGameLogic::get_frame() as u32;
                                let delta_time =
                                    (delta_time * self.movement_speed_multiplier) as f32;

                                match update_movement_with_pathfinding(
                                    unit_object_id,
                                    &mut loc_guard,
                                    path_state,
                                    &current_pos,
                                    current_angle,
                                    current_speed,
                                    condition,
                                    desired_speed,
                                    current_frame,
                                    delta_time,
                                    pathfinding,
                                ) {
                                    Ok(Some((new_pos, new_angle, new_speed))) => {
                                        if let Ok(mut obj_guard) = base_arc.write() {
                                            let _ = obj_guard.set_position(&new_pos);
                                            let _ = obj_guard.set_orientation(new_angle as Real);
                                            if let Some(physics) = obj_guard.get_physics() {
                                                if let Ok(mut phys_guard) = physics.lock() {
                                                    let delta = new_pos - current_pos;
                                                    let velocity = if delta_time > 0.0 {
                                                        delta / delta_time.max(0.0001)
                                                    } else {
                                                        Vec3D::ZERO
                                                    };
                                                    phys_guard.set_velocity(&velocity);
                                                    if delta_time > 0.0 {
                                                        let mut yaw_delta =
                                                            new_angle - current_angle;
                                                        let two_pi = std::f32::consts::PI * 2.0;
                                                        while yaw_delta > std::f32::consts::PI {
                                                            yaw_delta -= two_pi;
                                                        }
                                                        while yaw_delta < -std::f32::consts::PI {
                                                            yaw_delta += two_pi;
                                                        }
                                                        phys_guard.set_yaw_rate(
                                                            (yaw_delta / delta_time.max(0.0001))
                                                                as Real,
                                                        );
                                                        let turning = if yaw_delta > 0.0 {
                                                            1
                                                        } else if yaw_delta < 0.0 {
                                                            -1
                                                        } else {
                                                            0
                                                        };
                                                        phys_guard.set_turning(turning);
                                                        if matches!(
                                                            loc_guard.get_appearance(),
                                                            LocomotorAppearance::Thrust
                                                                | LocomotorAppearance::Wings
                                                                | LocomotorAppearance::Hover
                                                        ) {
                                                            let pitch_rate = loc_guard
                                                                .template
                                                                .pitch_by_z_vel_coef
                                                                * velocity.z;
                                                            let mut pitch_rate = pitch_rate;
                                                            if loc_guard.template.pitch_stiffness
                                                                > 0.0
                                                            {
                                                                pitch_rate *= loc_guard
                                                                    .template
                                                                    .pitch_stiffness;
                                                            }
                                                            if loc_guard.template.pitch_damping
                                                                > 0.0
                                                            {
                                                                pitch_rate *= (1.0
                                                                    - loc_guard
                                                                        .template
                                                                        .pitch_damping)
                                                                    .clamp(0.0, 1.0);
                                                            }
                                                            phys_guard.set_pitch_rate(pitch_rate);
                                                            let mut roll_rate =
                                                                loc_guard.template.thrust_roll
                                                                    * new_speed;
                                                            if loc_guard.template.roll_stiffness
                                                                > 0.0
                                                            {
                                                                roll_rate *= loc_guard
                                                                    .template
                                                                    .roll_stiffness;
                                                            }
                                                            if loc_guard.template.roll_damping > 0.0
                                                            {
                                                                roll_rate *= (1.0
                                                                    - loc_guard
                                                                        .template
                                                                        .roll_damping)
                                                                    .clamp(0.0, 1.0);
                                                            }
                                                            if loc_guard.template.wobble_rate > 0.0
                                                            {
                                                                let frame =
                                                                    TheGameLogic::get_frame()
                                                                        as f32;
                                                                let phase = (obj_guard.get_id()
                                                                    as f32)
                                                                    * 0.01;
                                                                let wobble_min =
                                                                    loc_guard.template.min_wobble;
                                                                let wobble_max =
                                                                    loc_guard.template.max_wobble;
                                                                let wobble_amp = wobble_max
                                                                    .max(wobble_min)
                                                                    - wobble_min;
                                                                if wobble_amp > 0.0 {
                                                                    let wobble = (frame
                                                                        * loc_guard
                                                                            .template
                                                                            .wobble_rate
                                                                        + phase)
                                                                        .sin()
                                                                        * wobble_amp
                                                                        + wobble_min;
                                                                    roll_rate += wobble;
                                                                }
                                                            }
                                                            phys_guard.set_roll_rate(roll_rate);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        self.facing_direction = new_angle as Real;
                                        self.current_speed = new_speed;
                                        return Ok(());
                                    }
                                    Ok(None) => {
                                        self.movement_state = MovementState::Idle;
                                        self.target_position = None;
                                        self.path_following_state = None;
                                        self.current_speed = 0.0;
                                        completed_move = true;

                                        if !self.waypoint_queue.is_empty() {
                                            let next_waypoint = self.waypoint_queue.remove(0);
                                            handle_waypoint = Some(next_waypoint.position);
                                        }
                                    }
                                    Err(_) => {
                                        self.path_following_state = None;
                                        self.current_speed = 0.0;
                                    }
                                }
                            }
                        }
                    }
                }

                // Handle waypoint outside of the borrow scope
                if let Some(waypoint_pos) = handle_waypoint {
                    self.move_to_position(waypoint_pos, false)?;
                    return Ok(());
                }

                if completed_move && self.target_position.is_none() {
                    // Movement finished during pathfinding update; skip extra movement math.
                    // Completion is handled after the movement state switch below.
                } else {
                    let current_pos = self.get_position();
                    let path_len = self.current_path.as_ref().map(|path| path.len());
                    let (active_target, using_path) = if let Some(path) = &self.current_path {
                        if self.path_index < path.len() {
                            (
                                Coord3D::new(
                                    path[self.path_index].x,
                                    path[self.path_index].y,
                                    target.z,
                                ),
                                true,
                            )
                        } else {
                            (target, false)
                        }
                    } else {
                        (target, false)
                    };

                    let dx = current_pos.x - active_target.x;
                    let dy = current_pos.y - active_target.y;
                    let distance = (dx * dx + dy * dy).sqrt();
                    let reach_distance = if (using_path || !self.waypoint_queue.is_empty())
                        && self.path_extra_distance > 0.0
                    {
                        self.path_extra_distance
                    } else {
                        1.0
                    };

                    if distance < reach_distance {
                        if using_path {
                            self.path_index += 1;
                            if let Some(path_len) = path_len {
                                if self.path_index >= path_len {
                                    self.current_path = None;
                                    let final_dx = current_pos.x - target.x;
                                    let final_dy = current_pos.y - target.y;
                                    let final_distance =
                                        (final_dx * final_dx + final_dy * final_dy).sqrt();
                                    if final_distance < 1.0 {
                                        self.movement_state = MovementState::Idle;
                                        self.target_position = None;
                                        self.current_speed = 0.0;
                                        completed_move = true;

                                        // Process next waypoint if available
                                        if !self.waypoint_queue.is_empty() {
                                            let next_waypoint = self.waypoint_queue.remove(0);
                                            self.move_to_position(next_waypoint.position, false)?;
                                        }
                                    }
                                }
                            }
                        } else {
                            // Close enough
                            self.movement_state = MovementState::Idle;
                            self.target_position = None;
                            self.current_speed = 0.0;
                            completed_move = true;

                            // Process next waypoint if available
                            if !self.waypoint_queue.is_empty() {
                                let next_waypoint = self.waypoint_queue.remove(0);
                                self.move_to_position(next_waypoint.position, false)?;
                            }
                        }
                    } else {
                        // Continue moving towards target
                        if let Some(locomotor) = &self.current_locomotor {
                            if let Ok(mut loc_guard) = locomotor.lock() {
                                let effective_delta = delta_time * self.movement_speed_multiplier;
                                let current = self.get_position();
                                let prev_angle = self.facing_direction;
                                let (new_pos, new_angle, new_speed) = loc_guard.move_towards(
                                    current,
                                    prev_angle,
                                    self.current_speed,
                                    active_target,
                                    desired_speed,
                                    condition,
                                    effective_delta,
                                );
                                self.current_speed = new_speed;
                                if let Ok(mut obj_guard) = self.base_arc().write() {
                                    let _ = obj_guard.set_position(&new_pos);
                                    let _ = obj_guard.set_orientation(new_angle as Real);
                                    if let Some(physics) = obj_guard.get_physics() {
                                        if let Ok(mut phys_guard) = physics.lock() {
                                            let delta = new_pos - current;
                                            let velocity = if effective_delta > 0.0 {
                                                delta / effective_delta.max(0.0001)
                                            } else {
                                                Vec3D::ZERO
                                            };
                                            phys_guard.set_velocity(&velocity);
                                            if effective_delta > 0.0 {
                                                let mut yaw_delta = new_angle - prev_angle;
                                                let two_pi = std::f32::consts::PI * 2.0;
                                                while yaw_delta > std::f32::consts::PI {
                                                    yaw_delta -= two_pi;
                                                }
                                                while yaw_delta < -std::f32::consts::PI {
                                                    yaw_delta += two_pi;
                                                }
                                                phys_guard.set_yaw_rate(
                                                    (yaw_delta / effective_delta.max(0.0001))
                                                        as Real,
                                                );
                                                let turning = if yaw_delta > 0.0 {
                                                    1
                                                } else if yaw_delta < 0.0 {
                                                    -1
                                                } else {
                                                    0
                                                };
                                                phys_guard.set_turning(turning);
                                                if matches!(
                                                    loc_guard.get_appearance(),
                                                    LocomotorAppearance::Thrust
                                                        | LocomotorAppearance::Wings
                                                        | LocomotorAppearance::Hover
                                                ) {
                                                    let pitch_rate =
                                                        loc_guard.template.pitch_by_z_vel_coef
                                                            * velocity.z;
                                                    let mut pitch_rate = pitch_rate;
                                                    if loc_guard.template.pitch_stiffness > 0.0 {
                                                        pitch_rate *=
                                                            loc_guard.template.pitch_stiffness;
                                                    }
                                                    if loc_guard.template.pitch_damping > 0.0 {
                                                        pitch_rate *= (1.0
                                                            - loc_guard.template.pitch_damping)
                                                            .clamp(0.0, 1.0);
                                                    }
                                                    phys_guard.set_pitch_rate(pitch_rate);
                                                    let mut roll_rate =
                                                        loc_guard.template.thrust_roll * new_speed;
                                                    if loc_guard.template.roll_stiffness > 0.0 {
                                                        roll_rate *=
                                                            loc_guard.template.roll_stiffness;
                                                    }
                                                    if loc_guard.template.roll_damping > 0.0 {
                                                        roll_rate *= (1.0
                                                            - loc_guard.template.roll_damping)
                                                            .clamp(0.0, 1.0);
                                                    }
                                                    if loc_guard.template.wobble_rate > 0.0 {
                                                        let frame =
                                                            TheGameLogic::get_frame() as f32;
                                                        let phase =
                                                            (obj_guard.get_id() as f32) * 0.01;
                                                        let wobble_min =
                                                            loc_guard.template.min_wobble;
                                                        let wobble_max =
                                                            loc_guard.template.max_wobble;
                                                        let wobble_amp =
                                                            wobble_max.max(wobble_min) - wobble_min;
                                                        if wobble_amp > 0.0 {
                                                            let wobble = (frame
                                                                * loc_guard.template.wobble_rate
                                                                + phase)
                                                                .sin()
                                                                * wobble_amp
                                                                + wobble_min;
                                                            roll_rate += wobble;
                                                        }
                                                    }
                                                    phys_guard.set_roll_rate(roll_rate);
                                                }
                                            }
                                        }
                                    }
                                }
                                self.facing_direction = new_angle;
                            }
                        }
                    }
                }
            }
        } else if self.movement_state == MovementState::TurningToFace {
            self.current_speed = 0.0;
            let angle_diff = self.desired_facing - self.facing_direction;
            let normalized_diff = Self::normalize_angle(angle_diff);

            if normalized_diff.abs() < 0.1 {
                // Finished turning
                self.facing_direction = self.desired_facing;
                self.movement_state = MovementState::Idle;
            } else {
                // Continue turning
                let turn_amount = self.turn_rate * delta_time;
                if normalized_diff > 0.0 {
                    self.facing_direction += turn_amount.min(normalized_diff);
                } else {
                    self.facing_direction += (-turn_amount).max(normalized_diff);
                }
            }
        } else {
            // Other movement states handled elsewhere
            self.current_speed = 0.0;
        }

        if completed_move {
            match prev_movement_state {
                MovementState::Patrolling => {
                    let order = self.current_order.take();
                    if let Some(UnitOrder::Patrol {
                        waypoints,
                        loop_patrol,
                    }) = order
                    {
                        let _ = self.process_patrol_order(&waypoints, loop_patrol);
                        self.current_order = Some(UnitOrder::Patrol {
                            waypoints,
                            loop_patrol,
                        });
                    }
                }
                MovementState::Following => {
                    let order = self.current_order.take();
                    if let Some(UnitOrder::Follow { target, distance }) = order {
                        let _ = self.process_follow_order(target, distance);
                        self.current_order = Some(UnitOrder::Follow { target, distance });
                    }
                }
                MovementState::Retreating => {
                    if matches!(self.current_order, Some(UnitOrder::Retreat { .. }))
                        && self.target_position.is_none()
                    {
                        self.current_order = None;
                        self.advance_order_queue();
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
    pub(super) fn move_to_position(
        &mut self,
        destination: Coord3D,
        _use_formation: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.target_position = Some(destination);
        self.movement_state = MovementState::Moving;
        self.current_speed = 0.0;
        self.current_path = None;
        self.path_index = 0;
        self.path_following_state = Some(PathFollowingState::new(destination));

        Ok(())
    }
    pub fn get_pathfind_layer(&self) -> PathfindLayerEnum {
        if self.can_fly {
            PathfindLayerEnum::Top
        } else {
            PathfindLayerEnum::Ground
        }
    }
    pub fn get_locomotor_surface_mask(&self) -> Option<LocomotorSurfaceTypeMask> {
        self.current_locomotor
            .as_ref()
            .and_then(|locomotor| locomotor.lock().ok())
            .map(|guard| guard.get_legal_surfaces())
    }
    pub fn get_crusher_level(&self) -> u32 {
        self.base_arc()
            .read()
            .map(|guard| guard.get_crusher_level())
            .unwrap_or(0)
    }
    pub(super) fn stop_movement(&mut self) {
        self.movement_state = MovementState::Idle;
        self.target_position = None;
        self.current_path = None;
        self.path_following_state = None;
        self.current_speed = 0.0;
        self.attack_move_active = false;
        self.path_extra_distance = 0.0;
        self.attack_move_resume_frame = 0;
        self.attack_target_lock_until = 0;
        self.waypoint_queue.clear();
    }
    pub(super) fn is_movement_active(&self) -> bool {
        matches!(
            self.movement_state,
            MovementState::Moving
                | MovementState::Following
                | MovementState::Patrolling
                | MovementState::Guarding
                | MovementState::Pursuing
                | MovementState::Retreating
                | MovementState::Backing
                | MovementState::Fleeing
        )
    }
    pub(super) fn normalize_angle(angle: Real) -> Real {
        use std::f32::consts::PI;
        let mut result = angle;
        while result > PI {
            result -= 2.0 * PI;
        }
        while result < -PI {
            result += 2.0 * PI;
        }
        result
    }
    pub(super) fn return_to_formation_position(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 258: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let leader_id = match self.group_leader {
            Some(id) => id,
            None => {
                self.return_to_formation = false;
                return Ok(());
            }
        };
        let Some(leader_pos) =
            crate::object::registry::OBJECT_REGISTRY.with_object(leader_id, |g| *g.get_position())
        else {
            self.group_leader = None;
            self.return_to_formation = false;
            return Ok(());
        };
        let current_pos = self.get_position();
        let dx = leader_pos.x - current_pos.x;
        let dy = leader_pos.y - current_pos.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance > self.follow_distance && self.can_move() && !self.is_movement_active() {
            self.move_to_position(leader_pos, false)?;
        } else if distance <= self.follow_distance {
            self.return_to_formation = false;
        }
        Ok(())
    }
    pub(super) fn update_facing(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let target_angle = self.desired_facing;
        let current = self.facing_direction;
        let mut delta = target_angle - current;
        let two_pi = std::f32::consts::PI * 2.0;
        while delta > std::f32::consts::PI {
            delta -= two_pi;
        }
        while delta < -std::f32::consts::PI {
            delta += two_pi;
        }
        let max_turn = (self.turn_rate.max(0.0) * delta_time).max(0.0);
        let adjust = delta.clamp(-max_turn, max_turn);
        let new_angle = current + adjust;
        self.facing_direction = new_angle;
        if let Ok(mut obj_guard) = self.base_arc().write() {
            let _ = obj_guard.set_orientation(new_angle as Real);
        }
        Ok(())
    }
    pub(super) fn check_status_effects(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let contained_by = self
            .base_arc()
            .read()
            .ok()
            .and_then(|guard| guard.get_contained_by());
        match contained_by {
            Some(container) => {
                self.is_garrisoned = true;
                self.garrison_building = Some(container);
                if matches!(self.current_order, Some(UnitOrder::Garrison { .. })) {
                    self.current_order = None;
                    self.advance_order_queue();
                }
                self.stop_movement();
            }
            None => {
                self.is_garrisoned = false;
                self.garrison_building = None;
            }
        }
        Ok(())
    }
    pub(super) fn update_animation_state(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut obj_guard) = self.base_arc().write() {
            match self.movement_state {
                MovementState::Moving
                | MovementState::TurningToFace
                | MovementState::Following
                | MovementState::Patrolling
                | MovementState::Guarding
                | MovementState::Pursuing
                | MovementState::Retreating
                | MovementState::Backing
                | MovementState::Fleeing => {
                    obj_guard.set_model_condition_state(ModelConditionFlags::MOVING);
                }
                MovementState::Idle | MovementState::Attacking => {
                    obj_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
            }
            if matches!(self.movement_state, MovementState::Attacking) {
                obj_guard.set_model_condition_state(ModelConditionFlags::ATTACKING);
            } else {
                obj_guard.clear_model_condition_state(ModelConditionFlags::ATTACKING);
            }
        }
        Ok(())
    }
}
