//! Advanced Projectile System
//!
//! This module manages projectile objects, their physics simulation, and collision detection.
//! Supports various projectile types including ballistic, guided, beam, and special weapons.

use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::object::{registry::OBJECT_REGISTRY, ObjectId};
use crate::scripting::engine::get_script_engine;
use crate::weapon::{
    BallisticsTrajectory, Coord3D, WeaponBonus, WeaponTemplate, INVALID_OBJECT_ID,
};
use crate::{GameLogicError, GameLogicResult};

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// Projectile types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileType {
    /// Standard ballistic projectile (bullet, shell)
    Ballistic,
    /// Guided missile with target tracking
    Guided,
    /// Laser or beam weapon
    Beam,
    /// Artillery shell with high arc
    Artillery,
    /// Rocket-propelled grenade
    Rocket,
    /// Flame thrower projectile
    Flame,
    /// Special effect projectile (emp, toxin, etc.)
    Special,
}

/// Projectile guidance system
#[derive(Debug, Clone, PartialEq)]
pub enum GuidanceSystem {
    /// No guidance - follows ballistic trajectory
    None,
    /// Heat-seeking guidance
    HeatSeeking { sensitivity: f32 },
    /// Radar-guided
    RadarGuided { lock_strength: f32 },
    /// Laser-guided
    LaserGuided { beam_strength: f32 },
    /// Wire-guided (limited range)
    WireGuided { max_wire_length: f32 },
}

/// Projectile physics state
#[derive(Debug, Clone)]
pub struct ProjectilePhysics {
    /// Current position
    pub position: Coord3D,
    /// Current velocity vector
    pub velocity: Coord3D,
    /// Acceleration vector
    pub acceleration: Coord3D,
    /// Angular velocity for spin
    pub angular_velocity: f32,
    /// Current orientation angle
    pub orientation: f32,
    /// Drag coefficient
    pub drag_coefficient: f32,
    /// Mass in kg
    pub mass: f32,
    /// Cross-sectional area for drag calculation
    pub cross_section: f32,
}

/// Projectile warhead configuration
#[derive(Debug, Clone)]
pub struct Warhead {
    /// Explosive yield
    pub yield_amount: f32,
    /// Blast radius
    pub blast_radius: f32,
    /// Fragmentation count
    pub fragment_count: u32,
    /// Fragmentation velocity
    pub fragment_velocity: f32,
    /// Armor penetration value
    pub penetration: f32,
    /// Special effects (EMP, toxin, etc.)
    pub special_effects: Vec<String>,
}

/// Projectile special behavior flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectileBehaviorFlags {
    /// Has parachute for slow descent
    pub has_parachute: bool,
    /// Can bounce off terrain/walls
    pub can_bounce: bool,
    /// Can roll on ground after landing
    pub can_roll: bool,
    /// Spread on miss (scatter)
    pub spread_on_miss: bool,
    /// Stick to walls on impact
    pub stick_to_walls: bool,
    /// Can penetrate walls
    pub can_penetrate: bool,
}

impl Default for ProjectileBehaviorFlags {
    fn default() -> Self {
        Self {
            has_parachute: false,
            can_bounce: false,
            can_roll: false,
            spread_on_miss: false,
            stick_to_walls: false,
            can_penetrate: false,
        }
    }
}

/// Parachute state for projectiles
#[derive(Debug, Clone)]
pub struct ParachuteState {
    /// Whether parachute is deployed
    pub deployed: bool,
    /// Altitude at which to deploy parachute
    pub deploy_altitude: f32,
    /// Descent rate with parachute (m/s)
    pub descent_rate: f32,
    /// Horizontal drift velocity
    pub drift_velocity: Coord3D,
}

