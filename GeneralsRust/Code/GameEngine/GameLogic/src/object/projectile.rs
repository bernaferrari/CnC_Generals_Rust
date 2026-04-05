//! Projectile class - Weapons fire and launched objects
//!
//! Projectiles are objects fired by weapons, including bullets, shells,
//! missiles, rockets, and other launched objects that travel through space.

use crate::common::ObjectID;
use crate::common::*;
use crate::damage::DamageInfo;
use crate::helpers::TheThingFactory;
use crate::object::behavior::dumb_projectile_behavior::DumbProjectileBehaviorModuleData;
use crate::object::draw::w3d_projectile_draw::W3DProjectileDrawModuleData;
use crate::object::update::missile_ai_update::MissileAIUpdateModuleData;
use crate::object::Object;
use crate::physics::{ballistics, get_physics_engine, AIR_RESISTANCE, GRAVITY};
use crate::team::Team;
use crate::weapon::{DamageType, WeaponTemplate};
use std::sync::{Arc, Mutex, RwLock};

/// Types of projectiles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileType {
    Bullet,    // Direct-fire ballistic
    Shell,     // Indirect-fire ballistic with arc
    Missile,   // Guided projectile
    Rocket,    // Unguided rocket
    Beam,      // Continuous energy beam
    Flame,     // Flame thrower
    Grenade,   // Thrown explosive
    Artillery, // Long-range indirect fire
    Torpedo,   // Underwater projectile
    Bomb,      // Dropped explosive
    Laser,     // Instant hit laser
    Plasma,    // Plasma projectile
    Ion,       // Ion cannon beam
    EMP,       // Electromagnetic pulse
    Chemical,  // Chemical/biological weapon
}

/// Movement patterns for projectiles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileMovement {
    Ballistic, // Follows physics trajectory
    Straight,  // Moves in straight line
    Guided,    // Guided towards target
    Homing,    // Homes in on target
    Parabolic, // Follows parabolic arc
    Beam,      // Instant hit beam
    Teleport,  // Instant teleport to target
    Bouncing,  // Bounces off surfaces
    Cluster,   // Splits into multiple projectiles
}

/// Guidance systems for smart projectiles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidanceType {
    None,            // Unguided
    LaserGuided,     // Laser designation
    InfraredSeeking, // Heat seeking
    RadarGuided,     // Radar homing
    GPS,             // GPS guided
    WireGuided,      // Wire guided
    BeamRiding,      // Rides a targeting beam
    AcousticHoming,  // Sound homing (torpedoes)
    OpticalTracking, // Visual tracking
}

/// Projectile-specific data and behavior
#[derive(Debug)]
pub struct Projectile {
    /// Base object functionality
    base_object: Arc<RwLock<Object>>,

    /// Projectile classification
    projectile_type: ProjectileType,
    movement_pattern: ProjectileMovement,
    guidance_type: GuidanceType,

    /// Source information
    source_weapon: Option<Arc<WeaponTemplate>>,
    source_object: ObjectID,
    source_position: Coord3D,
    source_team: Option<Arc<RwLock<Team>>>,

    /// Target information
    target_object: Option<ObjectID>,
    target_position: Coord3D,
    original_target_position: Coord3D,

    /// Physics and movement
    velocity: Coord3D,
    acceleration: Coord3D,
    max_speed: Real,
    turning_rate: Real, // For guided projectiles
    lifetime: Real,     // Maximum lifetime before self-destruct
    age: Real,          // Current age

    /// Ballistics (for physics-based projectiles)
    initial_velocity: Real,
    launch_angle: Real,
    gravity_scale: Real,        // Multiplier for gravity effect
    air_resistance_scale: Real, // Multiplier for air resistance

    /// Damage properties
    damage_amount: Real,
    damage_type: DamageType,
    damage_radius: Real,  // For area effect
    penetration: Real,    // Armor penetration
    damage_falloff: Real, // Damage reduction with distance
    friendly_fire: bool,  // Can damage own team

    /// Guidance parameters
    lock_on_range: Real, // Range to acquire new targets
    guidance_accuracy: Real, // How accurately it tracks target
    lock_lost_time: Real,    // Time since lost lock on target
    max_guidance_time: Real, // Maximum time guidance is active

    /// Visual and effects
    trail_length: Real, // Length of visual trail
    trail_color: Color,          // Color of trail
    muzzle_flash: bool,          // Shows muzzle flash
    impact_effects: Vec<String>, // Impact particle effects
    trail_effects: Vec<String>,  // Trail particle effects
    exhaust_particle_system_id: Option<ParticleSystemID>,

    /// Audio
    launch_sound: Option<String>,
    flight_sound: Option<String>,
    impact_sound: Option<String>,

    /// Detonation
    proximity_fuse: Real, // Detonate when this close to target
    impact_fuse: bool,   // Detonate on impact
    timer_fuse: Real,    // Detonate after this time
    altitude_fuse: Real, // Detonate at this altitude

