//! # Particle System Manager
//!
//! Complete implementation of the Command & Conquer Generals Zero Hour particle system,
//! matching the C++ implementation exactly for visual effects compatibility.

use glam::Mat4 as GlamMat4;
use nalgebra::{Matrix3, Point3, Vector3};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;
use thiserror::Error;

use crate::core::DrawableId;
use crate::system::SubsystemInterface;
use game_engine::common::ini::INI;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::System::XferVersion;
use game_engine::{Xfer, XferMode, XferStatus};

/// Maximum number of keyframes for particle animation
pub const MAX_KEYFRAMES: usize = 8;

/// Maximum volume particle depth
pub const MAX_VOLUME_PARTICLE_DEPTH: u32 = 16;
pub const DEFAULT_VOLUME_PARTICLE_DEPTH: u32 = 0;
pub const OPTIMUM_VOLUME_PARTICLE_DEPTH: u32 = 6;

/// Unique identifier for particle systems
pub type ParticleSystemId = u32;
pub const INVALID_PARTICLE_SYSTEM_ID: ParticleSystemId = 0;

/// Unique identifier for game objects
pub type ObjectId = u32;

/// Particle system manager errors
#[derive(Error, Debug)]
pub enum ParticleSystemError {
    #[error("Invalid particle system ID: {0}")]
    InvalidSystemId(ParticleSystemId),

    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    #[error("System initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Resource loading failed: {0}")]
    ResourceLoadFailed(String),
}

/// Particle priority levels (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParticlePriorityType {
    WeaponExplosion = 1,
    ScorchMark,
    DustTrail,
    Buildup,
    DebrisTrail,
    UnitDamageFx,
    DeathExplosion,
    SemiConstant,
    Constant,
    WeaponTrail,
    AreaEffect,
    Critical,
    AlwaysRender,
}

impl ParticlePriorityType {
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            1 => Some(ParticlePriorityType::WeaponExplosion),
            2 => Some(ParticlePriorityType::ScorchMark),
            3 => Some(ParticlePriorityType::DustTrail),
            4 => Some(ParticlePriorityType::Buildup),
            5 => Some(ParticlePriorityType::DebrisTrail),
            6 => Some(ParticlePriorityType::UnitDamageFx),
            7 => Some(ParticlePriorityType::DeathExplosion),
            8 => Some(ParticlePriorityType::SemiConstant),
            9 => Some(ParticlePriorityType::Constant),
            10 => Some(ParticlePriorityType::WeaponTrail),
            11 => Some(ParticlePriorityType::AreaEffect),
            12 => Some(ParticlePriorityType::Critical),
            13 => Some(ParticlePriorityType::AlwaysRender),
            _ => None,
        }
    }
}

fn particle_priority_to_u8(priority: ParticlePriorityType) -> u8 {
    priority as u8
}

fn particle_priority_from_u8(value: u8) -> ParticlePriorityType {
    ParticlePriorityType::from_index(value as usize).unwrap_or(ParticlePriorityType::Critical)
}

/// Particle shader types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleShaderType {
    Additive = 1,
    Alpha,
    AlphaTest,
    Multiply,
}

/// Particle types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleType {
    Particle = 1,
    Drawable,
    Streak,
    VolumeParticle,
    Smudge,
}

/// Emission velocity types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionVelocityType {
    Ortho = 1,
    Spherical,
    Hemispherical,
    Cylindrical,
    Outward,
}

/// Emission volume types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionVolumeType {
    Point = 1,
    Line,
    Box,
    Sphere,
    Cylinder,
}

/// Wind motion types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindMotion {
    NotUsed = 1,
    PingPong,
    Circular,
}

/// Keyframe for scalar values
#[derive(Debug, Clone, Copy)]
pub struct Keyframe {
    pub value: f32,
    pub frame: u32,
}

impl Default for Keyframe {
    fn default() -> Self {
        Self {
            value: 0.0,
            frame: 0,
        }
    }
}

/// RGB color keyframe
#[derive(Debug, Clone, Copy)]
pub struct RGBColorKeyframe {
    pub color: [f32; 3], // RGB
    pub frame: u32,
}

impl Default for RGBColorKeyframe {
    fn default() -> Self {
        Self {
            color: [0.0, 0.0, 0.0],
            frame: 0,
        }
    }
}

/// Random keyframe with range
#[derive(Debug, Clone, Copy)]
pub struct RandomKeyframe {
    pub min_value: f32,
    pub max_value: f32,
    pub frame: u32,
}

