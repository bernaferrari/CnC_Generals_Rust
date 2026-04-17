//! # FXList Integration with Particle System
//!
//! Bridges the gap between the FXList system (from C++ GameClient/FXList.cpp)
//! and the modern Rust particle system. Allows FXLists to spawn particle systems
//! and coordinate complex visual effects.
//!
//! This matches the C++ behavior where FXLists can contain ParticleSystemFXNuggets
//! that create and manage particle systems as part of larger effect sequences.
//!
//! FX Nugget types (matching C++ FXList.cpp):
//! - SoundFXNugget: Play audio events
//! - TracerFXNugget: Create tracer drawables between positions
//! - RayEffectFXNugget: Create ray effects (lasers, beams)
//! - LightPulseFXNugget: Create light pulses
//! - ViewShakeFXNugget: Trigger camera shake
//! - TerrainScorchFXNugget: Add scorch marks to terrain
//! - ParticleSystemFXNugget: Spawn particle systems
//! - FXListAtBonePosFXNugget: Execute FX at bone positions

use super::decals::{DecalManager, DecalSettings, DecalType};
use super::particle_manager::*;
use super::particle_presets;
use super::ray_effects::{RayEffectConfig, RayEffectManager};
use nalgebra::{Matrix3, Point3, Rotation3, Vector3};
use std::collections::HashMap;
use std::sync::Arc;

/// FX nugget trait - all FX nuggets implement this
pub trait FXNugget: Send + Sync {
    /// Execute FX at a position (matches C++ doFXPos)
    fn do_fx_pos(
        &self,
        primary: Point3<f32>,
        primary_mtx: Option<&Matrix3<f32>>,
        primary_speed: f32,
        secondary: Option<Point3<f32>>,
        override_radius: f32,
        context: &mut FXContext,
    );

    /// Execute FX on objects (matches C++ doFXObj)
    fn do_fx_obj(
        &self,
        primary_pos: Option<Point3<f32>>,
        primary_mtx: Option<&Matrix3<f32>>,
        secondary_pos: Option<Point3<f32>>,
        context: &mut FXContext,
    ) {
        // Default: delegate to do_fx_pos
        if let Some(pos) = primary_pos {
            self.do_fx_pos(pos, primary_mtx, 0.0, secondary_pos, 0.0, context);
        }
    }
}

/// Context passed to FX nuggets during execution
pub struct FXContext<'a> {
    pub particle_manager: &'a mut ParticleSystemManager,
    pub ray_effect_manager: Option<&'a mut RayEffectManager>,
    pub decal_manager: Option<&'a mut DecalManager>,
    pub current_frame: u32,
    pub local_player_index: i32,
}

/// Sound FX nugget - plays audio events (matches C++ SoundFXNugget)
pub struct SoundFXNugget {
    pub sound_name: String,
}

impl SoundFXNugget {
    pub fn new(sound_name: String) -> Self {
        Self { sound_name }
    }
}

impl FXNugget for SoundFXNugget {
    fn do_fx_pos(
        &self,
        primary: Point3<f32>,
        _primary_mtx: Option<&Matrix3<f32>>,
        _primary_speed: f32,
        _secondary: Option<Point3<f32>>,
        _override_radius: f32,
        _context: &mut FXContext,
    ) {
        // Audio playback would be triggered here via TheAudio->addAudioEvent
        // For now, we log the sound event
        log::debug!(
            "SoundFX: {} at ({}, {}, {})",
            self.sound_name,
            primary.x,
            primary.y,
            primary.z
        );
    }
}

/// Tracer FX nugget - creates tracer effects between positions (matches C++ TracerFXNugget)
pub struct TracerFXNugget {
    pub tracer_name: String,
    pub bone_name: String,
    pub speed: f32,
    pub decay_at: f32,
    pub length: f32,
    pub width: f32,
    pub color: [f32; 3],
    pub probability: f32,
}

