//! Missile locomotor controls and retained physics continuation.

use crate::{
    object::Movement,
    time::{LOGIC_FRAME_TIMESTEP, LOGIC_FRAMES_PER_SECOND},
};
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use super::projectile_flight::projectile_forward_transform;

/// Mutable state of the missile body that belongs to this update.
pub struct MissileBodyState<'a> {
    pub movement: &'a mut Movement,
    pub ground_height: &'a mut f32,
    pub was_airborne: &'a mut bool,
    pub precise_elevation: &'a mut bool,
    pub preferred_height: f32,
    pub min_speed: f32,
    pub immune_to_falling_damage: bool,
}

/// The receiving actor exposes its movement and delegates world effects through
/// the same object operations used by ordinary locomotors.
pub trait MissileBody {
    fn position(&self) -> Vec3;
    fn state(&mut self) -> MissileBodyState<'_>;
    fn set_gravity(&mut self, gravity: f32);
    fn raw_ground(&self, position: Vec3, fallback: f32) -> f32;
    fn highest_surface(&self) -> f32;
    fn apply_thrust(
        &mut self,
        goal: Vec3,
        distance: f32,
        speed: f32,
        frame: u32,
        braking: bool,
        ground: f32,
    );
    fn integrate_force(&mut self, frame: u32);
    fn finish_movement(&mut self, position: Vec3, frame: u32);
}

/// Locomotor commands for the update that is about to run. A frame that
/// enters KILL retains the prior turn cap and skips the remaining ATTACK body.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MissileSteering {
    NoTurn,
    Attack,
    LockTransition,
    Braking,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissileMotion {
    gravity: f32,
    no_turn_distance: f32,
    previous_position: Vec3,
    dive_distance: f32,
    /// ATTACK_NOTURN executes once before ATTACK, even for zero distance.
    attack_turn_enabled: bool,
    /// PhysicsBehavior's UPDATE_EVER_RUN, retained with the airborne latch.
    physics_initialized: bool,
    /// Locomotor limits left by the last MissileAI state, retained in KILL_SELF.
    last_controls: Option<(bool, bool, bool, bool)>,
    /// The parent MoveTo state has finished and the locomotor is maintaining
    /// position. MissileAI observes this state on its next callback.
    idle_position: Option<Vec3>,
}

impl MissileMotion {
    pub fn new(gravity: f32, no_turn_distance: f32, position: Vec3, dive_distance: f32) -> Self {
        Self {
            gravity,
            no_turn_distance,
            previous_position: position,
            dive_distance,
            attack_turn_enabled: false,
            physics_initialized: false,
            last_controls: None,
            idle_position: None,
        }
    }

    pub fn validate(&self) -> bool {
        self.gravity.is_finite()
            && self.no_turn_distance.is_finite()
            && self.previous_position.is_finite()
            && self.dive_distance.is_finite()
            && self
                .idle_position
                .is_none_or(|position| position.is_finite())
    }

    pub fn is_move_idle(&self) -> bool {
        self.idle_position.is_some()
    }

    pub fn restart_move(&mut self) {
        self.idle_position = None;
    }

    pub fn update_move(&mut self, position: Vec3, goal: Vec3, close_enough_distance: f32) {
        // AIInternalMoveToState checks its 3D path end before moving.
        if self.idle_position.is_none() && position.distance(goal) < close_enough_distance {
            self.idle_position = Some(position);
        }
    }

    pub fn launch_pose(transform: glam::Mat4, target: Vec3) -> Option<glam::Mat4> {
        let position = transform.w_axis.truncate();
        let mut direction = transform.x_axis.truncate().normalize_or_zero();
        if direction == Vec3::ZERO {
            return None;
        }
        let delta = target - position;
        direction.y += 2.0 * delta.y.max(0.0) / Vec3::new(delta.x, 0.0, delta.z).length().max(1.0);
        direction = direction.normalize_or_zero();
        Some(projectile_forward_transform(position, direction))
    }