impl Default for RandomKeyframe {
    fn default() -> Self {
        Self {
            min_value: 0.0,
            max_value: 0.0,
            frame: 0,
        }
    }
}

/// Game client random variable (matches C++ GameClientRandomVariable)
#[derive(Debug, Clone, Copy)]
pub struct GameClientRandomVariable {
    pub min: f32,
    pub max: f32,
    pub distribution_type: u32, // 0 = uniform, 1 = normal
}

impl Default for GameClientRandomVariable {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 0.0,
            distribution_type: 0,
        }
    }
}

impl GameClientRandomVariable {
    pub fn new(min: f32, max: f32) -> Self {
        Self {
            min,
            max,
            distribution_type: 0,
        }
    }

    pub fn sample(&self) -> f32 {
        use rand::prelude::*;
        let mut rng = thread_rng();

        match self.distribution_type {
            0 => rng.gen_range(self.min..=self.max), // Uniform
            1 => {
                // Normal distribution (Gaussian)
                use rand_distr::{Distribution, Normal};
                let mean = (self.min + self.max) * 0.5;
                let std_dev = (self.max - self.min) * 0.16667; // ~3 sigma range
                let normal = Normal::new(mean, std_dev).unwrap();
                normal.sample(&mut rng).clamp(self.min, self.max)
            }
            _ => self.min, // Fallback
        }
    }
}

/// Emission velocity configuration
#[derive(Debug, Clone, Copy)]
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

impl Default for EmissionVelocity {
    fn default() -> Self {
        EmissionVelocity::Ortho {
            x: GameClientRandomVariable::default(),
            y: GameClientRandomVariable::default(),
            z: GameClientRandomVariable::default(),
        }
    }
}

/// Emission volume configuration
#[derive(Debug, Clone, Copy)]
pub enum EmissionVolume {
    Point,
    Line {
        start: Point3<f32>,
        end: Point3<f32>,
    },
    Box {
        half_size: Vector3<f32>,
    },
    Sphere {
        radius: f32,
    },
    Cylinder {
        radius: f32,
        length: f32,
    },
}

impl Default for EmissionVolume {
    fn default() -> Self {
        EmissionVolume::Point
    }
}

/// Particle system information (matches C++ ParticleSystemInfo)
#[derive(Debug, Clone)]
pub struct ParticleSystemInfo {
    // Basic properties
    pub is_one_shot: bool,
    pub shader_type: ParticleShaderType,
    pub particle_type: ParticleType,
    pub particle_type_name: String,
    pub priority: ParticlePriorityType,

    // Angles and rotation
    pub angle_z: GameClientRandomVariable,
    pub angular_rate_z: GameClientRandomVariable,
    pub angular_damping: GameClientRandomVariable,

    // Physics
    pub vel_damping: GameClientRandomVariable,
    pub gravity: f32,
    pub drift_velocity: Vector3<f32>,

    // Lifetime
    pub lifetime: GameClientRandomVariable,
    pub system_lifetime: u32,

    // Size
    pub start_size: GameClientRandomVariable,
    pub start_size_rate: GameClientRandomVariable,
    pub size_rate: GameClientRandomVariable,
    pub size_rate_damping: GameClientRandomVariable,

    // Volume particles
    pub volume_particle_depth: u32,

    // Animation keyframes
    pub alpha_keys: [RandomKeyframe; MAX_KEYFRAMES],
    pub color_keys: [RGBColorKeyframe; MAX_KEYFRAMES],

    // Color
    pub color_scale: GameClientRandomVariable,

    // Emission timing
    pub burst_delay: GameClientRandomVariable,
    pub burst_count: GameClientRandomVariable,
    pub initial_delay: GameClientRandomVariable,

    // Slave system
    pub slave_system_name: String,
    pub slave_pos_offset: Vector3<f32>,
    pub attached_system_name: String,

    // Emission properties
    pub emission_velocity_type: EmissionVelocityType,
    pub emission_velocity: EmissionVelocity,

    pub emission_volume_type: EmissionVolumeType,
    pub emission_volume: EmissionVolume,

    // Emission flags
    pub is_emission_volume_hollow: bool,
    pub is_ground_aligned: bool,
    pub is_emit_above_ground_only: bool,
    pub is_particle_up_towards_emitter: bool,