impl Default for TracerFXNugget {
    fn default() -> Self {
        Self {
            tracer_name: "GenericTracer".to_string(),
            bone_name: String::new(),
            speed: 0.0,
            decay_at: 1.0,
            length: 10.0,
            width: 1.0,
            color: [1.0, 1.0, 1.0],
            probability: 1.0,
        }
    }
}

impl TracerFXNugget {
    pub fn new(tracer_name: String) -> Self {
        Self {
            tracer_name,
            ..Default::default()
        }
    }
}

impl FXNugget for TracerFXNugget {
    fn do_fx_pos(
        &self,
        primary: Point3<f32>,
        _primary_mtx: Option<&Matrix3<f32>>,
        primary_speed: f32,
        secondary: Option<Point3<f32>>,
        _override_radius: f32,
        context: &mut FXContext,
    ) {
        // Probability check (matches C++ line 151)
        if self.probability <= rand::random::<f32>() {
            return;
        }

        if let Some(sec_pos) = secondary {
            // Calculate direction and distance
            let dir = sec_pos - primary;
            let dist = dir.norm();
            let speed = if self.speed > 0.0 {
                self.speed
            } else {
                primary_speed
            };

            // Estimate frames to reach destination
            let adjusted_dist = dist - self.length;
            let frames = if adjusted_dist >= 0.0 && speed >= 0.0 {
                (adjusted_dist / speed * self.decay_at) as u32
            } else {
                1
            };

            // Create a tracer particle system
            if let Some(template) = context.particle_manager.find_template(&self.tracer_name) {
                if let Ok(system_id) = context
                    .particle_manager
                    .create_particle_system(&template, false)
                {
                    if let Some(system) =
                        context.particle_manager.find_particle_system_mut(system_id)
                    {
                        system.set_position(primary);
                        system.start();
                    }
                }
            }
        }
    }
}

/// Ray Effect FX nugget (matches C++ RayEffectFXNugget)
pub struct RayEffectFXNugget {
    pub template_name: String,
    pub primary_offset: Vector3<f32>,
    pub secondary_offset: Vector3<f32>,
}

impl Default for RayEffectFXNugget {
    fn default() -> Self {
        Self {
            template_name: String::new(),
            primary_offset: Vector3::zeros(),
            secondary_offset: Vector3::zeros(),
        }
    }
}

impl RayEffectFXNugget {
    pub fn new(template_name: String) -> Self {
        Self {
            template_name,
            ..Default::default()
        }
    }
}

impl FXNugget for RayEffectFXNugget {
    fn do_fx_pos(
        &self,
        primary: Point3<f32>,
        _primary_mtx: Option<&Matrix3<f32>>,
        _primary_speed: f32,
        secondary: Option<Point3<f32>>,
        _override_radius: f32,
        context: &mut FXContext,
    ) {
        if let (Some(sec_pos), Some(ray_mgr)) = (secondary, context.ray_effect_manager.as_mut()) {
            let source_pos = primary + self.primary_offset;
            let target_pos = sec_pos + self.secondary_offset;

            let config = RayEffectConfig::default().between(source_pos, target_pos);

            ray_mgr.spawn(config);
        }
    }
}

/// Light Pulse FX nugget (matches C++ LightPulseFXNugget)
pub struct LightPulseFXNugget {
    pub color: [f32; 3],
    pub radius: f32,
    pub bounding_circle_pct: f32,
    pub increase_frames: u32,
    pub decrease_frames: u32,
}

impl Default for LightPulseFXNugget {
    fn default() -> Self {
        Self {
            color: [0.0, 0.0, 0.0],
            radius: 0.0,
            bounding_circle_pct: 0.0,
            increase_frames: 0,
            decrease_frames: 0,
        }
    }
}

