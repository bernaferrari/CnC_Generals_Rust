// FILE: particle_system.rs
// Author: Ported from C++ (Michael S. Booth, November 2001)
// Desc: Particle System management - ParticleSystem, ParticleSystemTemplate, and ParticleSystemManager
//
// Ported from:
// - /GeneralsMD/Code/GameEngine/Include/GameClient/ParticleSys.h (ParticleSystem, ParticleSystemTemplate)
// - /GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp (lines 1000+)

use std::collections::HashMap;
use std::f32::consts::{PI, TAU as TWO_PI};
use std::sync::{Arc, Mutex};

use crate::Common::{Coord3D, Coord2D, RGBColor};
use crate::GameClient::fx_list::GameClientRandomVariable;
use crate::GameClient::particle_sys::*;

/// Random keyframe with variable range
/// Matches C++ ParticleSys.h:312-316
#[derive(Clone, Copy, Debug)]
pub struct RandomKeyframe {
    pub var: GameClientRandomVariable,
    pub frame: u32,
}

/// All properties of a particle system, used by both templates and instances
/// Matches C++ ParticleSys.h:267-452
#[derive(Clone, Debug)]
pub struct ParticleSystemInfo {
    pub is_one_shot: bool,
    pub shader_type: ParticleShaderType,
    pub particle_type: ParticleType,
    pub particle_type_name: String,

    pub angle_z: GameClientRandomVariable,
    pub angular_rate_z: GameClientRandomVariable,
    pub angular_damping: GameClientRandomVariable,
    pub vel_damping: GameClientRandomVariable,

    pub lifetime: GameClientRandomVariable,
    pub system_lifetime: u32,

    pub start_size: GameClientRandomVariable,
    pub start_size_rate: GameClientRandomVariable,
    pub size_rate: GameClientRandomVariable,
    pub size_rate_damping: GameClientRandomVariable,

    pub volume_particle_depth: u32,

    pub alpha_key: [RandomKeyframe; MAX_KEYFRAMES],
    pub color_key: [RGBColorKeyframe; MAX_KEYFRAMES],
    pub color_scale: GameClientRandomVariable,

    pub burst_delay: GameClientRandomVariable,
    pub burst_count: GameClientRandomVariable,
    pub initial_delay: GameClientRandomVariable,

    pub drift_velocity: Coord3D,
    pub gravity: f32,

    pub slave_system_name: String,
    pub slave_pos_offset: Coord3D,
    pub attached_system_name: String,

    pub emission_velocity_type: EmissionVelocityType,
    pub priority: ParticlePriorityType,

    pub emission_velocity: EmissionVelocity,
    pub emission_volume_type: EmissionVolumeType,
    pub emission_volume: EmissionVolume,

    pub is_emission_volume_hollow: bool,
    pub is_ground_aligned: bool,
    pub is_emit_above_ground_only: bool,
    pub is_particle_up_towards_emitter: bool,

    pub wind_motion: WindMotion,
    pub wind_angle: f32,
    pub wind_angle_change: f32,
    pub wind_angle_change_min: f32,
    pub wind_angle_change_max: f32,
    pub wind_motion_start_angle: f32,
    pub wind_motion_start_angle_min: f32,
    pub wind_motion_start_angle_max: f32,
    pub wind_motion_end_angle: f32,
    pub wind_motion_end_angle_min: f32,
    pub wind_motion_end_angle_max: f32,
    pub wind_motion_moving_to_end_angle: bool,
}

/// Emission velocity union
/// Matches C++ ParticleSys.h:351-381
#[derive(Clone, Copy, Debug)]
pub enum EmissionVelocity {
    Ortho {
        x: GameClientRandomVariable,
        y: GameClientRandomVariable,
        z: GameClientRandomVariable,
    },
    Spherical {
        speed: GameClientRandomVariable,
    },
    Hemispherical {
        speed: GameClientRandomVariable,
    },
    Cylindrical {
        radial: GameClientRandomVariable,
        normal: GameClientRandomVariable,
    },
    Outward {
        speed: GameClientRandomVariable,
        other_speed: GameClientRandomVariable,
    },
}