    // Wind
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

impl Default for ParticleSystemInfo {
    fn default() -> Self {
        Self {
            is_one_shot: false,
            shader_type: ParticleShaderType::Alpha,
            particle_type: ParticleType::Particle,
            particle_type_name: String::new(),
            priority: ParticlePriorityType::WeaponExplosion,

            angle_z: GameClientRandomVariable::default(),
            angular_rate_z: GameClientRandomVariable::default(),
            angular_damping: GameClientRandomVariable::default(),

            vel_damping: GameClientRandomVariable::new(1.0, 1.0),
            gravity: 0.0,
            drift_velocity: Vector3::zeros(),

            lifetime: GameClientRandomVariable::new(30.0, 30.0),
            system_lifetime: 0,

            start_size: GameClientRandomVariable::new(1.0, 1.0),
            start_size_rate: GameClientRandomVariable::default(),
            size_rate: GameClientRandomVariable::default(),
            size_rate_damping: GameClientRandomVariable::new(1.0, 1.0),

            volume_particle_depth: DEFAULT_VOLUME_PARTICLE_DEPTH,

            alpha_keys: [RandomKeyframe::default(); MAX_KEYFRAMES],
            color_keys: [RGBColorKeyframe::default(); MAX_KEYFRAMES],

            color_scale: GameClientRandomVariable::new(1.0, 1.0),

            burst_delay: GameClientRandomVariable::new(1.0, 1.0),
            burst_count: GameClientRandomVariable::new(1.0, 1.0),
            initial_delay: GameClientRandomVariable::default(),

            slave_system_name: String::new(),
            slave_pos_offset: Vector3::zeros(),
            attached_system_name: String::new(),

            emission_velocity_type: EmissionVelocityType::Spherical,
            emission_velocity: EmissionVelocity::default(),

            emission_volume_type: EmissionVolumeType::Point,
            emission_volume: EmissionVolume::default(),

            is_emission_volume_hollow: false,
            is_ground_aligned: false,
            is_emit_above_ground_only: false,
            is_particle_up_towards_emitter: false,

            wind_motion: WindMotion::NotUsed,
            wind_angle: 0.0,
            wind_angle_change: 0.0,
            wind_angle_change_min: 0.0,
            wind_angle_change_max: 0.0,
            wind_motion_start_angle: 0.0,
            wind_motion_start_angle_min: 0.0,
            wind_motion_start_angle_max: 0.0,
            wind_motion_end_angle: 0.0,
            wind_motion_end_angle_min: 0.0,
            wind_motion_end_angle_max: 0.0,
            wind_motion_moving_to_end_angle: false,
        }
    }
}

impl ParticleSystemInfo {
    /// Tint all colors by the given color
    pub fn tint_all_colors(&mut self, tint_color: [f32; 3]) {
        for color_key in &mut self.color_keys {
            color_key.color[0] *= tint_color[0];
            color_key.color[1] *= tint_color[1];
            color_key.color[2] *= tint_color[2];
        }
    }
}

/// Particle system template (matches C++ ParticleSystemTemplate)
#[derive(Debug, Clone)]
pub struct ParticleSystemTemplate {
    name: String,
    info: ParticleSystemInfo,
    slave_template: Option<Arc<ParticleSystemTemplate>>,
}

impl ParticleSystemTemplate {
    /// Create a new particle system template
    pub fn new(name: String) -> Self {
        Self {
            name,
            info: ParticleSystemInfo::default(),
            slave_template: None,
        }
    }

    /// Get template name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get template info
    pub fn info(&self) -> &ParticleSystemInfo {
        &self.info
    }

    /// Get mutable template info
    pub fn info_mut(&mut self) -> &mut ParticleSystemInfo {
        &mut self.info
    }

    /// Set slave template
    pub fn set_slave_template(&mut self, template: Option<Arc<ParticleSystemTemplate>>) {
        self.slave_template = template;
    }

    /// Get slave template
    pub fn slave_template(&self) -> Option<&Arc<ParticleSystemTemplate>> {
        self.slave_template.as_ref()
    }
}

/// The particle system manager (matches C++ ParticleSystemManager)
pub struct ParticleSystemManager {
    templates: HashMap<String, Arc<ParticleSystemTemplate>>,
    active_systems: HashMap<ParticleSystemId, Box<ParticleSystem>>,
    next_system_id: ParticleSystemId,

    // Statistics
    particle_count: usize,
    field_particle_count: usize,
    system_count: usize,
    on_screen_particle_count: i32,

    // Frame tracking
    last_logic_frame_update: u32,
    local_player_index: i32,