impl FXNugget for LightPulseFXNugget {
    fn do_fx_pos(
        &self,
        primary: Point3<f32>,
        _primary_mtx: Option<&Matrix3<f32>>,
        _primary_speed: f32,
        _secondary: Option<Point3<f32>>,
        _override_radius: f32,
        _context: &mut FXContext,
    ) {
        // Light pulse would be created via TheDisplay->createLightPulse
        log::debug!(
            "LightPulse: at ({}, {}, {}) radius={} color=({}, {}, {})",
            primary.x,
            primary.y,
            primary.z,
            self.radius,
            self.color[0],
            self.color[1],
            self.color[2]
        );
    }
}

/// View Shake FX nugget (matches C++ ViewShakeFXNugget)
pub struct ViewShakeFXNugget {
    pub shake_type: ShakeType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShakeType {
    Subtle,
    Normal,
    Strong,
    Severe,
    CineExtreme,
    CineInsane,
}

impl Default for ViewShakeFXNugget {
    fn default() -> Self {
        Self {
            shake_type: ShakeType::Normal,
        }
    }
}

impl FXNugget for ViewShakeFXNugget {
    fn do_fx_pos(
        &self,
        primary: Point3<f32>,
        _primary_mtx: Option<&Matrix3<f32>>,
        _primary_speed: f32,
        _secondary: Option<Point3<f32>>,
        _override_radius: f32,
        _context: &mut FXContext,
    ) {
        log::debug!(
            "ViewShake: {:?} at ({}, {}, {})",
            self.shake_type,
            primary.x,
            primary.y,
            primary.z
        );
    }
}

/// Terrain Scorch FX nugget (matches C++ TerrainScorchFXNugget)
pub struct TerrainScorchFXNugget {
    pub scorch_type: ScorchType,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScorchType {
    Scorch1,
    Scorch2,
    Scorch3,
    Scorch4,
    ShadowScorch,
    Random,
}

impl Default for TerrainScorchFXNugget {
    fn default() -> Self {
        Self {
            scorch_type: ScorchType::Random,
            radius: 0.0,
        }
    }
}

impl FXNugget for TerrainScorchFXNugget {
    fn do_fx_pos(
        &self,
        primary: Point3<f32>,
        _primary_mtx: Option<&Matrix3<f32>>,
        _primary_speed: f32,
        _secondary: Option<Point3<f32>>,
        _override_radius: f32,
        context: &mut FXContext,
    ) {
        // Determine scorch type (random if specified, matches C++ lines 428-431)
        let scorch_type = match self.scorch_type {
            ScorchType::Random => {
                // Random between SCORCH_1 and SCORCH_4
                let rand_val = rand::random::<u8>() % 4;
                match rand_val {
                    0 => DecalType::Scorch,
                    1 => DecalType::Scorch,
                    2 => DecalType::Scorch,
                    _ => DecalType::Scorch,
                }
            }
            _ => DecalType::Scorch,
        };

        // Create scorch decal
        if let Some(decal_mgr) = context.decal_manager.as_mut() {
            let settings = DecalSettings::scorch_mark(primary, self.radius);
            decal_mgr.create_decal(settings);
        }
    }
}

/// FXList at bone position nugget (matches C++ FXListAtBonePosFXNugget)
pub struct FXListAtBonePosFXNugget {
    pub fx_list: Option<Arc<FXList>>,
    pub bone_name: String,
    pub orient_to_bone: bool,
}

impl FXListAtBonePosFXNugget {
    pub fn new(fx_list: Arc<FXList>, bone_name: String) -> Self {
        Self {
            fx_list: Some(fx_list),
            bone_name,
            orient_to_bone: true,
        }
    }
}

impl FXNugget for FXListAtBonePosFXNugget {
    fn do_fx_pos(
        &self,
        _primary: Point3<f32>,
        _primary_mtx: Option<&Matrix3<f32>>,
        _primary_speed: f32,
        _secondary: Option<Point3<f32>>,
        _override_radius: f32,
        _context: &mut FXContext,
    ) {
        // C++ FXListAtBonePosFXNugget::doFXPos crashes with DEBUG_CRASH.
        // Position form cannot resolve bone positions without an object/drawable.
        // This matches C++ behavior: "You must use the object form for this effect"
    }