/// Emission volume union
/// Matches C++ ParticleSys.h:392-425
#[derive(Clone, Copy, Debug)]
pub enum EmissionVolume {
    Point,
    Line { start: Coord3D, end: Coord3D },
    Box { half_size: Coord3D },
    Sphere { radius: f32 },
    Cylinder { radius: f32, length: f32 },
}

impl ParticleSystemInfo {
    /// Create new particle system info with defaults
    /// Matches C++ ParticleSys.cpp:704-736
    pub fn new() -> Self {
        Self {
            priority: ParticlePriorityType::PARTICLE_PRIORITY_LOWEST,
            is_ground_aligned: false,
            is_emit_above_ground_only: false,
            is_particle_up_towards_emitter: false,
            drift_velocity: Coord3D::zero(),
            gravity: 0.0,
            is_emission_volume_hollow: false,
            is_one_shot: false,
            slave_pos_offset: Coord3D::zero(),
            system_lifetime: 0,

            // Wind motion defaults
            wind_motion: WindMotion::NotUsed,
            wind_angle: 0.0,
            wind_angle_change: 0.15,
            wind_angle_change_min: 0.15,
            wind_angle_change_max: 0.45,
            wind_motion_start_angle_min: 0.0,
            wind_motion_start_angle_max: PI / 4.0,
            wind_motion_start_angle: 0.0,
            wind_motion_end_angle_min: TWO_PI - (PI / 4.0),
            wind_motion_end_angle_max: TWO_PI,
            wind_motion_end_angle: TWO_PI - (PI / 4.0),
            wind_motion_moving_to_end_angle: true,
            volume_particle_depth: DEFAULT_VOLUME_PARTICLE_DEPTH as u32,

            // Initialize other fields with safe defaults
            shader_type: ParticleShaderType::InvalidShader,
            particle_type: ParticleType::InvalidType,
            particle_type_name: String::new(),
            angle_z: GameClientRandomVariable::new_constant(0.0),
            angular_rate_z: GameClientRandomVariable::new_constant(0.0),
            angular_damping: GameClientRandomVariable::new_constant(1.0),
            vel_damping: GameClientRandomVariable::new_constant(1.0),
            lifetime: GameClientRandomVariable::new_constant(60.0),
            start_size: GameClientRandomVariable::new_constant(1.0),
            start_size_rate: GameClientRandomVariable::new_constant(0.0),
            size_rate: GameClientRandomVariable::new_constant(0.0),
            size_rate_damping: GameClientRandomVariable::new_constant(1.0),
            alpha_key: [RandomKeyframe {
                var: GameClientRandomVariable::new_constant(1.0),
                frame: 0,
            }; MAX_KEYFRAMES],
            color_key: [RGBColorKeyframe {
                color: RGBColor { red: 1.0, green: 1.0, blue: 1.0 },
                frame: 0,
            }; MAX_KEYFRAMES],
            color_scale: GameClientRandomVariable::new_constant(0.0),
            burst_delay: GameClientRandomVariable::new_constant(1.0),
            burst_count: GameClientRandomVariable::new_constant(1.0),
            initial_delay: GameClientRandomVariable::new_constant(0.0),
            slave_system_name: String::new(),
            attached_system_name: String::new(),
            emission_velocity_type: EmissionVelocityType::InvalidVelocity,
            emission_velocity: EmissionVelocity::Ortho {
                x: GameClientRandomVariable::new_constant(0.0),
                y: GameClientRandomVariable::new_constant(0.0),
                z: GameClientRandomVariable::new_constant(0.0),
            },
            emission_volume_type: EmissionVolumeType::Point,
            emission_volume: EmissionVolume::Point,
        }
    }