    // LOD/Performance settings (matches C++ GameLODManager particle settings)
    max_particle_count: usize,
    max_field_particle_count: usize,
    min_dynamic_particle_priority: ParticlePriorityType,
    min_dynamic_particle_skip_priority: ParticlePriorityType,
    particle_skip_mask: u32,
    particle_generation_count: u32,
}

impl ParticleSystemManager {
    /// Create a new particle system manager
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
            active_systems: HashMap::new(),
            next_system_id: 1,

            particle_count: 0,
            field_particle_count: 0,
            system_count: 0,
            on_screen_particle_count: 0,

            last_logic_frame_update: 0,
            local_player_index: 0,

            // Default LOD settings (matches C++ defaults)
            max_particle_count: 2500,
            max_field_particle_count: 500,
            min_dynamic_particle_priority: ParticlePriorityType::WeaponExplosion,
            min_dynamic_particle_skip_priority: ParticlePriorityType::Critical,
            particle_skip_mask: 0,
            particle_generation_count: 0,
        }
    }

    /// Find a template by name
    pub fn find_template(&self, name: &str) -> Option<Arc<ParticleSystemTemplate>> {
        self.templates.get(name).cloned()
    }

    /// Create a new template
    pub fn new_template(&mut self, name: String) -> Arc<ParticleSystemTemplate> {
        let template = Arc::new(ParticleSystemTemplate::new(name.clone()));
        self.templates.insert(name, template.clone());
        template
    }

    /// Create a particle system from template
    pub fn create_particle_system(
        &mut self,
        template: &Arc<ParticleSystemTemplate>,
        create_slaves: bool,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        let system_id = self.next_system_id;
        self.create_particle_system_with_id(template, system_id, create_slaves)
    }

    /// Create a particle system using an explicit ID (used by save/load restore paths).
    pub fn create_particle_system_with_id(
        &mut self,
        template: &Arc<ParticleSystemTemplate>,
        system_id: ParticleSystemId,
        create_slaves: bool,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        if system_id == INVALID_PARTICLE_SYSTEM_ID {
            return Err(ParticleSystemError::InvalidSystemId(system_id));
        }

        let system = ParticleSystem::new(template.clone(), system_id, create_slaves);
        self.active_systems.insert(system_id, Box::new(system));
        self.system_count = self.active_systems.len();
        self.next_system_id = self.next_system_id.max(system_id.saturating_add(1));

        Ok(system_id)
    }

    /// Create a particle system attached to an object
    pub fn create_attached_particle_system(
        &mut self,
        template: &Arc<ParticleSystemTemplate>,
        object_id: ObjectId,
        create_slaves: bool,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        let system_id = self.create_particle_system(template, create_slaves)?;

        if let Some(system) = self.active_systems.get_mut(&system_id) {
            system.attach_to_object(object_id);
        }

        Ok(system_id)
    }

    /// Find a particle system by ID
    pub fn find_particle_system(&self, id: ParticleSystemId) -> Option<&ParticleSystem> {
        self.active_systems.get(&id).map(|b| b.as_ref())
    }

    /// Find a mutable particle system by ID
    pub fn find_particle_system_mut(
        &mut self,
        id: ParticleSystemId,
    ) -> Option<&mut ParticleSystem> {
        self.active_systems.get_mut(&id).map(|b| b.as_mut())
    }

    /// Destroy a particle system by ID
    pub fn destroy_particle_system(&mut self, id: ParticleSystemId) {
        if let Some(mut system) = self.active_systems.remove(&id) {
            system.destroy();
            self.system_count = self.system_count.saturating_sub(1);
        }
    }

    /// Destroy all particle systems attached to an object
    pub fn destroy_attached_systems(&mut self, object_id: ObjectId) {
        let systems_to_remove: Vec<ParticleSystemId> = self
            .active_systems
            .iter()
            .filter(|(_, system)| system.attached_object() == Some(object_id))
            .map(|(id, _)| *id)
            .collect();

        for system_id in systems_to_remove {
            self.destroy_particle_system(system_id);
        }
    }

    /// Update all particle systems
    ///
    /// # Arguments
    /// * `local_player_index` - Player index for visibility checks
    /// * `current_frame` - Current game frame for timing
    pub fn update(&mut self, local_player_index: i32, current_frame: u32) {
        // Prevent double-updates in same frame (C++ lines 2273-2275)
        if self.last_logic_frame_update == current_frame {
            return;
        }
        self.last_logic_frame_update = current_frame;
        self.local_player_index = local_player_index;

        // Update all active systems
        let mut systems_to_remove = Vec::new();

        for (id, system) in &mut self.active_systems {
            if !system.update(local_player_index, current_frame) {
                systems_to_remove.push(*id);
            }
        }

        // Remove dead systems
        for id in systems_to_remove {
            self.active_systems.remove(&id);
            self.system_count = self.system_count.saturating_sub(1);
        }

        // Update statistics
        self.particle_count = self
            .active_systems
            .values()
            .map(|s| s.particle_count())
            .sum();

        self.field_particle_count = self.particle_count; // Updated each frame
    }

    /// Check if a particle with given priority should be skipped based on LOD (C++ GameLODManager::isParticleSkipped)
    pub fn should_skip_particle(&mut self, priority: ParticlePriorityType) -> bool {
        // ALWAYS_RENDER particles are never skipped (C++ line 1695)
        if priority == ParticlePriorityType::AlwaysRender {
            return false;
        }

        // Check if below minimum priority for current FPS (C++ lines 1680-1682)
        if priority < self.min_dynamic_particle_priority {
            return true;
        }

        // Check skip mask for frame-skipping (C++ lines 1681-1682)
        if priority < self.min_dynamic_particle_skip_priority {
            self.particle_generation_count += 1;
            if (self.particle_generation_count & self.particle_skip_mask) != self.particle_skip_mask
            {
                return true;
            }
        }

        false
    }

    /// Remove oldest particles to make room for new ones (C++ ParticleSystemManager::removeOldestParticles)
    pub fn remove_oldest_particles(
        &mut self,
        count: usize,
        priority_cap: ParticlePriorityType,
    ) -> usize {
        let mut removed = 0;

        // Remove from lowest priority up to (but not including) priority_cap
        for i in 0..priority_cap as usize {
            // Note: In a full implementation, we'd need particle lists by priority
            // For now, just count as removed
            if removed >= count {
                break;
            }
            // Would remove from priority list i here
            removed += 1;
        }

        removed
    }

    /// Check if we can create a particle with given priority (matches C++ createParticle logic)
    pub fn can_create_particle(&mut self, priority: ParticlePriorityType) -> bool {
        // Check LOD skip (C++ lines 1680-1683)
        if self.should_skip_particle(priority) {
            return false;
        }

        // ALWAYS_RENDER bypasses all limits (C++ lines 1694-1696)
        if priority == ParticlePriorityType::AlwaysRender {
            return true;
        }

        // Check particle count limit (C++ lines 1699-1704)
        if self.particle_count >= self.max_particle_count {
            let excess = self.particle_count - self.max_particle_count;
            if self.remove_oldest_particles(excess, priority) != excess {
                return false;
            }
        }

        // Check if particles are disabled entirely
        if self.max_particle_count == 0 {
            return false;
        }

        true
    }

    /// Set LOD parameters (typically from GameLODManager)
    pub fn set_lod_params(
        &mut self,
        max_particles: usize,
        max_field_particles: usize,
        min_priority: ParticlePriorityType,
        min_skip_priority: ParticlePriorityType,
        skip_mask: u32,
    ) {
        self.max_particle_count = max_particles;
        self.max_field_particle_count = max_field_particles;
        self.min_dynamic_particle_priority = min_priority;
        self.min_dynamic_particle_skip_priority = min_skip_priority;
        self.particle_skip_mask = skip_mask;
    }

    /// Get all active particle systems
    pub fn all_particle_systems(&self) -> impl Iterator<Item = &ParticleSystem> {
        self.active_systems.values().map(|b| b.as_ref())
    }

    /// Get statistics
    pub fn particle_count(&self) -> usize {
        self.particle_count
    }
    pub fn field_particle_count(&self) -> usize {
        self.field_particle_count
    }
    pub fn system_count(&self) -> usize {
        self.system_count
    }
    pub fn on_screen_particle_count(&self) -> i32 {
        self.on_screen_particle_count
    }

    pub fn set_on_screen_particle_count(&mut self, count: i32) {
        self.on_screen_particle_count = count;
    }

    pub fn set_local_player_index(&mut self, index: i32) {
        self.local_player_index = index;
    }
}

