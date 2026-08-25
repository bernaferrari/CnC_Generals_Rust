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
use crate::effects::particle_ini_loader::ParticleSystemINIParser;
use crate::system::SubsystemInterface;
use game_engine::System::XferVersion;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::system::xfer::Xfer as CommonXfer;
use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
use game_engine::{Xfer, XferMode, XferStatus};
use std::io::Cursor;

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

    /// C++ ParticleSystemManager::createParticle LOD skip (priority below GameLOD floor).
    #[error("particle spawn skipped by LOD priority {0:?}")]
    LodSkipped(ParticlePriorityType),
}

/// Particle priority levels (matches C++ ParticleSys.h exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParticlePriorityType {
    /// C++ `INVALID_PRIORITY` / `ParticlePriorityNames[0] = "NONE"`.
    None = 0,
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
            0 => Some(ParticlePriorityType::None),
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
    /// C++ `INVALID_SHADER` / retail INI `Shader = NONE`.
    /// It is intentionally preserved rather than coerced into a visible blend
    /// mode; callers that do not implement the associated particle subtype
    /// fail closed.
    Invalid = 0,
    Additive = 1,
    Alpha,
    AlphaTest,
    Multiply,
}

/// Particle types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleType {
    /// C++ `INVALID_TYPE` / retail INI `Type = NONE`.
    Invalid = 0,
    Particle = 1,
    Drawable,
    Streak,
    VolumeParticle,
    Smudge,
}

/// Emission velocity types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionVelocityType {
    /// C++ `INVALID_VELOCITY` / retail INI `VelocityType = NONE`.
    Invalid = 0,
    Ortho = 1,
    Spherical,
    Hemispherical,
    Cylindrical,
    Outward,
}

/// Emission volume types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionVolumeType {
    /// C++ `INVALID_VOLUME` / retail INI `VolumeType = NONE`.
    Invalid = 0,
    Point = 1,
    Line,
    Box,
    Sphere,
    Cylinder,
}

/// Wind motion types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindMotion {
    /// C++ `NONE`; the shipped data normally uses `Unused` instead.
    Invalid = 0,
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
#[derive(Debug, Clone, Copy, Default)]
pub enum EmissionVolume {
    #[default]
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
            wind_angle_change: 0.15,
            wind_angle_change_min: 0.15,
            wind_angle_change_max: 0.45,
            wind_motion_start_angle: 0.0,
            wind_motion_start_angle_min: 0.0,
            wind_motion_start_angle_max: std::f32::consts::PI / 4.0,
            wind_motion_end_angle: std::f32::consts::TAU - (std::f32::consts::PI / 4.0),
            wind_motion_end_angle_min: std::f32::consts::TAU - (std::f32::consts::PI / 4.0),
            wind_motion_end_angle_max: std::f32::consts::TAU,
            wind_motion_moving_to_end_angle: true,
        }
    }
}

impl ParticleSystemInfo {
    /// Tint all colors by the given color
    pub fn tint_all_colors(&mut self, tint_color: [f32; 3]) {
        // C++ ParticleSys.cpp:744-750 — "This tints all but the first colorKey!!!"
        for color_key in self.color_keys.iter_mut().skip(1) {
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
    /// C++ keeps `m_allParticleSystemList` in creation order. The lookup map
    /// is useful for Rust ownership, but `HashMap` iteration changes update,
    /// blend, LOD-eviction, and save order. Keep the observable list
    /// explicitly and use the map only for ID lookup.
    active_system_order: Vec<ParticleSystemId>,
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
            active_system_order: Vec::new(),
            next_system_id: 1,

            particle_count: 0,
            field_particle_count: 0,
            system_count: 0,
            on_screen_particle_count: 0,

            last_logic_frame_update: 0,
            local_player_index: 0,

            max_particle_count: 2500,
            max_field_particle_count: 30,
            min_dynamic_particle_priority: ParticlePriorityType::WeaponExplosion,
            min_dynamic_particle_skip_priority: ParticlePriorityType::Critical,
            particle_skip_mask: 0,
            particle_generation_count: 0,
            preloaded_texture_assets: Vec::new(),
        }
    }