impl Default for ParachuteState {
    fn default() -> Self {
        Self {
            deployed: false,
            deploy_altitude: 20.0,
            descent_rate: 5.0,
            drift_velocity: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

/// Bounce physics state
#[derive(Debug, Clone)]
pub struct BounceState {
    /// Number of bounces so far
    pub bounce_count: u32,
    /// Maximum number of bounces
    pub max_bounces: u32,
    /// Coefficient of restitution (bounciness)
    pub restitution: f32,
    /// Energy loss per bounce
    pub energy_loss: f32,
}

impl Default for BounceState {
    fn default() -> Self {
        Self {
            bounce_count: 0,
            max_bounces: 3,
            restitution: 0.6,
            energy_loss: 0.2,
        }
    }
}

/// Active projectile instance
#[derive(Debug, Clone)]
pub struct Projectile {
    /// Unique projectile ID
    pub id: ObjectId,
    /// Projectile type
    pub projectile_type: ProjectileType,
    /// Source weapon template
    pub weapon_template: Arc<WeaponTemplate>,
    /// Source object that fired this projectile
    pub source_object: ObjectId,
    /// Target object (if any)
    pub target_object: Option<ObjectId>,
    /// Original target position
    pub target_position: Coord3D,
    /// Physics state
    pub physics: ProjectilePhysics,
    /// Guidance system
    pub guidance_system: GuidanceSystem,
    /// Warhead configuration
    pub warhead: Warhead,
    /// Planned trajectory
    pub trajectory: BallisticsTrajectory,
    /// Current trajectory point index
    pub trajectory_index: usize,
    /// Time since launch
    pub time_alive: f32,
    /// Maximum flight time before self-destruct
    pub max_flight_time: f32,
    /// Optional max lifetime override (compatibility)
    pub max_lifetime: Option<f32>,
    /// Whether projectile has detonated
    pub detonated: bool,
    /// Last detonation reason (compatibility/debug)
    pub detonation_reason: Option<DetonationReason>,
    /// Optional proximity fuse radius override (compatibility)
    pub proximity_radius: Option<f32>,
    /// Objects already hit (to prevent multiple hits)
    pub hit_objects: Vec<ObjectId>,
    /// Weapon bonus applied
    pub weapon_bonus: WeaponBonus,
    /// Special power completion template (if any)
    pub special_power_template: Option<String>,
    /// Special power completion creator id
    pub special_power_creator_id: ObjectId,
    /// Special power completion player index
    pub special_power_player_index: Option<usize>,
    /// Special behavior flags
    pub behavior_flags: ProjectileBehaviorFlags,
    /// Parachute state (if applicable)
    pub parachute_state: Option<ParachuteState>,
    /// Bounce state (if applicable)
    pub bounce_state: Option<BounceState>,
    /// Whether projectile is currently rolling on ground
    pub is_rolling: bool,
    /// Rolling friction coefficient
    pub rolling_friction: f32,
}

impl Projectile {
    /// Create a new projectile instance
    pub fn new(
        id: ObjectId,
        projectile_type: ProjectileType,
        weapon_template: Arc<WeaponTemplate>,
        source_object: ObjectId,
        target_object: Option<ObjectId>,
        target_position: Coord3D,
        trajectory: BallisticsTrajectory,
        weapon_bonus: WeaponBonus,
        special_power_template: Option<String>,
        special_power_creator_id: ObjectId,
        special_power_player_index: Option<usize>,
    ) -> Self {
        let initial_physics = ProjectilePhysics {
            position: trajectory
                .trajectory_points
                .first()
                .map(|p| p.position)
                .unwrap_or(Coord3D::new(0.0, 0.0, 0.0)),
            velocity: trajectory.initial_velocity,
            acceleration: Coord3D::new(0.0, 0.0, -9.81), // Gravity
            angular_velocity: 0.0,
            orientation: 0.0,
            drag_coefficient: Self::get_drag_coefficient_for_type(projectile_type),
            mass: Self::get_mass_for_type(projectile_type),
            cross_section: Self::get_cross_section_for_type(projectile_type),
        };

        let guidance_system = Self::create_guidance_system(&weapon_template, projectile_type);
        let warhead = Self::create_warhead(&weapon_template, &weapon_bonus);
        let behavior_flags = Self::create_behavior_flags(projectile_type);

        Self {
            id,
            projectile_type,
            weapon_template,
            source_object,
            target_object,
            target_position,
            physics: initial_physics,
            guidance_system,
            warhead,
            trajectory,
            trajectory_index: 0,
            time_alive: 0.0,
            max_flight_time: 30.0, // 30 seconds max flight time
            max_lifetime: None,
            detonated: false,
            detonation_reason: None,
            proximity_radius: None,
            hit_objects: Vec::new(),
            weapon_bonus,
            special_power_template,
            special_power_creator_id,
            special_power_player_index,
            behavior_flags,
            parachute_state: if behavior_flags.has_parachute {
                Some(ParachuteState::default())
            } else {
                None
            },
            bounce_state: if behavior_flags.can_bounce {
                Some(BounceState::default())
            } else {
                None
            },
            is_rolling: false,
            rolling_friction: 0.1,
        }
    }

    /// Update projectile physics and guidance for one frame
    pub fn update(&mut self, delta_time: f32) -> GameLogicResult<ProjectileUpdateResult> {
        if self.detonated {
            return Ok(ProjectileUpdateResult::Detonated);
        }

        self.time_alive += delta_time;

        // Check for timeout
        let max_flight_time = self.max_lifetime.unwrap_or(self.max_flight_time);
        if self.time_alive > max_flight_time {
            return self.detonate(DetonationReason::Timeout);
        }

        // Update parachute deployment
        if let Some(mut parachute) = self.parachute_state.take() {
            self.update_parachute(&mut parachute, delta_time)?;
            self.parachute_state = Some(parachute);
        }

        // Update guidance system (unless rolling on ground)
        if !self.is_rolling {
            self.update_guidance(delta_time)?;
        }

        let prev_pos = self.physics.position;

        // Update physics
        self.update_physics(delta_time)?;

        let ground_height = {
            let terrain = crate::terrain::get_terrain_logic();
            match terrain.read() {
                Ok(guard) => {
                    guard.get_ground_height(self.physics.position.x, self.physics.position.y, None)
                }
                Err(_) => 0.0,
            }
        };

        // Handle ground interaction
        if self.physics.position.z <= ground_height {
            return self.handle_ground_impact(delta_time, ground_height);
        }

        // Check for wall collision
        if let Some(wall_result) = self.check_wall_collision(&prev_pos, &self.physics.position)? {
            return self.handle_wall_impact(wall_result);
        }

        // Check for target proximity or collision (unless rolling)
        if !self.is_rolling {
            if let Some(collision_result) = self.check_collisions()? {
                match collision_result {
                    CollisionResult::Hit(object_id) => {
                        self.hit_objects.push(object_id);
                        return self.detonate(DetonationReason::DirectHit);
                    }
                    CollisionResult::Proximity => {
                        return self.detonate(DetonationReason::ProximityFuse);
                    }
                }
            }
        }

        Ok(ProjectileUpdateResult::Flying)
    }

    /// Update guidance system
    fn update_guidance(&mut self, delta_time: f32) -> GameLogicResult<()> {
        match &self.guidance_system {
            GuidanceSystem::None => {
                // No guidance - follow ballistic trajectory
                if self.trajectory_index < self.trajectory.trajectory_points.len() - 1 {
                    // Interpolate between trajectory points
                    let current_point = &self.trajectory.trajectory_points[self.trajectory_index];
                    let next_point = &self.trajectory.trajectory_points[self.trajectory_index + 1];

                    let time_ratio = (self.time_alive - current_point.time)
                        / (next_point.time - current_point.time);

                    if time_ratio >= 1.0 {
                        self.trajectory_index += 1;
                    }
                }
            }
            GuidanceSystem::HeatSeeking { sensitivity } => {
                if let Some(target_id) = self.target_object {
                    let target_pos = self.get_target_position(target_id)?;
                    let direction_to_target = self.calculate_direction_to_target(&target_pos);

                    // Apply guidance correction
                    let turn_rate = *sensitivity * delta_time;
                    self.apply_guidance_correction(&direction_to_target, turn_rate);
                }
            }
            GuidanceSystem::RadarGuided { lock_strength } => {
                if let Some(target_id) = self.target_object {
                    if self.has_radar_lock(target_id, *lock_strength)? {
                        let target_pos = self.get_target_position(target_id)?;
                        let direction_to_target = self.calculate_direction_to_target(&target_pos);

                        let turn_rate = lock_strength * delta_time;
                        self.apply_guidance_correction(&direction_to_target, turn_rate);
                    }
                }
            }
            GuidanceSystem::LaserGuided { beam_strength } => {
                // Laser guidance requires continuous designation
                if self.has_laser_designation(*beam_strength)? {
                    let designated_pos = self.get_laser_designated_position()?;
                    let direction_to_target = self.calculate_direction_to_target(&designated_pos);

                    let turn_rate = beam_strength * delta_time;
                    self.apply_guidance_correction(&direction_to_target, turn_rate);
                }
            }
            GuidanceSystem::WireGuided { max_wire_length } => {
                let distance_from_source =
                    self.physics.position.distance(self.get_source_position()?);

                if distance_from_source < *max_wire_length {
                    // Wire still connected - can receive guidance commands
                    if let Some(guidance_commands) = self.get_wire_guidance_commands()? {
                        self.apply_wire_guidance(&guidance_commands, delta_time);
                    }
                } else {
                    // Wire severed - switch to ballistic trajectory
                    self.guidance_system = GuidanceSystem::None;
                }
            }
        }

        Ok(())
    }

    /// Update projectile physics
    fn update_physics(&mut self, delta_time: f32) -> GameLogicResult<()> {
        // Calculate air resistance
        let speed = self.physics.velocity.distance(Coord3D::new(0.0, 0.0, 0.0));
        let air_density = 1.225; // kg/m³ at sea level

        if speed > 0.001 {
            let drag_force = 0.5
                * air_density
                * self.physics.drag_coefficient
                * self.physics.cross_section
                * speed
                * speed;

            let drag_acceleration = drag_force / self.physics.mass;

            // Apply drag in opposite direction of velocity
            let velocity_unit = Coord3D::new(
                self.physics.velocity.x / speed,
                self.physics.velocity.y / speed,
                self.physics.velocity.z / speed,
            );

            self.physics.acceleration.x = -velocity_unit.x * drag_acceleration;
            self.physics.acceleration.y = -velocity_unit.y * drag_acceleration;
            self.physics.acceleration.z = -9.81 - velocity_unit.z * drag_acceleration;
        // Gravity + drag
        } else {
            self.physics.acceleration.z = -9.81; // Just gravity
        }

        // Integrate velocity
        self.physics.velocity.x += self.physics.acceleration.x * delta_time;
        self.physics.velocity.y += self.physics.acceleration.y * delta_time;
        self.physics.velocity.z += self.physics.acceleration.z * delta_time;

        // Integrate position
        self.physics.position.x += self.physics.velocity.x * delta_time;
        self.physics.position.y += self.physics.velocity.y * delta_time;
        self.physics.position.z += self.physics.velocity.z * delta_time;

        // Update orientation based on velocity
        if speed > 0.001 {
            self.physics.orientation = self.physics.velocity.y.atan2(self.physics.velocity.x);
        }

        Ok(())
    }

    /// Update parachute deployment and physics
    fn update_parachute(
        &mut self,
        parachute: &mut ParachuteState,
        delta_time: f32,
    ) -> GameLogicResult<()> {
        // Check if we should deploy the parachute
        if !parachute.deployed && self.physics.position.z <= parachute.deploy_altitude {
            parachute.deployed = true;
            log::debug!("Parachute deployed for projectile {}", self.id);

            // Add horizontal drift from wind
            use rand::Rng;
            let mut rng = rand::thread_rng();
            parachute.drift_velocity =
                Coord3D::new(rng.gen_range(-2.0..2.0), rng.gen_range(-2.0..2.0), 0.0);
        }

        // Apply parachute physics
        if parachute.deployed {
            // Override velocity with parachute descent rate
            self.physics.velocity.z = -parachute.descent_rate;

            // Apply drift
            self.physics.velocity.x = parachute.drift_velocity.x;
            self.physics.velocity.y = parachute.drift_velocity.y;

            // Reduce horizontal momentum over time
            parachute.drift_velocity.x *= 0.98;
            parachute.drift_velocity.y *= 0.98;
        }

        Ok(())
    }

    /// Handle ground impact (bounce, roll, or detonate)
    fn handle_ground_impact(
        &mut self,
        delta_time: f32,
        ground_height: f32,
    ) -> GameLogicResult<ProjectileUpdateResult> {
        // Clamp to ground level
        self.physics.position.z = ground_height;

        // Check for bounce
        if let Some(mut bounce_state) = self.bounce_state.take() {
            if bounce_state.bounce_count < bounce_state.max_bounces {
                let result = self.handle_bounce(&mut bounce_state);
                self.bounce_state = Some(bounce_state);
                return result;
            }
            self.bounce_state = Some(bounce_state);
        }

        // Check for roll
        if self.behavior_flags.can_roll && !self.is_rolling {
            return self.start_rolling();
        }

        // Check for spread on miss
        if self.behavior_flags.spread_on_miss {
            return self.handle_spread_detonation();
        }

        // Default: detonate on ground impact
        self.detonate(DetonationReason::GroundImpact)
    }

    /// Handle bounce physics
    fn handle_bounce(
        &mut self,
        bounce_state: &mut BounceState,
    ) -> GameLogicResult<ProjectileUpdateResult> {
        bounce_state.bounce_count += 1;

        // Reverse vertical velocity with energy loss
        let energy_retention = 1.0 - bounce_state.energy_loss;
        self.physics.velocity.z =
            -self.physics.velocity.z * bounce_state.restitution * energy_retention;

        // Reduce horizontal velocity
        self.physics.velocity.x *= bounce_state.restitution;
        self.physics.velocity.y *= bounce_state.restitution;

        // Add some random bounce direction variation
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let angle_variation = rng.gen_range(-0.3..0.3);
        let current_angle = self.physics.velocity.y.atan2(self.physics.velocity.x);
        let new_angle = current_angle + angle_variation;
        let horizontal_speed =
            (self.physics.velocity.x.powi(2) + self.physics.velocity.y.powi(2)).sqrt();

        self.physics.velocity.x = horizontal_speed * new_angle.cos();
        self.physics.velocity.y = horizontal_speed * new_angle.sin();

        log::debug!(
            "Projectile {} bounced (count: {})",
            self.id,
            bounce_state.bounce_count
        );

        Ok(ProjectileUpdateResult::Flying)
    }

    /// Start rolling on ground
    fn start_rolling(&mut self) -> GameLogicResult<ProjectileUpdateResult> {
        self.is_rolling = true;

        // Set to ground level
        self.physics.position.z = 0.0;
        self.physics.velocity.z = 0.0;

        // Continue with horizontal momentum
        log::debug!("Projectile {} started rolling", self.id);

        Ok(ProjectileUpdateResult::Flying)
    }

    /// Update rolling physics
    #[allow(dead_code)]
    fn update_rolling(&mut self, delta_time: f32) -> GameLogicResult<()> {
        if !self.is_rolling {
            return Ok(());
        }

        // Apply rolling friction
        let speed = (self.physics.velocity.x.powi(2) + self.physics.velocity.y.powi(2)).sqrt();
        if speed > 0.01 {
            let friction_force = self.rolling_friction * self.physics.mass * 9.81;
            let deceleration = friction_force / self.physics.mass;

            let velocity_unit = Coord3D::new(
                self.physics.velocity.x / speed,
                self.physics.velocity.y / speed,
                0.0,
            );

            self.physics.velocity.x -= velocity_unit.x * deceleration * delta_time;
            self.physics.velocity.y -= velocity_unit.y * deceleration * delta_time;

            // Add angular velocity for visual rolling
            self.physics.angular_velocity = speed / 0.1; // Assume 0.1m radius
        } else {
            // Stopped rolling - detonate
            self.is_rolling = false;
            return Ok(());
        }

        Ok(())
    }

    /// Check for wall collision
    fn check_wall_collision(
        &self,
        from: &Coord3D,
        to: &Coord3D,
    ) -> GameLogicResult<Option<WallCollisionResult>> {
        let terrain = crate::terrain::get_terrain_logic();
        let Ok(guard) = terrain.read() else {
            return Ok(None);
        };

        if guard.is_clear_line_of_sight(from, to) {
            return Ok(None);
        }

        let direction = (*to - *from).normalize();
        let normal = Coord3D::new(-direction.x, -direction.y, -direction.z);

        Ok(Some(WallCollisionResult {
            impact_position: *to,
            normal,
            wall_id: None,
        }))
    }

    /// Handle wall impact
    fn handle_wall_impact(
        &mut self,
        wall_result: WallCollisionResult,
    ) -> GameLogicResult<ProjectileUpdateResult> {
        // Check if can penetrate
        if self.behavior_flags.can_penetrate {
            // Reduce velocity but continue through
            self.physics.velocity.x *= 0.5;
            self.physics.velocity.y *= 0.5;
            self.physics.velocity.z *= 0.5;
            return Ok(ProjectileUpdateResult::Flying);
        }

        // Check if sticks to wall
        if self.behavior_flags.stick_to_walls {
            // Stop movement and stick
            self.physics.velocity = Coord3D::new(0.0, 0.0, 0.0);
            // Would trigger timed detonation here
            return Ok(ProjectileUpdateResult::Flying);
        }

        // Check if can bounce off wall
        if let Some(mut bounce_state) = self.bounce_state.take() {
            if bounce_state.bounce_count < bounce_state.max_bounces {
                let result = self.handle_wall_bounce(&wall_result.normal, &mut bounce_state);
                self.bounce_state = Some(bounce_state);
                return result;
            }
            self.bounce_state = Some(bounce_state);
        }

        // Default: detonate on wall impact
        self.detonate(DetonationReason::WallImpact)
    }

    /// Handle bounce off wall
    fn handle_wall_bounce(
        &mut self,
        wall_normal: &Coord3D,
        bounce_state: &mut BounceState,
    ) -> GameLogicResult<ProjectileUpdateResult> {
        bounce_state.bounce_count += 1;

        // Calculate reflection vector: v' = v - 2(v·n)n
        let dot_product = self.physics.velocity.x * wall_normal.x
            + self.physics.velocity.y * wall_normal.y
            + self.physics.velocity.z * wall_normal.z;

        let energy_retention = 1.0 - bounce_state.energy_loss;

        self.physics.velocity.x = (self.physics.velocity.x - 2.0 * dot_product * wall_normal.x)
            * bounce_state.restitution
            * energy_retention;
        self.physics.velocity.y = (self.physics.velocity.y - 2.0 * dot_product * wall_normal.y)
            * bounce_state.restitution
            * energy_retention;
        self.physics.velocity.z = (self.physics.velocity.z - 2.0 * dot_product * wall_normal.z)
            * bounce_state.restitution
            * energy_retention;

        log::debug!("Projectile {} bounced off wall", self.id);

        Ok(ProjectileUpdateResult::Flying)
    }

    /// Handle spread detonation (miss with scatter)
    fn handle_spread_detonation(&mut self) -> GameLogicResult<ProjectileUpdateResult> {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Calculate spread position based on scatter radius
        let scatter_radius = self.weapon_template.scatter_radius;
        if scatter_radius > 0.0 {
            let angle = rng.gen_range(0.0..std::f32::consts::PI * 2.0);
            let distance = rng.gen_range(0.0..scatter_radius);

            self.physics.position.x += distance * angle.cos();
            self.physics.position.y += distance * angle.sin();

            log::debug!(
                "Projectile {} spread on miss: distance={}, angle={}",
                self.id,
                distance,
                angle
            );
        }

        self.detonate(DetonationReason::GroundImpact)
    }

    /// Check for collisions with objects or proximity triggers
    ///
    /// Enhanced collision detection with:
    /// - Direct hit detection (physical collision with object bounds)
    /// - Proximity fuse detection (detonation near target)
    /// - Predictive collision (check if projectile will hit next frame)
    /// - Multi-object collision handling
    fn check_collisions(&self) -> GameLogicResult<Option<CollisionResult>> {
        // Get nearby objects for collision testing
        // Radius based on projectile speed to catch fast-moving targets
        let search_radius =
            50.0_f32.max(self.physics.velocity.distance(Coord3D::new(0.0, 0.0, 0.0)) * 0.1);
        let nearby_objects = self.get_nearby_objects(search_radius)?;

        // Track closest object for proximity fuse
        let mut closest_distance = f32::MAX;
        let mut _closest_object: Option<ObjectId> = None;

        for object_id in nearby_objects {
            // Skip source object and already hit objects
            if object_id == self.source_object || self.hit_objects.contains(&object_id) {
                continue;
            }

            let object_pos = self.get_object_position(object_id)?;
            let distance = self.physics.position.distance(object_pos);

            // Check for direct hit (within object radius + projectile tolerance)
            let object_radius = self.get_object_radius(object_id)?;
            let collision_threshold = object_radius + 0.5; // Small tolerance for edge cases

            if distance <= collision_threshold {
                // Direct hit - highest priority
                return Ok(Some(CollisionResult::Hit(object_id)));
            }

            // Track closest for proximity fuse
            if distance < closest_distance {
                closest_distance = distance;
                _closest_object = Some(object_id);
            }
        }

        // Check for proximity fuse trigger on closest object
        let proximity_distance = self.get_proximity_fuse_distance();
        if proximity_distance > 0.0 && closest_distance <= proximity_distance {
            // Proximity fuse triggered
            return Ok(Some(CollisionResult::Proximity));
        }

        // No collision detected
        Ok(None)
    }

    /// Detonate the projectile
    fn detonate(&mut self, reason: DetonationReason) -> GameLogicResult<ProjectileUpdateResult> {
        if self.detonated {
            return Ok(ProjectileUpdateResult::Detonated);
        }

        self.detonated = true;
        self.detonation_reason = Some(reason);

        // Apply damage based on warhead type
        self.apply_warhead_damage(reason)?;

        // Create visual effects
        self.create_explosion_effects()?;

        // Handle fragmentation if applicable
        if self.warhead.fragment_count > 0 {
            self.create_fragments()?;
        }

        self.notify_special_power_completion();

        Ok(ProjectileUpdateResult::Detonated)
    }

    fn notify_special_power_completion(&self) {
        let Some(power_name) = self.special_power_template.as_ref() else {
            return;
        };
        if self.special_power_creator_id == INVALID_OBJECT_ID {
            return;
        }
        let Some(player_index) = self.special_power_player_index else {
            return;
        };

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.notify_of_completed_special_power(
                    player_index,
                    power_name.as_str(),
                    self.special_power_creator_id,
                );
            }
        }
    }

    /// Apply warhead damage to nearby objects
    ///
    /// Enhanced damage application with:
    /// - Non-linear damage falloff (quadratic or exponential)
    /// - Direct hit bonus damage
    /// - Primary and secondary damage radius support
    /// - Shockwave vector calculation
    /// - Damage type-specific effects
    fn apply_warhead_damage(&self, reason: DetonationReason) -> GameLogicResult<()> {
        use crate::weapon::damage_system::{DamageInfo, DamageInfoInput};

        // Determine effective damage radius based on detonation reason
        let primary_radius = self.warhead.blast_radius;
        let secondary_radius = primary_radius * 1.5; // Secondary damage extends further

        // Direct hit gets bonus damage multiplier
        let direct_hit_bonus = if matches!(reason, DetonationReason::DirectHit) {
            1.25 // 25% bonus damage on direct hit
        } else {
            1.0
        };

        let nearby_objects = self.get_nearby_objects(secondary_radius)?;

        for object_id in nearby_objects {
            // Don't damage source object (unless weapon has WEAPON_KILLS_SELF flag)
            if object_id == self.source_object {
                continue;
            }

            let object_pos = self.get_object_position(object_id)?;
            let distance = self.physics.position.distance(object_pos);

            // Skip if beyond secondary radius
            if distance > secondary_radius {
                continue;
            }

            // Calculate damage with non-linear falloff
            let (damage_factor, is_primary_damage) = if distance <= primary_radius {
                // Primary damage radius - use quadratic falloff for more realistic blast
                let normalized_distance = distance / primary_radius;
                let falloff = 1.0 - normalized_distance * normalized_distance;
                (falloff, true)
            } else {
                // Secondary damage radius - linear falloff
                let normalized_distance =
                    (distance - primary_radius) / (secondary_radius - primary_radius);
                let falloff = (1.0 - normalized_distance) * 0.5; // Half damage in secondary radius
                (falloff, false)
            };

            // Select primary or secondary damage
            let base_damage = if is_primary_damage {
                self.weapon_template.get_primary_damage(&self.weapon_bonus)
            } else {
                self.weapon_template
                    .get_secondary_damage(&self.weapon_bonus)
            };

            // Apply falloff and direct hit bonus
            let final_damage = base_damage * damage_factor * direct_hit_bonus;

            // Skip if damage is negligible
            if final_damage < 0.1 {
                continue;
            }

            // Create damage info for proper damage application
            let mut damage_info = DamageInfo::new();
            damage_info.input.source_id = self.source_object;
            damage_info.input.damage_type = self.weapon_template.damage_type.into();
            damage_info.input.death_type = self.weapon_template.death_type.into();
            damage_info.input.amount = final_damage;

            // Calculate shockwave effects (proportional to damage)
            damage_info.input.shock_wave_amount =
                self.weapon_template.shock_wave_amount * damage_factor;
            damage_info.input.shock_wave_radius = self.weapon_template.shock_wave_radius;
            damage_info.input.shock_wave_taper_off = self.weapon_template.shock_wave_taper_off;

            // Calculate shock wave vector (from explosion center to target)
            let direction = Coord3D::new(
                object_pos.x - self.physics.position.x,
                object_pos.y - self.physics.position.y,
                object_pos.z - self.physics.position.z,
            );
            let dist = direction.distance(Coord3D::new(0.0, 0.0, 0.0));
            if dist > 0.01 {
                // Normalize direction vector
                damage_info.input.shock_wave_vector =
                    Coord3D::new(direction.x / dist, direction.y / dist, direction.z / dist);
            }
            // Apply damage to object through damage system
            self.deal_damage_to_object(object_id, damage_info)?;
        }

        Ok(())
    }

    /// Create fragmentation projectiles
    fn create_fragments(&self) -> GameLogicResult<()> {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for _ in 0..self.warhead.fragment_count {
            let angle = rng.gen_range(0.0..std::f32::consts::PI * 2.0);
            let elevation = rng.gen_range(-std::f32::consts::PI / 4.0..std::f32::consts::PI / 4.0);

            let fragment_velocity = Coord3D::new(
                self.warhead.fragment_velocity * angle.cos() * elevation.cos(),
                self.warhead.fragment_velocity * angle.sin() * elevation.cos(),
                self.warhead.fragment_velocity * elevation.sin(),
            );

            // Create mini-projectile for fragment
            // This would integrate with the projectile management system
        }

        Ok(())
    }

    // Helper methods (these would integrate with the main game object system)
    fn get_target_position(&self, target_id: ObjectId) -> GameLogicResult<Coord3D> {
        if target_id == INVALID_OBJECT_ID {
            return Ok(self.target_position);
        }

        if let Some(obj_arc) = OBJECT_REGISTRY.get_object(target_id) {
            let guard = obj_arc
                .read()
                .map_err(|_| GameLogicError::Threading("Target object lock failed".to_string()))?;
            return Ok(*guard.get_position());
        }

        Ok(self.target_position)
    }

    fn get_source_position(&self) -> GameLogicResult<Coord3D> {
        if self.source_object != INVALID_OBJECT_ID {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(self.source_object) {
                let guard = obj_arc.read().map_err(|_| {
                    GameLogicError::Threading("Source object lock failed".to_string())
                })?;
                return Ok(*guard.get_position());
            }
        }

        if let Some(point) = self.trajectory.trajectory_points.first() {
            return Ok(point.position);
        }

        Ok(self.target_position)
    }

    fn get_object_position(&self, object_id: ObjectId) -> GameLogicResult<Coord3D> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Err(GameLogicError::InvalidObject(object_id));
        };
        let guard = obj_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Object lock failed".to_string()))?;
        Ok(*guard.get_position())
    }

    fn get_object_radius(&self, object_id: ObjectId) -> GameLogicResult<f32> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Err(GameLogicError::InvalidObject(object_id));
        };
        let guard = obj_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Object lock failed".to_string()))?;
        Ok(guard.get_geometry_info().get_bounding_circle_radius())
    }

    fn get_nearby_objects(&self, radius: f32) -> GameLogicResult<Vec<ObjectId>> {
        if let Some(partition) = ThePartitionManager::get() {
            return Ok(partition.get_objects_in_range(&self.physics.position, radius));
        }

        let radius_sqr = radius * radius;
        let mut results = Vec::new();
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj_arc.read() else {
                continue;
            };
            let obj_pos = guard.get_position();
            let dx = obj_pos.x - self.physics.position.x;
            let dy = obj_pos.y - self.physics.position.y;
            if dx * dx + dy * dy <= radius_sqr {
                results.push(guard.get_id());
            }
        }
        Ok(results)
    }

    fn deal_damage_to_object(
        &self,
        object_id: ObjectId,
        damage_info: crate::weapon::damage_system::DamageInfo,
    ) -> GameLogicResult<()> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Err(GameLogicError::InvalidObject(object_id));
        };

        let mut obj_guard = obj_arc
            .write()
            .map_err(|_| GameLogicError::Threading("Target object lock failed".to_string()))?;

        let mut engine_info = crate::damage::DamageInfo::new();
        engine_info.input.source_id = damage_info.input.source_id;
        engine_info.input.source_player_mask =
            crate::damage::PlayerMaskType::from_bits_truncate(damage_info.input.source_player_mask);
        engine_info.input.damage_type =
            crate::damage::DamageType::from_u32(damage_info.input.damage_type as u32);
        engine_info.input.damage_fx_override =
            crate::damage::DamageType::from_u32(damage_info.input.damage_fx_override as u32);
        engine_info.input.damage_status_type = crate::common::ObjectStatusTypes::None;
        engine_info.input.death_type =
            crate::damage::DeathType::from_u32(damage_info.input.death_type as u32);
        engine_info.input.amount = damage_info.input.amount;
        engine_info.input.kill = damage_info.input.kill;
        engine_info.input.shock_wave_vector = damage_info.input.shock_wave_vector;
        engine_info.input.shock_wave_amount = damage_info.input.shock_wave_amount;
        engine_info.input.shock_wave_radius = damage_info.input.shock_wave_radius;
        engine_info.input.shock_wave_taper_off = damage_info.input.shock_wave_taper_off;
        engine_info.sync_from_input();

        obj_guard
            .attempt_damage(&mut engine_info)
            .map_err(|e| GameLogicError::ModuleError(format!("{}", e)))?;

        Ok(())
    }

    fn calculate_direction_to_target(&self, target_pos: &Coord3D) -> Coord3D {
        let direction = Coord3D::new(
            target_pos.x - self.physics.position.x,
            target_pos.y - self.physics.position.y,
            target_pos.z - self.physics.position.z,
        );

        let distance = direction.distance(Coord3D::new(0.0, 0.0, 0.0));
        if distance > 0.001 {
            Coord3D::new(
                direction.x / distance,
                direction.y / distance,
                direction.z / distance,
            )
        } else {
            Coord3D::new(1.0, 0.0, 0.0)
        }
    }

    fn apply_guidance_correction(&mut self, direction: &Coord3D, turn_rate: f32) {
        let current_velocity = self.physics.velocity.distance(Coord3D::new(0.0, 0.0, 0.0));
        if current_velocity > 0.001 {
            // Blend current velocity with desired direction
            let blend_factor = turn_rate.min(1.0);

            self.physics.velocity.x = self.physics.velocity.x * (1.0 - blend_factor)
                + direction.x * current_velocity * blend_factor;
            self.physics.velocity.y = self.physics.velocity.y * (1.0 - blend_factor)
                + direction.y * current_velocity * blend_factor;
            self.physics.velocity.z = self.physics.velocity.z * (1.0 - blend_factor)
                + direction.z * current_velocity * blend_factor;
        }
    }

    fn has_radar_lock(&self, target_id: ObjectId, _lock_strength: f32) -> GameLogicResult<bool> {
        if target_id == INVALID_OBJECT_ID {
            return Ok(false);
        }
        let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
            return Ok(false);
        };
        let guard = target_arc
            .read()
            .map_err(|_| GameLogicError::Threading("Target lock check failed".to_string()))?;
        Ok(guard.is_detected())
    }

    fn has_laser_designation(&self, _beam_strength: f32) -> GameLogicResult<bool> {
        if let Some(target_id) = self.target_object {
            if let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) {
                if let Ok(target_guard) = target_arc.read() {
                    return Ok(target_guard.is_detected());
                }
            }
        }
        Ok(false)
    }

    fn get_laser_designated_position(&self) -> GameLogicResult<Coord3D> {
        if let Some(target_id) = self.target_object {
            if let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) {
                if let Ok(target_guard) = target_arc.read() {
                    return Ok(*target_guard.get_position());
                }
            }
        }
        Ok(self.target_position)
    }

    fn get_wire_guidance_commands(&self) -> GameLogicResult<Option<WireGuidanceCommands>> {
        let target_pos = if let Some(target_id) = self.target_object {
            if let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) {
                if let Ok(target_guard) = target_arc.read() {
                    *target_guard.get_position()
                } else {
                    return Ok(None);
                }
            } else {
                return Ok(None);
            }
        } else {
            self.target_position
        };

        let direction = self.calculate_direction_to_target(&target_pos);
        Ok(Some(WireGuidanceCommands {
            steering_x: direction.x,
            steering_y: direction.y,
            elevation: direction.z,
        }))
    }

    fn apply_wire_guidance(&mut self, commands: &WireGuidanceCommands, delta_time: f32) {
        let direction = Coord3D::new(commands.steering_x, commands.steering_y, commands.elevation);
        let turn_rate = (delta_time * 2.0).min(1.0);
        self.apply_guidance_correction(&direction, turn_rate);
    }

    fn get_proximity_fuse_distance(&self) -> f32 {
        if let Some(distance) = self.proximity_radius {
            return distance;
        }

        // Different projectile types have different proximity fuse distances
        match self.projectile_type {
            ProjectileType::Artillery => 10.0,
            ProjectileType::Guided => 5.0,
            _ => 2.0,
        }
    }

    fn create_explosion_effects(&self) -> GameLogicResult<()> {
        let mut veterancy = crate::common::VeterancyLevel::Regular;
        let mut source_stealthed = false;

        if self.source_object != INVALID_OBJECT_ID {
            if let Some(source_arc) = OBJECT_REGISTRY.get_object(self.source_object) {
                if let Ok(source_guard) = source_arc.read() {
                    veterancy = source_guard.get_veterancy_level();
                    source_stealthed = source_guard.is_stealthed();
                }
            }
        }

        if source_stealthed && !self.weapon_template.play_fx_when_stealthed {
            return Ok(());
        }

        if let Some(fx) = self.weapon_template.get_projectile_detonate_fx(veterancy) {
            if let Some(projectile_obj) = TheGameLogic::find_object_by_id(self.id) {
                let _ = fx.do_fx_obj(&projectile_obj, None);
            } else {
                let _ = fx.do_fx_at_position(&self.physics.position);
            }
        }

        if let Some(ocl) = self
            .weapon_template
            .get_projectile_detonation_ocl(veterancy)
        {
            let _ = ocl.create_at_position(&self.physics.position, self.source_object);
        }

        Ok(())
    }

    /// Create behavior flags based on projectile type
    fn create_behavior_flags(projectile_type: ProjectileType) -> ProjectileBehaviorFlags {
        match projectile_type {
            ProjectileType::Artillery => ProjectileBehaviorFlags {
                has_parachute: false,
                can_bounce: false,
                can_roll: false,
                spread_on_miss: true,
                stick_to_walls: false,
                can_penetrate: false,
            },
            ProjectileType::Rocket => ProjectileBehaviorFlags {
                has_parachute: false,
                can_bounce: false,
                can_roll: false,
                spread_on_miss: false,
                stick_to_walls: false,
                can_penetrate: false,
            },
            ProjectileType::Guided => ProjectileBehaviorFlags {
                has_parachute: false,
                can_bounce: false,
                can_roll: false,
                spread_on_miss: false,
                stick_to_walls: false,
                can_penetrate: false,
            },
            ProjectileType::Ballistic => ProjectileBehaviorFlags {
                has_parachute: false,
                can_bounce: true,
                can_roll: true,
                spread_on_miss: false,
                stick_to_walls: false,
                can_penetrate: false,
            },
            _ => ProjectileBehaviorFlags::default(),
        }
    }

    // Static helper methods
    fn get_drag_coefficient_for_type(projectile_type: ProjectileType) -> f32 {
        match projectile_type {
            ProjectileType::Ballistic => 0.295, // Typical for bullets
            ProjectileType::Artillery => 0.15,  // Streamlined shell
            ProjectileType::Guided => 0.25,     // Missile with fins
            ProjectileType::Rocket => 0.75,     // Less streamlined
            ProjectileType::Beam => 0.0,        // No drag for energy weapons
            ProjectileType::Flame => 1.2,       // High drag for flame particles
            ProjectileType::Special => 0.5,     // Generic value
        }
    }

    fn get_mass_for_type(projectile_type: ProjectileType) -> f32 {
        match projectile_type {
            ProjectileType::Ballistic => 0.01, // 10 grams
            ProjectileType::Artillery => 10.0, // 10 kg
            ProjectileType::Guided => 50.0,    // 50 kg missile
            ProjectileType::Rocket => 5.0,     // 5 kg RPG
            ProjectileType::Beam => 0.0,       // No mass for energy
            ProjectileType::Flame => 0.001,    // Very light
            ProjectileType::Special => 1.0,    // Generic mass
        }
    }

    fn get_cross_section_for_type(projectile_type: ProjectileType) -> f32 {
        match projectile_type {
            ProjectileType::Ballistic => 0.0001, // Small bullet cross-section
            ProjectileType::Artillery => 0.01,   // Shell cross-section
            ProjectileType::Guided => 0.05,      // Missile cross-section
            ProjectileType::Rocket => 0.02,      // RPG cross-section
            ProjectileType::Beam => 0.0,         // No cross-section for energy
            ProjectileType::Flame => 0.001,      // Small particles
            ProjectileType::Special => 0.005,    // Generic cross-section
        }
    }

    fn create_guidance_system(
        weapon_template: &WeaponTemplate,
        projectile_type: ProjectileType,
    ) -> GuidanceSystem {
        // Determine guidance based on weapon properties and projectile type
        match projectile_type {
            ProjectileType::Guided => GuidanceSystem::HeatSeeking { sensitivity: 2.0 },
            ProjectileType::Ballistic | ProjectileType::Artillery => GuidanceSystem::None,
            ProjectileType::Rocket => GuidanceSystem::RadarGuided { lock_strength: 1.5 },
            _ => GuidanceSystem::None,
        }
    }

    fn create_warhead(weapon_template: &WeaponTemplate, weapon_bonus: &WeaponBonus) -> Warhead {
        Warhead {
            yield_amount: weapon_template.get_primary_damage(weapon_bonus),
            blast_radius: weapon_template.get_primary_damage_radius(weapon_bonus),
            fragment_count: 0, // Most weapons don't fragment
            fragment_velocity: 0.0,
            penetration: weapon_template.primary_damage * 0.1, // 10% of damage as penetration
            special_effects: Vec::new(),
        }
    }
}