    /// Cluster/submunition properties
    submunition_count: u32, // Number of submunitions
    submunition_type: Option<String>, // Template for submunitions
    submunition_spread: Real,         // Spread angle for submunitions

    /// Special properties
    piercing: bool, // Can pass through targets
    bounces_remaining: u32,    // Number of bounces left
    bounce_angle_loss: Real,   // Angle loss per bounce
    can_be_shot_down: bool,    // Can be intercepted
    stealth_penetrating: bool, // Ignores stealth

    /// Status flags
    has_hit_target: bool,
    is_guided: bool,
    has_detonated: bool,
    is_intercepted: bool,
    guidance_active: bool,

    /// Countermeasures
    flare_vulnerability: Real, // Susceptibility to flares
    chaff_vulnerability: Real, // Susceptibility to chaff
    ecm_vulnerability: Real,   // Susceptibility to ECM

    /// Homing behavior
    homing_delay: Real, // Delay before homing becomes active
    homing_force: Real,    // Strength of homing
    prediction_time: Real, // How far ahead to predict target

    /// Physics integration
    physics_active: bool,
    collision_radius: Real,
    collision_height: Real,
}

impl Projectile {
    fn source_veterancy_level(&self) -> crate::common::VeterancyLevel {
        if self.source_object == INVALID_OBJECT_ID {
            return crate::common::VeterancyLevel::Regular;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.source_object)
            .and_then(|obj| obj.read().ok().map(|guard| guard.get_veterancy_level()))
            .unwrap_or(crate::common::VeterancyLevel::Regular)
    }
    pub fn base_object(&self) -> Arc<RwLock<Object>> {
        Arc::clone(&self.base_object)
    }

