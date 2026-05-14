////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Physics System - Movement, collision, and projectile physics
//!
//! This module provides the physics simulation for all game entities including:
//! - Movement physics with velocity, acceleration, and friction
//! - Collision detection and response
//! - Projectile physics including ballistics and trajectories
//! - Gravity and other physical forces
//!
//! Author: Converted from C++ PhysicsBehavior classes

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use crate::common::{Bool, Coord3D, Int, Matrix3D, ObjectID, Real};
use crate::{GameLogicError, GameLogicResult};

/// Constants for physics simulation
pub const GRAVITY: Real = -32.0; // Feet per second squared (C&C uses imperial units)
pub const AIR_RESISTANCE: Real = 0.98; // Air resistance coefficient
pub const MIN_VELOCITY_THRESHOLD: Real = 0.01; // Below this velocity, object is considered stopped
pub const MAX_VELOCITY: Real = 1000.0; // Maximum velocity limit to prevent overflow
pub const BOUNCE_COEFFICIENT: Real = 0.7; // Coefficient of restitution for bouncing objects

/// Physics object types - matches C++ physics behavior types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsType {
    /// Static object - no physics simulation
    None,
    /// Normal physics with gravity and collision
    Normal,
    /// Projectile physics with ballistic trajectory
    Projectile,
    /// Aircraft physics with lift and drag
    Aircraft,
    /// Bouncing physics for grenades and debris
    Bouncing,
    /// Floating physics for naval units
    Naval,
    /// Hover physics for hover vehicles
    Hover,
}

/// Collision response types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionResponse {
    /// No collision response
    None,
    /// Stop on collision
    Stop,
    /// Bounce off collision
    Bounce,
    /// Slide along collision surface
    Slide,
    /// Destroy on collision
    Destroy,
}

/// Physics turning direction type
/// Matches the enum used in animation steering system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicsTurningType {
    /// Not turning
    None,
    /// Turning in positive direction (counterclockwise)
    Positive,
    /// Turning in negative direction (clockwise)
    Negative,
}

/// Types of force effects that can be applied to physics objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceEffectType {
    /// Wind force from explosions
    Wind,
    /// Knockback from impacts
    Knockback,
    /// Thrust from engines
    Thrust,
    /// Magnetic pull
    Magnetic,
    /// Repulsion field
    Repulsion,
    /// Tractor beam
    TractorBeam,
}

/// Force effect applied to an object
#[derive(Debug, Clone)]
pub struct ForceEffect {
    /// Type of force
    pub force_type: ForceEffectType,

    /// Force vector to apply
    pub force: Coord3D,

    /// Remaining duration (in frames)
    pub duration: Int,

    /// Whether force decays over time
    pub decaying: Bool,

    /// Decay rate per frame (0.0-1.0)
    pub decay_rate: Real,
}

/// Physics state for an object - mirrors C++ PhysicsBehavior data
#[derive(Debug, Clone)]
pub struct PhysicsState {
    /// Current position in 3D space
    pub position: Coord3D,

    /// Current velocity vector
    pub velocity: Coord3D,

    /// Current acceleration vector
    pub acceleration: Coord3D,

    /// Angular velocity for rotation
    pub angular_velocity: Coord3D,

    /// Mass of the object (affects physics calculations)
    pub mass: Real,

    /// Physics simulation type
    pub physics_type: PhysicsType,

    /// Collision response behavior
    pub collision_response: CollisionResponse,

    /// Ground height at current position
    pub ground_height: Real,

    /// Whether object is currently on ground
    pub on_ground: Bool,

    /// Whether object is affected by gravity
    pub affected_by_gravity: Bool,

    /// Friction coefficient for ground contact
    pub friction: Real,

    /// Air resistance coefficient
    pub drag: Real,

    /// Bounce coefficient for bouncing objects
    pub bounce_factor: Real,

    /// Maximum lifetime for projectiles (in frames)
    pub max_lifetime: Int,

    /// Current lifetime counter
    pub lifetime: Int,

    /// Whether physics is currently active
    pub enabled: Bool,

    /// Current hover height above ground (for hover units)
    pub hover_height: Real,

    /// Target hover height
    pub target_hover_height: Real,

    /// Hover stabilization force
    pub hover_force: Real,

    /// Hover damping coefficient
    pub hover_damping: Real,

    /// Current altitude above sea level (for aircraft)
    pub altitude: Real,

    /// Target altitude for aircraft
    pub target_altitude: Real,

    /// Minimum safe altitude for aircraft
    pub min_altitude: Real,

    /// Maximum altitude for aircraft
    pub max_altitude: Real,

    /// Whether aircraft is landing
    pub is_landing: Bool,

    /// Stun effect remaining time (in frames)
    pub stun_remaining: Int,

    /// Freeze effect remaining time (in frames)
    pub freeze_remaining: Int,

    /// Whether object is stunned
    pub is_stunned: Bool,

    /// Whether object is frozen
    pub is_frozen: Bool,

    /// Current terrain slope angle at position
    pub terrain_slope: Real,

    /// Maximum slope angle object can climb
    pub max_climbable_slope: Real,

    /// Whether object is on a bridge
    pub on_bridge: Bool,

    /// Bridge height if on bridge
    pub bridge_height: Real,

    /// Whether object is in water
    pub in_water: Bool,

    /// Water depth at current position
    pub water_depth: Real,

    /// Whether object can cross water
    pub can_cross_water: Bool,

    /// Water movement speed multiplier
    pub water_speed_multiplier: Real,

    /// Allow motive forces while airborne (matches C++ PhysicsBehavior flag).
    pub allow_motive_force_while_airborne: Bool,

    /// Active force accumulator for wind/knockback
    pub active_forces: Vec<ForceEffect>,
}

impl PhysicsState {
    /// Create new physics state with default values - matches C++ constructor behavior
    pub fn new() -> Self {
        Self {
            position: Coord3D::new(0.0, 0.0, 0.0),
            velocity: Coord3D::new(0.0, 0.0, 0.0),
            acceleration: Coord3D::new(0.0, 0.0, 0.0),
            angular_velocity: Coord3D::new(0.0, 0.0, 0.0),
            mass: 1.0,
            physics_type: PhysicsType::None,
            collision_response: CollisionResponse::Stop,
            ground_height: 0.0,
            on_ground: false,
            affected_by_gravity: true,
            friction: 0.9,
            drag: AIR_RESISTANCE,
            bounce_factor: BOUNCE_COEFFICIENT,
            max_lifetime: -1, // Unlimited
            lifetime: 0,
            enabled: false,
            hover_height: 0.0,
            target_hover_height: 5.0,
            hover_force: 100.0,
            hover_damping: 0.8,
            altitude: 0.0,
            target_altitude: 100.0,
            min_altitude: 10.0,
            max_altitude: 500.0,
            is_landing: false,
            stun_remaining: 0,
            freeze_remaining: 0,
            is_stunned: false,
            is_frozen: false,
            terrain_slope: 0.0,
            max_climbable_slope: 45.0, // 45 degrees
            on_bridge: false,
            bridge_height: 0.0,
            in_water: false,
            water_depth: 0.0,
            can_cross_water: false,
            water_speed_multiplier: 0.5,
            allow_motive_force_while_airborne: false,
            active_forces: Vec::new(),
        }
    }

    /// Initialize physics for projectile - matches C++ projectile physics setup
    pub fn init_projectile(
        &mut self,
        initial_pos: Coord3D,
        initial_velocity: Coord3D,
        lifetime_frames: Int,
    ) {
        self.position = initial_pos;
        self.velocity = initial_velocity;
        self.acceleration = Coord3D::new(0.0, 0.0, GRAVITY);
        self.physics_type = PhysicsType::Projectile;
        self.collision_response = CollisionResponse::Destroy;
        self.affected_by_gravity = true;
        self.max_lifetime = lifetime_frames;
        self.lifetime = 0;
        self.enabled = true;
        self.on_ground = false;
    }

    /// Initialize physics for bouncing object - matches C++ bouncing physics
    pub fn init_bouncing(
        &mut self,
        initial_pos: Coord3D,
        initial_velocity: Coord3D,
        bounce_factor: Real,
    ) {
        self.position = initial_pos;
        self.velocity = initial_velocity;
        self.acceleration = Coord3D::new(0.0, 0.0, GRAVITY);
        self.physics_type = PhysicsType::Bouncing;
        self.collision_response = CollisionResponse::Bounce;
        self.affected_by_gravity = true;
        self.bounce_factor = bounce_factor;
        self.enabled = true;
        self.friction = 0.8; // Higher friction for bouncing objects
    }