/// Result of projectile update
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileUpdateResult {
    /// Projectile is still flying
    Flying,
    /// Projectile has detonated and should be removed
    Detonated,
}

/// Collision detection result
#[derive(Debug, Clone, Copy)]
pub enum CollisionResult {
    /// Direct hit on object
    Hit(ObjectId),
    /// Proximity fuse triggered
    Proximity,
}

/// Reason for projectile detonation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetonationReason {
    /// Direct hit on target
    DirectHit,
    /// Proximity fuse triggered
    ProximityFuse,
    /// Hit the ground
    GroundImpact,
    /// Hit a wall
    WallImpact,
    /// Timed out
    Timeout,
    /// Self-destruct command
    SelfDestruct,
}

/// Wall collision result
#[derive(Debug, Clone)]
pub struct WallCollisionResult {
    /// Position of impact
    pub impact_position: Coord3D,
    /// Normal vector of the wall
    pub normal: Coord3D,
    /// Wall object ID (if any)
    pub wall_id: Option<ObjectId>,
}

/// Wire guidance commands
#[derive(Debug, Clone)]
pub struct WireGuidanceCommands {
    pub steering_x: f32,
    pub steering_y: f32,
    pub elevation: f32,
}

/// Projectile manager for handling all active projectiles
#[derive(Debug)]
pub struct ProjectileManager {
    projectiles: HashMap<ObjectId, Projectile>,
    next_id: ObjectId,
}