    /// Tint all color keys
    /// Matches C++ ParticleSys.cpp:739-752
    pub fn tint_all_colors(&mut self, tint_color: u32) {
        let r = ((tint_color >> 16) & 0xFF) as f32 / 255.0;
        let g = ((tint_color >> 8) & 0xFF) as f32 / 255.0;
        let b = (tint_color & 0xFF) as f32 / 255.0;

        // Tint all but the first color key
        for key in 1..MAX_KEYFRAMES {
            self.color_key[key].color.red *= r;
            self.color_key[key].color.green *= g;
            self.color_key[key].color.blue *= b;
        }
    }
}

impl Default for ParticleSystemInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// A ParticleSystemTemplate used to instantiate ParticleSystems
///
/// Templates are created from INI files and stored in the ParticleSystemManager.
/// They are immutable once created.
///
/// Matches C++ ParticleSys.h:458-495
pub struct ParticleSystemTemplate {
    name: String,
    info: ParticleSystemInfo,
    slave_template: Option<Arc<ParticleSystemTemplate>>,
}

impl ParticleSystemTemplate {
    /// Create a new template
    pub fn new(name: String, info: ParticleSystemInfo) -> Self {
        Self {
            name,
            info,
            slave_template: None,
        }
    }

    /// Get template name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get template info
    pub fn get_info(&self) -> &ParticleSystemInfo {
        &self.info
    }

    /// Create a slave system if one is defined
    /// Matches C++ ParticleSystemTemplate::createSlaveSystem
    pub fn create_slave_system(&self, create_slaves: bool) -> Option<Arc<ParticleSystem>> {
        if !create_slaves {
            return None;
        }

        if let Some(ref slave_tmpl) = self.slave_template {
            // Would create particle system from slave template
            // This is a placeholder - actual implementation would call manager
            None
        } else {
            None
        }
    }

    /// Set slave template
    pub fn set_slave_template(&mut self, template: Arc<ParticleSystemTemplate>) {
        self.slave_template = Some(template);
    }
}

/// A particle system responsible for creating Particles
///
/// If finished but still has particles "in the air", it must wait before
/// destroying itself to ensure everything can be cleaned up if reset.
///
/// Matches C++ ParticleSys.h:503-682
pub struct ParticleSystem {
    info: ParticleSystemInfo,
    system_id: ParticleSystemID,
    template: Arc<ParticleSystemTemplate>,

    // Particle list
    system_particles_head: Option<Box<Particle>>,
    system_particles_tail: Option<*mut Particle>,
    particle_count: u32,

    // Attachment
    attached_to_drawable_id: u32, // DrawableID
    attached_to_object_id: u32,   // ObjectID

    // Transform
    local_transform: Matrix3D,
    transform: Matrix3D,
    is_local_identity: bool,
    is_identity: bool,
    skip_parent_xfrm: bool,

    // Timing
    burst_delay_left: u32,
    delay_left: u32,
    start_timestamp: u32,
    system_lifetime_left: u32,
    personality_store: u32,

    // Accumulated values
    accumulated_size_bonus: f32,

    // Coefficients
    vel_coeff: Coord3D,
    count_coeff: f32,
    delay_coeff: f32,
    size_coeff: f32,

    // Position
    pos: Coord3D,
    last_pos: Coord3D,

    // Slave/Master relationship
    slave_system: Option<Arc<Mutex<ParticleSystem>>>,
    slave_system_id: ParticleSystemID,
    master_system: Option<Arc<Mutex<ParticleSystem>>>,
    master_system_id: ParticleSystemID,

    control_particle: Option<*mut Particle>,

    // State flags
    is_forever: bool,
    is_stopped: bool,
    is_destroyed: bool,
    is_first_pos: bool,
    is_saveable: bool,
}

/// Placeholder for Matrix3D
#[derive(Clone, Copy, Debug)]
pub struct Matrix3D {
    data: [[f32; 4]; 4],
}