    /// Create a new Projectile
    pub fn new(
        base_object: Arc<RwLock<Object>>,
        weapon_template: Arc<WeaponTemplate>,
        source_object: ObjectID,
        source_position: Coord3D,
        target_position: Coord3D,
        target_object: Option<ObjectID>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let projectile_type = Self::determine_projectile_type(&weapon_template);
        let movement_pattern = Self::determine_movement_pattern(&weapon_template);
        let weapon_speed = weapon_template
            .weapon_speed
            .max(weapon_template.min_weapon_speed);
        let guided = Self::has_projectile_behavior(&weapon_template, "MissileAIUpdate")
            || Self::has_projectile_behavior(&weapon_template, "SmartBombTargetHomingUpdate");
        let missile_ai = Self::missile_ai_data(&weapon_template);
        let dumb_projectile = Self::dumb_projectile_data(&weapon_template);

        let initial_velocity = missile_ai
            .as_ref()
            .map(|data| {
                if data.use_weapon_speed {
                    weapon_speed
                } else if data.initial_velocity > 0.0 {
                    data.initial_velocity
                } else {
                    weapon_speed
                }
            })
            .unwrap_or(weapon_speed);

        let lifetime = missile_ai
            .as_ref()
            .and_then(|data| {
                if data.fuel_lifetime > 0 {
                    Some(data.fuel_lifetime as Real / LOGICFRAMES_PER_SECOND as Real)
                } else {
                    None
                }
            })
            .or_else(|| {
                dumb_projectile.as_ref().and_then(|data| {
                    if data.max_lifespan > 0 {
                        Some(data.max_lifespan as Real / LOGICFRAMES_PER_SECOND as Real)
                    } else {
                        None
                    }
                })
            })
            .or_else(|| {
                if weapon_template.continuous_fire_coast_frames > 0 {
                    Some(
                        weapon_template.continuous_fire_coast_frames as Real
                            / LOGICFRAMES_PER_SECOND as Real,
                    )
                } else {
                    None
                }
            })
            .unwrap_or(Real::INFINITY);

        let homing_delay = missile_ai
            .as_ref()
            .map(|data| {
                if data.initial_distance <= 0.0 {
                    0.0
                } else {
                    let speed = if data.use_weapon_speed {
                        weapon_speed
                    } else if data.initial_velocity > 0.0 {
                        data.initial_velocity
                    } else {
                        weapon_speed
                    };
                    if speed <= 0.0 {
                        0.0
                    } else {
                        data.initial_distance / speed
                    }
                }
            })
            .unwrap_or(0.0);

        let max_guidance_time = missile_ai
            .as_ref()
            .and_then(|data| {
                if data.fuel_lifetime > 0 {
                    Some(data.fuel_lifetime as Real / LOGICFRAMES_PER_SECOND as Real)
                } else {
                    None
                }
            })
            .unwrap_or(Real::INFINITY);

        let trail_effects = Self::projectile_trail_particle_name(&weapon_template)
            .map(|name| vec![name])
            .unwrap_or_default();
        let trail_length = Self::projectile_trail_interval_seconds(&weapon_template).unwrap_or(0.0);

        let friendly_fire = weapon_template
            .affects_mask
            .contains(crate::weapon::WeaponAffectsMask::ALLIES)
            || weapon_template
                .affects_mask
                .contains(crate::weapon::WeaponAffectsMask::SELF)
            || weapon_template
                .affects_mask
                .contains(crate::weapon::WeaponAffectsMask::KILLS_SELF);

        let guidance_type = if guided {
            GuidanceType::RadarGuided
        } else {
            GuidanceType::None
        };

        Ok(Projectile {
            base_object: base_object.clone(),
            projectile_type,
            movement_pattern,
            guidance_type,

            source_weapon: Some(weapon_template.clone()),
            source_object,
            source_position,
            source_team: None, // Set after creation from source object/team

            target_object,
            target_position,
            original_target_position: target_position,

            velocity: Coord3D::new(0.0, 0.0, 0.0),
            acceleration: Coord3D::new(0.0, 0.0, 0.0),
            max_speed: weapon_speed,
            turning_rate: 0.0,
            lifetime,
            age: 0.0,

            initial_velocity,
            launch_angle: Self::calculate_launch_angle(
                source_position,
                target_position,
                initial_velocity,
            ),
            gravity_scale: 1.0,
            air_resistance_scale: 1.0,

            damage_amount: weapon_template.primary_damage,
            damage_type: weapon_template.damage_type,
            damage_radius: weapon_template.primary_damage_radius,
            penetration: 0.0,
            damage_falloff: 0.0,
            friendly_fire,

            lock_on_range: weapon_template.continue_attack_range,
            guidance_accuracy: 0.0,
            lock_lost_time: 0.0,
            max_guidance_time,

            trail_length,
            trail_color: Color::white(),
            muzzle_flash: !weapon_template.projectile_name.is_empty()
                || !weapon_template.projectile_stream_name.is_empty(),
            impact_effects: {
                let veterancy = crate::helpers::TheGameLogic::find_object_by_id(source_object)
                    .and_then(|obj| obj.read().ok().map(|guard| guard.get_veterancy_level()))
                    .unwrap_or(crate::common::VeterancyLevel::Regular);
                if let Some(fx) = weapon_template.get_projectile_detonate_fx(veterancy) {
                    let level_name = match veterancy {
                        crate::common::VeterancyLevel::Regular => "Regular",
                        crate::common::VeterancyLevel::Veteran => "Veteran",
                        crate::common::VeterancyLevel::Elite => "Elite",
                        crate::common::VeterancyLevel::Heroic => "Heroic",
                    };
                    let fx_name = format!(
                        "Weapon:{}:ProjectileDetonateFX:{}",
                        weapon_template.name, level_name
                    );
                    let _ = crate::helpers::TheFXListStore::register_fx_list(&fx_name, fx.clone());
                    vec![fx_name]
                } else {
                    Vec::new()
                }
            },
            trail_effects,
            exhaust_particle_system_id: {
                let object_id = base_object
                    .read()
                    .map(|guard| guard.get_id())
                    .unwrap_or(INVALID_OBJECT_ID);
                if object_id == INVALID_OBJECT_ID {
                    None
                } else {
                    let veterancy = crate::helpers::TheGameLogic::find_object_by_id(source_object)
                        .and_then(|obj| obj.read().ok().map(|guard| guard.get_veterancy_level()))
                        .unwrap_or(crate::common::VeterancyLevel::Regular);
                    weapon_template
                        .get_projectile_exhaust(veterancy)
                        .and_then(|tmpl| {
                            let name = tmpl.name.as_str().trim();
                            if name.is_empty() {
                                return None;
                            }
                            let ps_manager = crate::helpers::TheParticleSystemManager::get()?;
                            let id = ps_manager.create_particle_system(Some(name))?;
                            ps_manager.attach_particle_system_to_object(id, object_id);
                            Some(id)
                        })
                }
            },

            launch_sound: if weapon_template.fire_sound.is_empty() {
                None
            } else {
                Some(weapon_template.fire_sound.name().to_string())
            },
            flight_sound: None,
            impact_sound: None,

            proximity_fuse: 0.0,
            impact_fuse: true,
            timer_fuse: 0.0,
            altitude_fuse: 0.0,

            submunition_count: 0,
            submunition_type: None,
            submunition_spread: 0.0,

            piercing: false,
            bounces_remaining: 0,
            bounce_angle_loss: 0.0,
            can_be_shot_down: true,
            stealth_penetrating: false,

            has_hit_target: false,
            is_guided: movement_pattern == ProjectileMovement::Guided
                || movement_pattern == ProjectileMovement::Homing,
            has_detonated: false,
            is_intercepted: false,
            guidance_active: false,

            flare_vulnerability: 0.0,
            chaff_vulnerability: 0.0,
            ecm_vulnerability: 0.0,

            homing_delay,
            homing_force: if guided { 1.0 } else { 0.0 },
            prediction_time: if guided { 0.25 } else { 0.0 },

            physics_active: movement_pattern == ProjectileMovement::Ballistic,
            collision_radius: 0.0,
            collision_height: 0.0,
        })
    }