    fn do_fx_obj(
        &self,
        primary_pos: Option<Point3<f32>>,
        primary_mtx: Option<&Matrix3<f32>>,
        _secondary_pos: Option<Point3<f32>>,
        context: &mut FXContext,
    ) {
        if let (Some(pos), Some(fx_list)) = (primary_pos, &self.fx_list) {
            // C++ FXListAtBonePosFXNugget::doFxAtBones:
            // First tries unadorned bone name, then 01,02,03... variants.
            // Bone position resolution requires drawable bone queries, which
            // are not yet threaded through FXContext. For now, execute the
            // nested FXList at the primary position (matching fallback behavior).
            fx_list.execute_fx_pos(pos, primary_mtx, 0.0, None, 0.0, context);

            // TODO: When drawable bone queries are integrated with FXContext,
            // implement full doFxAtBones matching C++ lines 711-728.
        }
    }
}

/// FXList - a collection of FX nuggets executed in order (matches C++ FXList)
pub struct FXList {
    pub nuggets: Vec<Arc<dyn FXNugget>>,
}

impl FXList {
    pub fn new() -> Self {
        Self {
            nuggets: Vec::new(),
        }
    }

    pub fn add_nugget(&mut self, nugget: Arc<dyn FXNugget>) {
        self.nuggets.push(nugget);
    }

    pub fn clear(&mut self) {
        self.nuggets.clear();
    }

    /// Execute FX at a position (matches C++ FXList::doFXPos)
    pub fn execute_fx_pos(
        &self,
        primary: Point3<f32>,
        primary_mtx: Option<&Matrix3<f32>>,
        primary_speed: f32,
        secondary: Option<Point3<f32>>,
        override_radius: f32,
        context: &mut FXContext,
    ) {
        for nugget in &self.nuggets {
            nugget.do_fx_pos(
                primary,
                primary_mtx,
                primary_speed,
                secondary,
                override_radius,
                context,
            );
        }
    }

    /// Execute FX on objects (matches C++ FXList::doFXObj)
    pub fn execute_fx_obj(
        &self,
        primary_pos: Option<Point3<f32>>,
        primary_mtx: Option<&Matrix3<f32>>,
        secondary_pos: Option<Point3<f32>>,
        context: &mut FXContext,
    ) {
        for nugget in &self.nuggets {
            nugget.do_fx_obj(primary_pos, primary_mtx, secondary_pos, context);
        }
    }
}

impl Default for FXList {
    fn default() -> Self {
        Self::new()
    }
}

/// FXList Store - manages all FXLists (matches C++ FXListStore)
pub struct FXListStore {
    pub fx_lists: HashMap<String, Arc<FXList>>,
}

impl FXListStore {
    pub fn new() -> Self {
        Self {
            fx_lists: HashMap::new(),
        }
    }

    pub fn find_fx_list(&self, name: &str) -> Option<&Arc<FXList>> {
        if name.eq_ignore_ascii_case("None") {
            return None;
        }
        self.fx_lists.get(name)
    }

    pub fn insert_fx_list(&mut self, name: String, fx_list: FXList) {
        self.fx_lists.insert(name, Arc::new(fx_list));
    }

    pub fn clear(&mut self) {
        self.fx_lists.clear();
    }
}

impl Default for FXListStore {
    fn default() -> Self {
        Self::new()
    }
}

/// FX nugget that spawns a particle system
/// Matches C++ ParticleSystemFXNugget from FXList.cpp:481-658
pub struct ParticleSystemFXNugget {
    /// Particle system template name
    pub template_name: String,

    /// Number of systems to spawn
    pub count: i32,

    /// Offset from primary position
    pub offset: Vector3<f32>,

    /// Random radius distribution
    pub radius: GameClientRandomVariable,

    /// Random height variation
    pub height: GameClientRandomVariable,