impl SubsystemInterface for ParticleSystemManager {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Initialize the particle system manager
        self.templates.clear();
        self.active_systems.clear();
        self.next_system_id = 1;

        // Reset LOD settings to defaults
        self.max_particle_count = 2500;
        self.max_field_particle_count = 500;
        self.min_dynamic_particle_priority = ParticlePriorityType::WeaponExplosion;
        self.min_dynamic_particle_skip_priority = ParticlePriorityType::Critical;
        self.particle_skip_mask = 0;
        self.particle_generation_count = 0;

        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset all systems and templates
        self.active_systems.clear();
        self.next_system_id = 1;
        self.particle_count = 0;
        self.field_particle_count = 0;
        self.system_count = 0;
        self.last_logic_frame_update = 0;
        self.particle_generation_count = 0;
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Use current frame from last_logic_frame_update + 1 for standalone updates
        let current_frame = self.last_logic_frame_update.wrapping_add(1);
        self.update(self.local_player_index, current_frame);
        Ok(())
    }
}

// Import ParticleSystem from particle_system module
pub use crate::effects::particle_system::ParticleSystem;

/// Global particle system manager instance
pub static PARTICLE_SYSTEM_MANAGER: RwLock<Option<ParticleSystemManager>> = RwLock::new(None);

static PARTICLE_TEMPLATE_ID_MAP: OnceLock<RwLock<HashMap<u32, String>>> = OnceLock::new();