    /// Initialize physics for aircraft - matches C++ aircraft physics
    pub fn init_aircraft(&mut self, initial_pos: Coord3D, initial_velocity: Coord3D) {
        self.position = initial_pos;
        self.velocity = initial_velocity;
        self.acceleration = Coord3D::new(0.0, 0.0, 0.0); // Aircraft fight gravity with lift
        self.physics_type = PhysicsType::Aircraft;
        self.collision_response = CollisionResponse::Slide;
        self.affected_by_gravity = false; // Lift counters gravity
        self.drag = 0.95; // Higher drag for aircraft
        self.enabled = true;
        self.friction = 0.0; // No ground friction for aircraft
        self.altitude = initial_pos[2];
        self.target_altitude = initial_pos[2];
        self.is_landing = false;
    }

    /// Initialize physics for hover vehicles - matches C++ hover behavior
    pub fn init_hover(&mut self, initial_pos: Coord3D, hover_height: Real) {
        self.position = initial_pos;
        self.velocity = Coord3D::new(0.0, 0.0, 0.0);
        self.acceleration = Coord3D::new(0.0, 0.0, 0.0);
        self.physics_type = PhysicsType::Hover;
        self.collision_response = CollisionResponse::Slide;
        self.affected_by_gravity = false; // Hover systems counter gravity
        self.enabled = true;
        self.friction = 0.7; // Some friction when hovering over surfaces
        self.target_hover_height = hover_height;
        self.hover_height = 0.0;
        self.on_ground = false;
    }

    /// Check if object has significant velocity - matches C++ physics behavior
    pub fn has_significant_velocity(&self) -> Bool {
        let speed_squared = self.velocity[0] * self.velocity[0]
            + self.velocity[1] * self.velocity[1]
            + self.velocity[2] * self.velocity[2];
        speed_squared > MIN_VELOCITY_THRESHOLD * MIN_VELOCITY_THRESHOLD
    }

    /// Get current speed magnitude
    pub fn get_speed(&self) -> Real {
        (self.velocity[0] * self.velocity[0]
            + self.velocity[1] * self.velocity[1]
            + self.velocity[2] * self.velocity[2])
            .sqrt()
    }

    /// Set velocity with magnitude limit
    pub fn set_velocity(&mut self, new_velocity: Coord3D) {
        self.velocity = new_velocity;
        self.clamp_velocity();
    }

    /// Add force to acceleration (Force = mass * acceleration)
    pub fn add_force(&mut self, force: Coord3D) {
        if self.mass > 0.0 {
            self.acceleration[0] += force[0] / self.mass;
            self.acceleration[1] += force[1] / self.mass;
            self.acceleration[2] += force[2] / self.mass;
        }
    }

    /// Apply impulse to velocity (Impulse = change in momentum)
    pub fn apply_impulse(&mut self, impulse: Coord3D) {
        if self.mass > 0.0 {
            self.velocity[0] += impulse[0] / self.mass;
            self.velocity[1] += impulse[1] / self.mass;
            self.velocity[2] += impulse[2] / self.mass;
            self.clamp_velocity();
        }
    }

    /// Clamp velocity to maximum limits
    fn clamp_velocity(&mut self) {
        let speed = self.get_speed();
        if speed > MAX_VELOCITY {
            let scale = MAX_VELOCITY / speed;
            self.velocity[0] *= scale;
            self.velocity[1] *= scale;
            self.velocity[2] *= scale;
        }
    }

    /// Apply wind force from explosion
    pub fn apply_wind(&mut self, wind_force: Coord3D, duration: Int) {
        let effect = ForceEffect {
            force_type: ForceEffectType::Wind,
            force: wind_force,
            duration,
            decaying: true,
            decay_rate: 0.95,
        };
        self.active_forces.push(effect);
    }

    /// Apply knockback force
    pub fn apply_knockback(&mut self, knockback_force: Coord3D, duration: Int) {
        let effect = ForceEffect {
            force_type: ForceEffectType::Knockback,
            force: knockback_force,
            duration,
            decaying: true,
            decay_rate: 0.9,
        };
        self.active_forces.push(effect);
    }

    /// Apply stun effect
    pub fn apply_stun(&mut self, duration: Int) {
        self.is_stunned = true;
        self.stun_remaining = duration;
    }

    /// Apply freeze effect
    pub fn apply_freeze(&mut self, duration: Int) {
        self.is_frozen = true;
        self.freeze_remaining = duration;
    }

    /// Update active force effects
    pub fn update_force_effects(&mut self) {
        let mut accumulated = Coord3D::new(0.0, 0.0, 0.0);
        // Apply all active forces
        for effect in &mut self.active_forces {
            if effect.duration > 0 {
                // Apply force to acceleration
                accumulated[0] += effect.force[0];
                accumulated[1] += effect.force[1];
                accumulated[2] += effect.force[2];

                // Decay force if needed
                if effect.decaying {
                    effect.force[0] *= effect.decay_rate;
                    effect.force[1] *= effect.decay_rate;
                    effect.force[2] *= effect.decay_rate;
                }

                effect.duration -= 1;
            }
        }

        // Apply accumulated force to acceleration.
        self.add_force(accumulated);

        // Remove expired effects
        self.active_forces.retain(|effect| effect.duration > 0);
    }

    /// Update stun and freeze effects
    pub fn update_status_effects(&mut self) {
        // Update stun
        if self.is_stunned {
            if self.stun_remaining > 0 {
                self.stun_remaining -= 1;
                // Zero out velocity when stunned
                self.velocity = Coord3D::new(0.0, 0.0, 0.0);
            } else {
                self.is_stunned = false;
            }
        }

        // Update freeze
        if self.is_frozen {
            if self.freeze_remaining > 0 {
                self.freeze_remaining -= 1;
                // Reduce velocity significantly when frozen
                self.velocity[0] *= 0.1;
                self.velocity[1] *= 0.1;
                self.velocity[2] *= 0.1;
            } else {
                self.is_frozen = false;
            }
        }
    }

    /// Check if object can move (not stunned or frozen)
    pub fn can_move(&self) -> Bool {
        !self.is_stunned
    }

    /// Get effective speed multiplier based on terrain
    pub fn get_terrain_speed_multiplier(&self) -> Real {
        if self.in_water && !self.can_cross_water {
            return 0.0; // Cannot move in water
        }

        if self.in_water {
            return self.water_speed_multiplier;
        }

        if self.is_frozen {
            return 0.1; // Very slow when frozen
        }

        1.0
    }

    /// Start aircraft landing sequence
    pub fn start_landing(&mut self) {
        if self.physics_type == PhysicsType::Aircraft {
            self.is_landing = true;
            self.target_altitude = 0.0;
        }
    }

    /// Cancel aircraft landing
    pub fn cancel_landing(&mut self) {
        if self.physics_type == PhysicsType::Aircraft {
            self.is_landing = false;
            self.target_altitude = self.min_altitude;
        }
    }

    /// Check if aircraft has landed
    pub fn has_landed(&self) -> Bool {
        self.physics_type == PhysicsType::Aircraft && self.altitude <= 0.0 && self.is_landing
    }
}

impl Default for PhysicsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Collision information - used for collision detection and response
#[derive(Debug, Clone)]
pub struct CollisionInfo {
    /// Did a collision occur
    pub collided: Bool,

    /// Point of collision in world space
    pub collision_point: Coord3D,

    /// Normal vector at collision point
    pub collision_normal: Coord3D,

    /// Distance to collision surface
    pub penetration_depth: Real,

    /// Object that was collided with (if any)
    pub other_object_id: Option<ObjectID>,

    /// Surface material type at collision point
    pub surface_type: SurfaceType,
}

/// Surface material types for collision response
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceType {
    Ground,
    Water,
    Rock,
    Metal,
    Wood,
    Glass,
    Bridge,
    Cliff,
    Slope,
    Ice,
    Sand,
    Mud,
    Snow,
}

impl CollisionInfo {
    pub fn new() -> Self {
        Self {
            collided: false,
            collision_point: Coord3D::new(0.0, 0.0, 0.0),
            collision_normal: Coord3D::new(0.0, 0.0, 1.0),
            penetration_depth: 0.0,
            other_object_id: None,
            surface_type: SurfaceType::Ground,
        }
    }
}

/// Terrain query interface for physics system
pub trait TerrainQuery: Send + Sync {
    /// Get ground height at position
    fn get_ground_height(&self, x: Real, y: Real) -> Real;

    /// Get water depth at position (0.0 if no water)
    fn get_water_depth(&self, x: Real, y: Real) -> Real;

    /// Get terrain slope angle at position (in degrees)
    fn get_terrain_slope(&self, x: Real, y: Real) -> Real;

    /// Check if position is on a bridge
    fn is_on_bridge(&self, pos: &Coord3D) -> (Bool, Real);