    /// Load the authoritative GameClient particle definitions without
    /// replacing Common's separate `ParticleSystem` INI parser.
    ///
    /// C++ `ParticleSystemManager::init` installs these templates before FX
    /// systems are created.  Keeping them on this manager lets an exact
    /// GameClient FX name win over the narrow host/preset fallback path.
    fn load_retail_particle_templates(&mut self) -> Result<usize, ParticleSystemError> {
        let parser = ParticleSystemINIParser::default();
        parser
            .load_particle_system_definitions("Data/INI/ParticleSystem.ini", self)
            .map_err(|error| ParticleSystemError::ResourceLoadFailed(error.to_string()))
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

        // C++ `TemplateMap` is an ordered `std::map<AsciiString, ...>`.
        // Parent number is therefore part of the observable definition when
        // several templates name the same slave; do not let HashMap seed
        // order choose a different parent each run.
        let mut templates: Vec<_> = self.templates.values().collect();
        templates.sort_by(|left, right| left.name().cmp(right.name()));
        for sys_template in templates {
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

        // `TemplateMap` is ordered in C++; keeping the preloading order stable
        // also keeps archive requests and first-frame texture residency stable.
        let mut templates: Vec<_> = self.templates.values().collect();
        templates.sort_by(|left, right| left.name().cmp(right.name()));
        for tmplate in templates {
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

    /// Register (or replace) a full template instance under its name.
    ///
    /// Used by combat residual presets and FX paths that must not depend on
    /// ParticleSystems.ini already being loaded.
    pub fn register_template(&mut self, template: Arc<ParticleSystemTemplate>) {
        let name = template.name().to_string();
        self.templates.insert(name, template);
    }

    /// Ensure a combat/FX preset template is registered by name.
    ///
    /// Looks up existing templates first, then falls back to
    /// [`crate::effects::particle_presets::get_preset_by_name`].
    pub fn ensure_preset_template(&mut self, name: &str) -> Option<Arc<ParticleSystemTemplate>> {
        if let Some(existing) = self.find_template(name) {
            return Some(existing);
        }
        let preset = crate::effects::particle_presets::get_preset_by_name(name)?;
        self.templates.insert(name.to_string(), preset.clone());
        Some(preset)
    }

    /// Create a particle system from a known combat/FX preset name at a position.
    ///
    /// Residual combat path: death/fire feedback without requiring full INI load.
    pub fn create_preset_system_at(
        &mut self,
        template_name: &str,
        pos: Point3<f32>,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        self.ensure_preset_template(template_name)
            .ok_or_else(|| ParticleSystemError::TemplateNotFound(template_name.to_string()))?;
        let id = self.create_particle_system_at(template_name, pos)?;
        if let Some(system) = self.find_particle_system_mut(id) {
            system.start();
        }
        Ok(id)
    }

    /// Host/combat residual entry point without requiring nalgebra at the call site.
    pub fn create_preset_system_xyz(
        &mut self,
        template_name: &str,
        x: f32,
        y: f32,
        z: f32,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        self.create_preset_system_at(template_name, Point3::new(x, y, z))
    }

    /// Number of active particle systems currently registered.
    pub fn active_system_count(&self) -> usize {
        self.active_systems.len()
    }

    /// Snapshot the C++ `m_allParticleSystemList` ordering for a phase that
    /// needs mutable map access. IDs can disappear during a phase, so callers
    /// must still re-check the lookup map before using one.
    fn active_system_ids_in_order(&self) -> Vec<ParticleSystemId> {
        self.active_system_order.clone()
    }

    fn remove_active_system(&mut self, id: ParticleSystemId) -> Option<Box<ParticleSystem>> {
        let removed = self.active_systems.remove(&id)?;
        if let Some(index) = self
            .active_system_order
            .iter()
            .position(|known| *known == id)
        {
            self.active_system_order.remove(index);
        }
        Some(removed)
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
        if self.active_systems.contains_key(&system_id) {
            return Err(ParticleSystemError::InitializationFailed(format!(
                "duplicate particle system ID {system_id}"
            )));
        }

        // C++ ParticleSystemManager::createParticleSystem always instantiates the
        // system. LOD / budget gates run later in createParticle per particle.
        let system = ParticleSystem::new(template.clone(), system_id, create_slaves);
        self.active_systems.insert(system_id, Box::new(system));
        self.active_system_order.push(system_id);
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
    ///
    /// C++ `ParticleSystem::destroy` (ParticleSys.cpp:1255-1261) only sets
    /// `m_isDestroyed` and cascades to the slave. The manager keeps the system
    /// registered so leftover particles keep updating until they die
    /// (ParticleSys.cpp:2028-2060). `ParticleSystemManager::update` then
    /// deletes the system when `update` returns false (ParticleSys.cpp:2923).
    pub fn destroy_particle_system(&mut self, id: ParticleSystemId) {
        let slave_id = self
            .active_systems
            .get(&id)
            .and_then(|s| s.slave_system_id());

        if let Some(system) = self.active_systems.get_mut(&id) {
            system.destroy();
        }

        // Cascade to slave (C++ line 1258-1260: m_slaveSystem->destroy())
        if let Some(slave_id) = slave_id {
            self.destroy_particle_system(slave_id);
        }
    }

    /// Destroy all particle systems attached to an object
    pub fn destroy_attached_systems(&mut self, object_id: ObjectId) {
        let systems_to_remove: Vec<ParticleSystemId> = self
            .active_system_ids_in_order()
            .into_iter()
            .filter(|id| {
                self.active_systems
                    .get(id)
                    .is_some_and(|system| system.attached_object() == Some(object_id))
            })
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

        // C++ per system: emit/createParticle then update those particles same frame.
        let emit_ids = self.active_system_ids_in_order();
        for id in emit_ids {
            let phase = {
                let Some(system) = self.active_systems.get_mut(&id) else {
                    continue;
                };
                system.begin_frame_emit(local_player_index, current_frame)
            };
            match phase {
                crate::effects::particle_system::FrameEmitPhase::Dead => {
                    // C++ ParticleSys.cpp:2028-2053 still ages leftover particles
                    // after destroy; only empty destroyed systems return false.
                    if let Some(system) = self.active_systems.get_mut(&id) {
                        if system.particle_count() > 0 {
                            system.finish_frame_integrate(current_frame);
                            let dead = system.take_dead_controlled_systems();
                            for dead_id in dead {
                                self.destroy_particle_system(dead_id);
                            }
                        }
                    }
                    continue;
                }
                crate::effects::particle_system::FrameEmitPhase::Delayed => continue,
                crate::effects::particle_system::FrameEmitPhase::Emitted => {}
            }

            let (infos, priority, ground_aligned, emit_above_ground, attached_name) = {
                let Some(system) = self.active_systems.get_mut(&id) else {
                    continue;
                };
                (
                    system.take_pending_emissions(),
                    system.priority(),
                    system.template().info().is_ground_aligned,
                    system.is_emit_above_ground_only(),
                    system.attached_system_name().to_string(),
                )
            };
            for info in infos {
                if emit_above_ground {
                    let ground = gamelogic::helpers::TheTerrainLogic::get()
                        .map(|terrain| {
                            terrain.get_ground_height(info.position.x, info.position.y, None)
                        })
                        .unwrap_or(0.0);
                    if info.position.z < ground {
                        continue;
                    }
                }
                let system_count = self
                    .active_systems
                    .get(&id)
                    .map(|s| s.particle_count())
                    .unwrap_or(0);
                if !self.can_create_particle_for_system(
                    priority,
                    ground_aligned,
                    system_count,
                    false,
                ) {
                    continue;
                }
                let personality = self
                    .active_systems
                    .get(&id)
                    .map(|s| s.personality_counter())
                    .unwrap_or(0);
                let mut particle = crate::effects::particle_system::Particle::new(
                    &info,
                    personality,
                    current_frame,
                );
                if !attached_name.is_empty() {
                    if let Some(tmpl) = self.find_template(&attached_name) {
                        if let Ok(att_id) = self.create_particle_system(&tmpl, true) {
                            particle.controlled_system = Some(att_id);
                            if let Some(att) = self.active_systems.get_mut(&att_id) {
                                att.set_control_particle_position(particle.position);
                            }
                        }
                    }
                }
                if let Some(system) = self.active_systems.get_mut(&id) {
                    system.push_particle(particle);
                    if system.slave_system_id().is_some() {
                        system.record_slave_emission();
                    }
                }
                self.particle_count += 1;
            }

            if let Some(system) = self.active_systems.get_mut(&id) {
                system.finish_frame_integrate(current_frame);
                let dead = system.take_dead_controlled_systems();
                for dead_id in dead {
                    self.destroy_particle_system(dead_id);
                }
            }
        }

        // Reposition systems controlled by a live particle (C++ ParticleSys.cpp:1948-1959).
        let control_pairs: Vec<(ParticleSystemId, nalgebra::Point3<f32>)> = self
            .active_system_ids_in_order()
            .into_iter()
            .flat_map(|sys_id| {
                self.active_systems
                    .get(&sys_id)
                    .into_iter()
                    .flat_map(|system| {
                        system.particles().iter().filter_map(|particle| {
                            particle
                                .controlled_system
                                .map(|att_id| (att_id, particle.position))
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        for (att_id, pos) in control_pairs {
            if let Some(att) = self.active_systems.get_mut(&att_id) {
                att.set_control_particle_position(pos);
            }
        }

        // Process slave particle emissions (C++ ParticleSys.cpp lines 2004-2009)
        let slave_work: Vec<(ParticleSystemId, ParticleSystemId, u32)> = self
            .active_system_ids_in_order()
            .into_iter()
            .filter_map(|master_id| {
                let system = self.active_systems.get_mut(&master_id)?;
                let count = system.drain_slave_emission_count();
                (count != 0)
                    .then(|| {
                        system
                            .slave_system_id()
                            .map(|slave_id| (master_id, slave_id, count))
                    })
                    .flatten()
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

            for info in merged_infos {
                let (priority, ground_aligned, system_count) = {
                    let Some(slave) = self.active_systems.get(&slave_id) else {
                        break;
                    };
                    (
                        slave.priority(),
                        slave.template().info().is_ground_aligned,
                        slave.particle_count(),
                    )
                };
                if !self.can_create_particle_for_system(
                    priority,
                    ground_aligned,
                    system_count,
                    false,
                ) {
                    continue;
                }
                if let Some(slave_system) = self.active_systems.get_mut(&slave_id) {
                    let particle = crate::effects::particle_system::Particle::new(
                        &info,
                        slave_system.personality_counter(),
                        current_frame,
                    );
                    slave_system.push_particle(particle);
                    self.particle_count += 1;
                }
            }
        }

        let systems_to_remove: Vec<ParticleSystemId> = self
            .active_system_ids_in_order()
            .into_iter()
            .filter(|id| {
                self.active_systems
                    .get(id)
                    .is_some_and(|system| system.should_remove())
            })
            .collect();
        for id in systems_to_remove {
            if let Some(system) = self.active_systems.get(&id) {
                let slave_id = system.slave_system_id();
                self.remove_active_system(id);
                self.system_count = self.system_count.saturating_sub(1);
                if let Some(slave_id) = slave_id {
                    self.destroy_particle_system(slave_id);
                }
            }
        }

        // Update statistics. Field cap is last-frame on-screen count
        // (C++ W3DParticleSys.cpp:124/205), not worldwide living particles.
        self.particle_count = self
            .active_systems
            .values()
            .map(|s| s.particle_count())
            .sum();
    }

    /// Check if a particle with given priority should be skipped based on LOD (C++ GameLODManager::isParticleSkipped)
    fn live_max_particle_count(&self) -> usize {
        let runtime = game_engine::common::global_data::read().max_particle_count;
        if runtime > 0 {
            return runtime as usize;
        }
        game_engine::common::ini::ini_game_data::get_global_data()
            .map(|data| data.read().max_particle_count)
            .filter(|&count| count > 0)
            .map(|count| count as usize)
            .unwrap_or(self.max_particle_count)
    }

    fn live_max_field_particle_count(&self) -> usize {
        let runtime = game_engine::common::global_data::read().max_field_particle_count;
        if runtime > 0 {
            return runtime as usize;
        }
        game_engine::common::ini::ini_game_data::get_global_data()
            .map(|data| data.read().max_field_particle_count)
            .filter(|&count| count > 0)
            .map(|count| count as usize)
            .unwrap_or(self.max_field_particle_count)
    }

    fn live_lod_priority(
        priority: game_engine::common::ini::ParticlePriorityType,
    ) -> ParticlePriorityType {
        ParticlePriorityType::from_index(priority as usize)
            .unwrap_or(ParticlePriorityType::WeaponExplosion)
    }

    /// Check if a particle with given priority should be skipped based on LOD (C++ GameLODManager::isParticleSkipped)
    pub fn should_skip_particle(&mut self, priority: ParticlePriorityType) -> bool {
        // ALWAYS_RENDER particles are never skipped (C++ line 1695)
        if priority == ParticlePriorityType::AlwaysRender {
            return false;
        }

        let (min_priority, min_skip_priority) = {
            let lod = game_engine::common::ini::get_game_lod_manager();
            (
                Self::live_lod_priority(lod.get_min_dynamic_particle_priority()),
                Self::live_lod_priority(lod.get_min_dynamic_particle_skip_priority()),
            )
        };

        // Check if below minimum priority for current FPS (C++ lines 1680-1682)
        if priority < min_priority {
            return true;
        }

        // Check skip mask for frame-skipping (C++ lines 1681-1682)
        if priority < min_skip_priority && game_engine::common::ini::is_particle_skipped() {
            return true;
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
        removed
    }

    /// C++ `ParticleSystem::createParticle` gates including field-particle cap.
    pub fn can_create_particle_for_system(
        &mut self,
        priority: ParticlePriorityType,
        is_ground_aligned: bool,
        system_particle_count: usize,
        force_create: bool,
    ) -> bool {
        if force_create {
            return true;
        }
        let use_fx = game_engine::common::ini::ini_game_data::get_global_data()
            .map(|data| data.read().use_fx)
            .unwrap_or(true);
        if !use_fx {
            return false;
        }
        if !self.can_create_particle(priority) {
            return false;
        }
        if system_particle_count > 0
            && priority == ParticlePriorityType::AreaEffect
            && is_ground_aligned
            && self.field_particle_count > self.live_max_field_particle_count()
        {
            return false;
        }
        true
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

        let max_particle_count = self.live_max_particle_count();

        // Check if particles are disabled entirely
        if max_particle_count == 0 {
            return false;
        }

        // Check particle count limit (C++ lines 1699-1704)
        if self.particle_count >= max_particle_count {
            let needed = self.particle_count - max_particle_count + 1;
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
        self.active_system_order
            .iter()
            .filter_map(|id| self.active_systems.get(id).map(|system| system.as_ref()))
    }
    pub fn for_each_particle_system_mut(&mut self, mut f: impl FnMut(&mut ParticleSystem)) {
        let order = self.active_system_order.clone();
        for id in order {
            if let Some(system) = self.active_systems.get_mut(&id) {
                f(system.as_mut());
            }
        }
    }

    /// C++ `doParticles` visible-box cull (AABB expanded by particle size).
    pub fn cull_particles_to_visible_box(
        &mut self,
        center: [f32; 3],
        extent: [f32; 3],
        max_per_system: usize,
    ) {
        self.for_each_particle_system_mut(|system| {
            let mut kept = 0usize;
            for particle in system.particles_mut() {
                if !particle.is_lifetime_active() {
                    particle.is_culled = true;
                    continue;
                }
                let size = particle.size;
                let outside = (particle.position.x - center[0]).abs() > extent[0] + size
                    || (particle.position.y - center[1]).abs() > extent[1] + size
                    || (particle.position.z - center[2]).abs() > extent[2] + size;
                if outside || kept >= max_per_system {
                    particle.is_culled = true;
                } else {
                    particle.is_culled = false;
                    kept += 1;
                }
            }
        });
        self.recount_on_screen_field_particles();
    }

    /// C++ `m_fieldParticleCount` — AREA_EFFECT + ground-aligned particles
    /// that passed the visible-box cull (drawn this frame).
    fn recount_on_screen_field_particles(&mut self) {
        self.field_particle_count = self
            .active_systems
            .values()
            .filter(|s| {
                s.priority() == ParticlePriorityType::AreaEffect
                    && s.template().info().is_ground_aligned
            })
            .map(|s| s.particles().iter().filter(|p| p.is_draw_alive()).count())
            .sum();
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
        self.all_particle_systems().collect()
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
        self.active_system_order.clear();
        self.next_system_id = 1;

        // Reset LOD settings to defaults
        self.max_particle_count = 2500;
        self.max_field_particle_count = 30;
        self.min_dynamic_particle_priority = ParticlePriorityType::WeaponExplosion;
        self.min_dynamic_particle_skip_priority = ParticlePriorityType::Critical;
        self.particle_skip_mask = 0;
        self.particle_generation_count = 0;

        // C++ Reference: INI ini; ini.load("Data\\INI\\ParticleSystem.ini", INI_LOAD_OVERWRITE, NULL);
        // Parse into this GameClient manager, rather than merely opening the
        // file through Common's independent compatibility registry.
        if let Err(e) = self.load_retail_particle_templates() {
            // Log warning but don't fail init — particle systems can be loaded later
            eprintln!("Warning: Failed to load Data/INI/ParticleSystem.ini: {}", e);
        }

        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Reset all systems and templates
        self.active_systems.clear();
        self.active_system_order.clear();
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
        let id = NameKeyGenerator::name_to_key(template.name());
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
                    system.set_emission_volume_sphere_radius(radius);
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
                    system.set_emission_volume_cylinder_radius(radius);
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

    fn set_particle_system_saveable(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        saveable: bool,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_saveable(saveable);
                }
            }
        }
    }

    fn rotate_particle_system_local_transform_z(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        angle: gamelogic::common::Real,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.rotate_local_transform_z(angle);
                }
            }
        }
    }

    fn set_particle_system_skip_parent_xfrm(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        enable: bool,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_skip_parent_xfrm(enable);
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

    fn set_particle_system_size_multiplier(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        multiplier: gamelogic::common::Real,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_size_multiplier(multiplier);
                }
            }
        }
    }

    fn trigger_particle_system(&self, system_id: gamelogic::common::ParticleSystemId) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.trigger();
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

    fn set_particle_system_lifetime(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        frames: gamelogic::common::UnsignedInt,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_system_lifetime(frames);
                }
            }
        }
    }

    fn set_particle_system_initial_delay(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        frames: gamelogic::common::UnsignedInt,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_initial_delay(frames);
                }
            }
        }
    }
}

