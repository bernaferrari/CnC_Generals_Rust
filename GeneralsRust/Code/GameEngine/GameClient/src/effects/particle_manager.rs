//! # Particle System Manager
//!
//! Complete implementation of the Command & Conquer Generals Zero Hour particle system,
//! matching the C++ implementation exactly for visual effects compatibility.

use glam::{Mat3, Mat4 as GlamMat4, Vec3};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Instant;
use thiserror::Error;

use crate::core::DrawableId;
use crate::effects::particle_ini_loader::ParticleSystemINIParser;
use crate::system::SubsystemInterface;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::random_value::get_game_client_random_value_real;
use game_engine::common::system::Snapshotable;
use game_engine::common::system::xfer::Xfer as CommonXfer;
use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
use game_engine::{Xfer, XferStatus};
use std::io::Cursor;

// Authored primitives are owned by a renderer-independent crate. Runtime,
// snapshot adapters, and definition catalogs remain on this GameClient owner.
pub use generals_particles::{
    DEFAULT_VOLUME_PARTICLE_DEPTH, EmissionVelocity, EmissionVelocityType, EmissionVolume,
    EmissionVolumeType, GameClientRandomVariable, INVALID_PARTICLE_SYSTEM_ID, Keyframe,
    MAX_KEYFRAMES, MAX_VOLUME_PARTICLE_DEPTH, OPTIMUM_VOLUME_PARTICLE_DEPTH, ObjectId,
    ParticlePriorityType, ParticleShaderType, ParticleSystemId, ParticleType, RGBColorKeyframe,
    RandomKeyframe, WindMotion,
};

/// Temporary engine RNG adapter. The extracted primitives accept an explicit
/// random stream; this extension preserves current production draw timing.
pub trait SampleParticleRandomVariable {
    fn sample(&self) -> f32;
}

impl SampleParticleRandomVariable for GameClientRandomVariable {
    fn sample(&self) -> f32 {
        self.sample_with(get_game_client_random_value_real)
    }
}

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

fn particle_priority_to_u8(priority: ParticlePriorityType) -> u8 {
    priority as u8
}