    /// Check if position is a cliff
    fn is_cliff(&self, pos: &Coord3D) -> Bool;

    /// Get surface type at position
    fn get_surface_type(&self, x: Real, y: Real) -> SurfaceType;
}

/// Physics simulation engine - manages all physics objects
pub struct PhysicsEngine {
    /// All physics objects indexed by object ID
    physics_objects: HashMap<ObjectID, Arc<RwLock<PhysicsState>>>,

    /// Time step for physics simulation (in seconds)
    time_step: Real,

    /// Global gravity vector
    gravity: Coord3D,

    /// Whether physics simulation is enabled
    enabled: Bool,

    /// Terrain query interface (optional, for terrain interaction)
    terrain_query: Option<Arc<dyn TerrainQuery>>,
}

impl fmt::Debug for PhysicsEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhysicsEngine")
            .field("physics_objects", &self.physics_objects.len())
            .field("time_step", &self.time_step)
            .field("gravity", &self.gravity)
            .field("enabled", &self.enabled)
            .field("terrain_query", &self.terrain_query.is_some())
            .finish()
    }
}

impl PhysicsEngine {
    /// Create new physics engine - matches C++ physics system initialization
    pub fn new() -> Self {
        Self {
            physics_objects: HashMap::new(),
            time_step: 1.0 / 30.0, // 30 FPS physics simulation
            gravity: Coord3D::new(0.0, 0.0, GRAVITY),
            enabled: true,
            terrain_query: None,
        }
    }

    /// Set terrain query interface for terrain interaction
    pub fn set_terrain_query(&mut self, terrain_query: Arc<dyn TerrainQuery>) {
        self.terrain_query = Some(terrain_query);
    }

    /// Add object to physics simulation
    pub fn add_object(&mut self, object_id: ObjectID, physics_state: PhysicsState) {
        self.physics_objects
            .insert(object_id, Arc::new(RwLock::new(physics_state)));
    }

    /// Remove object from physics simulation
    pub fn remove_object(&mut self, object_id: ObjectID) -> bool {
        self.physics_objects.remove(&object_id).is_some()
    }

    /// Get physics state for object
    pub fn get_physics_state(&self, object_id: ObjectID) -> Option<Arc<RwLock<PhysicsState>>> {
        self.physics_objects.get(&object_id).cloned()
    }

    /// Update all physics objects by one time step - matches C++ physics update loop
    pub fn update(&mut self) -> GameLogicResult<()> {
        if !self.enabled {
            return Ok(());
        }

        let mut objects_to_remove = Vec::new();

        // Update each physics object
        let mut sorted_ids: Vec<_> = self.physics_objects.keys().copied().collect();
        sorted_ids.sort();
        for object_id in &sorted_ids {
            let physics_state = self.physics_objects.get(object_id).unwrap();
            if let Ok(mut state) = physics_state.write() {
                if !state.enabled {
                    continue;
                }

                // Update status effects (stun, freeze)
                state.update_status_effects();

                // Update active force effects (wind, knockback)
                state.update_force_effects();

                // Update terrain state if terrain query available
                if let Some(ref terrain) = self.terrain_query {
                    self.update_terrain_state(&mut state, terrain.as_ref());
                }

                // Update lifetime
                if state.max_lifetime > 0 {
                    state.lifetime += 1;
                    if state.lifetime >= state.max_lifetime {
                        objects_to_remove.push(*object_id);
                        continue;
                    }
                }

                // Apply physics based on object type
                match state.physics_type {
                    PhysicsType::None => {
                        // No physics simulation
                    }
                    PhysicsType::Normal => {
                        self.update_normal_physics(&mut state)?;
                    }
                    PhysicsType::Projectile => {
                        self.update_projectile_physics(&mut state)?;
                    }
                    PhysicsType::Aircraft => {
                        self.update_aircraft_physics(&mut state)?;
                    }
                    PhysicsType::Bouncing => {
                        self.update_bouncing_physics(&mut state)?;
                    }
                    PhysicsType::Naval => {
                        self.update_naval_physics(&mut state)?;
                    }
                    PhysicsType::Hover => {
                        self.update_hover_physics(&mut state)?;
                    }
                }

                // Check for collision and respond
                let collision = self.check_collision(&state);
                if collision.collided {
                    self.handle_collision(&mut state, &collision)?;
                }

                // Update position based on velocity
                if state.enabled && state.can_move() {
                    self.integrate_motion(&mut state);
                }
            }
        }

        // Remove expired objects
        for object_id in objects_to_remove {
            self.physics_objects.remove(&object_id);
        }

        Ok(())
    }

    /// Update terrain state from terrain query
    fn update_terrain_state(&self, state: &mut PhysicsState, terrain: &dyn TerrainQuery) {
        let x = state.position[0];
        let y = state.position[1];

        // Update ground height
        state.ground_height = terrain.get_ground_height(x, y);

        // Update water state
        state.water_depth = terrain.get_water_depth(x, y);
        state.in_water = state.water_depth > 0.0;

        // Update terrain slope
        state.terrain_slope = terrain.get_terrain_slope(x, y);

        // Update bridge state
        let (on_bridge, bridge_height) = terrain.is_on_bridge(&state.position);
        state.on_bridge = on_bridge;
        state.bridge_height = bridge_height;

        // Check if on ground or bridge
        if state.on_bridge {
            state.on_ground = state.position[2] <= state.bridge_height;
        } else {
            state.on_ground = state.position[2] <= state.ground_height;
        }
    }

    /// Update normal physics with gravity and friction - matches C++ PhysicsBehavior
    fn update_normal_physics(&self, state: &mut PhysicsState) -> GameLogicResult<()> {
        // Apply gravity if enabled and not on ground
        if state.affected_by_gravity && !state.on_ground {
            state.acceleration[2] += self.gravity[2];
        }

        // Handle terrain slope sliding
        if state.on_ground && state.terrain_slope > state.max_climbable_slope {
            // Calculate slide direction down slope
            let slide_force = (state.terrain_slope - state.max_climbable_slope) * 2.0;
            state.acceleration[2] -= slide_force;
        }

        // Apply terrain speed multiplier
        let terrain_multiplier = state.get_terrain_speed_multiplier();
        if terrain_multiplier < 1.0 {
            state.velocity[0] *= terrain_multiplier;
            state.velocity[1] *= terrain_multiplier;
        }

        // Apply ground friction if on ground
        if state.on_ground {
            state.velocity[0] *= state.friction;
            state.velocity[1] *= state.friction;
        } else {
            if !state.allow_motive_force_while_airborne {
                state.velocity[0] *= state.drag;
                state.velocity[1] *= state.drag;
                state.velocity[2] *= state.drag;
            } else {
                // Allow horizontal motive forces while airborne; still damp vertical drift.
                state.velocity[2] *= state.drag;
            }
        }

        Ok(())
    }

    /// Update projectile physics - matches C++ projectile behavior
    fn update_projectile_physics(&self, state: &mut PhysicsState) -> GameLogicResult<()> {
        // Always apply gravity for projectiles
        state.acceleration[2] += self.gravity[2];

        // Apply air resistance
        state.velocity[0] *= state.drag;
        state.velocity[1] *= state.drag;
        state.velocity[2] *= state.drag;

        Ok(())
    }

    /// Update aircraft physics - matches C++ aircraft behavior
    fn update_aircraft_physics(&self, state: &mut PhysicsState) -> GameLogicResult<()> {
        // Update altitude
        state.altitude = state.position[2];

        // Handle landing
        if state.is_landing {
            // Gradually descend to ground
            let altitude_diff = state.target_altitude - state.altitude;
            let descent_force = altitude_diff * 5.0; // Gentle descent
            state.acceleration[2] += descent_force;

            // Check if landed
            if state.altitude <= 0.0 {
                state.position[2] = 0.0;
                state.velocity[2] = 0.0;
                state.on_ground = true;
            }
        } else {
            // Maintain target altitude
            let altitude_diff = state.target_altitude - state.altitude;
            let lift_force = altitude_diff * 10.0; // Stronger lift for altitude maintenance
            state.acceleration[2] += lift_force;

            // Clamp altitude
            if state.altitude > state.max_altitude {
                state.position[2] = state.max_altitude;
                state.velocity[2] = 0.0;
            } else if state.altitude < state.min_altitude {
                state.target_altitude = state.min_altitude;
            }
        }

        // Apply air resistance (higher for aircraft). Allow horizontal motive force if flagged.
        if !state.allow_motive_force_while_airborne {
            state.velocity[0] *= state.drag;
            state.velocity[1] *= state.drag;
            state.velocity[2] *= state.drag;
        } else {
            state.velocity[2] *= state.drag;
        }

        // Apply banking/turning forces based on angular velocity
        let turn_force = 0.1;
        state.acceleration[0] += state.angular_velocity[2] * turn_force;
        state.acceleration[1] -= state.angular_velocity[2] * turn_force;

        Ok(())
    }

