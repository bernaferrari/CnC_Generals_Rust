//! MissileAIUpdate - Guided missile behavior module
//!
//! Handles AI for guided missiles including:
//! - Launch sequence (PRELAUNCH → LAUNCH → IGNITION → ATTACK)
//! - Target tracking and lock-on mechanics
//! - Fuel management and expiration
//! - Countermeasure handling and jamming
//! - Collision detection and detonation
//! - Garrison hit kills (special anti-building missiles)
//!
//! Original C++ Author: Michael S. Booth, December 2001
//! Rust conversion: 2025

use crate::ai::{AiError, AIUpdateContext, AIModuleState, AIModulePriority,
               AIModuleType, AIUpdateModuleTrait, AIUpdateResult};
use crate::common::{Bool, Coord3D, ObjectID, Real, UnsignedInt};
use crate::helpers::get_game_logic_random_value_real;
use serde::{Deserialize, Serialize};

const BIGNUM: Real = 99999.0;
const APPROACH_HEIGHT: Real = 10.0;

/// Missile state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissileStateType {
    /// Pre-launch state, waiting in launcher
    Prelaunch = 0,
    /// Released from launcher, falling
    Launch = 1,
    /// Engines ignite
    Ignition = 2,
    /// Flying toward victim, no turning
    AttackNoTurn = 3,
    /// Flying toward victim with turning
    Attack = 4,
    /// Dead/inactive
    Dead = 5,
    /// Hit victim (instant kill)
    Kill = 6,
    /// Destroy self after detonation
    KillSelf = 7,
}

/// Missile AI module configuration data (from INI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissileAIUpdateModuleData {
    /// If true, attack object, not position
    pub try_to_follow_target: Bool,
    /// Number of frames till missile runs out of motive power (0 = infinite)
    pub fuel_lifetime: UnsignedInt,
    /// Delay in frames from when missile is 'fired' to when it starts moving
    pub ignition_delay: UnsignedInt,
    /// Initial velocity magnitude
    pub initial_velocity: Real,
    /// Distance to travel before turning is allowed
    pub initial_distance: Real,
    /// If I get this close to target, start ignoring preferred height
    pub dive_distance: Real,
    /// Ignition FX list name
    #[serde(default)]
    pub ignition_fx: Option<String>,
    /// If true, limit speed of projectile to the weapon's speed
    pub use_weapon_speed: Bool,
    /// If true, blow up when out of gas instead of just stopping
    pub detonate_on_no_fuel: Bool,
    /// Number of garrison units to kill on hit
    #[serde(default)]
    pub garrison_hit_kill_count: i32,
    /// The kinds of units that can be killed in garrison
    #[serde(default)]
    pub garrison_hit_kill_kindof: u64,
    /// The kinds of units that CANNOT be killed in garrison
    #[serde(default)]
    pub garrison_hit_kill_kindof_not: u64,
    /// FX for garrison kills
    #[serde(default)]
    pub garrison_hit_kill_fx: Option<String>,
    /// How far I scatter when jammed
    pub distance_scatter_when_jammed: Real,
    /// If I get this close to target, guaranteed hit (lock distance)
    pub lock_distance: Real,
    /// If true, kill() will be called instead of destroy on detonate
    pub detonate_calls_kill: Bool,
    /// Delay before destroying self after detonation
    pub kill_self_delay: UnsignedInt,
}

impl Default for MissileAIUpdateModuleData {
    fn default() -> Self {
        Self {
            try_to_follow_target: true,
            fuel_lifetime: 0,
            ignition_delay: 0,
            initial_velocity: 0.0,
            initial_distance: 0.0,
            dive_distance: 0.0,
            ignition_fx: None,
            use_weapon_speed: false,
            detonate_on_no_fuel: false,
            garrison_hit_kill_count: 0,
            garrison_hit_kill_kindof: 0,
            garrison_hit_kill_kindof_not: 0,
            garrison_hit_kill_fx: None,
            distance_scatter_when_jammed: 75.0,
            lock_distance: 75.0,
            detonate_calls_kill: false,
            kill_self_delay: 3, // Just long enough for contrail to catch up
        }
    }
}

/// Missile AI update module
pub struct MissileAIUpdate {
    /// Current AI state
    state: AIModuleState,
    /// Missile state machine state
    missile_state: MissileStateType,
    /// Frame when state was entered
    state_timestamp: UnsignedInt,
    /// Next frame to update target position
    next_target_track_time: UnsignedInt,