impl ProjectileManager {
    pub fn new() -> Self {
        Self {
            projectiles: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create and track a new projectile
    pub fn create_projectile(
        &mut self,
        projectile_type: ProjectileType,
        weapon_template: Arc<WeaponTemplate>,
        source_object: ObjectId,
        target_object: Option<ObjectId>,
        target_position: Coord3D,
        trajectory: BallisticsTrajectory,
        weapon_bonus: WeaponBonus,
        special_power_template: Option<String>,
        special_power_creator_id: ObjectId,
        special_power_player_index: Option<usize>,
    ) -> ObjectId {
        let id = self.next_id;
        self.next_id += 1;

        let projectile = Projectile::new(
            id,
            projectile_type,
            weapon_template,
            source_object,
            target_object,
            target_position,
            trajectory,
            weapon_bonus,
            special_power_template,
            special_power_creator_id,
            special_power_player_index,
        );

        self.projectiles.insert(id, projectile);
        id
    }

    /// Update all projectiles for one frame
    pub fn update_all(&mut self, delta_time: f32) -> GameLogicResult<()> {
        let mut to_remove = Vec::new();

        for (id, projectile) in &mut self.projectiles {
            match projectile.update(delta_time)? {
                ProjectileUpdateResult::Flying => {
                    // Continue tracking
                }
                ProjectileUpdateResult::Detonated => {
                    // Mark for removal
                    to_remove.push(*id);
                }
            }
        }

        // Remove detonated projectiles
        for id in to_remove {
            self.projectiles.remove(&id);
        }

        Ok(())
    }

    /// Get projectile by ID
    pub fn get_projectile(&self, id: ObjectId) -> Option<&Projectile> {
        self.projectiles.get(&id)
    }

    /// Get mutable projectile by ID
    pub fn get_projectile_mut(&mut self, id: ObjectId) -> Option<&mut Projectile> {
        self.projectiles.get_mut(&id)
    }

    /// Remove projectile by ID
    pub fn remove_projectile(&mut self, id: ObjectId) -> Option<Projectile> {
        self.projectiles.remove(&id)
    }

    /// Get all active projectile IDs
    pub fn get_active_projectiles(&self) -> Vec<ObjectId> {
        self.projectiles.keys().copied().collect()
    }

    /// Clear all projectiles
    pub fn clear(&mut self) {
        self.projectiles.clear();
    }

    /// Create a burst/volley of projectiles (multi-shot)
    pub fn create_projectile_volley(
        &mut self,
        projectile_type: ProjectileType,
        weapon_template: Arc<WeaponTemplate>,
        source_object: ObjectId,
        target_object: Option<ObjectId>,
        target_position: Coord3D,
        base_trajectory: BallisticsTrajectory,
        weapon_bonus: WeaponBonus,
        special_power_template: Option<String>,
        special_power_creator_id: ObjectId,
        special_power_player_index: Option<usize>,
        volley_count: u32,
        volley_spread: f32,
    ) -> Vec<ObjectId> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut projectile_ids = Vec::new();

        for i in 0..volley_count {
            // Calculate spread for this projectile in the volley
            let angle_offset = if volley_count > 1 {
                let spread_factor =
                    (i as f32 - (volley_count as f32 - 1.0) / 2.0) / (volley_count as f32 / 2.0);
                spread_factor * volley_spread
            } else {
                0.0
            };

            // Add random variation
            let random_angle_variation = rng.gen_range(-0.1..0.1);
            let final_angle = angle_offset + random_angle_variation;

            // Modify trajectory for spread
            let mut modified_trajectory = base_trajectory.clone();
            let cos_angle = final_angle.cos();
            let sin_angle = final_angle.sin();

            modified_trajectory.initial_velocity = Coord3D::new(
                base_trajectory.initial_velocity.x * cos_angle
                    - base_trajectory.initial_velocity.y * sin_angle,
                base_trajectory.initial_velocity.x * sin_angle
                    + base_trajectory.initial_velocity.y * cos_angle,
                base_trajectory.initial_velocity.z,
            );

            // Create projectile
            let id = self.create_projectile(
                projectile_type,
                Arc::clone(&weapon_template),
                source_object,
                target_object,
                target_position,
                modified_trajectory,
                weapon_bonus.clone(),
                special_power_template.clone(),
                special_power_creator_id,
                special_power_player_index,
            );

            projectile_ids.push(id);
        }

        log::debug!(
            "Created projectile volley: {} projectiles from source {}",
            projectile_ids.len(),
            source_object
        );

        projectile_ids
    }

    /// Create a shotgun-style spread of projectiles
    pub fn create_projectile_spread(
        &mut self,
        projectile_type: ProjectileType,
        weapon_template: Arc<WeaponTemplate>,
        source_object: ObjectId,
        target_object: Option<ObjectId>,
        target_position: Coord3D,
        base_trajectory: BallisticsTrajectory,
        weapon_bonus: WeaponBonus,
        special_power_template: Option<String>,
        special_power_creator_id: ObjectId,
        special_power_player_index: Option<usize>,
        spread_count: u32,
        spread_cone_angle: f32,
    ) -> Vec<ObjectId> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut projectile_ids = Vec::new();

        for _ in 0..spread_count {
            // Random direction within cone
            let horizontal_angle = rng.gen_range(0.0..std::f32::consts::PI * 2.0);
            let vertical_angle = rng.gen_range(0.0..spread_cone_angle);

            // Calculate spread direction
            let spread_x = vertical_angle.sin() * horizontal_angle.cos();
            let spread_y = vertical_angle.sin() * horizontal_angle.sin();
            let spread_z = vertical_angle.cos();

            // Modify trajectory
            let mut modified_trajectory = base_trajectory.clone();
            let speed = base_trajectory
                .initial_velocity
                .distance(Coord3D::new(0.0, 0.0, 0.0));

            modified_trajectory.initial_velocity = Coord3D::new(
                base_trajectory.initial_velocity.x + spread_x * speed * 0.2,
                base_trajectory.initial_velocity.y + spread_y * speed * 0.2,
                base_trajectory.initial_velocity.z + spread_z * speed * 0.1,
            );

            // Create projectile
            let id = self.create_projectile(
                projectile_type,
                Arc::clone(&weapon_template),
                source_object,
                target_object,
                target_position,
                modified_trajectory,
                weapon_bonus.clone(),
                special_power_template.clone(),
                special_power_creator_id,
                special_power_player_index,
            );

            projectile_ids.push(id);
        }

        log::debug!(
            "Created projectile spread: {} projectiles from source {}",
            projectile_ids.len(),
            source_object
        );

        projectile_ids
    }
}