/// C++ INIParticleSys.cpp — Common INI ParticleSystem blocks overlay this manager.
fn overlay_common_particle_system(
    name: &str,
    properties: &HashMap<String, String>,
    _load_type: game_engine::common::ini::INILoadType,
) {
    let Ok(mut guard) = get_particle_system_manager_mut() else {
        return;
    };
    let Some(manager) = guard.as_mut() else {
        return;
    };
    let parser = ParticleSystemINIParser::default();
    if let Err(err) = parser.overlay_from_property_map(name, properties, manager) {
        log::warn!("live ParticleSystem overlay '{name}' failed: {err}");
    }
}

/// Initialize the global particle system manager
pub fn initialize_particle_system_manager() -> Result<(), ParticleSystemError> {
    let mut manager_guard = PARTICLE_SYSTEM_MANAGER.write().map_err(|_| {
        ParticleSystemError::InitializationFailed("Failed to acquire write lock".to_string())
    })?;

    let mut manager = ParticleSystemManager::new();
    if let Err(error) = manager.load_retail_particle_templates() {
        // This is deliberately non-fatal just like C++'s optional asset path:
        // the manager remains valid, and a later GameClient initialization can
        // retry once the virtual file system has mounted the retail archives.
        log::warn!("Failed to load retail ParticleSystem.ini: {error}");
    }
    *manager_guard = Some(manager);
    game_engine::common::ini::register_particle_system_live_overlay(overlay_common_particle_system);
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
    // C++ saves the linked `m_allParticleSystemList` in its live creation
    // order.  Preserve that order rather than serializing a hash-map order or
    // sorting IDs, because alpha blending and subsequent update order remain
    // observable after a load.
    let system_ids = manager.active_system_ids_in_order();

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
            // C++ ParticleSys.cpp line 3273: `xferSnapshot(system)`.  The
            // prior Rust bridge wrote a handful of headers only, losing the
            // particles, transforms, timers, RNG personality, and attachment
            // state on every save/load. `ParticleSystem` already owns the full
            // Snapshotable parity implementation, so use it directly.
            let mut adapter = crate::core::game_client::RuntimeCommonXferAdapter::new(xfer);
            Snapshotable::xfer(&mut **system, &mut adapter).map_err(|_| XferStatus::InvalidData)?;
        }
    } else {
        manager.active_systems.clear();
        manager.active_system_order.clear();
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
            // Construct a temporary valid system, then let its exact
            // Snapshotable implementation restore the serialized ID and all
            // state just as C++ `xferSnapshot` does.
            let mut system = Box::new(ParticleSystem::new(template, 1, false));
            let mut adapter = crate::core::game_client::RuntimeCommonXferAdapter::new(xfer);
            Snapshotable::xfer(&mut *system, &mut adapter).map_err(|_| XferStatus::InvalidData)?;
            let system_id = system.system_id();
            if system_id == INVALID_PARTICLE_SYSTEM_ID
                || manager.active_systems.contains_key(&system_id)
            {
                return Err(XferStatus::InvalidData);
            }
            manager.next_system_id = manager.next_system_id.max(system_id.saturating_add(1));
            manager.active_systems.insert(system_id, system);
            manager.active_system_order.push(system_id);
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

    let system_ids = manager.active_system_ids_in_order();
    for system_id in system_ids {
        let Some(system) = manager.active_systems.get_mut(&system_id) else {
            continue;
        };
        Snapshotable::load_post_process(&mut **system).map_err(|_| XferStatus::InvalidData)?;
    }

    Ok(())
}