    /// ID of object that launched us
    launcher_id: ObjectID,
    /// ID of object we're targeting
    victim_id: ObjectID,
    /// Original target position when fired
    original_target_pos: Coord3D,
    /// Previous frame position
    prev_pos: Coord3D,

    /// Frame when fuel expires
    fuel_expiration_date: UnsignedInt,
    /// Distance left before turning is allowed
    no_turn_dist_left: Real,
    /// Maximum acceleration
    max_accel: Real,

    /// Is the warhead armed?
    is_armed: Bool,
    /// Was originally shot at a moving object?
    is_tracking_target: Bool,
    /// If true, missile will not cause damage when it detonates (flares)
    no_damage: Bool,
    /// Has been jammed by countermeasures?
    is_jammed: Bool,

    /// Frames till decoyed by countermeasures
    frames_till_decoyed: UnsignedInt,
    /// Exhaust particle system ID
    exhaust_id: Option<u32>,
    /// Detonation weapon template name
    detonation_weapon_tmpl: Option<String>,
    /// Exhaust particle system template name
    exhaust_sys_tmpl: Option<String>,

    /// Configuration data
    data: MissileAIUpdateModuleData,
}

impl MissileAIUpdate {
    pub fn new(data: MissileAIUpdateModuleData) -> Self {
        Self {
            state: AIModuleState::Idle,
            missile_state: MissileStateType::Prelaunch,
            state_timestamp: 0,
            next_target_track_time: 0x7fffffff,

            launcher_id: ObjectID::invalid(),
            victim_id: ObjectID::invalid(),
            original_target_pos: [0.0, 0.0, 0.0],
            prev_pos: [0.0, 0.0, 0.0],

            fuel_expiration_date: 0,
            no_turn_dist_left: data.initial_distance,
            max_accel: BIGNUM,

            is_armed: false,
            is_tracking_target: false,
            no_damage: false,
            is_jammed: false,

            frames_till_decoyed: 0,
            exhaust_id: None,
            detonation_weapon_tmpl: None,
            exhaust_sys_tmpl: None,

            data,
        }
    }

    /// Launch missile at object or position
    pub fn projectile_launch(
        &mut self,
        victim: Option<ObjectID>,
        victim_pos: &Coord3D,
        launcher: ObjectID,
        detonation_weapon: Option<String>,
        exhaust_sys: Option<String>,
    ) {
        self.launcher_id = launcher;
        self.detonation_weapon_tmpl = detonation_weapon;
        self.exhaust_sys_tmpl = exhaust_sys;

        // Position projectile at launcher
        // (Would call weapon positioning system here)

        self.projectile_fire(victim, victim_pos);
    }

    /// Fire the missile once positioned
    fn projectile_fire(&mut self, victim: Option<ObjectID>, victim_pos: &Coord3D) {
        self.original_target_pos = *victim_pos;

        // Apply initial velocity if configured
        if self.data.initial_velocity > 0.0 {
            // Would apply physics impulse here
        }

        // Switch to launch state
        self.switch_to_state(MissileStateType::Launch);

        // Set up target tracking
        if let Some(victim_id) = victim {
            if self.data.try_to_follow_target {
                self.victim_id = victim_id;
                self.is_tracking_target = true;
                // Would issue move-to-object command here
            }
        } else {
            // Fire at position
            self.victim_id = ObjectID::invalid();
            self.is_tracking_target = false;

            // Adjust target position if lock distance is used
            let mut target_pos = *victim_pos;
            if self.data.lock_distance > 0.0 {
                target_pos[2] += APPROACH_HEIGHT;
            }
            // Would issue move-to-position command here
        }
    }

    /// Handle collision with another object
    pub fn handle_collision(&mut self, other: Option<ObjectID>) -> Bool {
        // Check if warhead is armed
        if !self.is_armed {
            return true; // Pass through
        }

        // Check if we hit the ground vs an object
        if other.is_none() {
            // Ground collision - check if we're close to target
            // Would do distance check here
        }

        if let Some(other_id) = other {
            // Check if we should collide with this object
            // Would check weapon collision rules here

            // Handle garrison hit kills
            if self.data.garrison_hit_kill_count > 0 {
                // Would check if other is garrisonable building
                // Would kill garrison units matching kindof filters
                // Would play garrison hit FX
            }
        }

        // Detonate on collision
        self.detonate();

        true
    }

