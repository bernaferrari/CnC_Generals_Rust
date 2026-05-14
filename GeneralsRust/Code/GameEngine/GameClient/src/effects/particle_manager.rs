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
    pub distribution_type: u32,
    pub frame: u32,
}

impl Default for RandomKeyframe {
    fn default() -> Self {
        Self {
            min_value: 0.0,
            max_value: 0.0,
            distribution_type: 0,
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

    /// Get slave system name (matches C++ ParticleSystemTemplate::m_slaveSystemName)
    pub fn slave_system_name(&self) -> &str {
        &self.info.slave_system_name
    }

    /// Create a slave particle system from this template's slave template.
    /// Returns None if no slave system name is configured.
    /// (matches C++ ParticleSystemTemplate::createSlaveSystem)
    pub fn create_slave_system(
        &mut self,
        manager: &mut ParticleSystemManager,
        create_slaves: bool,
    ) -> Option<ParticleSystemId> {
        // Resolve slave template from name if not cached (C++ line 2785-2786)
        if self.slave_template.is_none() && !self.info.slave_system_name.is_empty() {
            self.slave_template = manager.find_template(&self.info.slave_system_name);
        }

        if let Some(ref slave_tmpl) = self.slave_template {
            manager
                .create_particle_system(slave_tmpl, create_slaves)
                .ok()
        } else {
            None
        }
    }
}

/// The particle system manager (matches C++ ParticleSystemManager)
pub struct ParticleSystemManager {
    pub(crate) templates: HashMap<String, Arc<ParticleSystemTemplate>>,
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
    preloaded_texture_assets: Vec<String>,
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
            preloaded_texture_assets: Vec::new(),
        }
    }

    /// Find a template by name
    pub fn find_template(&self, name: &str) -> Option<Arc<ParticleSystemTemplate>> {
        self.templates.get(name).cloned()
    }

    /// Find a particle system template's parent by slave system name.
    /// Searches templates for one whose slave_system_name matches `name`.
    /// `parent_num` selects the Nth match (0-indexed).
    /// (matches C++ ParticleSystemManager::findParentTemplate, ParticleSys.cpp:3040)
    pub fn find_parent_template(
        &self,
        name: &str,
        mut parent_num: i32,
    ) -> Option<Arc<ParticleSystemTemplate>> {
        if name.is_empty() {
            return None;
        }

        for sys_template in self.templates.values() {
            if sys_template.info().slave_system_name == name {
                if parent_num == 0 {
                    return Some(sys_template.clone());
                }
                parent_num -= 1;
            }
        }

        None
    }

    /// Preload particle texture assets for all templates.
    /// (matches C++ ParticleSystemManager::preloadAssets, ParticleSys.cpp:3204)
    pub fn preload_assets(&mut self) {
        self.preloaded_texture_assets.clear();

        for tmplate in self.templates.values() {
            let info = tmplate.info();
            if info.particle_type != ParticleType::Particle || info.particle_type_name.is_empty() {
                continue;
            }
            self.preloaded_texture_assets
                .push(info.particle_type_name.clone());
        }
    }