    pub fn set_source_team(&mut self, team: Option<Arc<RwLock<Team>>>) {
        self.source_team = team;
    }

    /// Launch the projectile
    pub fn launch(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Calculate initial velocity based on movement pattern
        match self.movement_pattern {
            ProjectileMovement::Ballistic => {
                self.calculate_ballistic_velocity()?;
            }

            ProjectileMovement::Straight => {
                let direction = (self.target_position - self.source_position).normalize();
                self.velocity = direction * self.max_speed;
            }

            ProjectileMovement::Guided | ProjectileMovement::Homing => {
                let direction = (self.target_position - self.source_position).normalize();
                self.velocity = direction * self.max_speed;
                self.guidance_active = self.homing_delay <= 0.0;
            }

            ProjectileMovement::Parabolic => {
                self.calculate_parabolic_velocity()?;
            }

            ProjectileMovement::Beam => {
                // Instant hit beam - handled differently
                self.hit_target_instantly()?;
                return Ok(());
            }

            _ => {
                // Default to straight movement
                let direction = (self.target_position - self.source_position).normalize();
                self.velocity = direction * self.max_speed;
            }
        }

        // Set initial position
        if let Ok(mut obj_guard) = self.base_object.write() {
            let _ = obj_guard.set_position(&self.source_position);
        }

        // Play launch sound
        if let Some(sound) = &self.launch_sound {
            self.play_sound_at_position(sound, &self.source_position);
        }

        Ok(())
    }

    /// Update projectile for one frame
    pub fn update(
        &mut self,
        delta_time: Real,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.age += delta_time;

        // Check if lifetime expired
        if self.age >= self.lifetime {
            self.detonate(DetonationReason::Lifetime)?;
            return Ok(false); // Projectile should be destroyed
        }

        // Update movement
        self.update_movement(delta_time)?;

        // Update guidance
        if self.is_guided {
            self.update_guidance(delta_time)?;
        }

        // Check for collision
        if self.check_collision()? {
            if self.impact_fuse {
                self.detonate(DetonationReason::Impact)?;
                return Ok(false);
            } else if !self.piercing {
                return Ok(false);
            }
        }

        // Check proximity fuse
        if self.proximity_fuse > 0.0 {
            let distance_to_target = self.get_distance_to_target();
            if distance_to_target <= self.proximity_fuse {
                self.detonate(DetonationReason::Proximity)?;
                return Ok(false);
            }
        }

        // Check timer fuse
        if self.timer_fuse > 0.0 && self.age >= self.timer_fuse {
            self.detonate(DetonationReason::Timer)?;
            return Ok(false);
        }

        // Check altitude fuse
        if self.altitude_fuse > 0.0 {
            let current_altitude = self.get_current_altitude();
            if current_altitude <= self.altitude_fuse {
                self.detonate(DetonationReason::Altitude)?;
                return Ok(false);
            }
        }

        Ok(true) // Projectile continues to exist
    }

    /// Update movement based on physics and guidance
    fn update_movement(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_pos = self.get_current_position();

        match self.movement_pattern {
            ProjectileMovement::Ballistic => {
                // Apply gravity and air resistance
                let gravity = Coord3D::new(0.0, 0.0, -GRAVITY * self.gravity_scale);
                self.acceleration = gravity;

                // Air resistance
                let speed = self.velocity.length();
                if speed > 0.0 {
                    let drag = -self.velocity.normalize()
                        * (speed * speed * AIR_RESISTANCE * self.air_resistance_scale);
                    self.acceleration = self.acceleration + drag;
                }

                // Update velocity and position
                self.velocity = self.velocity + self.acceleration * delta_time;
                let new_pos = current_pos + self.velocity * delta_time;
                self.set_position(new_pos)?;
            }

            ProjectileMovement::Straight => {
                // Move in straight line at constant speed
                let new_pos = current_pos + self.velocity * delta_time;
                self.set_position(new_pos)?;
            }

            ProjectileMovement::Guided | ProjectileMovement::Homing => {
                if self.guidance_active {
                    self.update_guided_movement(delta_time)?;
                } else {
                    // Move straight until guidance activates
                    let new_pos = current_pos + self.velocity * delta_time;
                    self.set_position(new_pos)?;
                }
            }

            ProjectileMovement::Bouncing => {
                // Handle bouncing logic
                let new_pos = current_pos + self.velocity * delta_time;
                if self.check_for_bounce(&new_pos)? {
                    self.handle_bounce()?;
                } else {
                    self.set_position(new_pos)?;
                }
            }

            _ => {
                // Default movement
                let new_pos = current_pos + self.velocity * delta_time;
                self.set_position(new_pos)?;
            }
        }

        Ok(())
    }