/// C++ `CHUNK_ParticleSystem` payload (`ParticleSys.cpp:3232-3323`).
pub fn capture_live_particle_system_xfer_bytes() -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut bytes);
        let mut xfer = CommonXferSave::new(cursor, 1);
        xfer_particle_system_manager_common(&mut xfer)?;
    }
    Ok(bytes)
}

pub fn restore_live_particle_system_from_xfer_bytes(bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || (bytes.len() == 1 && bytes[0] == 1) {
        return Ok(());
    }
    let mut xfer = CommonXferLoad::new(Cursor::new(bytes.to_vec()), 1);
    xfer_particle_system_manager_common(&mut xfer)?;
    load_post_process_particle_system_manager_state().map_err(|e| e.to_string())
}

fn xfer_particle_system_manager_common(xfer: &mut dyn CommonXfer) -> Result<(), String> {
    let current_version: u8 = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)
        .map_err(|e| e.to_string())?;

    let mut manager_guard = get_particle_system_manager_mut().map_err(|e| e.to_string())?;
    let manager = manager_guard.get_or_insert_with(ParticleSystemManager::new);

    xfer.xfer_unsigned_int(&mut manager.next_system_id)
        .map_err(|e| e.to_string())?;
    let system_ids = manager.active_system_ids_in_order();
    let mut system_count = system_ids.len() as u32;
    xfer.xfer_unsigned_int(&mut system_count)
        .map_err(|e| e.to_string())?;

    if xfer.get_xfer_mode() == game_engine::common::system::xfer::XferMode::Save {
        for system_id in system_ids {
            let Some(system) = manager.active_systems.get_mut(&system_id) else {
                return Err("missing particle system during save".into());
            };
            let mut template_name = if system.is_destroyed() || !system.is_saveable() {
                String::new()
            } else {
                system.template().name().to_string()
            };
            xfer.xfer_ascii_string(&mut template_name)
                .map_err(|e| e.to_string())?;
            if template_name.is_empty() {
                continue;
            }
            Snapshotable::xfer(&mut **system, xfer)?;
        }
    } else {
        manager.active_systems.clear();
        manager.active_system_order.clear();
        manager.particle_count = 0;
        manager.field_particle_count = 0;
        manager.system_count = 0;
        manager.on_screen_particle_count = 0;

        for _ in 0..system_count {
            let mut template_name = String::new();
            xfer.xfer_ascii_string(&mut template_name)
                .map_err(|e| e.to_string())?;
            if template_name.is_empty() {
                continue;
            }
            let template = manager
                .find_template(template_name.as_str())
                .ok_or_else(|| format!("unknown particle template '{template_name}'"))?;
            let mut system = Box::new(ParticleSystem::new(template, 1, false));
            Snapshotable::xfer(&mut *system, xfer)?;
            let system_id = system.system_id();
            if system_id == INVALID_PARTICLE_SYSTEM_ID
                || manager.active_systems.contains_key(&system_id)
            {
                return Err("invalid restored particle system id".into());
            }
            manager.next_system_id = manager.next_system_id.max(system_id.saturating_add(1));
            manager.active_systems.insert(system_id, system);
            manager.active_system_order.push(system_id);
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

pub fn register_particle_system_manager_bridge() {
    let _ =
        gamelogic::helpers::register_particle_system_manager(Arc::new(ParticleSystemManagerBridge));
}

/// Get reference to the global particle system manager
pub fn get_particle_system_manager()
-> Result<std::sync::RwLockReadGuard<'static, Option<ParticleSystemManager>>, ParticleSystemError> {
    PARTICLE_SYSTEM_MANAGER.read().map_err(|_| {
        ParticleSystemError::InitializationFailed("Failed to acquire read lock".to_string())
    })
}

/// Get mutable reference to the global particle system manager
pub fn get_particle_system_manager_mut()
-> Result<std::sync::RwLockWriteGuard<'static, Option<ParticleSystemManager>>, ParticleSystemError>
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
    fn attach_particle_system_to_object_helper_records_parent_object_id() {
        let _ = initialize_particle_system_manager();
        register_particle_system_manager_bridge();
        {
            let mut guard = get_particle_system_manager_mut().expect("global particle manager");
            let mgr = guard.get_or_insert_with(ParticleSystemManager::new);
            let _ = mgr.new_template("OclDummyAttachSmoke".to_string());
        }

        let object_id = 4_242u32;
        let sys_id =
            gamelogic::helpers::attach_particle_system_to_object("OclDummyAttachSmoke", object_id)
                .expect("registered template should create+attach");

        {
            let guard = get_particle_system_manager().expect("global particle manager");
            let mgr = guard.as_ref().expect("manager initialized");
            let system = mgr
                .find_particle_system(sys_id)
                .expect("created particle system");
            assert_eq!(system.attached_object(), Some(object_id));
        }

        assert!(
            gamelogic::helpers::attach_particle_system_to_object("NoSuchOclParticleTemplate", 1)
                .is_none(),
            "unknown template must fail-closed"
        );
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

    /// Residual combat path: death/fire presets create real registry entries
    /// without ParticleSystems.ini (fail-closed: not full W3D GPU parity).
    #[test]
    fn manager_update_emits_then_integrates_newborn_same_frame() {
        let mut manager = ParticleSystemManager::new();
        let mut template = ParticleSystemTemplate::new("BurstNow".to_string());
        {
            let info = template.info_mut();
            info.priority = ParticlePriorityType::AlwaysRender;
            info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
            info.burst_count = GameClientRandomVariable::new(1.0, 1.0);
            info.initial_delay = GameClientRandomVariable::new(0.0, 0.0);
            info.lifetime = GameClientRandomVariable::new(30.0, 30.0);
            info.system_lifetime = 0;
        }
        let template = Arc::new(template);
        let system_id = manager
            .create_particle_system(&template, false)
            .expect("system");

        manager.update(0, 1);

        let system = manager
            .find_particle_system(system_id)
            .expect("active system");
        assert!(
            system.particle_count() >= 1,
            "createParticle must commit before particle update"
        );
        let newborn = system.particles().front().expect("newborn");
        assert_eq!(
            newborn.lifetime_left,
            newborn.lifetime.saturating_sub(1),
            "C++ updates newborns the same frame they are created"
        );
    }

    #[test]
    fn create_preset_system_at_registers_combat_death_and_muzzle_entries() {
        let mut manager = ParticleSystemManager::new();
        assert_eq!(manager.active_system_count(), 0);

        let death_id = manager
            .create_preset_system_at("MediumExplosion", Point3::new(10.0, 0.0, 20.0))
            .expect("death explosion preset");
        let smoke_id = manager
            .create_preset_system_at("SmokePlume", Point3::new(10.0, 0.0, 20.0))
            .expect("death smoke preset");
        let muzzle_id = manager
            .create_preset_system_at("MuzzleFlash", Point3::new(0.0, 0.0, 0.0))
            .expect("muzzle flash preset");

        assert_eq!(manager.active_system_count(), 3);
        assert!(manager.find_particle_system(death_id).is_some());
        assert!(manager.find_particle_system(smoke_id).is_some());
        assert!(manager.find_particle_system(muzzle_id).is_some());

        let death = manager.find_particle_system(death_id).unwrap();
        assert_eq!(death.position(), Point3::new(10.0, 0.0, 20.0));
        assert!(!death.is_stopped(), "preset system should be started");

        // Unknown preset still fails closed (no silent empty placeholder success).
        assert!(
            manager
                .create_preset_system_at("TotallyUnknownCombatFx", Point3::origin())
                .is_err()
        );
    }

    /// C++ ParticleSys.cpp:1255 `ParticleSystem::destroy` only sets
    /// `m_isDestroyed`. ParticleSys.cpp:1966 stops emit; 2059 removes the
    /// system only after leftover particles die.
    #[test]
    fn destroy_particle_system_keeps_updating_until_particles_die() {
        let mut manager = ParticleSystemManager::new();
        let mut template = ParticleSystemTemplate::new("FadeOut".to_string());
        {
            let info = template.info_mut();
            info.priority = ParticlePriorityType::AlwaysRender;
            info.burst_delay = GameClientRandomVariable::new(0.0, 0.0);
            info.burst_count = GameClientRandomVariable::new(4.0, 4.0);
            info.initial_delay = GameClientRandomVariable::new(0.0, 0.0);
            info.lifetime = GameClientRandomVariable::new(30.0, 30.0);
            info.system_lifetime = 0; // forever — would keep emitting if not destroyed
        }
        let template = Arc::new(template);
        let system_id = manager
            .create_particle_system(&template, false)
            .expect("system");

        let mut info = crate::effects::particle_system::ParticleInfo::default();
        info.lifetime = 2;
        let particle = crate::effects::particle_system::Particle::new(&info, 0, 0);
        manager
            .find_particle_system_mut(system_id)
            .expect("active")
            .push_particle(particle);
        manager.particle_count = 1;

        manager.destroy_particle_system(system_id);

        let system = manager
            .find_particle_system(system_id)
            .expect("C++ destroy leaves the system registered");
        assert!(system.is_destroyed());
        assert_eq!(system.particle_count(), 1);
        assert_eq!(manager.system_count(), 1);

        manager.update(0, 1);
        let system = manager
            .find_particle_system(system_id)
            .expect("particles still in the air");
        assert!(system.is_destroyed());
        assert_eq!(
            system.particle_count(),
            1,
            "must not emit after destroy; leftover particle still alive"
        );
        assert_eq!(
            system.particles().front().expect("leftover").lifetime_left,
            1
        );

        manager.update(0, 2);
        assert!(
            manager.find_particle_system(system_id).is_none(),
            "C++ ParticleSys.cpp:2059 removes only after last particle dies"
        );
        assert_eq!(manager.system_count(), 0);
        assert_eq!(manager.particle_count(), 0);
    }

    #[test]
    fn live_particle_system_xfer_keeps_mid_flight_explosion() {
        let _ = initialize_particle_system_manager();
        let template_name = format!("LiveSaveBurst_{}", std::process::id());
        let system_id = {
            let mut guard = get_particle_system_manager_mut().expect("global particle manager");
            let mgr = guard.get_or_insert_with(ParticleSystemManager::new);
            mgr.active_systems.clear();
            mgr.active_system_order.clear();
            mgr.system_count = 0;
            mgr.particle_count = 0;
            let template = mgr.new_template(template_name.clone());
            let id = mgr.create_particle_system(&template, false).unwrap();
            if let Some(system) = mgr.find_particle_system_mut(id) {
                system.set_position(nalgebra::Point3::new(40.0, 8.0, 12.0));
            }
            id
        };

        let bytes = capture_live_particle_system_xfer_bytes().expect("capture particle xfer");
        assert!(
            bytes.len() > 1,
            "CHUNK_ParticleSystem must not be NullSnapshot v1"
        );
        assert_eq!(bytes[0], 1);

        {
            let mut guard = get_particle_system_manager_mut().expect("global particle manager");
            let mgr = guard.get_or_insert_with(ParticleSystemManager::new);
            mgr.active_systems.clear();
            mgr.active_system_order.clear();
            mgr.system_count = 0;
            mgr.particle_count = 0;
        }

        restore_live_particle_system_from_xfer_bytes(&bytes).expect("restore particle xfer");
        let guard = get_particle_system_manager().expect("global particle manager");
        let mgr = guard.as_ref().expect("manager after restore");
        let system = mgr
            .find_particle_system(system_id)
            .expect("mid-flight system must survive load");
        assert_eq!(system.template().name(), template_name);
        assert!(!system.is_destroyed());
    }

    #[test]
    fn visible_box_cull_marks_offscreen_and_caps_batch() {
        let mut manager = ParticleSystemManager::new();
        let template = manager.new_template("CullBox".to_string());
        let system_id = manager.create_particle_system(&template, false).unwrap();
        {
            let system = manager.find_particle_system_mut(system_id).expect("system");
            let mut near = crate::effects::particle_system::Particle::new(
                &crate::effects::particle_system::ParticleInfo::default(),
                0,
                0,
            );
            near.lifetime_left = 10;
            near.position = nalgebra::Point3::new(0.0, 0.0, 0.0);
            near.size = 1.0;
            system.push_particle(near);
            let mut far = crate::effects::particle_system::Particle::new(
                &crate::effects::particle_system::ParticleInfo::default(),
                0,
                0,
            );
            far.lifetime_left = 10;
            far.position = nalgebra::Point3::new(1000.0, 0.0, 0.0);
            far.size = 1.0;
            system.push_particle(far);
        }
        manager.cull_particles_to_visible_box([0.0, 0.0, 0.0], [10.0, 10.0, 10.0], 512);
        let system = manager.find_particle_system(system_id).unwrap();
        let particles: Vec<_> = system.particles().iter().collect();
        assert!(!particles[0].is_culled);
        assert!(particles[1].is_culled);
    }

    #[test]
    fn field_particle_count_is_on_screen_area_effect_only() {
        let mut manager = ParticleSystemManager::new();
        let mut template = manager.new_template("FieldCap".to_string());
        {
            let tmpl = std::sync::Arc::make_mut(&mut template);
            tmpl.info_mut().priority = ParticlePriorityType::AreaEffect;
            tmpl.info_mut().is_ground_aligned = true;
        }
        let system_id = manager.create_particle_system(&template, false).unwrap();
        {
            let system = manager.find_particle_system_mut(system_id).expect("system");
            let mut near = crate::effects::particle_system::Particle::new(
                &crate::effects::particle_system::ParticleInfo::default(),
                0,
                0,
            );
            near.lifetime_left = 10;
            near.position = nalgebra::Point3::new(0.0, 0.0, 0.0);
            near.size = 1.0;
            system.push_particle(near);
            let mut far = crate::effects::particle_system::Particle::new(
                &crate::effects::particle_system::ParticleInfo::default(),
                0,
                0,
            );
            far.lifetime_left = 10;
            far.position = nalgebra::Point3::new(1000.0, 0.0, 0.0);
            far.size = 1.0;
            system.push_particle(far);
        }
        manager.cull_particles_to_visible_box([0.0, 0.0, 0.0], [10.0, 10.0, 10.0], 512);
        assert_eq!(
            manager.field_particle_count(),
            1,
            "C++ m_fieldParticleCount counts only on-screen AREA_EFFECT ground-aligned particles"
        );
    }

    /// Empty destroyed systems are still removed on the next manager update
    /// (C++ ParticleSys.cpp:2059 `m_isDestroyed && !m_systemParticlesHead`).
    #[test]
    fn destroy_empty_system_removed_on_next_update() {
        let mut manager = ParticleSystemManager::new();
        let template = manager.new_template("EmptyFade".to_string());
        let system_id = manager.create_particle_system(&template, false).unwrap();

        manager.destroy_particle_system(system_id);
        assert!(
            manager
                .find_particle_system(system_id)
                .is_some_and(|s| s.is_destroyed())
        );
        assert_eq!(manager.system_count(), 1);

        manager.update(0, 1);
        assert!(manager.find_particle_system(system_id).is_none());
        assert_eq!(manager.system_count(), 0);
    }
}