    /// Texture asset names requested by the last preload pass.
    pub fn preloaded_texture_assets(&self) -> &[String] {
        &self.preloaded_texture_assets
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

    /// Create a particle system by template name at a given world position.
    ///
    /// Convenience wrapper that looks up the template by name, creates the
    /// system, and sets its initial position.
    pub fn create_particle_system_at(
        &mut self,
        template_name: &str,
        pos: Point3<f32>,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        let template = self
            .find_template(template_name)
            .ok_or_else(|| ParticleSystemError::TemplateNotFound(template_name.to_string()))?;
        let id = self.create_particle_system(&template, true)?;
        if let Some(system) = self.active_systems.get_mut(&id) {
            system.set_position(pos);
        }
        Ok(id)
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

    /// Destroy a particle system by ID.
    /// Cascades destruction to any slave system (C++ ParticleSystem::destroy line 1258-1261).
    pub fn destroy_particle_system(&mut self, id: ParticleSystemId) {
        let slave_id = self
            .active_systems
            .get(&id)
            .and_then(|s| s.slave_system_id());

        if let Some(mut system) = self.active_systems.remove(&id) {
            system.destroy();
            self.system_count = self.system_count.saturating_sub(1);
        }

        // Cascade to slave (C++ line 1258-1260: m_slaveSystem->destroy())
        if let Some(slave_id) = slave_id {
            self.destroy_particle_system(slave_id);
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

        // Process slave particle emissions (C++ ParticleSys.cpp lines 2004-2009)
        let slave_work: Vec<(ParticleSystemId, ParticleSystemId, u32)> = self
            .active_systems
            .iter_mut()
            .filter_map(|(master_id, system)| {
                let count = system.drain_slave_emission_count();
                if count == 0 {
                    return None;
                }
                system
                    .slave_system_id()
                    .map(|slave_id| (*master_id, slave_id, count))
            })
            .collect();

        for (master_id, slave_id, count) in slave_work {
            let merged_infos: Vec<crate::effects::particle_system::ParticleInfo> = {
                let master = match self.active_systems.get(&master_id) {
                    Some(m) => m.as_ref(),
                    None => continue,
                };
                let slave = match self.active_systems.get(&slave_id) {
                    Some(s) => s.as_ref(),
                    None => continue,
                };
                (0..count)
                    .map(|_| {
                        crate::effects::particle_system::merge_related_particle_systems(
                            master, slave, false,
                        )
                    })
                    .collect()
            };

            if let Some(slave_system) = self.active_systems.get_mut(&slave_id) {
                for info in merged_infos {
                    let particle = crate::effects::particle_system::Particle::new(
                        &info,
                        slave_system.personality_counter(),
                        current_frame,
                    );
                    slave_system.push_particle(particle);
                }
            }
        }

        for id in systems_to_remove {
            if let Some(system) = self.active_systems.get(&id) {
                let slave_id = system.slave_system_id();
                self.active_systems.remove(&id);
                self.system_count = self.system_count.saturating_sub(1);
                if let Some(slave_id) = slave_id {
                    self.destroy_particle_system(slave_id);
                }
            }
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

    /// Remove oldest particles to make room for new ones (C++ ParticleSys.cpp lines 3177-3199)
    /// Iterates from lowest priority to priority_cap, removing the oldest particle from each qualifying system.
    pub fn remove_oldest_particles(
        &mut self,
        count: usize,
        priority_cap: ParticlePriorityType,
    ) -> usize {
        let mut removed = 0;

<<<<<<< Updated upstream
        // Remove from lowest priority up to (but not including) priority_cap
        for priority_index in 1..priority_cap as usize {
            if removed >= count {
                break;
            }
            let Some(priority) = ParticlePriorityType::from_index(priority_index) else {
                continue;
            };
            let mut system_ids = self
                .active_systems
                .iter()
                .filter_map(|(id, system)| (system.priority() == priority).then_some(*id))
                .collect::<Vec<_>>();
            system_ids.sort_unstable();

            for system_id in system_ids {
                if removed >= count {
                    break;
                }
                let Some(system) = self.active_systems.get_mut(&system_id) else {
                    continue;
                };
                let to_remove = count - removed;
                removed += system.remove_oldest_particles(to_remove);
            }
        }

        self.particle_count = self.particle_count.saturating_sub(removed);
        self.field_particle_count = self.field_particle_count.saturating_sub(removed);
=======
        while removed < count {
            let mut did_remove = false;

            for system in self.active_systems.values_mut() {
                if system.template().info().priority < priority_cap && system.particle_count() > 0 {
                    if system.remove_oldest_particle() {
                        removed += 1;
                        did_remove = true;
                        if removed >= count {
                            break;
                        }
                    }
                }
            }

            if !did_remove {
                break;
            }
        }

        self.particle_count = self
            .active_systems
            .values()
            .map(|s| s.particle_count())
            .sum();

>>>>>>> Stashed changes
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

        // Check if particles are disabled entirely
        if self.max_particle_count == 0 {
            return false;
        }

        // Check particle count limit (C++ lines 1699-1704)
        if self.particle_count >= self.max_particle_count {
            let needed = self.particle_count - self.max_particle_count + 1;
            if self.remove_oldest_particles(needed, priority) != needed {
                return false;
            }
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

    // -----------------------------------------------------------------------
    // Convenience wrappers matching C++ API naming conventions
    // -----------------------------------------------------------------------

    /// Update all active particle systems for the given frame.
    ///
    /// Thin wrapper around [`Self::update`] that uses the stored local player
    /// index.  Matches the C++ `ParticleSystemManager::update(frame)` pattern.
    pub fn update_particle_systems(&mut self, frame: u32) {
        self.update(self.local_player_index, frame);
    }

    /// Collect references to all active particle systems for rendering.
    ///
    /// Callers pass the collected slice to
    /// [`ParticleRenderer::render_particles`].  Matches the C++ draw-path
    /// where the manager hands its system list to the renderer.
    pub fn draw_particle_systems(&self) -> Vec<&ParticleSystem> {
        self.active_systems.values().map(|b| b.as_ref()).collect()
    }

    /// Total number of living particles across all active systems.
    ///
    /// Alias for [`Self::particle_count`] matching the C++ getter name.
    pub fn get_particle_count(&self) -> usize {
        self.particle_count
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

    fn tint_particle_system_all_colors(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        color: gamelogic::common::Color,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.tint_all_colors([
                        color.r as f32 / 255.0,
                        color.g as f32 / 255.0,
                        color.b as f32 / 255.0,
                    ]);
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
            // C++ ParticleSys.cpp line 3273: xfer->xferSnapshot(system)
            // Uses game_engine::Xfer (not system::Xfer), so fields are xfer'd individually.
            let mut s_id = system.system_id();
            xfer.xfer_unsigned_int(&mut s_id)?;
            let mut attached_drawable = system.attached_drawable_id().0;
            xfer.xfer_drawable_id(&mut attached_drawable)?;
            let mut attached_object = system.attached_object_id();
            xfer.xfer_object_id(&mut attached_object)?;
            let mut is_stopped = system.is_stopped();
            xfer.xfer_bool(&mut is_stopped)?;
            let mut slave_id = system
                .slave_system_id()
                .unwrap_or(INVALID_PARTICLE_SYSTEM_ID);
            xfer.xfer_unsigned_int(&mut slave_id)?;
            let mut master_id = system
                .master_system_id()
                .unwrap_or(INVALID_PARTICLE_SYSTEM_ID);
            xfer.xfer_unsigned_int(&mut master_id)?;
            let mut p_count = system.particle_count() as u32;
            xfer.xfer_unsigned_int(&mut p_count)?;
        }
    } else {
        manager.active_systems.clear();
        manager.particle_count = 0;
        manager.field_particle_count = 0;
        manager.system_count = 0;
        manager.on_screen_particle_count = 0;

        for _ in 0..system_count {
            let mut template_name = String::new();
            xfer.xfer_ascii_string(&mut template_name)?;
            if template_name.is_empty() {
                continue;
            }

            let template = manager
                .find_template(template_name.as_str())
                .ok_or(XferStatus::InvalidData)?;
            let system_id = manager.next_system_id;
            let mut system = Box::new(ParticleSystem::new(template, system_id, false));

            let mut s_id = 0u32;
            xfer.xfer_unsigned_int(&mut s_id)?;
            let mut attached_drawable = 0u32;
            xfer.xfer_drawable_id(&mut attached_drawable)?;
            let mut attached_object = 0u32;
            xfer.xfer_object_id(&mut attached_object)?;
            let mut is_stopped = false;
            xfer.xfer_bool(&mut is_stopped)?;
            let mut slave_id = 0u32;
            xfer.xfer_unsigned_int(&mut slave_id)?;
            let mut master_id = 0u32;
            xfer.xfer_unsigned_int(&mut master_id)?;
            let mut p_count = 0u32;
            xfer.xfer_unsigned_int(&mut p_count)?;

            // C++ ParticleSys.cpp line 3305: system = createParticleSystem(template, FALSE)
            manager.next_system_id = manager
                .next_system_id
                .max(system.system_id().saturating_add(1));
            manager.active_systems.insert(system.system_id(), system);
        }

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

    #[test]
    fn preload_assets_matches_cpp_particle_texture_filter() {
        let mut manager = ParticleSystemManager::new();

        let mut particle = ParticleSystemTemplate::new("TextureParticle".to_string());
        particle.info_mut().particle_type = ParticleType::Particle;
        particle.info_mut().particle_type_name = "EXSmokePuff.tga".to_string();
        manager
            .templates
            .insert(particle.name().to_string(), Arc::new(particle));

        let mut drawable = ParticleSystemTemplate::new("DrawableParticle".to_string());
        drawable.info_mut().particle_type = ParticleType::Drawable;
        drawable.info_mut().particle_type_name = "EXExplosionDrawable".to_string();
        manager
            .templates
            .insert(drawable.name().to_string(), Arc::new(drawable));

        let mut unnamed = ParticleSystemTemplate::new("UnnamedParticle".to_string());
        unnamed.info_mut().particle_type = ParticleType::Particle;
        manager
            .templates
            .insert(unnamed.name().to_string(), Arc::new(unnamed));

        manager.preload_assets();

        assert_eq!(
            manager.preloaded_texture_assets(),
            &["EXSmokePuff.tga".to_string()]
        );
    }

    #[test]
    fn remove_oldest_particles_culls_real_low_priority_particles() {
        let mut manager = ParticleSystemManager::new();
        let mut template = ParticleSystemTemplate::new("Dust".to_string());
        template.info_mut().priority = ParticlePriorityType::DustTrail;
        let template = Arc::new(template);
        let system_id = manager
            .create_particle_system(&template, false)
            .expect("particle system");

        for frame in 0..5 {
            let particle = crate::effects::particle_system::Particle::new(
                &crate::effects::particle_system::ParticleInfo::default(),
                frame,
                frame,
            );
            manager
                .find_particle_system_mut(system_id)
                .expect("active system")
                .push_particle(particle);
        }
        manager.particle_count = 5;
        manager.field_particle_count = 5;

        let removed = manager.remove_oldest_particles(3, ParticlePriorityType::Buildup);

        assert_eq!(removed, 3);
        assert_eq!(manager.particle_count(), 2);
        assert_eq!(manager.field_particle_count(), 2);
        assert_eq!(
            manager
                .find_particle_system(system_id)
                .expect("active system")
                .particle_count(),
            2
        );
    }

    #[test]
    fn can_create_particle_frees_slot_when_exactly_at_limit() {
        let mut manager = ParticleSystemManager::new();
        manager.set_lod_params(
            2,
            2,
            ParticlePriorityType::WeaponExplosion,
            ParticlePriorityType::Critical,
            0,
        );

        let mut template = ParticleSystemTemplate::new("Dust".to_string());
        template.info_mut().priority = ParticlePriorityType::DustTrail;
        let template = Arc::new(template);
        let system_id = manager
            .create_particle_system(&template, false)
            .expect("particle system");

        for frame in 0..2 {
            let particle = crate::effects::particle_system::Particle::new(
                &crate::effects::particle_system::ParticleInfo::default(),
                frame,
                frame,
            );
            manager
                .find_particle_system_mut(system_id)
                .expect("active system")
                .push_particle(particle);
        }
        manager.particle_count = 2;
        manager.field_particle_count = 2;

        assert!(manager.can_create_particle(ParticlePriorityType::Buildup));
        assert_eq!(manager.particle_count(), 1);
        assert_eq!(
            manager
                .find_particle_system(system_id)
                .expect("active system")
                .particle_count(),
            1
        );
    }
}