    /// Detonate the missile
    fn detonate(&mut self) {
        // Fire detonation weapon if configured
        if self.detonation_weapon_tmpl.is_some() && !self.no_damage {
            // Would call weapon detonation system here
        }

        // Hide drawable
        // Would hide drawable here

        // Switch to kill self state
        self.switch_to_state(MissileStateType::KillSelf);
    }

    /// Switch missile state
    fn switch_to_state(&mut self, new_state: MissileStateType) {
        if self.missile_state != new_state {
            self.missile_state = new_state;
            // Would update timestamp from game logic here
        }
    }

    /// Update prelaunch state
    fn do_prelaunch_state(&mut self, context: &AIUpdateContext) {
        // Disable locomotor movement
        // Would set max acceleration/turn rate to 0 here
    }

    /// Update launch state
    fn do_launch_state(&mut self, context: &AIUpdateContext) {
        // Disable locomotor movement
        // Would set max acceleration/turn rate to 0 here

        // Check if ignition delay has elapsed
        let frames_in_state = context.current_frame.saturating_sub(self.state_timestamp);
        if frames_in_state >= self.data.ignition_delay {
            self.switch_to_state(MissileStateType::Ignition);
        }
    }

    /// Update ignition state
    fn do_ignition_state(&mut self, context: &AIUpdateContext) {
        // Enable locomotor
        // Would set max acceleration here
        // Would set turn rate to 0 here

        // Play ignition FX
        if let Some(ref _fx) = self.data.ignition_fx {
            // Would play FX here
        }

        // Create exhaust particle system
        if let Some(ref _exhaust) = self.exhaust_sys_tmpl {
            // Would create attached particle system here
            self.exhaust_id = Some(context.current_frame);
        }

        // Arm the warhead
        self.is_armed = true;

        // Calculate fuel expiration
        self.fuel_expiration_date = if self.data.fuel_lifetime > 0 {
            context.current_frame + self.data.fuel_lifetime
        } else {
            0x7fffffff // Infinite fuel
        };

        // Switch to attack state
        self.switch_to_state(MissileStateType::AttackNoTurn);
    }

    /// Update attack state
    fn do_attack_state(&mut self, context: &AIUpdateContext, turn_ok: Bool) {
        // Check if fuel has expired
        if context.current_frame >= self.fuel_expiration_date {
            if self.data.detonate_on_no_fuel {
                self.detonate();
                return;
            }

            // Disable locomotor
            // Would set max acceleration/turn rate to 0 here

            // Destroy exhaust
            self.toss_exhaust();
        } else {
            // Update locomotor parameters
            // Would set max acceleration here
            // Would set turn rate based on turn_ok here
        }

        // Check for lock-on
        if self.data.lock_distance > 0.0 {
            let mut lock_distance_squared = self.data.lock_distance;

            // Calculate distance to target
            // Would get distance here

            // If tracking immobile target, halve lock distance
            if !self.is_tracking_target {
                lock_distance_squared *= 0.5;
            }
            lock_distance_squared = lock_distance_squared * lock_distance_squared;

            // Check if within lock range
            // Would compare distances here

            // If locked and not tracking, switch to original target pos
            // Would update goal here

            // If locked, switch to KILL state (instant hit)
            // self.switch_to_state(MissileStateType::Kill);
        }

        // Update no-turn distance
        if self.no_turn_dist_left > 0.0 {
            let dist_traveled = 0.0; // Would calculate from prev_pos
            self.no_turn_dist_left -= dist_traveled;

            if self.no_turn_dist_left <= 0.0 {
                // Allow turning now
                self.switch_to_state(MissileStateType::Attack);
            }
        }

        // Check for dive distance
        if self.data.dive_distance > 0.0 {
            // Would check distance to target
            // If close enough, would adjust preferred height
        }
    }

    /// Update kill self state
    fn do_kill_self_state(&mut self, context: &AIUpdateContext) {
        // Wait for contrail to catch up
        let frames_in_state = context.current_frame.saturating_sub(self.state_timestamp);
        if frames_in_state < self.data.kill_self_delay {
            return;
        }

        // Destroy or kill object
        if self.data.detonate_calls_kill {
            // Would call kill() here
        } else {
            // Would call destroy here
        }

        self.switch_to_state(MissileStateType::Dead);
    }