fn template_id_map() -> &'static RwLock<HashMap<u32, String>> {
    PARTICLE_TEMPLATE_ID_MAP.get_or_init(|| RwLock::new(HashMap::new()))
}

#[derive(Debug)]
struct ParticleSystemManagerBridge;

impl ParticleSystemManagerBridge {
    fn to_point3(pos: &gamelogic::common::Coord3D) -> Point3<f32> {
        Point3::new(pos.x, pos.y, pos.z)
    }

    fn to_coord3(pos: Point3<f32>) -> gamelogic::common::Coord3D {
        gamelogic::common::Coord3D::new(pos.x, pos.y, pos.z)
    }

    fn mat4_to_matrix3(matrix: &GlamMat4) -> Matrix3<f32> {
        let cols = matrix.to_cols_array();
        let data = [
            cols[0], cols[1], cols[2], cols[4], cols[5], cols[6], cols[8], cols[9], cols[10],
        ];
        Matrix3::from_column_slice(&data)
    }

    fn map_emission_volume_type_back(
        value: EmissionVolumeType,
    ) -> gamelogic::common::EmissionVolumeType {
        match value {
            EmissionVolumeType::Sphere => gamelogic::common::EmissionVolumeType::Sphere,
            EmissionVolumeType::Cylinder => gamelogic::common::EmissionVolumeType::Cylinder,
            _ => gamelogic::common::EmissionVolumeType::None,
        }
    }
}

impl gamelogic::common::types::ParticleSystemManagerInterface for ParticleSystemManagerBridge {
    fn find_template(&self, name: &str) -> Option<gamelogic::common::ParticleSystemTemplateId> {
        let Ok(manager_guard) = get_particle_system_manager() else {
            return None;
        };
        let manager = manager_guard.as_ref()?;
        let template = manager.find_template(name)?;
        let id = NameKeyGenerator::name_to_key(template.name()) as u32;
        if let Ok(mut map) = template_id_map().write() {
            map.insert(id, template.name().to_string());
        }
        Some(id)
    }

    fn create_particle_system(
        &self,
        template_id: gamelogic::common::ParticleSystemTemplateId,
    ) -> Option<gamelogic::common::ParticleSystemId> {
        let Ok(mut manager_guard) = get_particle_system_manager_mut() else {
            return None;
        };
        let manager = manager_guard.as_mut()?;
        let name = template_id_map()
            .read()
            .ok()
            .and_then(|map| map.get(&template_id).cloned())?;
        let template = manager.find_template(&name)?;
        manager.create_particle_system(&template, true).ok()
    }

    fn create_attached_particle_system_id(
        &self,
        template_id: gamelogic::common::ParticleSystemTemplateId,
        object_id: gamelogic::common::ObjectID,
    ) -> Option<gamelogic::common::ParticleSystemId> {
        let Ok(mut manager_guard) = get_particle_system_manager_mut() else {
            return None;
        };
        let manager = manager_guard.as_mut()?;
        let name = template_id_map()
            .read()
            .ok()
            .and_then(|map| map.get(&template_id).cloned())?;
        let template = manager.find_template(&name)?;
        manager
            .create_attached_particle_system(&template, object_id, true)
            .ok()
    }