    /// Delay before spawning (frames)
    pub delay: GameClientRandomVariable,

    /// Rotation around axes
    pub rotate_x: f32,
    pub rotate_y: f32,
    pub rotate_z: f32,

    /// Orientation flags
    pub orient_to_object: bool,
    pub ricochet: bool,
    pub attach_to_object: bool,
    pub create_at_ground_height: bool,
    pub use_callers_radius: bool,
}

impl Default for ParticleSystemFXNugget {
    fn default() -> Self {
        Self {
            template_name: String::new(),
            count: 1,
            offset: Vector3::zeros(),
            radius: GameClientRandomVariable::new(0.0, 0.0),
            height: GameClientRandomVariable::new(0.0, 0.0),
            delay: GameClientRandomVariable::new(-1.0, -1.0),
            rotate_x: 0.0,
            rotate_y: 0.0,
            rotate_z: 0.0,
            orient_to_object: false,
            ricochet: false,
            attach_to_object: false,
            create_at_ground_height: false,
            use_callers_radius: false,
        }
    }
}

impl ParticleSystemFXNugget {
    /// Create a new particle system FX nugget
    pub fn new(template_name: String) -> Self {
        Self {
            template_name,
            ..Default::default()
        }
    }

    /// Execute the FX at a position
    /// Matches C++ ParticleSystemFXNugget::doFXPos
    pub fn do_fx_pos(
        &self,
        primary: Point3<f32>,
        primary_mtx: Option<&Matrix3<f32>>,
        override_radius: f32,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        self.really_do_fx(primary, primary_mtx, None, override_radius, manager)
    }

    /// Execute the FX attached to an object
    /// Matches C++ ParticleSystemFXNugget::doFXObj
    pub fn do_fx_obj(
        &self,
        primary: Point3<f32>,
        primary_mtx: Option<&Matrix3<f32>>,
        object_id: Option<ObjectId>,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        self.really_do_fx(primary, primary_mtx, object_id, 0.0, manager)
    }

    /// Actually create the particle systems
    /// Matches C++ ParticleSystemFXNugget::reallyDoFX (lines 570-641)
    fn really_do_fx(
        &self,
        primary: Point3<f32>,
        mtx: Option<&Matrix3<f32>>,
        object_id: Option<ObjectId>,
        override_radius: f32,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        let mut created_systems = Vec::new();

        // Find or create template
        let template =
            if let Some(preset) = particle_presets::get_preset_by_name(&self.template_name) {
                preset
            } else if let Some(tmpl) = manager.find_template(&self.template_name) {
                tmpl
            } else {
                // Template not found, return empty
                return created_systems;
            };

        // Apply offset with matrix transformation
        let mut offset = self.offset;
        if let Some(matrix) = mtx {
            offset = matrix * offset;
        }

        // Create multiple systems based on count
        for _ in 0..self.count {
            let radius = self.radius.sample();
            let height_offset = self.height.sample();
            let angle = rand::random::<f32>() * 2.0 * std::f32::consts::PI;

            let mut spawn_pos = Point3::new(
                primary.x + offset.x + radius * angle.cos(),
                primary.y + offset.y + radius * angle.sin(),
                primary.z + offset.z + height_offset,
            );

            // Ground height adjustment
            if self.create_at_ground_height {
                // C++ parity intent: clamp to terrain. Until terrain query is threaded through
                // this FX path, keep the caller-provided ground plane instead of forcing 0.0.
                spawn_pos.z = primary.z + offset.z + height_offset;
            }

            // Create particle system
            let result = if let Some(obj_id) = object_id {
                if self.attach_to_object {
                    manager.create_attached_particle_system(&template, obj_id, true)
                } else {
                    manager.create_particle_system(&template, true)
                }
            } else {
                manager.create_particle_system(&template, true)
            };

            if let Ok(system_id) = result {
                // Set system position
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    system.set_position(spawn_pos);

                    // Apply rotation if specified
                    if self.rotate_x != 0.0 || self.rotate_y != 0.0 || self.rotate_z != 0.0 {
                        let rotation_matrix = Rotation3::from_euler_angles(
                            self.rotate_x,
                            self.rotate_y,
                            self.rotate_z,
                        )
                        .into_inner();
                        system.set_local_transform(rotation_matrix);
                    }

                    // Apply caller's radius if requested
                    if override_radius > 0.0 && self.use_callers_radius {
                        match system.get_emission_volume_type() {
                            EmissionVolumeType::Sphere => {
                                system.set_emission_volume_sphere_radius(override_radius);
                            }
                            EmissionVolumeType::Cylinder => {
                                system.set_emission_volume_cylinder_radius(override_radius);
                            }
                            _ => {}
                        }
                    }

                    // Start the system
                    system.start();
                }

                created_systems.push(system_id);
            }
        }