    /// Destroy exhaust particle system
    fn toss_exhaust(&mut self) {
        if let Some(_exhaust_id) = self.exhaust_id {
            // Would destroy particle system here
            self.exhaust_id = None;
        }
    }

    /// Handle being jammed by countermeasures
    pub fn projectile_now_jammed(&mut self) {
        self.is_jammed = true;
        self.victim_id = ObjectID::invalid();

        // Scatter to ground
        let mut scatter_pos = self.original_target_pos;

        let scatter = self.data.distance_scatter_when_jammed;
        scatter_pos[0] += get_game_logic_random_value_real(-scatter, scatter);
        scatter_pos[1] += get_game_logic_random_value_real(-scatter, scatter);
        scatter_pos[2] = 0.0; // Ground level

        // Would update goal position here
    }

    /// Check if projectile is armed
    pub fn projectile_is_armed(&self) -> Bool {
        self.is_armed
    }

    /// Get launcher ID
    pub fn projectile_get_launcher_id(&self) -> ObjectID {
        self.launcher_id
    }

    /// Set frames till countermeasure diversion
    pub fn set_frames_till_countermeasure_diversion(&mut self, frames: UnsignedInt) {
        self.frames_till_decoyed = frames;
    }
}

impl AIUpdateModuleTrait for MissileAIUpdate {
    fn get_module_type(&self) -> AIModuleType {
        AIModuleType::Missile
    }

    fn get_priority(&self) -> AIModulePriority {
        AIModulePriority::Critical // Missiles always update
    }

    fn get_state(&self) -> AIModuleState {
        self.state
    }

    fn init(&mut self, context: &AIUpdateContext) -> AIUpdateResult<()> {
        self.state = AIModuleState::Idle;
        self.missile_state = MissileStateType::Prelaunch;
        self.state_timestamp = context.current_frame;
        Ok(())
    }

    fn reset(&mut self) -> AIUpdateResult<()> {
        self.missile_state = MissileStateType::Prelaunch;
        self.is_armed = false;
        self.toss_exhaust();
        Ok(())
    }

    fn update(&mut self, context: &mut AIUpdateContext) -> AIUpdateResult<()> {
        // Update based on current missile state
        match self.missile_state {
            MissileStateType::Prelaunch => {
                self.do_prelaunch_state(context);
            }
            MissileStateType::Launch => {
                self.do_launch_state(context);
            }
            MissileStateType::Ignition => {
                self.do_ignition_state(context);
            }
            MissileStateType::AttackNoTurn => {
                self.do_attack_state(context, false);
            }
            MissileStateType::Attack => {
                self.do_attack_state(context, true);
            }
            MissileStateType::KillSelf => {
                self.do_kill_self_state(context);
            }
            MissileStateType::Kill => {
                // Instant kill state
                self.detonate();
            }
            MissileStateType::Dead => {
                // Do nothing, missile is dead
            }
        }

        // Update position tracking
        self.prev_pos = context.position;

        Ok(())
    }

    fn should_update(&self, _context: &AIUpdateContext) -> bool {
        // Always update missiles until dead
        self.missile_state != MissileStateType::Dead
    }
}

impl Default for MissileAIUpdate {
    fn default() -> Self {
        Self::new(MissileAIUpdateModuleData::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missile_state_machine() {
        let data = MissileAIUpdateModuleData {
            ignition_delay: 15,
            fuel_lifetime: 300,
            ..Default::default()
        };

        let mut missile = MissileAIUpdate::new(data);
        assert_eq!(missile.missile_state, MissileStateType::Prelaunch);

        // Launch the missile
        missile.switch_to_state(MissileStateType::Launch);
        assert_eq!(missile.missile_state, MissileStateType::Launch);
    }

    #[test]
    fn test_projectile_arming() {
        let mut missile = MissileAIUpdate::default();
        assert!(!missile.projectile_is_armed());

        // Arm the missile
        missile.is_armed = true;
        assert!(missile.projectile_is_armed());
    }

    #[test]
    fn test_jamming() {
        let mut missile = MissileAIUpdate::default();
        missile.victim_id = ObjectID::from_raw(123);
        assert!(!missile.is_jammed);

        // Jam the missile
        missile.projectile_now_jammed();
        assert!(missile.is_jammed);
        assert_eq!(missile.victim_id, ObjectID::invalid());
    }
}
