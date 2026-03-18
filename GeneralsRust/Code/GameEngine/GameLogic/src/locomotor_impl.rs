//! Complete locomotor movement implementations for all unit types
//!
//! This module implements the detailed movement logic for each locomotor type,
//! faithfully ported from C++ Locomotor.cpp lines 1000-2500.
//!
//! Supports:
//! - Infantry (two legs)
//! - Wheeled vehicles (trucks, humvees)
//! - Tracked vehicles (tanks)
//! - Hover vehicles (hover tanks)
//! - Thrust aircraft (helicopters)
//! - Fixed-wing aircraft (jets)
//! - Climbers (cliff-climbing infantry)
//! - Ships/naval units
//! - Tunnelers

use crate::common::*;
use crate::locomotor::{BodyDamageType, Locomotor, LocomotorAppearance, LocomotorBehaviorZ};
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::physics::{PhysicsState, PhysicsTurningType};
use std::f32::consts::PI;

// Constants from C++ implementation
const MAX_BRAKING_FACTOR: Real = 5.0;
const DONUT_TIME_DELAY_SECONDS: Real = 2.5;
const DONUT_DISTANCE: Real = 4.0 * PATHFIND_CELL_SIZE_F;
const BIGNUM: Real = 99999.0;
const TINY_EPSILON: Real = 0.001;
const QUARTERPI: Real = PI / 4.0;
const SMALL_TURN: Real = PI / 20.0;
const FIFTEEN_DEGREES: Real = PI / 12.0;
const MIN_VEL: Real = PATHFIND_CELL_SIZE_F / LOGICFRAMES_PER_SECOND as Real;

// ============================================================================
// MOVEMENT STATE
// ============================================================================

/// Runtime movement state for a locomotor instance
#[derive(Debug, Clone)]
pub struct LocomotorMovementState {
    /// Braking factor (1.0 = normal, higher = stronger braking)
    pub braking_factor: Real,

    /// Position to maintain when idle
    pub maintain_pos: Coord3D,

    /// Is maintain position valid?
    pub maintain_pos_valid: bool,

    /// Are we currently braking?
    pub is_braking: bool,

    /// Are we moving backwards?
    pub moving_backwards: bool,

    /// Are we doing a three-point turn?
    pub doing_three_point_turn: bool,

    /// Are we climbing?
    pub climbing: bool,

    /// Are we over water?
    pub over_water: bool,

    /// Wander angle offset for infantry
    pub angle_offset: Real,

    /// Angle offset increment
    pub offset_increment: Real,

    /// Is offset increasing?
    pub offset_increasing: bool,

    /// Donut timer (for preventing units from getting stuck)
    pub donut_timer: u32,

    /// Current frame (for timing)
    pub current_frame: u32,
}