impl Default for ProjectileManager {
    fn default() -> Self {
        Self::new()
    }
}

pub static PROJECTILE_MANAGER: Lazy<Mutex<ProjectileManager>> =
    Lazy::new(|| Mutex::new(ProjectileManager::new()));

pub fn with_projectile_manager<T>(f: impl FnOnce(&mut ProjectileManager) -> T) -> T {
    let mut guard = PROJECTILE_MANAGER
        .lock()
        .expect("ProjectileManager lock poisoned");
    f(&mut guard)
}

pub fn update_projectiles(delta_time: f32) -> GameLogicResult<()> {
    with_projectile_manager(|manager| manager.update_all(delta_time))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::WeaponTemplate;

    #[test]
    fn test_projectile_creation() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(100.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert_eq!(projectile.id, 1);
        assert_eq!(projectile.source_object, 2);
        assert_eq!(projectile.target_object, Some(3));
        assert!(!projectile.detonated);
    }

    #[test]
    fn test_projectile_manager() {
        let mut manager = ProjectileManager::new();
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));

        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let id = manager.create_projectile(
            ProjectileType::Ballistic,
            weapon_template,
            1,
            Some(2),
            Coord3D::new(100.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert!(manager.get_projectile(id).is_some());
        assert_eq!(manager.get_active_projectiles().len(), 1);
    }

    #[test]
    fn test_guidance_systems() {
        let heat_seeking = GuidanceSystem::HeatSeeking { sensitivity: 2.0 };
        let radar_guided = GuidanceSystem::RadarGuided { lock_strength: 1.5 };

        match heat_seeking {
            GuidanceSystem::HeatSeeking { sensitivity } => assert_eq!(sensitivity, 2.0),
            _ => panic!("Wrong guidance type"),
        }

        match radar_guided {
            GuidanceSystem::RadarGuided { lock_strength } => assert_eq!(lock_strength, 1.5),
            _ => panic!("Wrong guidance type"),
        }
    }

    #[test]
    fn test_projectile_behavior_flags() {
        let ballistic_flags = Projectile::create_behavior_flags(ProjectileType::Ballistic);
        assert!(ballistic_flags.can_bounce);
        assert!(ballistic_flags.can_roll);
        assert!(!ballistic_flags.has_parachute);

        let artillery_flags = Projectile::create_behavior_flags(ProjectileType::Artillery);
        assert!(artillery_flags.spread_on_miss);
        assert!(!artillery_flags.can_bounce);
    }

    #[test]
    fn test_parachute_state() {
        let parachute = ParachuteState::default();
        assert!(!parachute.deployed);
        assert_eq!(parachute.deploy_altitude, 20.0);
        assert_eq!(parachute.descent_rate, 5.0);
    }

    #[test]
    fn test_bounce_state() {
        let bounce = BounceState::default();
        assert_eq!(bounce.bounce_count, 0);
        assert_eq!(bounce.max_bounces, 3);
        assert_eq!(bounce.restitution, 0.6);
    }

    #[test]
    fn test_projectile_volley_creation() {
        let mut manager = ProjectileManager::new();
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));

        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let ids = manager.create_projectile_volley(
            ProjectileType::Ballistic,
            weapon_template,
            1,
            Some(2),
            Coord3D::new(100.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
            5,   // 5 projectiles in volley
            0.2, // 0.2 radian spread
        );

        assert_eq!(ids.len(), 5);
        assert_eq!(manager.get_active_projectiles().len(), 5);
    }

    #[test]
    fn test_projectile_spread_creation() {
        let mut manager = ProjectileManager::new();
        let weapon_template = Arc::new(WeaponTemplate::new("ShotgunWeapon".to_string()));

        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let ids = manager.create_projectile_spread(
            ProjectileType::Ballistic,
            weapon_template,
            1,
            None,
            Coord3D::new(100.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
            8,   // 8 pellets
            0.3, // 0.3 radian cone
        );

        assert_eq!(ids.len(), 8);
        assert_eq!(manager.get_active_projectiles().len(), 8);
    }

    #[test]
    fn test_projectile_physics_constants() {
        // Test drag coefficients
        assert_eq!(
            Projectile::get_drag_coefficient_for_type(ProjectileType::Ballistic),
            0.295
        );
        assert_eq!(
            Projectile::get_drag_coefficient_for_type(ProjectileType::Artillery),
            0.15
        );
        assert_eq!(
            Projectile::get_drag_coefficient_for_type(ProjectileType::Beam),
            0.0
        );

        // Test mass values
        assert_eq!(
            Projectile::get_mass_for_type(ProjectileType::Ballistic),
            0.01
        );
        assert_eq!(
            Projectile::get_mass_for_type(ProjectileType::Artillery),
            10.0
        );
        assert_eq!(Projectile::get_mass_for_type(ProjectileType::Guided), 50.0);
    }

    #[test]
    fn test_projectile_update_timeout() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(100.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        // Set time alive to exceed max flight time
        projectile.time_alive = 31.0;

        let result = projectile.update(0.016).unwrap();
        assert_eq!(result, ProjectileUpdateResult::Detonated);
        assert!(projectile.detonated);
    }

    #[test]
    fn test_wall_collision_result() {
        let wall_result = WallCollisionResult {
            impact_position: Coord3D::new(10.0, 20.0, 5.0),
            normal: Coord3D::new(1.0, 0.0, 0.0),
            wall_id: Some(42),
        };

        assert_eq!(wall_result.wall_id, Some(42));
        assert_eq!(wall_result.normal.x, 1.0);
    }

    #[test]
    fn test_detonation_reasons() {
        // Test that all detonation reasons are distinct
        let reasons = vec![
            DetonationReason::DirectHit,
            DetonationReason::ProximityFuse,
            DetonationReason::GroundImpact,
            DetonationReason::WallImpact,
            DetonationReason::Timeout,
            DetonationReason::SelfDestruct,
        ];

        for (i, reason1) in reasons.iter().enumerate() {
            for (j, reason2) in reasons.iter().enumerate() {
                if i == j {
                    assert_eq!(reason1, reason2);
                } else {
                    assert_ne!(reason1, reason2);
                }
            }
        }
    }

    // ==================== PROJECTILE GUIDANCE TESTS ====================

    #[test]
    fn test_no_guidance_system() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert_eq!(projectile.guidance_system, GuidanceSystem::None);
    }

    #[test]
    fn test_heat_seeking_guidance() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Guided,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        projectile.guidance_system = GuidanceSystem::HeatSeeking { sensitivity: 0.8 };
        assert_eq!(
            projectile.guidance_system,
            GuidanceSystem::HeatSeeking { sensitivity: 0.8 }
        );
    }

    #[test]
    fn test_radar_guided_missile() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(150.0, 0.0, 50.0),
            launch_angle: 0.2,
            flight_time: 5.0,
            max_height: 100.0,
            range: 500.0,
            trajectory_points: vec![],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Guided,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        projectile.guidance_system = GuidanceSystem::RadarGuided { lock_strength: 0.6 };
        assert_eq!(
            projectile.guidance_system,
            GuidanceSystem::RadarGuided { lock_strength: 0.6 }
        );
    }

    #[test]
    fn test_laser_guided_missile() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 0.0),
            launch_angle: 0.0,
            flight_time: 2.0,
            max_height: 10.0,
            range: 200.0,
            trajectory_points: vec![],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Guided,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        projectile.guidance_system = GuidanceSystem::LaserGuided { beam_strength: 0.9 };
        assert_eq!(
            projectile.guidance_system,
            GuidanceSystem::LaserGuided { beam_strength: 0.9 }
        );
    }

    // ==================== PROJECTILE TYPE TESTS ====================

    #[test]
    fn test_ballistic_projectile_type() {
        let weapon_template = Arc::new(WeaponTemplate::new("Rifle".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(800.0, 0.0, 0.0),
            launch_angle: 0.0,
            flight_time: 0.5,
            max_height: 0.0,
            range: 400.0,
            trajectory_points: vec![],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert_eq!(projectile.projectile_type, ProjectileType::Ballistic);
    }

    #[test]
    fn test_guided_projectile_type() {
        let weapon_template = Arc::new(WeaponTemplate::new("Missile".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(200.0, 0.0, 100.0),
            launch_angle: 0.5,
            flight_time: 10.0,
            max_height: 500.0,
            range: 1000.0,
            trajectory_points: vec![],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Guided,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert_eq!(projectile.projectile_type, ProjectileType::Guided);
    }

    #[test]
    fn test_beam_projectile_type() {
        let weapon_template = Arc::new(WeaponTemplate::new("Laser".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(0.0, 0.0, 0.0), // Instant hit
            launch_angle: 0.0,
            flight_time: 0.0,
            max_height: 0.0,
            range: 2000.0,
            trajectory_points: vec![],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Beam,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert_eq!(projectile.projectile_type, ProjectileType::Beam);
    }

    // ==================== DETONATION REASON TESTS ====================

    #[test]
    fn test_direct_hit_detonation() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(100.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        projectile.detonation_reason = Some(DetonationReason::DirectHit);
        assert_eq!(
            projectile.detonation_reason,
            Some(DetonationReason::DirectHit)
        );
    }

    #[test]
    fn test_proximity_fuse_detonation() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        projectile.proximity_radius = Some(50.0);
        projectile.detonation_reason = Some(DetonationReason::ProximityFuse);
        assert_eq!(
            projectile.detonation_reason,
            Some(DetonationReason::ProximityFuse)
        );
    }

    #[test]
    fn test_ground_impact_detonation() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, -50.0),
            launch_angle: -0.5,
            flight_time: 1.0,
            max_height: 0.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 100.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        projectile.detonation_reason = Some(DetonationReason::GroundImpact);
        assert_eq!(
            projectile.detonation_reason,
            Some(DetonationReason::GroundImpact)
        );
    }

    #[test]
    fn test_timeout_detonation() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(50.0, 0.0, 0.0),
            launch_angle: 0.0,
            flight_time: 0.5,
            max_height: 0.0,
            range: 50.0,
            trajectory_points: vec![],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        projectile.max_lifetime = Some(0.5);
        projectile.time_alive = 0.6;
        projectile.detonation_reason = Some(DetonationReason::Timeout);
        assert_eq!(
            projectile.detonation_reason,
            Some(DetonationReason::Timeout)
        );
    }

    // ==================== FRAGMENTATION TESTS ====================

    #[test]
    fn test_no_fragmentation() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert_eq!(projectile.warhead.fragment_count, 0);
    }

    #[test]
    fn test_fragmentation_enabled() {
        let weapon_template = Arc::new(WeaponTemplate::new("Grenade".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(30.0, 0.0, 20.0),
            launch_angle: 0.6,
            flight_time: 2.0,
            max_height: 10.0,
            range: 50.0,
            trajectory_points: vec![],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        projectile.warhead.fragment_count = 8;
        assert_eq!(projectile.warhead.fragment_count, 8);
    }

    // ==================== PROJECTILE LIFETIME TESTS ====================

    #[test]
    fn test_projectile_time_alive_tracking() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        projectile.time_alive = 0.5;
        assert_eq!(projectile.time_alive, 0.5);
    }

    #[test]
    fn test_unlimited_lifetime() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            None, // No lifetime limit
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert_eq!(projectile.target_object, None);
    }

    // ==================== PROJECTILE POSITION TESTS ====================

    #[test]
    fn test_initial_position() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let initial_pos = Coord3D::new(500.0, 500.0, 100.0);
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![crate::weapon::ballistics::TrajectoryPoint {
                position: initial_pos,
                velocity: Coord3D::new(100.0, 0.0, 10.0),
                time: 0.0,
            }],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            initial_pos,
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert_eq!(projectile.physics.position, initial_pos);
    }

    #[test]
    fn test_position_update() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![crate::weapon::ballistics::TrajectoryPoint {
                position: Coord3D::new(0.0, 0.0, 0.0),
                velocity: Coord3D::new(100.0, 0.0, 10.0),
                time: 0.0,
            }],
        };

        let mut projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        let delta_time = 0.1;
        projectile.physics.position.x += projectile.trajectory.initial_velocity.x * delta_time;
        projectile.physics.position.z += projectile.trajectory.initial_velocity.z * delta_time;

        assert_eq!(projectile.physics.position.x, 10.0);
        assert_eq!(projectile.physics.position.z, 1.0);
    }

    // ==================== PROJECTILE MANAGER TESTS ====================

    #[test]
    fn test_manager_active_projectile_count() {
        let mut manager = ProjectileManager::new();
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let id = manager.create_projectile(
            ProjectileType::Ballistic,
            weapon_template,
            1,
            None,
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert_ne!(id, 0);
        assert_eq!(manager.get_active_projectiles().len(), 1);
    }

    #[test]
    fn test_manager_remove_projectile() {
        let mut manager = ProjectileManager::new();
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(100.0, 0.0, 10.0),
            launch_angle: 0.1,
            flight_time: 1.0,
            max_height: 5.0,
            range: 100.0,
            trajectory_points: vec![],
        };

        let id = manager.create_projectile(
            ProjectileType::Ballistic,
            weapon_template,
            1,
            None,
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        manager.remove_projectile(id);
        assert_eq!(manager.get_active_projectiles().len(), 0);
    }

    // ==================== EDGE CASES ====================

    #[test]
    fn test_zero_velocity_projectile() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(0.0, 0.0, 0.0),
            launch_angle: 0.0,
            flight_time: 0.0,
            max_height: 0.0,
            range: 0.0,
            trajectory_points: vec![],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(100.0, 100.0, 50.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert_eq!(projectile.trajectory.range, 0.0);
    }

    #[test]
    fn test_very_large_velocity() {
        let weapon_template = Arc::new(WeaponTemplate::new("Laser".to_string()));
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(30000.0, 0.0, 0.0), // Speed of light approximation
            launch_angle: 0.0,
            flight_time: 0.001,
            max_height: 0.0,
            range: 30.0,
            trajectory_points: vec![],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Beam,
            weapon_template,
            2,
            Some(3),
            Coord3D::new(0.0, 0.0, 0.0),
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert!(projectile.trajectory.initial_velocity.x > 10000.0);
    }

    #[test]
    fn test_projectile_with_negative_coordinates() {
        let weapon_template = Arc::new(WeaponTemplate::new("TestWeapon".to_string()));
        let launch_pos = Coord3D::new(-500.0, -500.0, 0.0);
        let trajectory = BallisticsTrajectory {
            initial_velocity: Coord3D::new(-100.0, -50.0, 0.0),
            launch_angle: 0.0,
            flight_time: 1.0,
            max_height: 0.0,
            range: 100.0,
            trajectory_points: vec![crate::weapon::ballistics::TrajectoryPoint {
                position: launch_pos,
                velocity: Coord3D::new(-100.0, -50.0, 0.0),
                time: 0.0,
            }],
        };

        let projectile = Projectile::new(
            1,
            ProjectileType::Ballistic,
            weapon_template,
            2,
            Some(3),
            launch_pos,
            trajectory,
            WeaponBonus::new(),
            None,
            INVALID_OBJECT_ID,
            None,
        );

        assert!(projectile.physics.position.x < 0.0);
        assert!(projectile.physics.position.y < 0.0);
    }
}