    /// Update guidance system for smart projectiles
    fn update_guidance(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.guidance_active {
            if self.age >= self.homing_delay {
                self.guidance_active = true;
            } else {
                return Ok(());
            }
        }

        if self.age > self.max_guidance_time {
            self.guidance_active = false;
            return Ok(());
        }

        // Get target position (predicted if target is moving)
        let target_pos = if let Some(target_id) = self.target_object {
            self.get_predicted_target_position(target_id)?
        } else {
            self.target_position
        };

        let current_pos = self.get_current_position();
        let to_target = target_pos - current_pos;
        let distance = to_target.length();

        if distance > 0.0 {
            let desired_direction = to_target.normalize();
            let current_direction = self.velocity.normalize();

            // Calculate steering force
            let steering_force = (desired_direction - current_direction) * self.homing_force;

            // Apply turning rate limitation
            let max_turn = self.turning_rate * delta_time;
            let steering_magnitude = steering_force.length().min(max_turn);
            let limited_steering = steering_force.normalize() * steering_magnitude;

            // Update velocity direction
            self.velocity =
                (self.velocity + limited_steering * delta_time).normalize() * self.max_speed;
        }

        Ok(())
    }

    /// Detonate the projectile
    fn detonate(
        &mut self,
        _reason: DetonationReason,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.has_detonated {
            return Ok(());
        }

        self.has_detonated = true;
        let detonation_pos = self.get_current_position();

        // Create damage info
        let damage_info = DamageInfo::with_simple(
            self.damage_amount,
            self.source_object,
            crate::damage::DamageType::Explosion,
            crate::damage::DeathType::Normal,
        );

        // Apply area damage if radius > 0
        if self.damage_radius > 0.0 {
            self.apply_area_damage(&damage_info)?;
        } else {
            // Apply direct damage to target
            if let Some(target_id) = self.target_object {
                self.apply_damage_to_target(target_id, &damage_info)?;
            }
        }

        // Spawn submunitions if any
        if self.submunition_count > 0 {
            self.spawn_submunitions(detonation_pos)?;
        }

        if let Some(weapon_template) = self.source_weapon.as_ref() {
            let veterancy = self.source_veterancy_level();
            if let Some(ocl) = weapon_template.get_projectile_detonation_ocl(veterancy) {
                let _ = ocl.create_at_position(&detonation_pos, self.source_object);
            }
        }

        // Play impact effects
        self.play_impact_effects(&detonation_pos);

        // Play impact sound
        if let Some(sound) = &self.impact_sound {
            self.play_sound_at_position(sound, &detonation_pos);
        }

        Ok(())
    }

    /// Check for collision with terrain or objects
    fn check_collision(&self) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let current_pos = self.get_current_position();

        // Check terrain collision
        if current_pos.z <= 0.0 {
            // Simplified ground check
            return Ok(true);
        }

        // Check object collision (would use spatial partitioning system)
        // This would query nearby objects and check for intersection