fn particle_priority_from_u8(value: u8) -> ParticlePriorityType {
    ParticlePriorityType::from_index(value as usize).unwrap_or(ParticlePriorityType::Critical)
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
    pub drift_velocity: Vec3,

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
    pub slave_pos_offset: Vec3,
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
            drift_velocity: Vec3::ZERO,

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
            slave_pos_offset: Vec3::ZERO,
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

/// Identity of one actual system-ID namespace. Numeric IDs may be reused after
/// reset/load; retaining this immutable token never shares manager mutation.
#[derive(Debug, Clone)]
pub struct ParticleSystemOwner(std::sync::Weak<()>);

impl PartialEq for ParticleSystemOwner {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Weak::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for ParticleSystemOwner {}
impl ParticleSystemOwner {
    /// Only the actual manager keeps this namespace alive; receipts cannot.
    pub fn is_live(&self) -> bool {
        self.0.strong_count() != 0
    }
}

/// The particle system manager (matches C++ ParticleSystemManager).
pub struct ParticleSystemManager {
    runtime_owner: Arc<()>,
    pub(crate) templates: HashMap<String, Arc<ParticleSystemTemplate>>,
    active_systems: HashMap<ParticleSystemId, Box<ParticleSystem>>,
    /// C++ keeps `m_allParticleSystemList` in creation order. The lookup map
    /// is useful for Rust ownership, but `HashMap` iteration changes update,
    /// blend, LOD-eviction, and save order. Keep the observable list
    /// explicitly and use the map only for ID lookup.
    active_system_order: Vec<ParticleSystemId>,
    next_system_id: ParticleSystemId,
    next_particle_ordinal: u64,

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
    preloaded_texture_assets: Vec<String>,
}

impl ParticleSystemManager {
    pub fn runtime_owner(&self) -> ParticleSystemOwner {
        ParticleSystemOwner(Arc::downgrade(&self.runtime_owner))
    }

    fn renew_system_namespace(&mut self) {
        self.runtime_owner = Arc::new(());
    }

    /// Create a new particle system manager
    pub fn new() -> Self {
        Self {
            runtime_owner: Arc::new(()),
            templates: HashMap::new(),
            active_systems: HashMap::new(),
            active_system_order: Vec::new(),
            next_system_id: 1,
            next_particle_ordinal: 1,

            particle_count: 0,
            field_particle_count: 0,
            system_count: 0,
            on_screen_particle_count: 0,

            last_logic_frame_update: 0,
            local_player_index: 0,

            max_particle_count: 2500,
            max_field_particle_count: 30,
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
        pos: Vec3,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        self.ensure_preset_template(template_name)
            .ok_or_else(|| ParticleSystemError::TemplateNotFound(template_name.to_string()))?;
        let id = self.create_particle_system_at(template_name, pos)?;
        if let Some(system) = self.find_particle_system_mut(id) {
            system.start();
        }
        Ok(id)
    }

    /// Host/combat residual entry point without requiring extra math crates at the call site.
    pub fn create_preset_system_xyz(
        &mut self,
        template_name: &str,
        x: f32,
        y: f32,
        z: f32,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        self.create_preset_system_at(template_name, Vec3::new(x, y, z))
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

    /// Native receiving Draw-phase creation. Every authored slave observes the
    /// same explicitly admitted Client frame; prior manager updates do not select it.
    pub(crate) fn create_particle_system_for_client_frame(
        &mut self,
        template: &Arc<ParticleSystemTemplate>,
        create_slaves: bool,
        creation_frame: u32,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        self.create_authored_system_chain(
            template,
            self.next_system_id,
            create_slaves,
            &mut get_game_client_random_value_real,
            creation_frame,
        )
    }

    /// Create a particle system by template name at a given world position.
    ///
    /// Convenience wrapper that looks up the template by name, creates the
    /// system, and sets its initial position.
    pub fn create_particle_system_at(
        &mut self,
        template_name: &str,
        pos: Vec3,
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
        self.create_particle_system_with_id_and_random(
            template,
            system_id,
            create_slaves,
            &mut get_game_client_random_value_real,
        )
    }

    /// Create with this manager's explicit initialization stream. No ambient
    /// client random state is consulted beneath this boundary.
    pub fn create_particle_system_with_id_and_random(
        &mut self,
        template: &Arc<ParticleSystemTemplate>,
        system_id: ParticleSystemId,
        create_slaves: bool,
        random: &mut dyn FnMut(f32, f32) -> f32,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        self.create_authored_system_chain(template, system_id, create_slaves, random, 0)
    }

    pub fn create_particle_system_with_random(
        &mut self,
        template: &Arc<ParticleSystemTemplate>,
        create_slaves: bool,
        random: &mut dyn FnMut(f32, f32) -> f32,
    ) -> Result<ParticleSystemId, ParticleSystemError> {
        self.create_particle_system_with_id_and_random(
            template,
            self.next_system_id,
            create_slaves,
            random,
        )
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
        let mut visited = HashSet::new();
        let mut next = Some(id);
        while let Some(id) = next {
            if !visited.insert(id) {
                break;
            }
            let Some(system) = self.active_systems.get_mut(&id) else {
                break;
            };
            // Preserve C++ parent-before-slave marking without input-sized
            // recursion. Premarked nodes can still have live descendants.
            system.destroy();
            next = system.slave_system_id();
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
        self.update_with_particle_lod(
            local_player_index,
            current_frame,
            &mut AmbientParticleLod,
            current_frame,
            None,
        );
    }

    /// Use this operation's explicitly owned LOD state, including its skip counter.
    /// Other application services (FX enablement and capacity limits) are unchanged.
    pub fn update_with_lod(
        &mut self,
        local_player_index: i32,
        current_frame: u32,
        lod: &mut game_engine::common::ini::GameLODManager,
    ) {
        self.update_with_particle_lod(local_player_index, current_frame, lod, current_frame, None);
    }

    pub(crate) fn update_with_receiving_drawables(
        &mut self,
        local_player_index: i32,
        logic_frame: u32,
        client_frame: u32,
        drawable_pose: &dyn Fn(
            DrawableId,
        )
            -> Option<crate::effects::particle_system::DrawableParticlePose>,
    ) {
        self.update_with_particle_lod(
            local_player_index,
            logic_frame,
            &mut AmbientParticleLod,
            client_frame,
            Some(drawable_pose),
        );
    }

    fn update_with_particle_lod(
        &mut self,
        local_player_index: i32,
        logic_frame: u32,
        lod: &mut dyn ParticleLod,
        client_frame: u32,
        drawable_pose: Option<
            &dyn Fn(DrawableId) -> Option<crate::effects::particle_system::DrawableParticlePose>,
        >,
    ) {
        // CPP ParticleSys2906-2911 deduplicates the actual GameLogic clock.
        // Individual systems/particles observe the separate Client clock.
        if self.last_logic_frame_update == logic_frame {
            return;
        }
        self.last_logic_frame_update = logic_frame;
        self.local_player_index = local_player_index;

        // C++ per system: emit/createParticle then update those particles same frame.
        let mut next_system = 0;
        while next_system < self.active_system_order.len() {
            let id = self.active_system_order[next_system];
            let phase = {
                let system = self
                    .active_systems
                    .get_mut(&id)
                    .expect("particle admission order references a registered system");
                system.prepare_frame_emit_with_drawable_poses(
                    local_player_index,
                    client_frame,
                    drawable_pose,
                )
            };
            // Attachment resolution and direct module destruction can mark the
            // current system without borrowing its descendants. Apply the
            // canonical destroy cascade before natural retirement detaches them.
            let destroyed_slave = {
                let system = self
                    .active_systems
                    .get(&id)
                    .expect("prepared particle system remains admitted");
                system
                    .is_destroyed()
                    .then(|| system.slave_system_id())
                    .flatten()
            };
            if let Some(slave_id) = destroyed_slave {
                self.destroy_particle_system(slave_id);
            }
            match phase {
                crate::effects::particle_system::FrameEmitPhase::Dead => {
                    self.retire_particle_system(id);
                    continue;
                }
                crate::effects::particle_system::FrameEmitPhase::Delayed => {
                    next_system += 1;
                    continue;
                }
                crate::effects::particle_system::FrameEmitPhase::Emitted => {}
            }

            let (cursor, priority, ground_aligned, emit_above_ground, attached_name) = {
                let system = self
                    .active_systems
                    .get_mut(&id)
                    .expect("current particle system remains admitted before burst setup");
                (
                    system.begin_emission_burst(),
                    system.priority(),
                    system.info().is_ground_aligned,
                    system.is_emit_above_ground_only(),
                    system.attached_system_name().to_string(),
                )
            };
            if let Some(mut cursor) = cursor {
                loop {
                    let info = self
                        .active_systems
                        .get_mut(&id)
                        .expect("current particle system remains admitted during burst")
                        .next_emission(&mut cursor);
                    let Some(info) = info else {
                        break;
                    };
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
                        .expect("current particle system remains admitted for budget")
                        .particle_count();
                    if !self.can_create_particle_for_system_with_lod(
                        priority,
                        ground_aligned,
                        system_count,
                        false,
                        lod,
                    ) {
                        continue;
                    }
                    let personality = self
                        .active_systems
                        .get(&id)
                        .expect("current particle system remains admitted for creation")
                        .personality_counter();
                    let particle = crate::effects::particle_system::Particle::new(
                        &info,
                        personality,
                        client_frame,
                    );
                    let position = particle.position;
                    // C++ createParticle admits the parent before attached ctors.
                    let admission = self
                        .admit_particle(id, particle)
                        .expect("particle admission identity available");
                    if !attached_name.is_empty() {
                        if let Some(tmpl) = self.find_template(&attached_name) {
                            let created = if drawable_pose.is_some() {
                                self.create_particle_system_for_client_frame(
                                    &tmpl,
                                    true,
                                    client_frame,
                                )
                            } else {
                                self.create_particle_system(&tmpl, true)
                            };
                            if let Ok(att_id) = created {
                                self.active_systems
                                    .get_mut(&att_id)
                                    .expect("successful attached constructor admits its system")
                                    .set_control_particle_position(position);
                                let parent = self.active_systems.get_mut(&id)
                                    .expect("attached constructor cannot remove its parent system")
                                    .particles_mut().iter_mut()
                                    .find(|particle| particle.admission_ordinal == Some(admission))
                                    .expect("attached constructor cannot remove the admitted parent particle");
                                parent.controlled_system = Some(att_id);
                            }
                        }
                    }
                    self.admit_slave_for_master(id, priority, client_frame, lod);
                }
                self.active_systems
                    .get_mut(&id)
                    .expect("current particle system remains admitted through burst completion")
                    .finish_emission_burst(cursor);
            }

            self.integrate_particle_system(id, client_frame)
                .expect("current system remains admitted through integration");
            self.refresh_controlled_particle_origins(id);
            if self
                .active_systems
                .get(&id)
                .expect("current particle system remains admitted")
                .should_remove()
            {
                self.retire_particle_system(id);
            } else {
                next_system += 1;
            }
        }

        // Update statistics. Field cap is last-frame on-screen count
        // (C++ W3DParticleSys.cpp:124/205), not worldwide living particles.
        debug_assert_eq!(
            self.particle_count,
            self.active_systems
                .values()
                .map(|system| system.particle_count())
                .sum::<usize>()
        );
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

    /// Compatibility boundary; explicit receivers use `update_with_lod`.
    pub fn should_skip_particle(&mut self, priority: ParticlePriorityType) -> bool {
        AmbientParticleLod.should_skip(priority)
    }

    /// Remove oldest particles to make room for new ones (C++ ParticleSys.cpp lines 3177-3199)
    /// Actual removal count is the Rust API result; budget admission separately
    /// preserves the original unsigned-loop return/rejection behavior.
    pub fn remove_oldest_particles(
        &mut self,
        count: usize,
        priority_cap: ParticlePriorityType,
    ) -> usize {
        let mut removed = 0;
        while removed < count {
            let candidate =
                self.active_systems
                    .iter()
                    .flat_map(|(&id, system)| {
                        let priority = system.priority();
                        system.particles().iter().enumerate().filter_map(
                            move |(index, particle)| {
                                (priority as usize >= 1 && priority < priority_cap).then(|| {
                                    let ordinal = particle
                                        .admission_ordinal
                                        .expect("receiver particle has admission identity");
                                    ((priority as usize, ordinal), id, index)
                                })
                            },
                        )
                    })
                    .min_by_key(|(key, _, _)| *key);
            let Some((_, id, index)) = candidate else {
                break;
            };
            self.remove_admitted_particle(id, index);
            removed += 1;
        }
        removed
    }

    /// Compatibility entrypoint using the application's current LOD service.
    pub fn can_create_particle_for_system(
        &mut self,
        priority: ParticlePriorityType,
        is_ground_aligned: bool,
        system_particle_count: usize,
        force_create: bool,
    ) -> bool {
        self.can_create_particle_for_system_with_lod(
            priority,
            is_ground_aligned,
            system_particle_count,
            force_create,
            &mut AmbientParticleLod,
        )
    }

    /// Compatibility entrypoint using the application's current LOD service.
    pub fn can_create_particle(&mut self, priority: ParticlePriorityType) -> bool {
        self.can_create_particle_with_lod(priority, &mut AmbientParticleLod)
    }

    /// Standalone capacity fallbacks; live application limits remain primary.
    pub fn set_fallback_particle_limits(
        &mut self,
        max_particles: usize,
        max_field_particles: usize,
    ) {
        self.max_particle_count = max_particles;
        self.max_field_particle_count = max_field_particles;
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
        // W3DParticleSys.cpp:137-224 skips Drawable/SMUD systems in its
        // point-group path and counts only the first MAX_POINTS_PER_GROUP
        // visible particles. Smudges use their own uncapped visible loop.
        let mut field_count = 0usize;
        self.for_each_particle_system_mut(|system| {
            let is_smudge = super::particle_renderer::system_is_heat_smudge(system);
            // C++ counts in this loop before shader/type GPU dispatch;
            // even an unsupported non-Drawable type takes its ordinary cap.
            let uses_point_group =
                system.info().particle_type != ParticleType::Drawable && !is_smudge;
            let is_field_system = uses_point_group
                && system.priority() == ParticlePriorityType::AreaEffect
                && system.info().is_ground_aligned;
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
                if outside || (uses_point_group && kept >= max_per_system) {
                    particle.is_culled = true;
                } else {
                    particle.is_culled = false;
                    kept += 1;
                    if is_field_system {
                        field_count += 1;
                    }
                }
            }
        });
        self.field_particle_count = field_count;
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
        self.renew_system_namespace();
        self.active_systems.clear();
        self.active_system_order.clear();
        self.next_system_id = 1;
        self.next_particle_ordinal = 1;

        // Reset LOD settings to defaults
        self.max_particle_count = 2500;
        self.max_field_particle_count = 30;

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
        self.renew_system_namespace();
        self.active_systems.clear();
        self.active_system_order.clear();
        self.next_system_id = 1;
        self.next_particle_ordinal = 1;
        self.particle_count = 0;
        self.field_particle_count = 0;
        self.system_count = 0;
        // C++ reset differs from construction: frame zero must run once.
        self.last_logic_frame_update = u32::MAX;
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
    fn to_point3(pos: &gamelogic::common::Coord3D) -> Vec3 {
        Vec3::new(pos.x, pos.y, pos.z)
    }

    fn to_coord3(pos: Vec3) -> gamelogic::common::Coord3D {
        gamelogic::common::Coord3D::new(pos.x, pos.y, pos.z)
    }

    fn mat4_to_matrix3(matrix: &GlamMat4) -> Mat3 {
        let cols = matrix.to_cols_array();
        let data = [
            cols[0], cols[1], cols[2], cols[4], cols[5], cols[6], cols[8], cols[9], cols[10],
        ];
        Mat3::from_cols_array(&data)
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
                    system.set_velocity_multiplier(Vec3::new(
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
    fn set_particle_lifetime_range(
        &self,
        system_id: gamelogic::common::ParticleSystemId,
        min: f32,
        max: f32,
    ) {
        if let Ok(mut manager_guard) = get_particle_system_manager_mut() {
            if let Some(manager) = manager_guard.as_mut() {
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_lifetime_range(min, max);
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

#[path = "particle_manager/state.rs"]
mod state;
pub use state::{
    ParticleSystemRestoreReceipt, capture_live_particle_system_xfer_bytes,
    load_post_process_particle_system_manager_state, restore_live_particle_system_from_xfer_bytes,
    xfer_particle_system_manager_state,
};

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
            manager.admit_particle(system_id, particle).unwrap();
        }
        assert_eq!(manager.particle_count(), 5);
        manager.field_particle_count = 5;

        let removed = manager.remove_oldest_particles(3, ParticlePriorityType::Buildup);

        assert_eq!(removed, 3);
        assert_eq!(manager.particle_count(), 2);
        assert_eq!(manager.field_particle_count(), 5);
        assert_eq!(
            manager
                .find_particle_system(system_id)
                .expect("active system")
                .particle_count(),
            2
        );
    }

    #[test]
    fn can_create_particle_preserves_exact_cap_and_rejects_after_excess_eviction() {
        let mut manager = ParticleSystemManager::new();
        manager.set_fallback_particle_limits(2, 2);

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
            manager.admit_particle(system_id, particle).unwrap();
        }
        assert_eq!(manager.particle_count(), 2);
        manager.field_particle_count = 2;

        // C++ ParticleSys.cpp:1699-1708 admits at the cap. Only an
        // already over-cap population triggers eviction of an older particle.
        assert_eq!(manager.live_max_particle_count(), 2);
        assert!(manager.can_create_particle(ParticlePriorityType::Buildup));
        assert_eq!(manager.particle_count(), 2);
        assert_eq!(
            manager
                .find_particle_system(system_id)
                .unwrap()
                .particle_count(),
            2
        );

        manager
            .admit_particle(
                system_id,
                crate::effects::particle_system::Particle::new(
                    &crate::effects::particle_system::ParticleInfo::default(),
                    2,
                    2,
                ),
            )
            .unwrap();
        assert_eq!(manager.particle_count(), 3);
        manager.field_particle_count = 3;
        // The C++ unsigned post-decrement in removeOldestParticles reports
        // requested + 1, so this incoming particle is rejected after eviction.
        assert!(!manager.can_create_particle(ParticlePriorityType::Buildup));
        assert_eq!(manager.particle_count(), 2);
        let system = manager.find_particle_system(system_id).unwrap();
        assert_eq!(system.particle_count(), 2);
        assert_eq!(system.particles().front().unwrap().create_timestamp, 1);
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
            // C++ Alpha particles with unset alpha keys die as invisible.
            // This fixture observes a visible newborn's lifetime tick.
            info.alpha_keys[0] = RandomKeyframe {
                min_value: 1.0,
                max_value: 1.0,
                frame: 0,
                distribution_type: 0,
            };
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
        assert_eq!(
            system.particle_count(),
            1,
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
            .create_preset_system_at("MediumExplosion", Vec3::new(10.0, 0.0, 20.0))
            .expect("death explosion preset");
        let smoke_id = manager
            .create_preset_system_at("SmokePlume", Vec3::new(10.0, 0.0, 20.0))
            .expect("death smoke preset");
        let muzzle_id = manager
            .create_preset_system_at("MuzzleFlash", Vec3::new(0.0, 0.0, 0.0))
            .expect("muzzle flash preset");

        assert_eq!(manager.active_system_count(), 3);
        assert!(manager.find_particle_system(death_id).is_some());
        assert!(manager.find_particle_system(smoke_id).is_some());
        assert!(manager.find_particle_system(muzzle_id).is_some());

        let death = manager.find_particle_system(death_id).unwrap();
        assert_eq!(death.position(), Vec3::new(10.0, 0.0, 20.0));
        assert!(!death.is_stopped(), "preset system should be started");

        // Unknown preset still fails closed (no silent empty placeholder success).
        assert!(
            manager
                .create_preset_system_at("TotallyUnknownCombatFx", Vec3::ZERO)
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
        // Keep the leftover visible until its finite lifetime expires.
        info.alpha_keys[0].value = 1.0;
        let particle = crate::effects::particle_system::Particle::new(&info, 0, 0);
        manager.admit_particle(system_id, particle).unwrap();
        assert_eq!(manager.particle_count(), 1);

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
                system.set_position(Vec3::new(40.0, 8.0, 12.0));
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
            let mut near = crate::effects::particle_system::Particle::new(
                &crate::effects::particle_system::ParticleInfo::default(),
                0,
                0,
            );
            near.lifetime_left = 10;
            near.position = Vec3::new(0.0, 0.0, 0.0);
            near.size = 1.0;
            manager.admit_particle(system_id, near).unwrap();
            let mut far = crate::effects::particle_system::Particle::new(
                &crate::effects::particle_system::ParticleInfo::default(),
                0,
                0,
            );
            far.lifetime_left = 10;
            far.position = Vec3::new(1000.0, 0.0, 0.0);
            far.size = 1.0;
            manager.admit_particle(system_id, far).unwrap();
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
            let mut near = crate::effects::particle_system::Particle::new(
                &crate::effects::particle_system::ParticleInfo::default(),
                0,
                0,
            );
            near.lifetime_left = 10;
            near.position = Vec3::new(0.0, 0.0, 0.0);
            near.size = 1.0;
            manager.admit_particle(system_id, near).unwrap();
            let mut far = crate::effects::particle_system::Particle::new(
                &crate::effects::particle_system::ParticleInfo::default(),
                0,
                0,
            );
            far.lifetime_left = 10;
            far.position = Vec3::new(1000.0, 0.0, 0.0);
            far.size = 1.0;
            manager.admit_particle(system_id, far).unwrap();
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

#[cfg(test)]
#[path = "particle_draw_owner_tests.rs"]
mod particle_draw_owner_tests;

#[path = "particle_manager/slave_factory.rs"]
mod slave_factory;

#[path = "particle_manager/emission.rs"]
mod emission;

#[cfg(test)]
#[path = "particle_manager/reset_clock_tests.rs"]
mod reset_clock_tests;

#[cfg(test)]
#[path = "particle_manager/destroy_traversal_tests.rs"]
mod destroy_traversal_tests;

#[path = "particle_manager/admission.rs"]
mod admission;

#[path = "particle_manager/lod.rs"]
mod lod;
use lod::{AmbientParticleLod, ParticleLod};

#[cfg(test)]
#[path = "particle_manager/field_count_tests.rs"]
mod field_count_tests;