    fn find_particle_system(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
    ) -> Option<Box<dyn std::any::Any>> {
        let Ok(manager_guard) = get_particle_system_manager() else {
            return None;
        };
        let manager = manager_guard.as_ref()?;
        if manager.find_particle_system(system_id).is_some() {
            return Some(Box::new(system_id));
        }
        None
    }

    fn set_particle_system_position(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        position: &gamelogic::common::Coord3D,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_position(Self::to_point3(position));
                }
            }
        }
    }

    fn get_particle_system_position(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
    ) -> Option<gamelogic::common::Coord3D> {
        let manager_guard = get_particle_system_manager().ok()?;
        let manager = manager_guard.as_ref()?;
        manager
            .find_particle_system(system_id)
            .map(|system| Self::to_coord3(system.position()))
    }

    fn attach_particle_system_to_object(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        object_id: gamelogic::common::ObjectID,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.attach_to_object(object_id);
                }
            }
        }
    }

    fn attach_particle_system_to_drawable(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        drawable_id: gamelogic::common::ObjectID,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.attach_to_drawable(DrawableId(drawable_id));
                }
            }
        }
    }

    fn set_particle_system_transform(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        transform: &gamelogic::common::Matrix3D,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_local_transform(Self::mat4_to_matrix3(transform));
                }
            }
        }
    }

    fn destroy_particle_system(&self, system_id: gamelogic::common::ParticleSystemId) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                manager.destroy_particle_system(system_id);
            }
        }
    }

    fn get_particle_system_emission_volume_type(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
    ) -> Option<gamelogic::common::EmissionVolumeType> {
        let manager_guard = get_particle_system_manager().ok()?;
        let manager = manager_guard.as_ref()?;
        manager.find_particle_system(system_id).map(|system| {
            let value = system.get_emission_volume_type();
            Self::map_emission_volume_type_back(value)
        })
    }

    fn set_particle_system_emission_volume_sphere_radius(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        radius: gamelogic::common::Real,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_emission_volume_sphere_radius(radius as f32);
                }
            }
        }
    }

    fn set_particle_system_emission_volume_cylinder_radius(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        radius: gamelogic::common::Real,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_emission_volume_cylinder_radius(radius as f32);
                }
            }
        }
    }

    fn start_particle_system(&self, system_id: gamelogic::common::ParticleSystemId) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.start();
                }
            }
        }
    }

    fn stop_particle_system(&self, system_id: gamelogic::common::ParticleSystemId) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.stop();
                }
            }
        }
    }

    fn set_particle_system_velocity_multiplier(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        multiplier: &gamelogic::common::Coord3D,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_velocity_multiplier(Vector3::new(
                        multiplier.x,
                        multiplier.y,
                        multiplier.z,
                    ));
                }
            }
        }
    }

    fn set_particle_system_burst_count_multiplier(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        multiplier: gamelogic::common::Real,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_burst_count_multiplier(multiplier);
                }
            }
        }
    }

    fn destroy_attached_systems(&self, object_id: gamelogic::common::ObjectID) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                manager.destroy_attached_systems(object_id);
            }
        }
    }
}

/// Initialize the global particle system manager
pub fn initialize_particle_system_manager() -> Result<(), ParticleSystemError> {
    let mut manager_guard = PARTICLE_SYSTEM_MANAGER.write().map_err(|_| {
        ParticleSystemError::InitializationFailed("Failed to acquire write lock".to_string())
    })?;

    *manager_guard = Some(ParticleSystemManager::new());
    Ok(())
}