    pub fn advance_body(
        &mut self,
        body: &mut impl MissileBody,
        goal: Vec3,
        ignited: bool,
        powered: bool,
        steering: MissileSteering,
        frame: u32,
    ) -> bool {
        // KILL_SELF uses the controls observed before this frame's travel
        // updates the no-turn distance.
        self.last_controls = Some((
            ignited,
            powered,
            steering != MissileSteering::NoTurn
                && self.attack_turn_enabled
                && self.no_turn_distance <= 0.0,
            steering == MissileSteering::Braking,
        ));
        body.set_gravity(self.gravity);
        let pos = body.position();
        let fallback = *body.state().ground_height;
        let ground = body.raw_ground(pos, fallback);
        let state = body.state();
        *state.ground_height = ground;
        if !self.physics_initialized {
            *state.was_airborne = pos.y > ground;
            self.physics_initialized = true;
        }
        if ignited && self.no_turn_distance > 0.0 {
            self.no_turn_distance -= pos.distance(self.previous_position);
            self.previous_position = pos;
        }

        let movement = state.movement;
        let old_turn = movement.turn_rate;
        let old_turn_damaged = movement.turn_rate_damaged;
        let old_accel = movement.acceleration;
        let old_accel_damaged = movement.acceleration_damaged;
        if !powered {
            movement.acceleration = 0.0;
            movement.acceleration_damaged = 0.0;
        }
        let braking = steering == MissileSteering::Braking;
        let turning = steering != MissileSteering::NoTurn
            && self.attack_turn_enabled
            && self.no_turn_distance <= 0.0;
        if !powered || !(braking || turning) {
            movement.turn_rate = 0.0;
            movement.turn_rate_damaged = 0.0;
        }
        if steering == MissileSteering::Attack && state.preferred_height > 0.0 {
            let horizontal = Vec3::new(goal.x - pos.x, 0.0, goal.z - pos.z);
            if horizontal.length_squared() < self.dive_distance * self.dive_distance {
                *state.precise_elevation = true;
            }
        }
        // ATTACK_NOTURN transitions after setting this frame's turn cap.
        if steering == MissileSteering::Attack && self.no_turn_distance <= 0.0 {
            self.attack_turn_enabled = true;
        }
        let goal = self.idle_position.unwrap_or(goal);
        let speed = if self.is_move_idle() {
            state.min_speed
        } else {
            movement.max_speed
        };
        let surface = body.highest_surface();
        body.apply_thrust(goal, pos.distance(goal), speed, frame, braking, surface);

        let movement = body.state().movement;
        movement.turn_rate = old_turn;
        movement.turn_rate_damaged = old_turn_damaged;
        movement.acceleration = old_accel;
        movement.acceleration_damaged = old_accel_damaged;
        let braking_speed = (movement.velocity.length() / LOGIC_FRAMES_PER_SECOND)
            .max(10.0 / LOGIC_FRAMES_PER_SECOND);
        body.integrate_force(frame);
        let mut next = if braking {
            // Close-in locomotion is direct. Physics does not translate braking projectiles.
            let delta = goal - pos;
            if !self.is_move_idle() && delta.length_squared() > 0.001 * 0.001 {
                pos + delta.clamp_length_max(braking_speed)
            } else {
                pos
            }
        } else {
            pos + body.state().movement.velocity / LOGIC_FRAMES_PER_SECOND
        };

        let fallback = *body.state().ground_height;
        let ground = body.raw_ground(next, fallback);
        let state = body.state();
        *state.ground_height = ground;
        if next.y <= ground {
            state.movement.velocity.y =
                (state.movement.velocity.y + (ground - next.y) * LOGIC_FRAMES_PER_SECOND).min(0.0);
            next.y = ground;
        }
        let airborne = next.y > ground;
        let ground_contact = *state.was_airborne && !airborne && !state.immune_to_falling_damage;
        *state.was_airborne = airborne;
        body.finish_movement(next, frame);
        ground_contact
    }

    pub fn advance_held_body(
        &mut self,
        body: &mut impl MissileBody,
        goal: Vec3,
        frame: u32,
    ) -> bool {
        let (ignited, powered, turn, braking) = self
            .last_controls
            .expect("KILL_SELF requires the preceding missile locomotor state");
        let saved_turn = self.attack_turn_enabled;
        self.attack_turn_enabled = turn;
        let steering = if braking {
            MissileSteering::Braking
        } else if turn {
            MissileSteering::Attack
        } else {
            MissileSteering::NoTurn
        };
        let contact = self.advance_body(body, goal, ignited, powered, steering, frame);
        self.attack_turn_enabled = saved_turn;
        self.last_controls = Some((ignited, powered, turn, braking));
        contact
    }
}