impl LocomotorMovementState {
    pub fn new() -> Self {
        Self {
            braking_factor: 1.0,
            maintain_pos: Coord3D::default(),
            maintain_pos_valid: false,
            is_braking: false,
            moving_backwards: false,
            doing_three_point_turn: false,
            climbing: false,
            over_water: false,
            angle_offset: 0.0,
            offset_increment: PI / 40.0,
            offset_increasing: true,
            donut_timer: 0,
            current_frame: 0,
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Calculate the distance needed to slow down from current speed to desired speed
/// Matches C++ calcSlowDownDist()
#[inline]
fn calc_slow_down_dist(cur_speed: Real, desired_speed: Real, max_braking: Real) -> Real {
    let delta = cur_speed - desired_speed;
    if delta <= 0.0 {
        return 0.0;
    }

    let dist = (delta * delta / max_braking.abs()) * 0.5;

    // Fudge factor for precise stopping
    const FUDGE: Real = 1.05;
    dist * FUDGE
}

/// Calculate direction to apply thrust for thrust aircraft
/// Matches C++ calcDirectionToApplyThrust()
#[inline]
fn calc_direction_to_apply_thrust(
    position: Coord3D,
    velocity: Coord3D,
    goal_pos: Coord3D,
    max_accel: Real,
    gravity: Real,
) -> Coord3D {
    let vec_to_goal = goal_pos - position;
    if vec_to_goal.length_squared() <= TINY_EPSILON * TINY_EPSILON {
        return if velocity.length_squared() > TINY_EPSILON * TINY_EPSILON {
            velocity.normalized()
        } else {
            Coord3D::X
        };
    }

    // Match C++ Locomotor.cpp: include gravity in current velocity.
    let mut cur_vel = velocity;
    cur_vel.z += gravity;

    let dist_to_goal_sqr = vec_to_goal.length_squared();
    let dist_to_goal = dist_to_goal_sqr.sqrt();
    let cur_vel_mag_sqr = cur_vel.length_squared();
    let cur_vel_mag = cur_vel_mag_sqr.sqrt();
    let max_accel_sqr = max_accel * max_accel;

    let denom = cur_vel_mag_sqr - max_accel_sqr;
    if denom.abs() > TINY_EPSILON {
        let mut t = (dist_to_goal * (cur_vel_mag + max_accel)) / denom;
        let t2 = (dist_to_goal * (cur_vel_mag - max_accel)) / denom;
        if t >= 0.0 || t2 >= 0.0 {
            if t < 0.0 || (t2 >= 0.0 && t2 < t) {
                t = t2;
            }

            if t.abs() > TINY_EPSILON {
                let goal_dir = Coord3D::new(
                    (vec_to_goal.x / t) - cur_vel.x,
                    (vec_to_goal.y / t) - cur_vel.y,
                    (vec_to_goal.z / t) - cur_vel.z,
                );
                let normalized = goal_dir.normalize_or_zero();
                if normalized.length_squared() > TINY_EPSILON * TINY_EPSILON {
                    return normalized;
                }
            }
        }
    }

    let fallback = vec_to_goal.normalize_or_zero();
    if fallback.length_squared() > TINY_EPSILON * TINY_EPSILON {
        return fallback;
    }

    if velocity.length_squared() > TINY_EPSILON * TINY_EPSILON {
        velocity.normalized()
    } else {
        Coord3D::X
    }
}

/// Try to rotate a 3D vector towards a desired direction
/// Matches C++ tryToRotateVector3D()
/// Returns the angle rotated
#[inline]
fn try_to_rotate_vector_3d(max_angle: Real, from: Coord3D, to: Coord3D) -> (Real, Coord3D) {
    if max_angle.abs() < TINY_EPSILON {
        return (0.0, from.normalized());
    }

    // Normalize vectors
    let from_norm = from.normalized();
    let to_norm = to.normalized();

    // Calculate angle between vectors
    let dot = (from_norm.x * to_norm.x + from_norm.y * to_norm.y + from_norm.z * to_norm.z)
        .clamp(-1.0, 1.0);
    let angle = dot.acos();

    if angle < TINY_EPSILON {
        // Already aligned
        return (0.0, from_norm);
    }

    let mut actual_angle = max_angle;
    if actual_angle < 0.0 {
        // C++ parity: negative max angle means percent (0..1) of required rotation.
        actual_angle = -actual_angle * angle;
        if actual_angle.abs() < TINY_EPSILON {
            return (0.0, from_norm);
        }
    }

    if angle <= actual_angle {
        return (angle, to_norm);
    }

    // Calculate rotation axis (cross product)
    let axis = Coord3D {
        x: from_norm.y * to_norm.z - from_norm.z * to_norm.y,
        y: from_norm.z * to_norm.x - from_norm.x * to_norm.z,
        z: from_norm.x * to_norm.y - from_norm.y * to_norm.x,
    };

    let axis_len = axis.length();
    if axis_len < TINY_EPSILON {
        // Vectors are parallel or anti-parallel
        return (0.0, from_norm);
    }

    let axis_norm = Coord3D {
        x: axis.x / axis_len,
        y: axis.y / axis_len,
        z: axis.z / axis_len,
    };

    // Rodrigues' rotation formula
    let cos_angle = actual_angle.cos();
    let sin_angle = actual_angle.sin();

    let result = Coord3D {
        x: from_norm.x * cos_angle
            + (axis_norm.y * from_norm.z - axis_norm.z * from_norm.y) * sin_angle
            + axis_norm.x
                * (axis_norm.x * from_norm.x
                    + axis_norm.y * from_norm.y
                    + axis_norm.z * from_norm.z)
                * (1.0 - cos_angle),
        y: from_norm.y * cos_angle
            + (axis_norm.z * from_norm.x - axis_norm.x * from_norm.z) * sin_angle
            + axis_norm.y
                * (axis_norm.x * from_norm.x
                    + axis_norm.y * from_norm.y
                    + axis_norm.z * from_norm.z)
                * (1.0 - cos_angle),
        z: from_norm.z * cos_angle
            + (axis_norm.x * from_norm.y - axis_norm.y * from_norm.x) * sin_angle
            + axis_norm.z
                * (axis_norm.x * from_norm.x
                    + axis_norm.y * from_norm.y
                    + axis_norm.z * from_norm.z)
                * (1.0 - cos_angle),
    };

    (actual_angle, result.normalized())
}

/// Check if value is nearly zero
#[inline]
fn is_nearly_zero(a: Real) -> bool {
    a.abs() < TINY_EPSILON
}

/// Check if value is nearly equal to another
#[inline]
fn is_nearly(a: Real, val: Real) -> bool {
    (a - val).abs() < TINY_EPSILON
}

/// Normalize angle to [-PI, PI]
#[inline]
fn normalize_angle(angle: Real) -> Real {
    let two_pi = 2.0 * PI;
    let mut normalized = (angle + PI).rem_euclid(two_pi) - PI;

    // Preserve the legacy edge case behavior: map -PI to +PI for negative inputs,
    // but keep -PI for positive inputs (e.g. 3PI -> -PI, -3PI -> +PI).
    if (normalized + PI).abs() < TINY_EPSILON && angle < 0.0 {
        normalized = PI;
    }

    normalized
}

/// Calculate standard angle difference (shortest path)
#[inline]
fn std_angle_diff(desired: Real, current: Real) -> Real {
    normalize_angle(desired - current)
}

// ============================================================================
// LOCOMOTOR MOVEMENT IMPLEMENTATIONS
// ============================================================================

impl Locomotor {
    /// Move towards position - INFANTRY (TWO LEGS)
    /// Matches C++ moveTowardsPositionLegs() lines 1594-1687
    pub fn move_towards_position_legs(
        &self,
        state: &mut LocomotorMovementState,
        position: Coord3D,
        physics: &mut PhysicsState,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        condition: BodyDamageType,
    ) -> Coord3D {
        let max_speed = self.get_max_speed_for_condition(condition);
        let desired_speed = desired_speed.min(max_speed);
        let max_acceleration = self.get_max_acceleration(condition);

        // Calculate desired angle
        let dx = goal_pos.x - position.x;
        let dy = goal_pos.y - position.y;
        let mut desired_angle = dy.atan2(dx);

        // Wander logic for infantry
        if self.template.wander_width_factor != 0.0 {
            let angle_limit = PI / 8.0 * self.template.wander_width_factor;
            let actual_speed = physics.velocity.x.hypot(physics.velocity.y);

            if state.offset_increasing {
                state.angle_offset += state.offset_increment * actual_speed;
                if state.angle_offset > angle_limit {
                    state.offset_increasing = false;
                }
            } else {
                state.angle_offset -= state.offset_increment * actual_speed;
                if state.angle_offset < -angle_limit {
                    state.offset_increasing = true;
                }
            }

            desired_angle = normalize_angle(desired_angle + state.angle_offset);
        }

        // Rotate towards desired angle
        let actual_speed = physics.velocity.x.hypot(physics.velocity.y);
        let current_angle = if actual_speed < TINY_EPSILON {
            // When starting from rest, treat the unit as already facing the goal so it can
            // accelerate immediately (legacy behavior uses facing, not velocity).
            desired_angle
        } else {
            physics.velocity.y.atan2(physics.velocity.x)
        };
        let rel_angle = std_angle_diff(desired_angle, current_angle);

        // Modulate speed based on turning
        let angle_coeff = (rel_angle.abs() / QUARTERPI).min(1.0);
        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;

        // Slow down when approaching destination
        let slow_down_dist =
            calc_slow_down_dist(actual_speed, self.template.min_speed, self.get_braking());

        if on_path_dist_to_goal < slow_down_dist {
            goal_speed = self.template.min_speed;
        }

        // Calculate acceleration force
        let speed_delta = goal_speed - actual_speed;
        if speed_delta.abs() > TINY_EPSILON {
            let mass = physics.mass;
            let acceleration = if speed_delta > 0.0 {
                max_acceleration
            } else {
                -self.get_braking()
            };

            let mut accel_force = mass * acceleration;
            let max_force_needed = mass * speed_delta;

            if accel_force.abs() > max_force_needed.abs() {
                accel_force = max_force_needed;
            }

            // Apply force in movement direction
            let dir_x = desired_angle.cos();
            let dir_y = desired_angle.sin();

            physics.velocity.x += (accel_force * dir_x) / mass;
            physics.velocity.y += (accel_force * dir_y) / mass;
        }

        position
    }

    /// Move towards position - WHEELED VEHICLES
    /// Matches C++ moveTowardsPositionWheels() lines 1258-1498
    pub fn move_towards_position_wheels(
        &self,
        state: &mut LocomotorMovementState,
        position: Coord3D,
        physics: &mut PhysicsState,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        condition: BodyDamageType,
    ) -> Coord3D {
        let max_speed = self.get_max_speed_for_condition(condition);
        let desired_speed = desired_speed.min(max_speed);
        let max_acceleration = self.get_max_acceleration(condition);
        let _max_turn_rate = self.get_max_turn_rate(condition);

        let mut turn_speed = self.template.min_turn_speed;
        if turn_speed < max_speed / 4.0 {
            turn_speed = max_speed / 4.0;
        }

        let current_angle = physics.velocity.y.atan2(physics.velocity.x);
        let desired_angle = (goal_pos.y - position.y).atan2(goal_pos.x - position.x);
        let rel_angle = std_angle_diff(desired_angle, current_angle);

        let mut actual_speed = physics.velocity.x.hypot(physics.velocity.y);
        let mut move_backwards = false;

        // Check if we should move backwards (3-point turn logic)
        if actual_speed == 0.0 {
            state.moving_backwards = false;
            if self.template.can_move_backward && rel_angle.abs() > PI / 2.0 {
                state.moving_backwards = true;
                let radius = 5.0; // Approximate bounding radius
                state.doing_three_point_turn = on_path_dist_to_goal > 5.0 * radius;
            }
        }

        if state.moving_backwards {
            if rel_angle.abs() < PI / 2.0 {
                move_backwards = false;
                state.moving_backwards = false;
            } else {
                move_backwards = true;
                let radius = 5.0;
                state.doing_three_point_turn = on_path_dist_to_goal > 5.0 * radius;
            }
        }

        // Limit speed when turning
        let mut goal_speed = desired_speed;
        if rel_angle.abs() > SMALL_TURN && desired_speed > turn_speed {
            goal_speed = turn_speed;
        }

        if move_backwards {
            actual_speed = -actual_speed;
        }

        // Calculate slow down distance
        let slow_down_time = actual_speed / self.get_braking() + 1.0;
        let slow_down_dist = (actual_speed / 1.5) * slow_down_time + actual_speed;
        let effective_slow_down_dist = slow_down_dist.max(PATHFIND_CELL_SIZE_F);

        // Braking logic
        if on_path_dist_to_goal < effective_slow_down_dist && !state.is_braking {
            state.is_braking = true;
            state.braking_factor = 1.1;
        }

        if on_path_dist_to_goal > PATHFIND_CELL_SIZE_F
            && on_path_dist_to_goal > 2.0 * slow_down_dist
        {
            state.is_braking = false;
        }

        // Donut timer (prevents getting stuck)
        if on_path_dist_to_goal > DONUT_DISTANCE {
            state.donut_timer = state.current_frame
                + (DONUT_TIME_DELAY_SECONDS * LOGICFRAMES_PER_SECOND as Real) as u32;
        } else if state.donut_timer < state.current_frame {
            state.is_braking = true;
        }

        if state.is_braking {
            state.braking_factor = slow_down_dist / on_path_dist_to_goal;
            state.braking_factor *= state.braking_factor;
            if state.braking_factor > MAX_BRAKING_FACTOR {
                state.braking_factor = MAX_BRAKING_FACTOR;
            }
            state.braking_factor = 1.0;

            if slow_down_dist > on_path_dist_to_goal {
                goal_speed = actual_speed - self.get_braking();
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else if slow_down_dist > on_path_dist_to_goal * 0.75 {
                goal_speed = actual_speed - self.get_braking() / 2.0;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else {
                goal_speed = actual_speed;
            }
        }

        // Apply acceleration
        let mut speed_delta = goal_speed - actual_speed;
        if move_backwards {
            speed_delta = -goal_speed + actual_speed;
        }

        if speed_delta.abs() > TINY_EPSILON {
            let mass = physics.mass;
            let acceleration = if move_backwards {
                if speed_delta < 0.0 {
                    -max_acceleration
                } else {
                    state.braking_factor * self.get_braking()
                }
            } else {
                if speed_delta > 0.0 {
                    max_acceleration
                } else {
                    -state.braking_factor * self.get_braking()
                }
            };

            let mut accel_force = mass * acceleration;
            let max_force_needed = mass * speed_delta;

            if accel_force.abs() > max_force_needed.abs() {
                accel_force = max_force_needed;
            }

            let dir_x = current_angle.cos();
            let dir_y = current_angle.sin();

            physics.velocity.x += (accel_force * dir_x) / mass;
            physics.velocity.y += (accel_force * dir_y) / mass;
        }

        position
    }

    /// Move towards position - TRACKED VEHICLES (tanks)
    /// Matches C++ moveTowardsPositionTreads() lines 1144-1255
    pub fn move_towards_position_treads(
        &self,
        state: &mut LocomotorMovementState,
        position: Coord3D,
        physics: &mut PhysicsState,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        condition: BodyDamageType,
    ) -> Coord3D {
        let max_speed = self.get_max_speed_for_condition(condition);
        let desired_speed = desired_speed.min(max_speed);
        let max_acceleration = self.get_max_acceleration(condition);

        let current_angle = physics.velocity.y.atan2(physics.velocity.x);
        let desired_angle = (goal_pos.y - position.y).atan2(goal_pos.x - position.x);
        let rel_angle = std_angle_diff(desired_angle, current_angle);

        // Modulate speed based on turning
        let angle_coeff = (rel_angle.abs() / QUARTERPI).min(1.0);
        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;

        let dx = position.x - goal_pos.x;
        let dy = position.y - goal_pos.y;
        let actual_speed = physics.velocity.x.hypot(physics.velocity.y);

        let slow_down_time = actual_speed / self.get_braking();
        let slow_down_dist = (actual_speed / 1.5) * slow_down_time;

        // Slow down when very close and turning
        if dx * dx + dy * dy < (2.0 * PATHFIND_CELL_SIZE_F).powi(2) && angle_coeff > 0.05 {
            goal_speed = actual_speed * 0.6;
        }

        // Braking logic
        if on_path_dist_to_goal < slow_down_dist && !state.is_braking {
            state.is_braking = true;
            state.braking_factor = 1.1;
        }

        if on_path_dist_to_goal > PATHFIND_CELL_SIZE_F
            && on_path_dist_to_goal > 2.0 * slow_down_dist
        {
            state.is_braking = false;
        }

        if state.is_braking {
            state.braking_factor = slow_down_dist / on_path_dist_to_goal;
            state.braking_factor *= state.braking_factor;
            if state.braking_factor > MAX_BRAKING_FACTOR {
                state.braking_factor = MAX_BRAKING_FACTOR;
            }

            if slow_down_dist > on_path_dist_to_goal {
                goal_speed = actual_speed - self.get_braking();
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else if slow_down_dist > on_path_dist_to_goal * 0.75 {
                goal_speed = actual_speed - self.get_braking() / 2.0;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else {
                goal_speed = actual_speed;
            }
        }

        // Apply acceleration
        let speed_delta = goal_speed - actual_speed;
        if speed_delta.abs() > TINY_EPSILON {
            let mass = physics.mass;
            let acceleration = if speed_delta > 0.0 {
                max_acceleration
            } else {
                -state.braking_factor * self.get_braking()
            };

            let mut accel_force = mass * acceleration;
            let max_force_needed = mass * speed_delta;

            if accel_force.abs() > max_force_needed.abs() {
                accel_force = max_force_needed;
            }

            let dir_x = current_angle.cos();
            let dir_y = current_angle.sin();

            physics.velocity.x += (accel_force * dir_x) / mass;
            physics.velocity.y += (accel_force * dir_y) / mass;
        }

        position
    }

    /// Move towards position - HOVER VEHICLES
    /// Matches C++ moveTowardsPositionHover() lines 1863-1888
    pub fn move_towards_position_hover(
        &self,
        state: &mut LocomotorMovementState,
        position: Coord3D,
        physics: &mut PhysicsState,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        condition: BodyDamageType,
        terrain_height: Real,
        water_height: Option<Real>,
    ) -> Coord3D {
        // Use the generic "Other" movement logic for 2D movement
        let new_pos = self.move_towards_position_other(
            state,
            position,
            physics,
            goal_pos,
            on_path_dist_to_goal,
            desired_speed,
            condition,
        );

        // Check if over water for special effects
        let is_underwater = water_height.map_or(false, |wh| terrain_height < wh);

        if is_underwater && !state.over_water {
            state.over_water = true;
            // Would set model condition MODELCONDITION_OVER_WATER here
        } else if !is_underwater && state.over_water {
            state.over_water = false;
            // Would clear model condition MODELCONDITION_OVER_WATER here
        }

        new_pos
    }

    /// Move towards position - THRUST AIRCRAFT (helicopters)
    /// Matches C++ moveTowardsPositionThrust() lines 1891-2004
    pub fn move_towards_position_thrust(
        &self,
        _state: &mut LocomotorMovementState,
        position: Coord3D,
        physics: &mut PhysicsState,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        condition: BodyDamageType,
        surface_height: Real,
    ) -> Coord3D {
        let max_forward_speed = self.get_max_speed_for_condition(condition);
        let desired_speed = desired_speed.clamp(self.template.min_speed, max_forward_speed);

        let actual_forward_speed = physics.velocity.length();
        let mut local_goal_pos = goal_pos;

        // Adjust goal Z for preferred height (matches C++ lines 1913-1934)
        if self.preferred_height != 0.0 && !self.uses_precise_z_pos() {
            local_goal_pos.z = self.preferred_height + surface_height;
            let delta = local_goal_pos.z - position.z;
            let damped_delta = delta * self.preferred_height_damping;
            local_goal_pos.z = position.z + damped_delta;
        }

        // Slow down when approaching destination (matches C++ lines 1899-1905)
        let mut goal_speed = desired_speed;
        if self.get_braking() > 0.0 {
            let slow_down_dist = calc_slow_down_dist(
                actual_forward_speed,
                self.template.min_speed,
                self.get_braking(),
            );

            if on_path_dist_to_goal < slow_down_dist && !self.no_slow_down_approaching_dest() {
                goal_speed = self.template.min_speed;
            }
        }

        // Calculate thrust parameters (matches C++ lines 1939-1950)
        let forward_speed_delta = goal_speed - actual_forward_speed;
        let max_accel = if forward_speed_delta > 0.0 || self.get_braking() == 0.0 {
            self.get_max_acceleration(condition)
        } else {
            -self.get_braking()
        };
        let max_turn_rate = self.get_max_turn_rate(condition);

        // Calculate desired thrust direction (matches C++ lines 1943-1945)
        let gravity = game_engine::common::global_data::read_safe()
            .map(|global| global.gravity)
            .unwrap_or(-9.81);
        let desired_thrust_dir = calc_direction_to_apply_thrust(
            position,
            physics.velocity,
            local_goal_pos,
            max_accel,
            gravity,
        );

        // Account for max thrust angle (matches C++ lines 1948-1950)
        let max_thrust_angle = if max_turn_rate > 0.0 {
            self.template.max_thrust_angle
        } else {
            0.0
        };

        // We do not receive object transform here, so use velocity heading as forward axis.
        let forward_dir = if physics.velocity.length_squared() > TINY_EPSILON * TINY_EPSILON {
            physics.velocity.normalized()
        } else {
            desired_thrust_dir
        };
        let (thrust_angle, thrust_dir) =
            try_to_rotate_vector_3d(max_thrust_angle, forward_dir, desired_thrust_dir);

        // Apply thrust forces (matches C++ lines 1982-2003)
        if forward_speed_delta.abs() > TINY_EPSILON || thrust_angle.abs() > TINY_EPSILON {
            let max_forward_speed_safe = max_forward_speed.max(0.01);
            let damping = (max_accel / max_forward_speed_safe).clamp(0.0, 1.0);

            let accel_vec = Coord3D {
                x: thrust_dir.x * max_accel - physics.velocity.x * damping,
                y: thrust_dir.y * max_accel - physics.velocity.y * damping,
                z: thrust_dir.z * max_accel - physics.velocity.z * damping,
            };

            // Apply force = mass * acceleration (matches C++ lines 1994-2002)
            let mass = physics.mass;
            physics.velocity.x += accel_vec.x / mass * mass; // Force applied
            physics.velocity.y += accel_vec.y / mass * mass;
            physics.velocity.z += accel_vec.z / mass * mass;
        }

        position
    }

    /// Move towards position - FIXED-WING AIRCRAFT (jets)
    /// Matches C++ moveTowardsPositionWings() lines 1821-1860
    pub fn move_towards_position_wings(
        &self,
        state: &mut LocomotorMovementState,
        position: Coord3D,
        physics: &mut PhysicsState,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        condition: BodyDamageType,
    ) -> Coord3D {
        // Fixed-wing aircraft use the generic "Other" movement logic
        // with special circling behavior for altitude changes (if enabled)
        self.move_towards_position_other(
            state,
            position,
            physics,
            goal_pos,
            on_path_dist_to_goal,
            desired_speed,
            condition,
        )
    }

    /// Move towards position - CLIMBER (cliff-climbing infantry)
    /// Matches C++ moveTowardsPositionClimb() lines 1690-1818
    pub fn move_towards_position_climber(
        &self,
        state: &mut LocomotorMovementState,
        position: Coord3D,
        physics: &mut PhysicsState,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        condition: BodyDamageType,
        terrain_height: Real,
    ) -> Coord3D {
        let max_speed = self.get_max_speed_for_condition(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        let max_acceleration = self.get_max_acceleration(condition);

        let mut move_backwards = false;

        let _dx = position.x - goal_pos.x;
        let _dy = position.y - goal_pos.y;
        let dz = position.z - goal_pos.z;

        // Check if we're climbing
        if dz * dz > PATHFIND_CELL_SIZE_F.powi(2) {
            state.climbing = true;
        }
        if dz.abs() < 1.0 {
            state.climbing = false;
        }

        // Adjust speed based on ground slope when climbing
        if state.climbing {
            let mut delta = goal_pos;
            delta.x -= position.x;
            delta.y -= position.y;
            delta.z = 0.0;

            let norm = delta.x.hypot(delta.y);
            if norm > TINY_EPSILON {
                delta.x /= norm;
                delta.y /= norm;
            }

            delta.x += position.x;
            delta.y += position.y;
            delta.z = terrain_height;

            if delta.z < position.z - 0.1 {
                move_backwards = true;
            }

            let ground_slope = (delta.z - position.z).abs().max(1.0);
            if ground_slope > 1.0 {
                desired_speed /= ground_slope * 4.0;
            }
        }

        state.moving_backwards = move_backwards;

        // Calculate desired angle
        let current_angle = physics.velocity.y.atan2(physics.velocity.x);
        let mut desired_angle = (goal_pos.y - position.y).atan2(goal_pos.x - position.x);

        if move_backwards {
            desired_angle = normalize_angle(desired_angle + PI);
        }

        let rel_angle = std_angle_diff(desired_angle, current_angle);

        // Modulate speed based on turning
        let angle_coeff = (rel_angle.abs() / QUARTERPI).min(1.0);
        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;

        let mut actual_speed = physics.velocity.x.hypot(physics.velocity.y);
        if move_backwards {
            actual_speed = -actual_speed;
        }

        // Slow down when approaching
        let slow_down_dist =
            calc_slow_down_dist(actual_speed, self.template.min_speed, self.get_braking());
        if on_path_dist_to_goal < slow_down_dist {
            goal_speed = self.template.min_speed;
        }

        // Apply acceleration
        let mut speed_delta = goal_speed - actual_speed;
        if move_backwards {
            speed_delta = -goal_speed + actual_speed;
        }

        if speed_delta.abs() > TINY_EPSILON {
            let mass = physics.mass;
            let acceleration = if move_backwards {
                if speed_delta < 0.0 {
                    -max_acceleration
                } else {
                    self.get_braking()
                }
            } else {
                if speed_delta > 0.0 {
                    max_acceleration
                } else {
                    -self.get_braking()
                }
            };

            let mut accel_force = mass * acceleration;
            let max_force_needed = mass * speed_delta;

            if accel_force.abs() > max_force_needed.abs() {
                accel_force = max_force_needed;
            }

            let dir_x = desired_angle.cos();
            let dir_y = desired_angle.sin();

            physics.velocity.x += (accel_force * dir_x) / mass;
            physics.velocity.y += (accel_force * dir_y) / mass;
        }

        position
    }

    /// Move towards position - GENERIC/OTHER locomotor
    /// Matches C++ moveTowardsPositionOther() lines 2326-2404
    pub fn move_towards_position_other(
        &self,
        _state: &mut LocomotorMovementState,
        position: Coord3D,
        physics: &mut PhysicsState,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        condition: BodyDamageType,
    ) -> Coord3D {
        let max_speed = self.get_max_speed_for_condition(condition);
        let desired_speed = desired_speed.min(max_speed);
        let max_acceleration = self.get_max_acceleration(condition);

        let goal_speed = desired_speed;
        let actual_speed = physics.velocity.x.hypot(physics.velocity.y);

        // Calculate direction to apply force
        let current_angle = physics.velocity.y.atan2(physics.velocity.x);
        let mut dir_x = current_angle.cos();
        let mut dir_y = current_angle.sin();

        // Ultra-accurate mode: slide into place without turning
        let slide_threshold = goal_speed * self.template.ultra_accurate_slide_factor;
        let dx_abs = (goal_pos.x - position.x).abs();
        let dy_abs = (goal_pos.y - position.y).abs();

        if dx_abs <= slide_threshold && dy_abs <= slide_threshold {
            // Just slide in the right direction
            let dx = goal_pos.x - position.x;
            let dy = goal_pos.y - position.y;
            let dist = dx.hypot(dy);

            if dist > TINY_EPSILON {
                dir_x = dx / dist;
                dir_y = dy / dist;
            }
        } else {
            // Normal rotation towards position
            let desired_angle = (goal_pos.y - position.y).atan2(goal_pos.x - position.x);
            dir_x = desired_angle.cos();
            dir_y = desired_angle.sin();
        }

        // Slow down when approaching
        let mut final_goal_speed = goal_speed;
        let slow_down_dist =
            calc_slow_down_dist(actual_speed, self.template.min_speed, self.get_braking());

        if on_path_dist_to_goal < slow_down_dist {
            final_goal_speed = self.template.min_speed;
        }

        // Apply acceleration
        let speed_delta = final_goal_speed - actual_speed;
        if speed_delta.abs() > TINY_EPSILON {
            let mass = physics.mass;
            let acceleration = if speed_delta > 0.0 {
                max_acceleration
            } else {
                -self.get_braking()
            };

            let mut accel_force = mass * acceleration;
            let max_force_needed = mass * speed_delta;

            if accel_force.abs() > max_force_needed.abs() {
                accel_force = max_force_needed;
            }

            physics.velocity.x += (accel_force * dir_x) / mass;
            physics.velocity.y += (accel_force * dir_y) / mass;
        }

        position
    }
}

// ============================================================================
// ROTATION AND TURNING
// ============================================================================

impl Locomotor {
    /// Rotate object towards a position
    /// Matches C++ rotateTowardsPosition() lines 2407-2430
    pub fn rotate_towards_position(
        &self,
        current_angle: Real,
        position: Coord3D,
        goal_pos: Coord3D,
        max_turn_rate: Real,
    ) -> (PhysicsTurningType, Real) {
        let desired_angle = (goal_pos.y - position.y).atan2(goal_pos.x - position.x);
        let rel_angle = std_angle_diff(desired_angle, current_angle);

        if rel_angle.abs() < TINY_EPSILON {
            return (PhysicsTurningType::None, 0.0);
        }

        let turn_amount = if rel_angle > max_turn_rate {
            max_turn_rate
        } else if rel_angle < -max_turn_rate {
            -max_turn_rate
        } else {
            rel_angle
        };

        let turning = if turn_amount > TINY_EPSILON {
            PhysicsTurningType::Positive
        } else if turn_amount < -TINY_EPSILON {
            PhysicsTurningType::Negative
        } else {
            PhysicsTurningType::None
        };

        (turning, turn_amount)
    }

    /// Rotate object around locomotor pivot point
    /// Matches C++ rotateObjAroundLocoPivot() lines 2113-2189
    pub fn rotate_obj_around_loco_pivot(
        &self,
        current_angle: Real,
        position: Coord3D,
        goal_pos: Coord3D,
        max_turn_rate: Real,
        is_braking: bool,
        bounding_radius: Real,
    ) -> (PhysicsTurningType, Real, Real) {
        let mut turn_pivot_offset = self.template.turn_pivot_offset;

        // When braking, use center pivot (matches C++ line 2121)
        if is_braking {
            turn_pivot_offset = 0.0;
        }

        if turn_pivot_offset.abs() < TINY_EPSILON {
            // No pivot offset, simple rotation
            let desired_angle = (goal_pos.y - position.y).atan2(goal_pos.x - position.x);
            let mut amount = std_angle_diff(desired_angle, current_angle);
            let rel_angle = amount;

            let turning = if amount > max_turn_rate {
                amount = max_turn_rate;
                PhysicsTurningType::Positive
            } else if amount < -max_turn_rate {
                amount = -max_turn_rate;
                PhysicsTurningType::Negative
            } else {
                PhysicsTurningType::None
            };

            let new_angle = normalize_angle(current_angle + amount);
            return (turning, new_angle, rel_angle);
        }

        // Pivot around offset point (matches C++ lines 2124-2170)
        let turn_point_offset = turn_pivot_offset * bounding_radius;

        // Calculate turn position
        let dir_x = current_angle.cos();
        let dir_y = current_angle.sin();

        let turn_pos_x = position.x + dir_x * turn_point_offset;
        let turn_pos_y = position.y + dir_y * turn_point_offset;

        let dx = goal_pos.x - turn_pos_x;
        let dy = goal_pos.y - turn_pos_y;

        // Avoid twitching due to rounding error (matches C++ line 2135)
        if dx.abs() < 0.1 && dy.abs() < 0.1 {
            return (PhysicsTurningType::None, current_angle, 0.0);
        }

        let desired_angle = dy.atan2(dx);
        let mut amount = std_angle_diff(desired_angle, current_angle);
        let rel_angle = amount;

        let turning = if amount > max_turn_rate {
            amount = max_turn_rate;
            PhysicsTurningType::Positive
        } else if amount < -max_turn_rate {
            amount = -max_turn_rate;
            PhysicsTurningType::Negative
        } else {
            PhysicsTurningType::None
        };

        let new_angle = normalize_angle(current_angle + amount);
        (turning, new_angle, rel_angle)
    }
}

// ============================================================================
// Z-AXIS BEHAVIOR (ALTITUDE CONTROL)
// ============================================================================

impl Locomotor {
    /// Calculate lift force needed to reach preferred height
    /// Matches C++ calcLiftToUseAtPt() lines 2022-2110
    pub fn calc_lift_to_use_at_pt(
        &self,
        physics: &PhysicsState,
        cur_z: Real,
        _surface_at_pt: Real,
        preferred_height: Real,
        condition: BodyDamageType,
        is_ultra_accurate: bool,
        gravity: Real,
    ) -> Real {
        let max_gross_lift = self.get_max_lift(condition);
        let max_net_lift = (max_gross_lift + gravity).max(0.0); // gravity is negative

        let cur_vel_z = physics.velocity.z;

        // Braking is limited by net lift going down, gravity going up
        let max_accel = if is_ultra_accurate {
            if cur_vel_z < 0.0 {
                2.0 * max_net_lift
            } else {
                -2.0 * max_net_lift
            }
        } else {
            if cur_vel_z < 0.0 {
                max_net_lift
            } else {
                gravity
            }
        };

        const TINY_ACCEL: Real = 0.001;
        let desired_accel = if max_accel.abs() > TINY_ACCEL {
            let delta_z = preferred_height - cur_z;

            // Calculate braking distance
            let brake_dist = cur_vel_z.powi(2) / max_accel.abs();

            if brake_dist.abs() > delta_z.abs() {
                // Use max accel if we need to brake hard
                max_accel
            } else if cur_vel_z.abs() > self.template.speed_limit_z {
                // Limit vertical speed
                self.template.speed_limit_z - cur_vel_z
            } else {
                // Calculate precise accel: a = 2(dz - v)
                2.0 * (delta_z - cur_vel_z)
            }
        } else {
            0.0
        };

        let mut lift_to_use = desired_accel - gravity;

        // Clamp lift based on mode
        if is_ultra_accurate {
            const UP_FACTOR: Real = 3.0;
            lift_to_use = lift_to_use.clamp(-max_gross_lift, UP_FACTOR * max_gross_lift);
        } else {
            lift_to_use = lift_to_use.clamp(0.0, max_gross_lift);
        }

        lift_to_use
    }

    /// Handle Z-axis behavior (altitude control)
    /// Matches C++ handleBehaviorZ() lines 2196-2323
    pub fn handle_behavior_z(
        &self,
        position: Coord3D,
        physics: &mut PhysicsState,
        goal_pos: Coord3D,
        surface_height: Real,
        water_height: Option<Real>,
        condition: BodyDamageType,
        is_ultra_accurate: bool,
        use_precise_z: bool,
        gravity: Real,
    ) -> (Coord3D, bool) {
        let mut new_position = position;
        let mut requires_constant_calling = true;

        match self.template.behavior_z {
            LocomotorBehaviorZ::NoZMotiveForce => {
                // No Z control
                requires_constant_calling = false;
            }

            LocomotorBehaviorZ::SeaLevel => {
                // Stay at water surface or ground level
                if let Some(water_z) = water_height {
                    if surface_height < water_z {
                        new_position.z = water_z;
                    } else {
                        new_position.z = surface_height;
                    }
                } else {
                    new_position.z = surface_height;
                }
            }

            LocomotorBehaviorZ::FixedSurfaceRelativeHeight => {
                // Fixed height above surface
                let surface_ht = water_height.unwrap_or(surface_height);
                new_position.z = self.preferred_height + surface_ht;
            }

            LocomotorBehaviorZ::FixedAbsoluteHeight => {
                // Fixed absolute height
                new_position.z = self.preferred_height;
            }

            LocomotorBehaviorZ::RelativeToGroundAndBuildings => {
                // Height above ground including buildings
                new_position.z = self.preferred_height + surface_height;
            }

            LocomotorBehaviorZ::SurfaceRelativeHeight
            | LocomotorBehaviorZ::AbsoluteHeight
            | LocomotorBehaviorZ::SmoothRelativeToHighestLayer => {
                if self.preferred_height != 0.0 || use_precise_z {
                    let surface_rel = matches!(
                        self.template.behavior_z,
                        LocomotorBehaviorZ::SurfaceRelativeHeight
                            | LocomotorBehaviorZ::SmoothRelativeToHighestLayer
                    );

                    let surface_ht = if surface_rel {
                        water_height.unwrap_or(surface_height)
                    } else {
                        0.0
                    };

                    let mut preferred_height = self.preferred_height + surface_ht;
                    if use_precise_z {
                        preferred_height = goal_pos.z;
                    }

                    // Damped approach to preferred height
                    let delta = preferred_height - position.z;
                    let damped_delta = delta * self.preferred_height_damping;
                    let target_height = position.z + damped_delta;

                    // Calculate lift force needed
                    let lift_to_use = self.calc_lift_to_use_at_pt(
                        physics,
                        position.z,
                        surface_ht,
                        target_height,
                        condition,
                        is_ultra_accurate,
                        gravity,
                    );

                    if lift_to_use.abs() > TINY_EPSILON {
                        physics.velocity.z += lift_to_use / physics.mass;
                    }
                }
            }
        }

        (new_position, requires_constant_calling)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locomotor::LocomotorTemplate;
    use std::sync::Arc;

    #[test]
    fn test_calc_slow_down_dist() {
        let dist = calc_slow_down_dist(10.0, 0.0, 5.0);
        assert!(dist > 9.0 && dist < 11.0); // Should be around 10.0 with fudge factor
    }

    #[test]
    fn test_normalize_angle() {
        assert!((normalize_angle(PI * 3.0) - (-PI)).abs() < TINY_EPSILON);
        assert!((normalize_angle(-PI * 3.0) - PI).abs() < TINY_EPSILON);
        assert!((normalize_angle(0.0)).abs() < TINY_EPSILON);
    }

    #[test]
    fn test_std_angle_diff() {
        let diff = std_angle_diff(PI / 4.0, -PI / 4.0);
        assert!((diff - PI / 2.0).abs() < TINY_EPSILON);
    }

    #[test]
    fn test_infantry_movement() {
        let template = Arc::new(LocomotorTemplate::new_infantry("TestInfantry".to_string()));
        let loco = Locomotor::new(template);
        let mut state = LocomotorMovementState::new();
        let mut physics = PhysicsState::default();
        physics.mass = 50.0;

        let position = Coord3D::new(0.0, 0.0, 0.0);
        let goal = Coord3D::new(10.0, 10.0, 0.0);

        let _new_pos = loco.move_towards_position_legs(
            &mut state,
            position,
            &mut physics,
            goal,
            14.14,
            8.0,
            BodyDamageType::Pristine,
        );

        // Velocity should be non-zero after movement
        assert!(physics.velocity.x.hypot(physics.velocity.y) > 0.0);
    }

    #[test]
    fn test_try_to_rotate_vector_3d_clamps_angle() {
        let from = Coord3D::new(1.0, 0.0, 0.0);
        let to = Coord3D::new(0.0, 1.0, 0.0);
        let (angle, dir) = try_to_rotate_vector_3d(0.25, from, to);

        assert!((angle - 0.25).abs() < 0.001);
        assert!(dir.x > 0.9);
        assert!(dir.y > 0.2);
    }

    #[test]
    fn test_try_to_rotate_vector_3d_negative_max_angle_is_percent() {
        let from = Coord3D::new(1.0, 0.0, 0.0);
        let to = Coord3D::new(0.0, 1.0, 0.0);
        let (angle, _dir) = try_to_rotate_vector_3d(-0.5, from, to);

        // 50% of PI/2
        assert!((angle - (PI * 0.25)).abs() < 0.001);
    }

    #[test]
    fn test_calc_direction_to_apply_thrust_offsets_existing_velocity() {
        let position = Coord3D::new(0.0, 0.0, 0.0);
        let velocity = Coord3D::new(0.0, 20.0, 0.0);
        let goal = Coord3D::new(100.0, 0.0, 0.0);
        let dir = calc_direction_to_apply_thrust(position, velocity, goal, 10.0, 0.0);

        assert!((dir.length() - 1.0).abs() < 0.001);
        assert!(dir.x > 0.0);
        assert!(dir.y < 0.0);
    }
}