        Ok(false)
    }

    /// Get current position from the base object
    fn get_current_position(&self) -> Coord3D {
        if let Ok(obj_guard) = self.base_object.read() {
            *obj_guard.get_position()
        } else {
            Coord3D::new(0.0, 0.0, 0.0)
        }
    }

    /// Set position on the base object
    fn set_position(
        &self,
        position: Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut obj_guard) = self.base_object.write() {
            let _ = obj_guard.set_position(&position);
        }
        Ok(())
    }

    /// Calculate ballistic trajectory velocity
    fn calculate_ballistic_velocity(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(velocity) = ballistics::calculate_launch_velocity(
            self.source_position,
            self.target_position,
            self.initial_velocity,
        ) {
            self.velocity = velocity;
        } else {
            // Fallback to direct trajectory
            let displacement = self.target_position - self.source_position;
            let direction = displacement.normalize();
            self.velocity = direction * self.initial_velocity;
        }

        Ok(())
    }

    /// Calculate launch angle for ballistic trajectory
    fn calculate_launch_angle(source: Coord3D, target: Coord3D, velocity: Real) -> Real {
        let displacement = target - source;
        let horizontal_distance =
            (displacement.x * displacement.x + displacement.y * displacement.y).sqrt();
        let vertical_distance = displacement.z;

        // Calculate optimal launch angle
        let gravity = GRAVITY;
        let v_squared = velocity * velocity;
        let discriminant = v_squared * v_squared
            - gravity
                * (gravity * horizontal_distance * horizontal_distance
                    + 2.0 * vertical_distance * v_squared);

        if discriminant >= 0.0 {
            let angle1 =
                ((v_squared - discriminant.sqrt()) / (gravity * horizontal_distance)).atan();
            let angle2 =
                ((v_squared + discriminant.sqrt()) / (gravity * horizontal_distance)).atan();

            // Choose the lower angle for direct fire
            angle1.min(angle2)
        } else {
            // No ballistic solution: aim directly at the target vector.
            if horizontal_distance <= f32::EPSILON {
                if vertical_distance >= 0.0 {
                    std::f32::consts::FRAC_PI_2
                } else {
                    -std::f32::consts::FRAC_PI_2
                }
            } else {
                vertical_distance.atan2(horizontal_distance)
            }
        }
    }

    // Helper methods and type definitions
    fn determine_projectile_type(weapon_template: &WeaponTemplate) -> ProjectileType {
        // Analyze weapon template to determine projectile type
        if Self::has_projectile_behavior(weapon_template, "MissileAIUpdate")
            || Self::has_projectile_behavior(weapon_template, "SmartBombTargetHomingUpdate")
        {
            ProjectileType::Missile
        } else if Self::has_projectile_behavior(weapon_template, "DumbProjectileBehavior") {
            ProjectileType::Shell
        } else if !weapon_template.laser_name.is_empty() {
            ProjectileType::Beam
        } else {
            ProjectileType::Bullet
        }
    }

    fn determine_movement_pattern(weapon_template: &WeaponTemplate) -> ProjectileMovement {
        if !weapon_template.laser_name.is_empty() {
            ProjectileMovement::Beam
        } else if Self::has_projectile_behavior(weapon_template, "MissileAIUpdate")
            || Self::has_projectile_behavior(weapon_template, "SmartBombTargetHomingUpdate")
        {
            ProjectileMovement::Guided
        } else if Self::has_projectile_behavior(weapon_template, "DumbProjectileBehavior") {
            ProjectileMovement::Ballistic
        } else {
            ProjectileMovement::Straight
        }
    }

    fn projectile_template(
        weapon_template: &WeaponTemplate,
    ) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        let name = weapon_template.projectile_name.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("NONE") {
            return None;
        }
        TheThingFactory::find_template(name)
    }

    fn has_projectile_behavior(weapon_template: &WeaponTemplate, behavior_name: &str) -> bool {
        let Some(template) = Self::projectile_template(weapon_template) else {
            return false;
        };
        template
            .get_behavior_module_info()
            .iter()
            .any(|info| info.name.as_str() == behavior_name)
    }

    fn missile_ai_data(weapon_template: &WeaponTemplate) -> Option<MissileAIUpdateModuleData> {
        let template = Self::projectile_template(weapon_template)?;
        for info in template.get_behavior_module_info() {
            if info.name.as_str() != "MissileAIUpdate" {
                continue;
            }
            if let Some(data) = info
                .data
                .as_any()
                .downcast_ref::<MissileAIUpdateModuleData>()
            {
                return Some(data.clone());
            }
        }
        None
    }

    fn dumb_projectile_data(
        weapon_template: &WeaponTemplate,
    ) -> Option<DumbProjectileBehaviorModuleData> {
        let template = Self::projectile_template(weapon_template)?;
        for info in template.get_behavior_module_info() {
            if info.name.as_str() != "DumbProjectileBehavior" {
                continue;
            }
            if let Some(data) = info
                .data
                .as_any()
                .downcast_ref::<DumbProjectileBehaviorModuleData>()
            {
                return Some(data.clone());
            }
        }
        None
    }

    fn projectile_trail_particle_name(weapon_template: &WeaponTemplate) -> Option<String> {
        let template = Self::projectile_template(weapon_template)?;
        for info in template.get_draw_module_info() {
            if info.name.as_str() != "W3DProjectileDraw" {
                continue;
            }
            if let Some(data) = info
                .data
                .as_any()
                .downcast_ref::<W3DProjectileDrawModuleData>()
            {
                let name = data.trail_particle_system.as_str().trim();
                return if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                };
            }
        }
        None
    }

    fn projectile_trail_interval_seconds(weapon_template: &WeaponTemplate) -> Option<Real> {
        let template = Self::projectile_template(weapon_template)?;
        for info in template.get_draw_module_info() {
            if info.name.as_str() != "W3DProjectileDraw" {
                continue;
            }
            if let Some(data) = info
                .data
                .as_any()
                .downcast_ref::<W3DProjectileDrawModuleData>()
            {
                return Some(if data.trail_interval_frames == 0 {
                    0.0
                } else {
                    data.trail_interval_frames as Real / LOGICFRAMES_PER_SECOND as Real
                });
            }
        }
        None
    }

    /// Calculate parabolic velocity for artillery-style arc
    fn calculate_parabolic_velocity(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Similar to ballistic but with specific arc constraints
        let displacement = self.target_position - self.source_position;
        let horizontal_distance =
            (displacement.x * displacement.x + displacement.y * displacement.y).sqrt();
        let vertical_distance = displacement.z;

        // Use 45-degree angle for maximum range
        let launch_angle = std::f32::consts::PI / 4.0;
        let cos_angle = launch_angle.cos();
        let sin_angle = launch_angle.sin();

        // Calculate required velocity
        let gravity = GRAVITY;
        let velocity_squared = (gravity * horizontal_distance)
            / (2.0
                * cos_angle
                * cos_angle
                * (horizontal_distance * sin_angle.tan() - vertical_distance));

        if velocity_squared > 0.0 {
            let velocity = velocity_squared.sqrt();
            let direction = displacement.normalize();

            self.velocity = Coord3D::new(
                direction.x * velocity * cos_angle,
                direction.y * velocity * cos_angle,
                velocity * sin_angle,
            );
        } else {
            // Fallback to direct trajectory
            let direction = displacement.normalize();
            self.velocity = direction * self.initial_velocity;
        }

        Ok(())
    }

    /// Instant hit for beam weapons (no travel time)
    fn hit_target_instantly(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let damage_info = DamageInfo::with_simple(
            self.damage_amount,
            self.source_object,
            crate::damage::DamageType::Explosion,
            crate::damage::DeathType::Normal,
        );

        // Apply damage immediately at target
        if let Some(target_id) = self.target_object {
            self.apply_damage_to_target(target_id, &damage_info)?;
        }

        // Play effects at both ends
        self.play_impact_effects(&self.target_position);

        // Mark as detonated
        self.has_detonated = true;
        Ok(())
    }

    /// Play sound at a specific position
    fn play_sound_at_position(&self, _sound: &str, _position: &Coord3D) {
        // Would integrate with audio system
        // TheGameAudio->playSound(sound, position);
    }

    /// Update guided movement (proportional navigation)
    fn update_guided_movement(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get target position (current or predicted)
        let target_pos = if let Some(target_id) = self.target_object {
            self.get_predicted_target_position(target_id)?
        } else {
            self.target_position
        };

        let current_pos = self.get_current_position();
        let to_target = target_pos - current_pos;
        let distance = to_target.length();

        if distance > 0.1 {
            // Proportional navigation: steer toward predicted intercept point
            let desired_velocity = to_target.normalize() * self.max_speed;

            // Calculate steering acceleration
            let velocity_change = desired_velocity - self.velocity;
            let max_accel = self.homing_force * delta_time;

            // Limit acceleration magnitude
            let accel_magnitude = velocity_change.length().min(max_accel);
            if accel_magnitude > 0.0 {
                let acceleration = velocity_change.normalize() * accel_magnitude / delta_time;

                // Update velocity
                self.velocity = self.velocity + acceleration * delta_time;

                // Maintain speed
                let current_speed = self.velocity.length();
                if current_speed > 0.0 {
                    self.velocity = self.velocity.normalize() * self.max_speed;
                }
            }

            // Update position
            let new_pos = current_pos + self.velocity * delta_time;
            self.set_position(new_pos)?;
        }

        Ok(())
    }

    /// Check if projectile should bounce
    fn check_for_bounce(
        &self,
        new_pos: &Coord3D,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Check if hit ground or wall
        if new_pos.z <= 0.0 && self.bounces_remaining > 0 {
            return Ok(true);
        }

        // Would also check wall/object collisions
        Ok(false)
    }

    /// Handle bounce physics
    fn handle_bounce(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.bounces_remaining == 0 {
            return Ok(());
        }

        self.bounces_remaining -= 1;

        // Reflect velocity (simplified - would use surface normal)
        self.velocity.z = -self.velocity.z * (1.0 - self.bounce_angle_loss);
        self.velocity.x *= 0.9; // Energy loss
        self.velocity.y *= 0.9;

        Ok(())
    }

    /// Get predicted target position based on target velocity
    fn get_predicted_target_position(
        &self,
        target_id: ObjectID,
    ) -> Result<Coord3D, Box<dyn std::error::Error + Send + Sync>> {
        let Some(target_arc) = crate::helpers::TheGameLogic::find_object_by_id(target_id) else {
            return Ok(self.target_position);
        };
        let Ok(target_guard) = target_arc.read() else {
            return Ok(self.target_position);
        };
        let current_target_pos = *target_guard.get_position();
        drop(target_guard);

        if self.prediction_time <= 0.0 {
            return Ok(current_target_pos);
        }

        let target_velocity = get_physics_engine()
            .read()
            .ok()
            .and_then(|engine| engine.get_object_velocity(target_id))
            .unwrap_or(Coord3D::ZERO);

        Ok(current_target_pos + target_velocity * self.prediction_time)
    }

    /// Get distance to target
    fn get_distance_to_target(&self) -> Real {
        (self.get_current_position() - self.target_position).length()
    }

    /// Get current altitude above ground
    fn get_current_altitude(&self) -> Real {
        self.get_current_position().z
    }

    /// Apply area-of-effect damage
    fn apply_area_damage(
        &self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let detonation_pos = self.get_current_position();
        let Some(partition) = crate::helpers::ThePartitionManager::get() else {
            return Ok(());
        };

        let source_arc = if self.source_object != INVALID_OBJECT_ID {
            crate::helpers::TheGameLogic::find_object_by_id(self.source_object)
        } else {
            None
        };

        let projectile_id = self
            .base_object
            .read()
            .ok()
            .map(|guard| guard.get_id())
            .unwrap_or(INVALID_OBJECT_ID);

        for candidate_id in partition.get_objects_in_range(&detonation_pos, self.damage_radius) {
            if candidate_id == projectile_id {
                continue;
            }

            let Some(candidate_arc) = crate::helpers::TheGameLogic::find_object_by_id(candidate_id)
            else {
                continue;
            };

            let (distance, should_damage) = {
                let Ok(candidate_guard) = candidate_arc.read() else {
                    continue;
                };
                if candidate_guard.is_effectively_dead() {
                    continue;
                }

                let distance = (*candidate_guard.get_position() - detonation_pos).length();
                if distance > self.damage_radius {
                    continue;
                }

                let should_damage = if !self.friendly_fire {
                    if let Some(source) = source_arc.as_ref() {
                        if let Ok(source_guard) = source.read() {
                            let rel = source_guard.relationship_to(&candidate_guard);
                            matches!(
                                rel,
                                crate::common::Relationship::Enemies
                                    | crate::common::Relationship::Neutral
                            )
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                } else {
                    true
                };

                (distance, should_damage)
            };

            if !should_damage {
                continue;
            }

            let multiplier = if self.damage_falloff > 0.0 && self.damage_radius > f32::EPSILON {
                let edge_fraction = (distance / self.damage_radius).clamp(0.0, 1.0);
                // `damage_falloff` is treated as the fraction removed at max radius.
                1.0 - self.damage_falloff.clamp(0.0, 1.0) * edge_fraction
            } else {
                1.0
            };

            let mut scaled_damage = damage_info.clone();
            scaled_damage.input.amount = (damage_info.input.amount * multiplier).max(0.0);
            scaled_damage.sync_from_input();
            if scaled_damage.input.amount <= 0.0 {
                continue;
            }

            if let Ok(mut candidate_guard) = candidate_arc.write() {
                let _ = candidate_guard.attempt_damage(&mut scaled_damage);
            };
        }

        Ok(())
    }

    /// Apply damage to specific target
    fn apply_damage_to_target(
        &self,
        target_id: ObjectID,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(target_arc) = crate::helpers::TheGameLogic::find_object_by_id(target_id) else {
            return Ok(());
        };

        if !self.friendly_fire && self.source_object != INVALID_OBJECT_ID {
            if let Some(source_arc) =
                crate::helpers::TheGameLogic::find_object_by_id(self.source_object)
            {
                if let (Ok(source_guard), Ok(target_guard)) = (source_arc.read(), target_arc.read())
                {
                    let rel = source_guard.relationship_to(&target_guard);
                    if !matches!(
                        rel,
                        crate::common::Relationship::Enemies | crate::common::Relationship::Neutral
                    ) {
                        return Ok(());
                    }
                }
            }
        }

        let mut damage = damage_info.clone();
        if let Ok(mut target_guard) = target_arc.write() {
            let _ = target_guard.attempt_damage(&mut damage);
        }

        Ok(())
    }

    /// Spawn submunitions (cluster bombs, etc)
    fn spawn_submunitions(
        &self,
        position: Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.submunition_count == 0 || self.submunition_type.is_none() {
            return Ok(());
        }

        // Spawn multiple projectiles in a spread pattern
        for i in 0..self.submunition_count {
            let angle = (i as f32) * (2.0 * std::f32::consts::PI / self.submunition_count as f32);
            let spread_angle = self.submunition_spread;

            // Calculate offset position
            let offset = Coord3D::new(spread_angle * angle.cos(), spread_angle * angle.sin(), 0.0);

            let _submunition_pos = position + offset;

            // Would create new projectile object
            // TheThingFactory->createProjectile(self.submunition_type, position, submunition_pos);
        }

        Ok(())
    }

    /// Play impact effects
    fn play_impact_effects(&self, position: &Coord3D) {
        if self.impact_effects.is_empty() {
            return;
        }

        for effect in &self.impact_effects {
            if let Some(fx) = crate::helpers::TheFXListStore::find_fx_list(effect.as_str()) {
                let _ = fx.do_fx_at_position(position);
                continue;
            }

            if let Some(ps_manager) = crate::helpers::TheParticleSystemManager::get() {
                if let Some(system_id) = ps_manager.create_particle_system(Some(effect.as_str())) {
                    ps_manager.set_particle_system_position(system_id, position);
                }
            }
        }
    }
}

/// Reasons for projectile detonation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetonationReason {
    Impact,
    Proximity,
    Timer,
    Altitude,
    Lifetime,
    Intercept,
}

/// Extension trait for Object to provide Projectile-specific functionality
pub trait ProjectileExt {
    /// Get projectile-specific data if this object is a projectile
    fn as_projectile(&self) -> Option<&Projectile>;
    fn as_projectile_mut(&mut self) -> Option<&mut Projectile>;
}