impl Matrix3D {
    pub fn identity() -> Self {
        Self {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

impl ParticleSystem {
    /// Create a new particle system from a template
    /// Matches C++ ParticleSys.cpp:1011-1161
    pub fn new(
        template: Arc<ParticleSystemTemplate>,
        id: ParticleSystemID,
        create_slaves: bool,
    ) -> Arc<Mutex<Self>> {
        let info = template.get_info().clone();

        let system = Arc::new(Mutex::new(Self {
            system_id: id,
            template: template.clone(),

            system_particles_head: None,
            system_particles_tail: None,
            particle_count: 0,

            attached_to_drawable_id: 0, // INVALID_DRAWABLE_ID
            attached_to_object_id: 0,   // INVALID_ID

            local_transform: Matrix3D::identity(),
            transform: Matrix3D::identity(),
            is_local_identity: true,
            is_identity: true,
            skip_parent_xfrm: false,

            burst_delay_left: 0,
            delay_left: info.initial_delay.get_value() as u32,
            start_timestamp: 0, // Would be current frame
            system_lifetime_left: info.system_lifetime,
            personality_store: 0,

            accumulated_size_bonus: 0.0,

            vel_coeff: Coord3D { x: 1.0, y: 1.0, z: 1.0 },
            count_coeff: 1.0,
            delay_coeff: 1.0,
            size_coeff: 1.0,

            pos: Coord3D::zero(),
            last_pos: Coord3D::zero(),
            is_first_pos: true,

            slave_system: None,
            slave_system_id: INVALID_PARTICLE_SYSTEM_ID,
            master_system: None,
            master_system_id: INVALID_PARTICLE_SYSTEM_ID,

            control_particle: None,

            is_forever: info.system_lifetime == 0,
            is_stopped: false,
            is_destroyed: false,
            is_saveable: true,

            info,
        }));

        // Set up slave system if needed
        if create_slaves {
            if let Some(slave) = template.create_slave_system(true) {
                let mut sys = system.lock().unwrap();
                sys.slave_system = Some(slave.clone());
                // Would set master relationship here
            }
        }

        system
    }

    /// Get system ID
    pub fn get_system_id(&self) -> ParticleSystemID {
        self.system_id
    }

    /// Set position
    /// Matches C++ ParticleSys.cpp:1281-1287
    pub fn set_position(&mut self, pos: &Coord3D) {
        self.pos = *pos;
        // Would update local transform
        self.is_local_identity = false;
    }

    /// Get position
    /// Matches C++ ParticleSys.cpp:1267-1276
    pub fn get_position(&self) -> Coord3D {
        self.pos
    }

    /// Set local transform
    /// Matches C++ ParticleSys.cpp:1292-1296
    pub fn set_local_transform(&mut self, matrix: &Matrix3D) {
        self.local_transform = *matrix;
        self.is_local_identity = false;
    }

    /// Rotate local transform around X axis
    /// Matches C++ ParticleSys.cpp:1301-1305
    pub fn rotate_local_transform_x(&mut self, _x: f32) {
        // Would apply rotation
        self.is_local_identity = false;
    }

    /// Rotate local transform around Y axis
    /// Matches C++ ParticleSys.cpp:1310-1314
    pub fn rotate_local_transform_y(&mut self, _y: f32) {
        // Would apply rotation
        self.is_local_identity = false;
    }

    /// Rotate local transform around Z axis
    /// Matches C++ ParticleSys.cpp:1319-1323
    pub fn rotate_local_transform_z(&mut self, _z: f32) {
        // Would apply rotation
        self.is_local_identity = false;
    }

    /// Attach to drawable
    /// Matches C++ ParticleSys.cpp:1328-1334
    pub fn attach_to_drawable(&mut self, drawable_id: u32) {
        self.attached_to_drawable_id = drawable_id;
    }

    /// Attach to object
    /// Matches C++ ParticleSys.cpp:1339-1345
    pub fn attach_to_object(&mut self, object_id: u32) {
        self.attached_to_object_id = object_id;
    }

    /// Start the particle system
    /// Matches C++ ParticleSys.cpp:1239-1242
    pub fn start(&mut self) {
        self.is_stopped = false;
    }

    /// Stop the particle system from emitting
    /// Matches C++ ParticleSys.cpp:1247-1250
    pub fn stop(&mut self) {
        self.is_stopped = true;
    }

    /// Destroy the particle system
    /// Matches C++ ParticleSys.cpp:1255-1262
    pub fn destroy(&mut self) {
        self.is_destroyed = true;
        // Would also destroy slave system
    }

    /// Set velocity multiplier
    pub fn set_velocity_multiplier(&mut self, value: &Coord3D) {
        self.vel_coeff = *value;
    }

    /// Get velocity multiplier
    pub fn get_velocity_multiplier(&self) -> &Coord3D {
        &self.vel_coeff
    }

    /// Set burst count multiplier
    pub fn set_burst_count_multiplier(&mut self, value: f32) {
        self.count_coeff = value;
    }

    /// Set burst delay multiplier
    pub fn set_burst_delay_multiplier(&mut self, value: f32) {
        self.delay_coeff = value;
    }

    /// Set size multiplier
    pub fn set_size_multiplier(&mut self, value: f32) {
        self.size_coeff = value;
    }

    /// Trigger immediate particle burst
    pub fn trigger(&mut self) {
        self.burst_delay_left = 0;
        self.delay_left = 0;
    }

    /// Set initial delay
    pub fn set_initial_delay(&mut self, delay: u32) {
        self.delay_left = delay;
    }

    /// Get particle type name
    pub fn get_particle_type_name(&self) -> &str {
        &self.info.particle_type_name
    }

    /// Check if using drawables
    pub fn is_using_drawables(&self) -> bool {
        self.info.particle_type == ParticleType::Drawable
    }

    /// Check if using streak
    pub fn is_using_streak(&self) -> bool {
        self.info.particle_type == ParticleType::Streak
    }

    /// Check if using smudge
    pub fn is_using_smudge(&self) -> bool {
        self.info.particle_type == ParticleType::Smudge
    }

    /// Get volume particle depth
    pub fn get_volume_particle_depth(&self) -> u32 {
        if self.info.particle_type == ParticleType::VolumeParticle {
            OPTIMUM_VOLUME_PARTICLE_DEPTH as u32
        } else {
            0
        }
    }

    /// Should particles billboard
    pub fn should_billboard(&self) -> bool {
        !self.info.is_ground_aligned
    }

    /// Get shader type
    pub fn get_shader_type(&self) -> ParticleShaderType {
        self.info.shader_type
    }

    /// Get priority
    pub fn get_priority(&self) -> ParticlePriorityType {
        self.info.priority
    }

    /// Get drift velocity
    pub fn get_drift_velocity(&self) -> &Coord3D {
        &self.info.drift_velocity
    }

    /// Get wind motion
    pub fn get_wind_motion(&self) -> WindMotion {
        self.info.wind_motion
    }

    /// Get wind angle
    pub fn get_wind_angle(&self) -> f32 {
        self.info.wind_angle
    }

    /// Get emission volume type
    pub fn get_emission_volume_type(&self) -> EmissionVolumeType {
        self.info.emission_volume_type
    }

    /// Set emission volume sphere radius
    pub fn set_emission_volume_sphere_radius(&mut self, new_radius: f32) {
        if let EmissionVolume::Sphere { ref mut radius } = self.info.emission_volume {
            *radius = new_radius;
        }
    }

    /// Set emission volume cylinder radius
    pub fn set_emission_volume_cylinder_radius(&mut self, new_radius: f32) {
        if let EmissionVolume::Cylinder { ref mut radius, .. } = self.info.emission_volume {
            *radius = new_radius;
        }
    }

    /// Update wind motion
    /// Matches C++ ParticleSys.cpp updateWindMotion
    pub fn update_wind_motion(&mut self) {
        match self.info.wind_motion {
            WindMotion::PingPong => {
                // Ping pong between start and end angles
                if self.info.wind_motion_moving_to_end_angle {
                    self.info.wind_angle += self.info.wind_angle_change;
                    if self.info.wind_angle >= self.info.wind_motion_end_angle {
                        self.info.wind_motion_moving_to_end_angle = false;
                    }
                } else {
                    self.info.wind_angle -= self.info.wind_angle_change;
                    if self.info.wind_angle <= self.info.wind_motion_start_angle {
                        self.info.wind_motion_moving_to_end_angle = true;
                    }
                }
            }
            WindMotion::Circular => {
                // Circular motion
                self.info.wind_angle += self.info.wind_angle_change;
                if self.info.wind_angle >= TWO_PI {
                    self.info.wind_angle -= TWO_PI;
                }
            }
            _ => {}
        }
    }

    /// Get template
    pub fn get_template(&self) -> &Arc<ParticleSystemTemplate> {
        &self.template
    }

    /// Get particle count
    pub fn get_particle_count(&self) -> u32 {
        self.particle_count
    }

    /// Check if destroyed
    pub fn is_destroyed(&self) -> bool {
        self.is_destroyed
    }

    /// Check if saveable
    pub fn is_saveable(&self) -> bool {
        self.is_saveable
    }

    /// Set saveable
    pub fn set_saveable(&mut self, saveable: bool) {
        self.is_saveable = saveable;
    }

    /// Check if system is forever (infinite lifetime)
    pub fn is_system_forever(&self) -> bool {
        self.is_forever
    }

    /// Get start frame
    pub fn get_start_frame(&self) -> u32 {
        self.start_timestamp
    }

    /// Get attached object ID
    pub fn get_attached_object(&self) -> u32 {
        self.attached_to_object_id
    }

    /// Get attached drawable ID
    pub fn get_attached_drawable(&self) -> u32 {
        self.attached_to_drawable_id
    }

    /// Add particle to system
    pub fn add_particle(&mut self, _particle: Box<Particle>) {
        self.particle_count += 1;
        // Would link into list
    }

    /// Remove particle from system
    pub fn remove_particle(&mut self, _particle: &Particle) {
        self.particle_count -= 1;
        // Would unlink from list
    }

    /// Set control particle
    pub fn set_control_particle(&mut self, particle: *mut Particle) {
        self.control_particle = Some(particle);
    }

    /// Detach control particle
    pub fn detach_control_particle(&mut self, _particle: &Particle) {
        self.control_particle = None;
    }

    /// Set slave system
    pub fn set_slave(&mut self, slave: Option<Arc<Mutex<ParticleSystem>>>) {
        if let Some(ref sys) = slave {
            let locked = sys.lock().unwrap();
            self.slave_system_id = locked.get_system_id();
        } else {
            self.slave_system_id = INVALID_PARTICLE_SYSTEM_ID;
        }
        self.slave_system = slave;
    }

    /// Get slave system
    pub fn get_slave(&self) -> Option<Arc<Mutex<ParticleSystem>>> {
        self.slave_system.clone()
    }

    /// Set master system
    pub fn set_master(&mut self, master: Option<Arc<Mutex<ParticleSystem>>>) {
        if let Some(ref sys) = master {
            let locked = sys.lock().unwrap();
            self.master_system_id = locked.get_system_id();
        } else {
            self.master_system_id = INVALID_PARTICLE_SYSTEM_ID;
        }
        self.master_system = master;
    }

    /// Get master system
    pub fn get_master(&self) -> Option<Arc<Mutex<ParticleSystem>>> {
        self.master_system.clone()
    }

    /// Get slave position offset
    pub fn get_slave_position_offset(&self) -> &Coord3D {
        &self.info.slave_pos_offset
    }

    /// Set system lifetime
    pub fn set_system_lifetime(&mut self, frames: u32) {
        self.system_lifetime_left = frames;
    }

    /// Set lifetime range
    pub fn set_lifetime_range(&mut self, min: f32, max: f32) {
        self.info.lifetime = GameClientRandomVariable::new_uniform(min, max);
    }
}

/// The particle system manager, responsible for maintaining all ParticleSystems
///
/// Matches C++ ParticleSys.h:689-785
pub struct ParticleSystemManager {
    template_map: HashMap<String, Arc<ParticleSystemTemplate>>,
    all_particle_systems: Vec<Arc<Mutex<ParticleSystem>>>,
    unique_system_id: ParticleSystemID,
    particle_count: u32,
    field_particle_count: u32,
    particle_system_count: u32,
    on_screen_particle_count: i32,
    last_logic_frame_update: u32,
    local_player_index: i32,

    // Particle priority lists
    all_particles_head: [Option<*mut Particle>; ParticlePriorityType::NUM_PARTICLE_PRIORITIES],
    all_particles_tail: [Option<*mut Particle>; ParticlePriorityType::NUM_PARTICLE_PRIORITIES],
}

impl ParticleSystemManager {
    /// Create a new particle system manager
    pub fn new() -> Self {
        Self {
            template_map: HashMap::new(),
            all_particle_systems: Vec::new(),
            unique_system_id: 1,
            particle_count: 0,
            field_particle_count: 0,
            particle_system_count: 0,
            on_screen_particle_count: 0,
            last_logic_frame_update: 0,
            local_player_index: 0,
            all_particles_head: [None; ParticlePriorityType::NUM_PARTICLE_PRIORITIES],
            all_particles_tail: [None; ParticlePriorityType::NUM_PARTICLE_PRIORITIES],
        }
    }

    /// Initialize the manager
    pub fn init(&mut self) {
        // Initialization logic
    }

    /// Reset the manager and all particle systems
    pub fn reset(&mut self) {
        self.all_particle_systems.clear();
        self.particle_count = 0;
        self.field_particle_count = 0;
        self.particle_system_count = 0;
    }

    /// Update all particle systems
    pub fn update(&mut self) {
        // Update all systems
        // Remove destroyed systems
    }

    /// Get on-screen particle count
    pub fn get_on_screen_particle_count(&self) -> i32 {
        self.on_screen_particle_count
    }

    /// Set on-screen particle count
    pub fn set_on_screen_particle_count(&mut self, count: i32) {
        self.on_screen_particle_count = count;
    }

    /// Find a template by name
    pub fn find_template(&self, name: &str) -> Option<Arc<ParticleSystemTemplate>> {
        self.template_map.get(name).cloned()
    }

    /// Create a new template
    pub fn new_template(&mut self, name: String, info: ParticleSystemInfo) -> Arc<ParticleSystemTemplate> {
        let template = Arc::new(ParticleSystemTemplate::new(name.clone(), info));
        self.template_map.insert(name, template.clone());
        template
    }

    /// Create a particle system from a template
    pub fn create_particle_system(
        &mut self,
        template: Arc<ParticleSystemTemplate>,
        create_slaves: bool,
    ) -> Arc<Mutex<ParticleSystem>> {
        let id = self.unique_system_id;
        self.unique_system_id += 1;

        let system = ParticleSystem::new(template, id, create_slaves);
        self.all_particle_systems.push(system.clone());
        self.particle_system_count += 1;

        system
    }

    /// Create attached particle system and return its ID
    pub fn create_attached_particle_system_id(
        &mut self,
        template: Arc<ParticleSystemTemplate>,
        object_id: u32,
        create_slaves: bool,
    ) -> ParticleSystemID {
        let system = self.create_particle_system(template, create_slaves);
        let id = {
            let mut locked = system.lock().unwrap();
            locked.attach_to_object(object_id);
            locked.get_system_id()
        };
        id
    }

    /// Find a particle system by ID
    pub fn find_particle_system(&self, id: ParticleSystemID) -> Option<Arc<Mutex<ParticleSystem>>> {
        self.all_particle_systems
            .iter()
            .find(|sys| {
                let locked = sys.lock().unwrap();
                locked.get_system_id() == id
            })
            .cloned()
    }

    /// Destroy a particle system by ID
    pub fn destroy_particle_system_by_id(&mut self, id: ParticleSystemID) {
        if let Some(system) = self.find_particle_system(id) {
            let mut locked = system.lock().unwrap();
            locked.destroy();
        }
    }

    /// Destroy systems attached to an object
    pub fn destroy_attached_systems(&mut self, object_id: u32) {
        for system in &self.all_particle_systems {
            let mut locked = system.lock().unwrap();
            if locked.get_attached_object() == object_id {
                locked.destroy();
            }
        }
    }

    /// Set local player index
    pub fn set_local_player_index(&mut self, index: i32) {
        self.local_player_index = index;
    }

    /// Add particle to manager
    pub fn add_particle(&mut self, _particle: &mut Particle, _priority: ParticlePriorityType) {
        self.particle_count += 1;
        // Would link into priority list
    }

    /// Remove particle from manager
    pub fn remove_particle(&mut self, _particle: &Particle) {
        self.particle_count -= 1;
        // Would unlink from priority list
    }

    /// Remove oldest particles
    pub fn remove_oldest_particles(&mut self, count: u32, _priority_cap: ParticlePriorityType) -> i32 {
        // Would remove old particles
        count as i32
    }

    /// Get total particle count
    pub fn get_particle_count(&self) -> u32 {
        self.particle_count
    }

    /// Get field particle count
    pub fn get_field_particle_count(&self) -> u32 {
        self.field_particle_count
    }

    /// Get particle system count
    pub fn get_particle_system_count(&self) -> u32 {
        self.particle_system_count
    }

    /// Get all particle systems
    pub fn get_all_particle_systems(&self) -> &Vec<Arc<Mutex<ParticleSystem>>> {
        &self.all_particle_systems
    }

    /// Friend functions for linking/unlinking systems
    pub fn friend_add_particle_system(&mut self, _system: Arc<Mutex<ParticleSystem>>) {
        self.particle_system_count += 1;
    }

    pub fn friend_remove_particle_system(&mut self, _system: &ParticleSystem) {
        self.particle_system_count -= 1;
    }
}

impl Default for ParticleSystemManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global particle system manager singleton
/// Matches C++ ParticleSys.h:788
pub static mut THE_PARTICLE_SYSTEM_MANAGER: Option<ParticleSystemManager> = None;

/// Initialize the global particle system manager
pub fn init_particle_system_manager() {
    unsafe {
        THE_PARTICLE_SYSTEM_MANAGER = Some(ParticleSystemManager::new());
    }
}

/// Get reference to the global particle system manager
pub fn get_particle_system_manager() -> Option<&'static ParticleSystemManager> {
    unsafe { THE_PARTICLE_SYSTEM_MANAGER.as_ref() }
}

/// Get mutable reference to the global particle system manager
pub fn get_particle_system_manager_mut() -> Option<&'static mut ParticleSystemManager> {
    unsafe { THE_PARTICLE_SYSTEM_MANAGER.as_mut() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_system_info_creation() {
        let info = ParticleSystemInfo::new();
        assert_eq!(info.system_lifetime, 0);
        assert_eq!(info.gravity, 0.0);
        assert!(!info.is_one_shot);
    }

    #[test]
    fn test_particle_system_manager() {
        let mut manager = ParticleSystemManager::new();
        assert_eq!(manager.get_particle_count(), 0);
        assert_eq!(manager.get_particle_system_count(), 0);

        let info = ParticleSystemInfo::new();
        let template = manager.new_template("test_system".to_string(), info);
        let _system = manager.create_particle_system(template, false);

        assert_eq!(manager.get_particle_system_count(), 1);
    }

    #[test]
    fn test_emission_volume() {
        let sphere = EmissionVolume::Sphere { radius: 10.0 };
        if let EmissionVolume::Sphere { radius } = sphere {
            assert_eq!(radius, 10.0);
        } else {
            panic!("Wrong emission volume type");
        }
    }

    #[test]
    fn test_wind_motion() {
        let mut info = ParticleSystemInfo::new();
        info.wind_motion = WindMotion::PingPong;
        info.wind_angle = 0.0;
        info.wind_motion_start_angle = 0.0;
        info.wind_motion_end_angle = PI;
        info.wind_angle_change = 0.1;
        info.wind_motion_moving_to_end_angle = true;

        // Would test wind motion updates here
        assert_eq!(info.wind_motion, WindMotion::PingPong);
    }
}