        created_systems
    }
}

/// FXList bridge for particle effects
/// Allows FXLists to create and manage particle systems
pub struct FXListParticleBridge {
    /// Registered FX nuggets by name
    nuggets: HashMap<String, ParticleSystemFXNugget>,

    /// Active particle systems spawned by FXLists
    active_systems: Vec<ParticleSystemId>,
}

impl FXListParticleBridge {
    pub fn new() -> Self {
        Self {
            nuggets: HashMap::new(),
            active_systems: Vec::new(),
        }
    }

    /// Register a particle system FX nugget
    pub fn register_nugget(&mut self, name: String, nugget: ParticleSystemFXNugget) {
        self.nuggets.insert(name, nugget);
    }

    /// Execute FX by name
    pub fn execute_fx(
        &mut self,
        name: &str,
        position: Point3<f32>,
        transform: Option<&Matrix3<f32>>,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        if let Some(nugget) = self.nuggets.get(name) {
            let systems = nugget.do_fx_pos(position, transform, 0.0, manager);
            self.active_systems.extend(systems.clone());
            systems
        } else {
            Vec::new()
        }
    }

    /// Execute FX with object attachment
    pub fn execute_fx_on_object(
        &mut self,
        name: &str,
        position: Point3<f32>,
        transform: Option<&Matrix3<f32>>,
        object_id: ObjectId,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        if let Some(nugget) = self.nuggets.get(name) {
            let systems = nugget.do_fx_obj(position, transform, Some(object_id), manager);
            self.active_systems.extend(systems.clone());
            systems
        } else {
            Vec::new()
        }
    }

    /// Clean up finished systems
    pub fn cleanup_finished_systems(&mut self, manager: &mut ParticleSystemManager) {
        self.active_systems.retain(|&system_id| {
            if let Some(system) = manager.find_particle_system(system_id) {
                !system.is_destroyed() && system.particle_count() > 0
            } else {
                false
            }
        });
    }

    /// Get active system count
    pub fn active_system_count(&self) -> usize {
        self.active_systems.len()
    }
}

impl Default for FXListParticleBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for common FX operations
pub mod helpers {
    use super::*;

    /// Create explosion FX at position
    pub fn create_explosion_at(
        position: Point3<f32>,
        explosion_type: &str,
        manager: &mut ParticleSystemManager,
    ) -> Option<ParticleSystemId> {
        let template = particle_presets::get_preset_by_name(explosion_type)?;
        let system_id = manager.create_particle_system(&template, true).ok()?;

        if let Some(system) = manager.find_particle_system_mut(system_id) {
            system.set_position(position);
            system.trigger(); // Immediate burst
        }

        Some(system_id)
    }