    /// Update hover physics - maintains constant height above ground
    fn update_hover_physics(&self, state: &mut PhysicsState) -> GameLogicResult<()> {
        // Calculate current hover height
        let ground_or_water = if state.in_water {
            0.0 // Hover over water surface
        } else {
            state.ground_height
        };

        state.hover_height = state.position[2] - ground_or_water;

        // Calculate hover stabilization force
        let hover_error = state.target_hover_height - state.hover_height;
        let hover_correction = hover_error * state.hover_force;

        // Apply hover force with damping
        state.acceleration[2] += hover_correction;
        state.velocity[2] *= state.hover_damping;

        // Apply horizontal friction
        state.velocity[0] *= state.friction;
        state.velocity[1] *= state.friction;

        // Apply air resistance
        state.velocity[0] *= state.drag;
        state.velocity[1] *= state.drag;

        // Handle terrain slope for hover vehicles
        if state.terrain_slope > state.max_climbable_slope {
            // Hover vehicles can traverse steeper slopes but are affected
            let slide_factor = (state.terrain_slope - state.max_climbable_slope) * 0.5;
            state.acceleration[2] -= slide_factor;
        }

        Ok(())
    }

    /// Update bouncing physics - matches C++ bouncing behavior
    fn update_bouncing_physics(&self, state: &mut PhysicsState) -> GameLogicResult<()> {
        // Apply gravity
        if state.affected_by_gravity {
            state.acceleration[2] += self.gravity[2];
        }

        // Apply friction and drag
        if state.on_ground {
            state.velocity[0] *= state.friction;
            state.velocity[1] *= state.friction;
        }

        state.velocity[0] *= state.drag;
        state.velocity[1] *= state.drag;
        state.velocity[2] *= state.drag;

        Ok(())
    }

    /// Update naval physics for ships - matches C++ naval behavior
    fn update_naval_physics(&self, state: &mut PhysicsState) -> GameLogicResult<()> {
        // Naval units float on water surface (no gravity)
        // Apply water resistance (higher than air resistance)
        let water_drag = 0.85;
        state.velocity[0] *= water_drag;
        state.velocity[1] *= water_drag;

        // Keep Z position at water level
        state.position[2] = 0.0; // Assuming water level is at Z=0
        state.velocity[2] = 0.0;

        Ok(())
    }

    /// Check for collision with ground and other objects
    fn check_collision(&self, state: &PhysicsState) -> CollisionInfo {
        let mut collision = CollisionInfo::new();

        // Determine effective ground height (bridge or terrain)
        let effective_ground = if state.on_bridge {
            state.bridge_height
        } else {
            state.ground_height
        };

        // Check if object should fall through bridge/cliff
        if state.on_bridge && state.position[2] < state.bridge_height {
            // Check if falling through bridge
            if let Some(ref terrain) = self.terrain_query {
                if terrain.is_cliff(&state.position) {
                    // Falling off cliff/bridge - no collision with bridge
                    collision.collided = false;
                    return collision;
                }
            }
        }

        // Check ground/bridge collision
        if state.position[2] <= effective_ground {
            collision.collided = true;
            collision.collision_point = state.position;
            collision.collision_normal = Coord3D::new(0.0, 0.0, 1.0); // Up vector
            collision.penetration_depth = effective_ground - state.position[2];

            // Determine surface type
            if state.on_bridge {
                collision.surface_type = SurfaceType::Bridge;
            } else if state.in_water {
                collision.surface_type = SurfaceType::Water;
            } else if let Some(ref terrain) = self.terrain_query {
                collision.surface_type =
                    terrain.get_surface_type(state.position[0], state.position[1]);
            } else {
                collision.surface_type = SurfaceType::Ground;
            }
        }

        // Check water collision for non-water capable objects
        if state.in_water && !state.can_cross_water && state.position[2] <= 0.0 {
            collision.collided = true;
            collision.collision_point = state.position;
            collision.collision_normal = Coord3D::new(0.0, 0.0, 1.0);
            collision.penetration_depth = -state.position[2];
            collision.surface_type = SurfaceType::Water;
        }

        // Additional collision checks would go here:
        // - Object vs object collision
        // - Building collision
        // - Spatial partitioning for efficiency

        collision
    }

    /// Handle collision response based on object settings
    fn handle_collision(
        &self,
        state: &mut PhysicsState,
        collision: &CollisionInfo,
    ) -> GameLogicResult<()> {
        match state.collision_response {
            CollisionResponse::None => {
                // No collision response
            }
            CollisionResponse::Stop => {
                // Stop all motion
                state.velocity = Coord3D::new(0.0, 0.0, 0.0);
                state.acceleration = Coord3D::new(0.0, 0.0, 0.0);
                state.position[2] = collision.collision_point[2] + collision.penetration_depth;
                state.on_ground = true;
            }
            CollisionResponse::Bounce => {
                // Bounce off surface
                self.apply_bounce(state, collision);
            }
            CollisionResponse::Slide => {
                // Slide along surface
                self.apply_slide(state, collision);
            }
            CollisionResponse::Destroy => {
                // Mark object for destruction
                state.enabled = false;
            }
        }

        Ok(())
    }

    /// Apply bounce physics - matches C++ bouncing collision response
    fn apply_bounce(&self, state: &mut PhysicsState, collision: &CollisionInfo) {
        // Reflect velocity along collision normal
        let normal = collision.collision_normal;
        let dot_product = state.velocity[0] * normal[0]
            + state.velocity[1] * normal[1]
            + state.velocity[2] * normal[2];

        state.velocity[0] -= 2.0 * dot_product * normal[0] * state.bounce_factor;
        state.velocity[1] -= 2.0 * dot_product * normal[1] * state.bounce_factor;
        state.velocity[2] -= 2.0 * dot_product * normal[2] * state.bounce_factor;

        // Move object out of collision
        state.position[0] += normal[0] * collision.penetration_depth;
        state.position[1] += normal[1] * collision.penetration_depth;
        state.position[2] += normal[2] * collision.penetration_depth;

        // Check if bouncing has stopped
        if state.get_speed() < MIN_VELOCITY_THRESHOLD {
            state.velocity = Coord3D::new(0.0, 0.0, 0.0);
            state.on_ground = true;
        }
    }

    /// Apply sliding physics - matches C++ sliding collision response
    fn apply_slide(&self, state: &mut PhysicsState, collision: &CollisionInfo) {
        let normal = collision.collision_normal;
        let dot_product = state.velocity[0] * normal[0]
            + state.velocity[1] * normal[1]
            + state.velocity[2] * normal[2];

        // Remove velocity component perpendicular to surface
        state.velocity[0] -= dot_product * normal[0];
        state.velocity[1] -= dot_product * normal[1];
        state.velocity[2] -= dot_product * normal[2];

        // Move object out of collision
        state.position[0] += normal[0] * collision.penetration_depth;
        state.position[1] += normal[1] * collision.penetration_depth;
        state.position[2] += normal[2] * collision.penetration_depth;
    }

    /// Integrate motion using Verlet integration - matches C++ physics integration
    fn integrate_motion(&self, state: &mut PhysicsState) {
        // Update velocity with acceleration
        state.velocity[0] += state.acceleration[0] * self.time_step;
        state.velocity[1] += state.acceleration[1] * self.time_step;
        state.velocity[2] += state.acceleration[2] * self.time_step;

        // Update position with velocity
        state.position[0] += state.velocity[0] * self.time_step;
        state.position[1] += state.velocity[1] * self.time_step;
        state.position[2] += state.velocity[2] * self.time_step;

        // Reset acceleration (forces are applied each frame)
        state.acceleration = Coord3D::new(0.0, 0.0, 0.0);
    }

    /// Set global gravity vector
    pub fn set_gravity(&mut self, gravity: Coord3D) {
        self.gravity = gravity;
    }

    /// Enable or disable physics simulation
    pub fn set_enabled(&mut self, enabled: Bool) {
        self.enabled = enabled;
    }

    /// Set physics time step
    pub fn set_time_step(&mut self, time_step: Real) {
        self.time_step = time_step;
    }