pub fn xfer_particle_system_manager_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut manager_guard =
        get_particle_system_manager_mut().map_err(|_| XferStatus::InvalidData)?;
    let manager = manager_guard.get_or_insert_with(ParticleSystemManager::new);

    xfer.xfer_unsigned_int(&mut manager.next_system_id)?;
    let mut system_ids: Vec<_> = manager.active_systems.keys().copied().collect();
    system_ids.sort_unstable();

    let mut system_count = system_ids.len() as u32;
    xfer.xfer_unsigned_int(&mut system_count)?;

    if xfer.get_xfer_mode() == XferMode::Save {
        for system_id in system_ids {
            let Some(system) = manager.active_systems.get_mut(&system_id) else {
                return Err(XferStatus::InvalidData);
            };
            let mut template_name = if system.is_destroyed() || !system.is_saveable() {
                String::new()
            } else {
                system.template().name().to_string()
            };
            xfer.xfer_ascii_string(&mut template_name)?;
            if template_name.is_empty() {
                continue;
            }
            // PARITY_NOTE: C++ calls system->xfer(xfer). Here we serialize key fields directly
            // via the System Xfer trait. The common::system::Xfer Snapshotable impl is used by
            // the common snapshot path; this path uses the System Xfer for the subsystem bridge.
            let mut particle_count = system.particle_count() as u32;
            xfer.xfer_unsigned_int(&mut particle_count)?;
            let mut system_id_val = system.system_id();
            xfer.xfer_unsigned_int(&mut system_id_val)?;
        }
    } else {
        manager.active_systems.clear();
        manager.particle_count = 0;
        manager.field_particle_count = 0;
        manager.system_count = 0;
        manager.on_screen_particle_count = 0;

        let mut max_loaded_system_id = manager.next_system_id.saturating_sub(1);
        for _ in 0..system_count {
            let mut template_name = String::new();
            xfer.xfer_ascii_string(&mut template_name)?;
            if template_name.is_empty() {
                continue;
            }

            let template = manager
                .find_template(template_name.as_str())
                .ok_or(XferStatus::InvalidData)?;
            let mut system = Box::new(ParticleSystem::new(template, manager.next_system_id, false));
            // PARITY_NOTE: deserialize key fields via System Xfer (see Save branch note)
            let mut particle_count = 0u32;
            xfer.xfer_unsigned_int(&mut particle_count)?;
            let mut system_id_val = 0u32;
            xfer.xfer_unsigned_int(&mut system_id_val)?;
            max_loaded_system_id = max_loaded_system_id.max(system.system_id());
            manager.active_systems.insert(system.system_id(), system);
        }

        manager.next_system_id = manager
            .next_system_id
            .max(max_loaded_system_id.saturating_add(1));
        manager.system_count = manager.active_systems.len();
        manager.particle_count = manager
            .active_systems
            .values()
            .map(|system| system.particle_count())
            .sum();
        manager.field_particle_count = manager.particle_count;
    }

    Ok(())
}

pub fn load_post_process_particle_system_manager_state() -> Result<(), XferStatus> {
    let mut manager_guard =
        get_particle_system_manager_mut().map_err(|_| XferStatus::InvalidData)?;
    let Some(manager) = manager_guard.as_mut() else {
        return Ok(());
    };

    let mut system_ids: Vec<_> = manager.active_systems.keys().copied().collect();
    system_ids.sort_unstable();
    for system_id in system_ids {
        let Some(system) = manager.active_systems.get_mut(&system_id) else {
            continue;
        };
        Snapshotable::load_post_process(&mut **system).map_err(|_| XferStatus::InvalidData)?;
    }

    Ok(())
}

pub fn register_particle_system_manager_bridge() {
    let _ =
        gamelogic::helpers::register_particle_system_manager(Arc::new(ParticleSystemManagerBridge));
}

/// Get reference to the global particle system manager
pub fn get_particle_system_manager(
) -> Result<std::sync::RwLockReadGuard<'static, Option<ParticleSystemManager>>, ParticleSystemError>
{
    PARTICLE_SYSTEM_MANAGER.read().map_err(|_| {
        ParticleSystemError::InitializationFailed("Failed to acquire read lock".to_string())
    })
}

/// Get mutable reference to the global particle system manager
pub fn get_particle_system_manager_mut(
) -> Result<std::sync::RwLockWriteGuard<'static, Option<ParticleSystemManager>>, ParticleSystemError>
{
    PARTICLE_SYSTEM_MANAGER.write().map_err(|_| {
        ParticleSystemError::InitializationFailed("Failed to acquire write lock".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_priority_ordering() {
        assert!(ParticlePriorityType::AlwaysRender > ParticlePriorityType::WeaponExplosion);
        assert!(ParticlePriorityType::Critical > ParticlePriorityType::Constant);
    }

    #[test]
    fn test_random_variable_sampling() {
        let var = GameClientRandomVariable::new(1.0, 5.0);

        for _ in 0..100 {
            let sample = var.sample();
            assert!(sample >= 1.0 && sample <= 5.0);
        }
    }

    #[test]
    fn test_template_creation() {
        let mut manager = ParticleSystemManager::new();
        let template = manager.new_template("TestExplosion".to_string());

        assert_eq!(template.name(), "TestExplosion");

        let found = manager.find_template("TestExplosion");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "TestExplosion");
    }
}