    /// Create weapon fire FX (muzzle flash + smoke)
    pub fn create_weapon_fire_fx(
        muzzle_position: Point3<f32>,
        muzzle_direction: Vector3<f32>,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        let mut systems = Vec::new();

        // Muzzle flash
        if let Some(flash_template) = particle_presets::get_preset_by_name("MuzzleFlash") {
            if let Ok(flash_id) = manager.create_particle_system(&flash_template, false) {
                if let Some(flash_system) = manager.find_particle_system_mut(flash_id) {
                    flash_system.set_position(muzzle_position);
                    flash_system.trigger();
                    systems.push(flash_id);
                }
            }
        }

        // Shell casing smoke
        if let Some(smoke_template) = particle_presets::get_preset_by_name("ShellCasingSmoke") {
            if let Ok(smoke_id) = manager.create_particle_system(&smoke_template, false) {
                if let Some(smoke_system) = manager.find_particle_system_mut(smoke_id) {
                    // Offset slightly to side for ejection
                    let side_offset = Vector3::new(-muzzle_direction.y, muzzle_direction.x, 0.0)
                        .normalize()
                        * 2.0;
                    smoke_system.set_position(muzzle_position + side_offset);
                    smoke_system.trigger();
                    systems.push(smoke_id);
                }
            }
        }

        systems
    }

    /// Create building destruction FX
    pub fn create_building_destruction_fx(
        building_center: Point3<f32>,
        building_size: f32,
        manager: &mut ParticleSystemManager,
    ) -> Vec<ParticleSystemId> {
        let mut systems = Vec::new();

        // Main explosion
        if let Some(explosion) = create_explosion_at(building_center, "LargeExplosion", manager) {
            systems.push(explosion);
        }

        // Collapse dust
        if let Some(dust_template) = particle_presets::get_preset_by_name("BuildingCollapseDust") {
            if let Ok(dust_id) = manager.create_particle_system(&dust_template, false) {
                if let Some(dust_system) = manager.find_particle_system_mut(dust_id) {
                    dust_system.set_position(building_center);
                    dust_system.trigger();
                    systems.push(dust_id);
                }
            }
        }

        // Debris
        if let Some(debris_template) = particle_presets::get_preset_by_name("BuildingDebris") {
            if let Ok(debris_id) = manager.create_particle_system(&debris_template, false) {
                if let Some(debris_system) = manager.find_particle_system_mut(debris_id) {
                    debris_system.set_position(building_center);
                    debris_system.trigger();
                    systems.push(debris_id);
                }
            }
        }

        systems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_system_fx_nugget() {
        let nugget = ParticleSystemFXNugget::new("SmallExplosion".to_string());
        assert_eq!(nugget.template_name, "SmallExplosion");
        assert_eq!(nugget.count, 1);
    }

    #[test]
    fn test_fxlist_bridge() {
        let mut bridge = FXListParticleBridge::new();
        let nugget = ParticleSystemFXNugget::new("MuzzleFlash".to_string());

        bridge.register_nugget("TestFX".to_string(), nugget);
        assert_eq!(bridge.active_system_count(), 0);
    }

    #[test]
    fn test_explosion_helper() {
        let mut manager = ParticleSystemManager::new();
        let position = Point3::new(100.0, 200.0, 0.0);

        let system_id = helpers::create_explosion_at(position, "SmallExplosion", &mut manager);
        assert!(system_id.is_some());

        if let Some(id) = system_id {
            let system = manager.find_particle_system(id);
            assert!(system.is_some());
        }
    }

    #[test]
    fn test_weapon_fire_helper() {
        let mut manager = ParticleSystemManager::new();
        let muzzle_pos = Point3::new(10.0, 20.0, 5.0);
        let muzzle_dir = Vector3::new(1.0, 0.0, 0.0);

        let systems = helpers::create_weapon_fire_fx(muzzle_pos, muzzle_dir, &mut manager);
        assert!(!systems.is_empty());
    }

    #[test]
    fn test_building_destruction_helper() {
        let mut manager = ParticleSystemManager::new();
        let building_center = Point3::new(50.0, 50.0, 0.0);

        let systems = helpers::create_building_destruction_fx(building_center, 20.0, &mut manager);
        assert!(!systems.is_empty());
    }
}