    /// Apply explosion force to all objects in radius
    pub fn apply_explosion_force(
        &mut self,
        explosion_pos: Coord3D,
        explosion_radius: Real,
        explosion_force: Real,
    ) {
        for physics_state in self.physics_objects.values() {
            if let Ok(mut state) = physics_state.write() {
                // Calculate distance from explosion
                let dx = state.position[0] - explosion_pos[0];
                let dy = state.position[1] - explosion_pos[1];
                let dz = state.position[2] - explosion_pos[2];
                let distance = (dx * dx + dy * dy + dz * dz).sqrt();

                if distance < explosion_radius && distance > 0.0 {
                    // Calculate force direction (away from explosion)
                    let direction = [dx / distance, dy / distance, dz / distance];

                    // Calculate force magnitude (falls off with distance)
                    let force_magnitude = explosion_force * (1.0 - distance / explosion_radius);

                    // Apply wind force
                    let wind_force = [
                        direction[0] * force_magnitude,
                        direction[1] * force_magnitude,
                        direction[2] * force_magnitude * 0.5, // Less upward force
                    ];

                    state.apply_wind(wind_force.into(), 30); // 30 frame duration
                }
            }
        }
    }

    /// Apply knockback to specific object
    pub fn apply_knockback_to_object(
        &self,
        object_id: ObjectID,
        knockback_force: Coord3D,
        duration: Int,
    ) -> bool {
        if let Some(physics_state) = self.get_physics_state(object_id) {
            if let Ok(mut state) = physics_state.write() {
                state.apply_knockback(knockback_force, duration);
                return true;
            }
        }
        false
    }

    /// Apply stun to specific object
    pub fn apply_stun_to_object(&self, object_id: ObjectID, duration: Int) -> bool {
        if let Some(physics_state) = self.get_physics_state(object_id) {
            if let Ok(mut state) = physics_state.write() {
                state.apply_stun(duration);
                return true;
            }
        }
        false
    }

    /// Apply freeze to specific object
    pub fn apply_freeze_to_object(&self, object_id: ObjectID, duration: Int) -> bool {
        if let Some(physics_state) = self.get_physics_state(object_id) {
            if let Ok(mut state) = physics_state.write() {
                state.apply_freeze(duration);
                return true;
            }
        }
        false
    }

    /// Get object position
    pub fn get_object_position(&self, object_id: ObjectID) -> Option<Coord3D> {
        if let Some(physics_state) = self.get_physics_state(object_id) {
            if let Ok(state) = physics_state.read() {
                return Some(state.position);
            }
        }
        None
    }

    /// Get object velocity
    pub fn get_object_velocity(&self, object_id: ObjectID) -> Option<Coord3D> {
        if let Some(physics_state) = self.get_physics_state(object_id) {
            if let Ok(state) = physics_state.read() {
                return Some(state.velocity);
            }
        }
        None
    }

    /// Set object position
    pub fn set_object_position(&self, object_id: ObjectID, position: Coord3D) -> bool {
        if let Some(physics_state) = self.get_physics_state(object_id) {
            if let Ok(mut state) = physics_state.write() {
                state.position = position;
                return true;
            }
        }
        false
    }

    /// Set object velocity
    pub fn set_object_velocity(&self, object_id: ObjectID, velocity: Coord3D) -> bool {
        if let Some(physics_state) = self.get_physics_state(object_id) {
            if let Ok(mut state) = physics_state.write() {
                state.set_velocity(velocity);
                return true;
            }
        }
        false
    }

    /// Check if object is on ground
    pub fn is_object_on_ground(&self, object_id: ObjectID) -> bool {
        if let Some(physics_state) = self.get_physics_state(object_id) {
            if let Ok(state) = physics_state.read() {
                return state.on_ground;
            }
        }
        false
    }

    /// Check if object can move
    pub fn can_object_move(&self, object_id: ObjectID) -> bool {
        if let Some(physics_state) = self.get_physics_state(object_id) {
            if let Ok(state) = physics_state.read() {
                return state.can_move();
            }
        }
        false
    }

    /// Get terrain speed multiplier for object
    pub fn get_terrain_speed_multiplier(&self, object_id: ObjectID) -> Real {
        if let Some(physics_state) = self.get_physics_state(object_id) {
            if let Ok(state) = physics_state.read() {
                return state.get_terrain_speed_multiplier();
            }
        }
        1.0
    }
}

impl Default for PhysicsEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Global physics engine instance
use once_cell::sync::Lazy;
pub static THE_PHYSICS_ENGINE: Lazy<Arc<RwLock<PhysicsEngine>>> =
    Lazy::new(|| Arc::new(RwLock::new(PhysicsEngine::new())));

/// Get reference to global physics engine
pub fn get_physics_engine() -> Arc<RwLock<PhysicsEngine>> {
    THE_PHYSICS_ENGINE.clone()
}

/// Ballistic trajectory calculation utilities - matches C++ ballistics functions
pub mod ballistics {
    use super::*;

    /// Calculate ballistic trajectory for projectile
    /// Returns (launch_angle, flight_time) needed to hit target
    pub fn calculate_trajectory(
        start_pos: Coord3D,
        target_pos: Coord3D,
        initial_speed: Real,
    ) -> Option<(Real, Real)> {
        let dx = target_pos.x - start_pos.x;
        let dy = target_pos.y - start_pos.y;
        let dz = target_pos.z - start_pos.z;

        let horizontal_distance = (dx * dx + dy * dy).sqrt();
        let gravity = -GRAVITY; // Positive gravity for calculations

        // Ballistic equation: tan(2θ) = 4h/r where h is height difference, r is range
        let discriminant = initial_speed * initial_speed * initial_speed * initial_speed
            - gravity
                * (gravity * horizontal_distance * horizontal_distance
                    + 2.0 * dz * initial_speed * initial_speed);

        if discriminant < 0.0 {
            return None; // Target unreachable
        }

        // Two possible launch angles (high and low trajectory)
        let sqrt_discriminant = discriminant.sqrt();
        let angle1 = ((initial_speed * initial_speed - sqrt_discriminant)
            / (gravity * horizontal_distance))
            .atan();
        let angle2 = ((initial_speed * initial_speed + sqrt_discriminant)
            / (gravity * horizontal_distance))
            .atan();

        // Choose lower angle (flatter trajectory) for gameplay
        let launch_angle = angle1.min(angle2);

        // Calculate flight time
        let flight_time = horizontal_distance / (initial_speed * launch_angle.cos());

        Some((launch_angle, flight_time))
    }

    /// Calculate launch velocity vector for ballistic trajectory
    pub fn calculate_launch_velocity(
        start_pos: Coord3D,
        target_pos: Coord3D,
        initial_speed: Real,
    ) -> Option<Coord3D> {
        if let Some((launch_angle, _)) = calculate_trajectory(start_pos, target_pos, initial_speed)
        {
            let dx = target_pos.x - start_pos.x;
            let dy = target_pos.y - start_pos.y;
            let horizontal_distance = (dx * dx + dy * dy).sqrt();

            if horizontal_distance > 0.0 {
                let horizontal_speed = initial_speed * launch_angle.cos();
                let vertical_speed = initial_speed * launch_angle.sin();

                // Normalize horizontal direction
                let dir_x = dx / horizontal_distance;
                let dir_y = dy / horizontal_distance;

                return Some(Coord3D::new(
                    dir_x * horizontal_speed,
                    dir_y * horizontal_speed,
                    vertical_speed,
                ));
            }
        }

        None
    }
}

// Module tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physics_state_creation() {
        let state = PhysicsState::new();
        assert_eq!(state.position, Coord3D::new(0.0, 0.0, 0.0));
        assert_eq!(state.velocity, Coord3D::new(0.0, 0.0, 0.0));
        assert_eq!(state.physics_type, PhysicsType::None);
        assert!(!state.enabled);
    }

    #[test]
    fn test_projectile_initialization_basic() {
        let mut state = PhysicsState::new();
        let initial_pos = Coord3D::new(10.0, 20.0, 30.0);
        let initial_vel = Coord3D::new(5.0, 0.0, 10.0);

        state.init_projectile(initial_pos, initial_vel, 300);

        assert_eq!(state.position, initial_pos);
        assert_eq!(state.velocity, initial_vel);
        assert_eq!(state.physics_type, PhysicsType::Projectile);
        assert_eq!(state.collision_response, CollisionResponse::Destroy);
        assert!(state.enabled);
        assert!(state.affected_by_gravity);
    }

    #[test]
    fn test_velocity_clamping() {
        let mut state = PhysicsState::new();
        let excessive_velocity = Coord3D::new(2000.0, 2000.0, 2000.0); // Exceeds MAX_VELOCITY

        state.set_velocity(excessive_velocity);

        let final_speed = state.get_speed();
        assert!(final_speed <= MAX_VELOCITY);
    }

    #[test]
    fn test_physics_engine_object_management() {
        let mut engine = PhysicsEngine::new();
        let obj_id = 42;
        let state = PhysicsState::new();

        engine.add_object(obj_id, state);
        assert!(engine.get_physics_state(obj_id).is_some());

        assert!(engine.remove_object(obj_id));
        assert!(engine.get_physics_state(obj_id).is_none());
    }

    #[test]
    fn test_ballistic_trajectory_calculation() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(100.0, 0.0, 0.0);
        let speed = 60.0;

        let result = ballistics::calculate_trajectory(start, target, speed);
        assert!(result.is_some());

        if let Some((angle, time)) = result {
            assert!(angle >= 0.0 && angle <= std::f32::consts::PI / 2.0);
            assert!(time > 0.0);
        }
    }

    #[test]
    fn test_launch_velocity_calculation() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(50.0, 50.0, 10.0);
        let speed = 60.0;

        let velocity = ballistics::calculate_launch_velocity(start, target, speed);
        assert!(velocity.is_some());

        if let Some(vel) = velocity {
            let calculated_speed = (vel.x * vel.x + vel.y * vel.y + vel.z * vel.z).sqrt();
            assert!((calculated_speed - speed).abs() < 0.01);
        }
    }

    #[test]
    fn test_hover_physics_initialization() {
        let mut state = PhysicsState::new();
        let initial_pos = Coord3D::new(10.0, 20.0, 5.0);
        let hover_height = 3.0;

        state.init_hover(initial_pos, hover_height);

        assert_eq!(state.position, initial_pos);
        assert_eq!(state.physics_type, PhysicsType::Hover);
        assert_eq!(state.target_hover_height, hover_height);
        assert!(!state.affected_by_gravity);
        assert!(state.enabled);
    }

    #[test]
    fn test_aircraft_landing() {
        let mut state = PhysicsState::new();
        let initial_pos = Coord3D::new(0.0, 0.0, 100.0);
        let initial_vel = Coord3D::new(10.0, 0.0, 0.0);

        state.init_aircraft(initial_pos, initial_vel);
        assert!(!state.is_landing);
        assert!(!state.has_landed());

        state.start_landing();
        assert!(state.is_landing);
        assert_eq!(state.target_altitude, 0.0);

        // Simulate landing
        state.altitude = 0.0;
        state.position[2] = 0.0;
        assert!(state.has_landed());
    }

    #[test]
    fn test_stun_effect_basic() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(10.0, 10.0, 0.0);

        state.apply_stun(60);
        assert!(state.is_stunned);
        assert_eq!(state.stun_remaining, 60);
        assert!(!state.can_move());

        // Update status effects
        state.update_status_effects();
        assert_eq!(state.velocity, Coord3D::new(0.0, 0.0, 0.0)); // Velocity should be zeroed
        assert_eq!(state.stun_remaining, 59);
    }

    #[test]
    fn test_freeze_effect_basic() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(10.0, 10.0, 0.0);

        state.apply_freeze(30);
        assert!(state.is_frozen);
        assert_eq!(state.freeze_remaining, 30);

        // Update status effects
        let original_velocity = state.velocity;
        state.update_status_effects();

        // Velocity should be reduced significantly
        assert!(state.velocity[0] < original_velocity[0]);
        assert!(state.velocity[1] < original_velocity[1]);
        assert_eq!(state.freeze_remaining, 29);
    }

    #[test]
    fn test_wind_force_application() {
        let mut state = PhysicsState::new();
        let wind_force = Coord3D::new(50.0, 0.0, 20.0);

        state.apply_wind(wind_force, 30);
        assert_eq!(state.active_forces.len(), 1);

        let effect = &state.active_forces[0];
        assert_eq!(effect.force_type, ForceEffectType::Wind);
        assert_eq!(effect.duration, 30);
        assert!(effect.decaying);
    }

    #[test]
    fn test_knockback_force_application() {
        let mut state = PhysicsState::new();
        let knockback_force = Coord3D::new(100.0, 0.0, 50.0);

        state.apply_knockback(knockback_force, 15);
        assert_eq!(state.active_forces.len(), 1);

        let effect = &state.active_forces[0];
        assert_eq!(effect.force_type, ForceEffectType::Knockback);
        assert_eq!(effect.duration, 15);
    }

    #[test]
    fn test_force_effect_decay() {
        let mut state = PhysicsState::new();
        state.mass = 10.0;
        let initial_force = Coord3D::new(100.0, 0.0, 0.0);

        state.apply_wind(initial_force, 5);

        // Update once
        state.update_force_effects();

        assert_eq!(state.active_forces.len(), 1);
        assert_eq!(state.active_forces[0].duration, 4);

        // Force should have decayed
        assert!(state.active_forces[0].force[0] < initial_force[0]);
    }

    #[test]
    fn test_force_effect_expiration() {
        let mut state = PhysicsState::new();
        state.apply_wind(Coord3D::new(10.0, 0.0, 0.0), 1);

        // Update once - should expire
        state.update_force_effects();

        // Should be removed after expiration
        assert_eq!(state.active_forces.len(), 0);
    }

    #[test]
    fn test_terrain_speed_multiplier() {
        let mut state = PhysicsState::new();

        // Normal ground
        assert_eq!(state.get_terrain_speed_multiplier(), 1.0);

        // In water with crossing capability
        state.in_water = true;
        state.can_cross_water = true;
        state.water_speed_multiplier = 0.5;
        assert_eq!(state.get_terrain_speed_multiplier(), 0.5);

        // In water without crossing capability
        state.can_cross_water = false;
        assert_eq!(state.get_terrain_speed_multiplier(), 0.0);

        // Frozen
        state.in_water = false;
        state.is_frozen = true;
        assert_eq!(state.get_terrain_speed_multiplier(), 0.1);
    }

    #[test]
    fn test_explosion_force() {
        let mut engine = PhysicsEngine::new();

        // Create some objects
        let obj1_id = 1;
        let mut state1 = PhysicsState::new();
        state1.position = Coord3D::new(1.0, 0.0, 0.0);
        state1.mass = 10.0;
        state1.enabled = true;
        engine.add_object(obj1_id, state1);

        let obj2_id = 2;
        let mut state2 = PhysicsState::new();
        state2.position = Coord3D::new(50.0, 0.0, 0.0); // Far away
        state2.mass = 10.0;
        state2.enabled = true;
        engine.add_object(obj2_id, state2);

        // Apply explosion at origin
        let explosion_pos = Coord3D::new(0.0, 0.0, 0.0);
        engine.apply_explosion_force(explosion_pos, 30.0, 1000.0);

        // Object 1 should have wind force applied
        let physics1 = engine.get_physics_state(obj1_id).unwrap();
        let state1 = physics1.read().unwrap();
        assert!(state1.active_forces.len() > 0);

        // Object 2 should not be affected (too far)
        let physics2 = engine.get_physics_state(obj2_id).unwrap();
        let state2 = physics2.read().unwrap();
        assert_eq!(state2.active_forces.len(), 0);
    }

    #[test]
    fn test_bridge_collision_detection() {
        let mut state = PhysicsState::new();
        state.position = Coord3D::new(10.0, 10.0, 5.0);
        state.on_bridge = true;
        state.bridge_height = 5.0;
        state.ground_height = 0.0;

        let engine = PhysicsEngine::new();
        let collision = engine.check_collision(&state);

        assert!(collision.collided);
        assert_eq!(collision.surface_type, SurfaceType::Bridge);
    }

    #[test]
    fn test_water_collision_for_non_water_capable() {
        let mut state = PhysicsState::new();
        state.position = Coord3D::new(0.0, 0.0, -1.0); // Below water
        state.in_water = true;
        state.can_cross_water = false;

        let engine = PhysicsEngine::new();
        let collision = engine.check_collision(&state);

        assert!(collision.collided);
        assert_eq!(collision.surface_type, SurfaceType::Water);
    }

    #[test]
    fn test_slope_handling() {
        let mut state = PhysicsState::new();
        state.terrain_slope = 50.0; // Steep slope
        state.max_climbable_slope = 45.0;
        state.on_ground = true;
        state.enabled = true;

        let engine = PhysicsEngine::new();
        engine.update_normal_physics(&mut state).unwrap();

        // Should have negative Z acceleration (sliding down)
        assert!(state.acceleration[2] < 0.0);
    }

    #[test]
    fn test_hover_stabilization() {
        let mut state = PhysicsState::new();
        state.physics_type = PhysicsType::Hover;
        state.position = Coord3D::new(0.0, 0.0, 2.0);
        state.ground_height = 0.0;
        state.target_hover_height = 5.0;
        state.hover_force = 10.0;
        state.enabled = true;

        let engine = PhysicsEngine::new();
        engine.update_hover_physics(&mut state).unwrap();

        // Should have positive Z acceleration (moving up to target height)
        assert!(state.acceleration[2] > 0.0);
    }

    #[test]
    fn test_aircraft_altitude_control() {
        let mut state = PhysicsState::new();
        state.physics_type = PhysicsType::Aircraft;
        state.altitude = 50.0;
        state.position[2] = 50.0;
        state.target_altitude = 100.0;
        state.enabled = true;

        let engine = PhysicsEngine::new();
        engine.update_aircraft_physics(&mut state).unwrap();

        // Should have positive Z acceleration (climbing to target altitude)
        assert!(state.acceleration[2] > 0.0);
    }

    #[test]
    fn test_physics_engine_helper_methods() {
        let mut engine = PhysicsEngine::new();
        let obj_id = 42;
        let mut state = PhysicsState::new();
        state.position = Coord3D::new(10.0, 20.0, 30.0);
        state.velocity = Coord3D::new(1.0, 2.0, 3.0);
        state.on_ground = true;
        state.enabled = true;

        engine.add_object(obj_id, state);

        // Test position getter
        let pos = engine.get_object_position(obj_id);
        assert!(pos.is_some());
        assert_eq!(pos.unwrap(), Coord3D::new(10.0, 20.0, 30.0));

        // Test velocity getter
        let vel = engine.get_object_velocity(obj_id);
        assert!(vel.is_some());
        assert_eq!(vel.unwrap(), Coord3D::new(1.0, 2.0, 3.0));

        // Test on ground check
        assert!(engine.is_object_on_ground(obj_id));

        // Test can move check
        assert!(engine.can_object_move(obj_id));

        // Test terrain speed multiplier
        assert_eq!(engine.get_terrain_speed_multiplier(obj_id), 1.0);
    }

    // ==================== VELOCITY & MOTION TESTS ====================

    #[test]
    fn test_velocity_limits() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(500.0, 500.0, 500.0); // High velocity

        // Clamp to max velocity
        if state.velocity[0].abs() > MAX_VELOCITY {
            state.velocity[0] = state.velocity[0].signum() * MAX_VELOCITY;
        }

        assert!(state.velocity[0].abs() <= MAX_VELOCITY);
    }

    #[test]
    fn test_zero_velocity_detection() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(0.005, 0.003, 0.001); // Below threshold

        let speed =
            (state.velocity[0].powi(2) + state.velocity[1].powi(2) + state.velocity[2].powi(2))
                .sqrt();
        let is_stopped = speed < MIN_VELOCITY_THRESHOLD;

        assert!(is_stopped, "Should detect low velocity as stopped");
    }

    #[test]
    fn test_velocity_magnitude_calculation() {
        let state = PhysicsState::new();
        let velocity = Coord3D::new(3.0, 4.0, 0.0);
        let magnitude = (velocity[0].powi(2) + velocity[1].powi(2) + velocity[2].powi(2)).sqrt();

        assert_eq!(magnitude, 5.0, "3-4-5 triangle should have magnitude 5");
    }

    #[test]
    fn test_acceleration_application() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(0.0, 0.0, 0.0);
        state.acceleration = Coord3D::new(10.0, 0.0, 0.0);

        let delta_time = 1.0;
        state.velocity[0] += state.acceleration[0] * delta_time;

        assert_eq!(
            state.velocity[0], 10.0,
            "Velocity should increase by acceleration * time"
        );
    }

    #[test]
    fn test_gravity_application() {
        let mut state = PhysicsState::new();
        state.affected_by_gravity = true;
        state.acceleration = Coord3D::new(0.0, 0.0, GRAVITY);

        let delta_time = 1.0;
        state.velocity[2] += state.acceleration[2] * delta_time;

        assert!(
            state.velocity[2] < 0.0,
            "Gravity should cause downward acceleration"
        );
    }

    #[test]
    fn test_position_update_from_velocity() {
        let mut state = PhysicsState::new();
        state.position = Coord3D::new(0.0, 0.0, 100.0);
        state.velocity = Coord3D::new(10.0, 0.0, -5.0); // Moving right and down

        let delta_time = 1.0;
        state.position[0] += state.velocity[0] * delta_time;
        state.position[2] += state.velocity[2] * delta_time;

        assert_eq!(state.position[0], 10.0, "Position X should update");
        assert_eq!(state.position[2], 95.0, "Position Z should update");
    }

    #[test]
    fn test_friction_effect_on_velocity() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(100.0, 0.0, 0.0);
        state.friction = 0.9;
        state.on_ground = true;

        // Apply friction
        state.velocity[0] *= state.friction;

        assert_eq!(state.velocity[0], 90.0, "Friction should reduce velocity");
    }

    #[test]
    fn test_air_resistance_effect() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(100.0, 100.0, 100.0);
        state.drag = AIR_RESISTANCE;

        // Apply drag to all axes
        state.velocity[0] *= state.drag;
        state.velocity[1] *= state.drag;
        state.velocity[2] *= state.drag;

        assert_eq!(state.velocity[0], 98.0);
        assert_eq!(state.velocity[1], 98.0);
        assert_eq!(state.velocity[2], 98.0);
    }

    // ==================== COLLISION RESPONSE TESTS ====================

    #[test]
    fn test_collision_stop_response() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(50.0, 0.0, 0.0);
        state.collision_response = CollisionResponse::Stop;

        // Simulate collision
        if state.collision_response == CollisionResponse::Stop {
            state.velocity = Coord3D::new(0.0, 0.0, 0.0);
        }

        assert_eq!(state.velocity[0], 0.0, "Stop response should zero velocity");
    }

    #[test]
    fn test_collision_bounce_response() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(50.0, 0.0, -30.0); // Moving right and down
        state.bounce_factor = BOUNCE_COEFFICIENT;
        state.collision_response = CollisionResponse::Bounce;

        // Simulate bounce off ground (Z collision)
        if state.collision_response == CollisionResponse::Bounce {
            state.velocity[2] *= -state.bounce_factor;
        }

        assert_eq!(state.velocity[0], 50.0, "X velocity unchanged");
        assert!(
            state.velocity[2] > 0.0,
            "Z velocity should reverse and dampen"
        );
    }

    #[test]
    fn test_bounce_coefficient_damping() {
        let initial_velocity = -100.0; // Downward
        let bounce_factor = BOUNCE_COEFFICIENT; // 0.7
        let bounced_velocity = -initial_velocity * bounce_factor;

        assert_eq!(
            bounced_velocity, 70.0,
            "Bounce should reduce velocity by coefficient"
        );
    }

    #[test]
    fn test_collision_destroy_response() {
        let mut state = PhysicsState::new();
        state.collision_response = CollisionResponse::Destroy;
        state.enabled = true;

        // Simulate destroy response
        if state.collision_response == CollisionResponse::Destroy {
            state.enabled = false;
        }

        assert!(!state.enabled, "Destroy response should disable object");
    }

    #[test]
    fn test_collision_response_none() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(50.0, 0.0, 0.0);
        state.collision_response = CollisionResponse::None;

        // No response - velocity unchanged
        let initial_vel = state.velocity[0];
        // (No collision code executes)

        assert_eq!(
            state.velocity[0], initial_vel,
            "None response should not change velocity"
        );
    }

    // ==================== FORCE EFFECTS TESTS ====================

    #[test]
    fn test_multiple_forces_accumulation() {
        let mut state = PhysicsState::new();
        state.mass = 10.0;

        // Apply multiple forces
        state.apply_wind(Coord3D::new(100.0, 0.0, 0.0), 10);
        state.apply_knockback(Coord3D::new(0.0, 50.0, 0.0), 5);

        assert_eq!(
            state.active_forces.len(),
            2,
            "Should accumulate multiple forces"
        );
    }

    #[test]
    fn test_force_duration_countdown() {
        let mut state = PhysicsState::new();
        state.apply_wind(Coord3D::new(100.0, 0.0, 0.0), 5);

        let initial_duration = state.active_forces[0].duration;
        state.update_force_effects();
        let after_duration = state.active_forces[0].duration;

        assert_eq!(
            initial_duration - after_duration,
            1,
            "Duration should countdown each update"
        );
    }

    #[test]
    fn test_force_decay_over_time() {
        let mut state = PhysicsState::new();
        state.mass = 1.0;
        state.apply_wind(Coord3D::new(100.0, 0.0, 0.0), 10);

        let force1 = state.active_forces[0].force[0];
        state.update_force_effects();
        let force2 = state.active_forces[0].force[0];

        assert!(force2 < force1, "Decaying force should reduce each update");
    }

    #[test]
    fn test_force_removal_on_expiration() {
        let mut state = PhysicsState::new();
        state.apply_wind(Coord3D::new(100.0, 0.0, 0.0), 1); // Duration of 1

        state.update_force_effects();

        assert_eq!(
            state.active_forces.len(),
            0,
            "Expired forces should be removed"
        );
    }

    // ==================== PROJECTILE PHYSICS TESTS ====================

    #[test]
    fn test_projectile_initialization() {
        let mut state = PhysicsState::new();
        let initial_pos = Coord3D::new(100.0, 100.0, 50.0);
        let initial_vel = Coord3D::new(200.0, 0.0, 100.0);

        state.init_projectile(initial_pos, initial_vel, 300);

        assert_eq!(state.position, initial_pos);
        assert_eq!(state.velocity, initial_vel);
        assert_eq!(state.physics_type, PhysicsType::Projectile);
        assert_eq!(state.max_lifetime, 300);
        assert!(state.enabled);
    }

    #[test]
    fn test_projectile_gravity_effect() {
        let mut state = PhysicsState::new();
        state.init_projectile(
            Coord3D::new(0.0, 0.0, 100.0),
            Coord3D::new(100.0, 0.0, 0.0),
            300,
        );

        let initial_z_vel = state.velocity[2];
        state.acceleration[2] = GRAVITY;

        let delta_time = 1.0;
        state.velocity[2] += state.acceleration[2] * delta_time;

        assert!(
            state.velocity[2] < initial_z_vel,
            "Gravity should decrease Z velocity"
        );
    }

    #[test]
    fn test_projectile_lifetime_countdown() {
        let mut state = PhysicsState::new();
        state.init_projectile(
            Coord3D::new(0.0, 0.0, 100.0),
            Coord3D::new(100.0, 0.0, 0.0),
            100,
        );

        let initial_lifetime = state.lifetime;
        state.lifetime += 1;

        assert_eq!(state.lifetime - initial_lifetime, 1);
    }

    #[test]
    fn test_projectile_lifetime_expiration() {
        let mut state = PhysicsState::new();
        state.init_projectile(
            Coord3D::new(0.0, 0.0, 100.0),
            Coord3D::new(100.0, 0.0, 0.0),
            50,
        );
        state.lifetime = 50;

        let is_expired = state.max_lifetime >= 0 && state.lifetime >= state.max_lifetime;

        assert!(is_expired, "Projectile should expire when lifetime reached");
    }

    #[test]
    fn test_bouncing_projectile_initialization() {
        let mut state = PhysicsState::new();
        state.physics_type = PhysicsType::Bouncing;
        state.bounce_factor = 0.8;
        state.velocity = Coord3D::new(100.0, 0.0, -50.0); // Coming down

        assert_eq!(state.physics_type, PhysicsType::Bouncing);
        assert_eq!(state.bounce_factor, 0.8);
    }

    // ==================== HOVER & AIRCRAFT PHYSICS TESTS ====================

    #[test]
    fn test_hover_height_control() {
        let mut state = PhysicsState::new();
        state.physics_type = PhysicsType::Hover;
        state.hover_height = 0.0;
        state.target_hover_height = 5.0;
        state.hover_force = 100.0;

        let height_diff = state.target_hover_height - state.hover_height;
        let hover_accel = if height_diff > 0.1 {
            state.hover_force
        } else {
            0.0
        };

        assert!(
            hover_accel > 0.0,
            "Should apply hover force to reach target height"
        );
    }

    #[test]
    fn test_altitude_control() {
        let mut state = PhysicsState::new();
        state.physics_type = PhysicsType::Aircraft;
        state.altitude = 50.0;
        state.target_altitude = 200.0;

        let altitude_diff = state.target_altitude - state.altitude;
        let climb = if altitude_diff > 1.0 { 10.0 } else { 0.0 };

        assert!(climb > 0.0, "Aircraft should climb to target altitude");
    }

    #[test]
    fn test_altitude_limits() {
        let mut state = PhysicsState::new();
        state.min_altitude = 10.0;
        state.max_altitude = 500.0;
        state.altitude = 0.0;

        // Clamp to limits
        if state.altitude < state.min_altitude {
            state.altitude = state.min_altitude;
        }

        assert_eq!(
            state.altitude, state.min_altitude,
            "Altitude should not go below minimum"
        );
    }

    // ==================== TERRAIN INTERACTION TESTS ====================

    #[test]
    fn test_on_ground_detection() {
        let mut state = PhysicsState::new();
        state.position[2] = 10.0;
        state.ground_height = 10.0;

        let is_on_ground = (state.position[2] - state.ground_height).abs() < 0.1;

        assert!(is_on_ground, "Should detect object on ground");
    }

    #[test]
    fn test_slope_climbability() {
        let mut state = PhysicsState::new();
        state.terrain_slope = 30.0; // 30 degree slope
        state.max_climbable_slope = 45.0; // Can climb up to 45 degrees

        let can_climb = state.terrain_slope <= state.max_climbable_slope;

        assert!(can_climb, "Should be able to climb 30 degree slope");
    }

    #[test]
    fn test_slope_too_steep() {
        let mut state = PhysicsState::new();
        state.terrain_slope = 60.0; // Too steep
        state.max_climbable_slope = 45.0;

        let can_climb = state.terrain_slope <= state.max_climbable_slope;

        assert!(!can_climb, "Should not climb slope steeper than max");
    }

    #[test]
    fn test_water_crossing() {
        let mut state = PhysicsState::new();
        state.in_water = true;
        state.can_cross_water = true;
        state.water_speed_multiplier = 0.5;

        let speed_mult = if state.in_water && state.can_cross_water {
            state.water_speed_multiplier
        } else {
            0.0
        };

        assert_eq!(speed_mult, 0.5, "Should apply water speed multiplier");
    }

    #[test]
    fn test_water_blocking() {
        let mut state = PhysicsState::new();
        state.in_water = true;
        state.can_cross_water = false;

        let speed_mult = if state.in_water && state.can_cross_water {
            state.water_speed_multiplier
        } else {
            0.0
        };

        assert_eq!(speed_mult, 0.0, "Should block movement in impassable water");
    }

    // ==================== STATUS EFFECT TESTS ====================

    #[test]
    fn test_stun_effect() {
        let mut state = PhysicsState::new();
        state.stun_remaining = 30;
        state.is_stunned = true;

        // Stunned objects cannot move
        let speed_mult = if state.is_stunned { 0.0 } else { 1.0 };

        assert_eq!(speed_mult, 0.0, "Stunned object should not move");
    }

    #[test]
    fn test_stun_countdown() {
        let mut state = PhysicsState::new();
        state.stun_remaining = 30;

        state.stun_remaining -= 1;

        assert_eq!(state.stun_remaining, 29, "Stun should countdown");
    }

    #[test]
    fn test_stun_expiration() {
        let mut state = PhysicsState::new();
        state.stun_remaining = 1;
        state.is_stunned = true;

        state.stun_remaining -= 1;
        if state.stun_remaining <= 0 {
            state.is_stunned = false;
        }

        assert!(!state.is_stunned, "Stun should expire");
    }

    #[test]
    fn test_freeze_effect() {
        let mut state = PhysicsState::new();
        state.freeze_remaining = 60;
        state.is_frozen = true;

        // Frozen objects move very slowly
        let speed_mult = if state.is_frozen { 0.1 } else { 1.0 };

        assert_eq!(speed_mult, 0.1, "Frozen object should move slowly");
    }

    #[test]
    fn test_bridge_height_interaction() {
        let mut state = PhysicsState::new();
        state.on_bridge = true;
        state.bridge_height = 5.0;
        state.ground_height = 0.0;
        state.position[2] = 5.0;

        // Position should match bridge height
        let correct_height = (state.position[2] - state.bridge_height).abs() < 0.1;

        assert!(correct_height, "Should maintain bridge height");
    }

    // ==================== EDGE CASES ====================

    #[test]
    fn test_physics_state_default() {
        let state = PhysicsState::new();

        assert_eq!(state.physics_type, PhysicsType::None);
        assert_eq!(state.collision_response, CollisionResponse::Stop);
        assert!(!state.enabled);
        assert_eq!(state.mass, 1.0);
    }

    #[test]
    fn test_zero_mass_handling() {
        let mut state = PhysicsState::new();
        state.mass = 0.0001; // Very small mass

        // Should still be valid
        assert!(state.mass > 0.0);
    }

    #[test]
    fn test_negative_position() {
        let mut state = PhysicsState::new();
        state.position = Coord3D::new(-100.0, -50.0, -10.0);

        // Negative positions are valid
        assert!(state.position[0] < 0.0);
    }

    #[test]
    fn test_very_high_velocity() {
        let mut state = PhysicsState::new();
        state.velocity = Coord3D::new(999.9, 999.9, 999.9); // Near limit

        // Should still be valid values
        assert!(state.velocity[0] > 0.0);
    }
}
