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

use super::decals::DecalManager;
use super::particle_manager::*;
use super::particle_presets;
use super::ray_effects::{RayEffectConfig, RayEffectManager};
use nalgebra::{Matrix3, Point3, Vector3};
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
    pub bone_query: Option<&'a dyn FXBoneQuery>,
    pub current_frame: u32,
    pub local_player_index: i32,
}

/// Current client bone transform resolved from the primary object/drawable.
#[derive(Clone, Copy, Debug)]
pub struct FXBoneTransform {
    pub position: Point3<f32>,
    pub transform: Matrix3<f32>,
}

/// Adapter for C++ `Drawable::getCurrentClientBonePositions`.
pub trait FXBoneQuery {
    fn current_client_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: usize,
        max_bones: usize,
    ) -> Vec<FXBoneTransform>;
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
        // Matches C++ SoundFXNugget::doFXPos (FXList.cpp lines 78-88):
        //   AudioEventRTS sound(m_soundName);
        //   if (primary) sound.setPosition(primary);
        //   TheAudio->addAudioEvent(&sound);
        game_engine::common::audio::gameplay_audio_dispatch::dispatch_positional_sound(
            &self.sound_name,
            primary.x,
            primary.y,
            primary.z,
        );
    }

    fn do_fx_obj(
        &self,
        primary_pos: Option<Point3<f32>>,
        _primary_mtx: Option<&Matrix3<f32>>,
        _secondary_pos: Option<Point3<f32>>,
        _context: &mut FXContext,
    ) {
        // C++ SoundFXNugget::doFXObj (FXList.cpp:90-99): setPlayerIndex + setPosition.
        // Leftover integration has no Object; the live GameClient runner sets
        // the controlling-player index. Position-only here still hits TheAudio.
        use game_engine::common::audio::audio_event_rts::{AudioEventRts, Coord3D as AudioCoord3D};
        use game_engine::common::audio::game_audio::{
            get_global_audio_manager, initialize_global_audio_manager,
        };

        if self.sound_name.is_empty() {
            return;
        }
        let mut event = AudioEventRts::with_event_name(&self.sound_name);
        if let Some(pos) = primary_pos {
            event.set_position(&AudioCoord3D {
                x: pos.x,
                y: pos.y,
                z: pos.z,
            });
        }
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        if let Ok(mut manager) = manager.lock() {
            let _ = manager.add_audio_event(&event);
        }
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
        // Probability check (matches C++ FXList.cpp:150-151) — client stream.
        if self.probability <= crate::GameClientRandomValueReal!(0.0, 1.0) {
            return;
        }

        if let Some(sec_pos) = secondary {
            let speed = if self.speed == 0.0 {
                primary_speed
            } else {
                self.speed
            };
            let _ = crate::effects::tracer_fx::spawn_tracer_drawable_like_cpp(
                &self.tracer_name,
                [primary.x, primary.y, primary.z],
                [sec_pos.x, sec_pos.y, sec_pos.z],
                speed,
                self.length,
                self.width,
                self.color,
                self.decay_at,
                context.current_frame,
            );
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
        if let Some(sec_pos) = secondary {
            let source_pos = primary + self.primary_offset;
            let target_pos = sec_pos + self.secondary_offset;
            let _ = crate::effects::ray_effect_system::create_ray_effect_by_template(
                [source_pos.x, source_pos.y, source_pos.z],
                [target_pos.x, target_pos.y, target_pos.z],
                &self.template_name,
            );
            if let Some(ray_mgr) = context.ray_effect_manager.as_mut() {
                let config = RayEffectConfig::default().between(source_pos, target_pos);
                ray_mgr.spawn(config);
            }
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
        // C++ LightPulseFXNugget::doFXPos → TheDisplay->createLightPulse(pos, color, 1, radius, ...)
        let _ = crate::fx_list::create_display_light_pulse(crate::fx_list::DisplayLightPulse {
            pos: [primary.x, primary.y, primary.z],
            color: self.color,
            inner_radius: 1.0,
            outer_radius: self.radius,
            increase_frames: self.increase_frames,
            decay_frames: self.decrease_frames,
        });
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
        // C++ ViewShakeFXNugget::doFXPos → TheTacticalView->shake(primary, type)
        let kind = match self.shake_type {
            ShakeType::Subtle => crate::display::view::CameraShakeType::Subtle,
            ShakeType::Normal => crate::display::view::CameraShakeType::Normal,
            ShakeType::Strong => crate::display::view::CameraShakeType::Strong,
            ShakeType::Severe => crate::display::view::CameraShakeType::Severe,
            ShakeType::CineExtreme => crate::display::view::CameraShakeType::CineExtreme,
            ShakeType::CineInsane => crate::display::view::CameraShakeType::CineInsane,
        };
        crate::display::view::with_tactical_view(|view| {
            view.shake(
                &crate::display::view::Point3::new(primary.x, primary.y, primary.z),
                kind,
            );
        });
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
        // C++ TerrainScorchFXNugget::doFXPos → TheGameClient->addScorch(pos, radius, type)
        let scorch_idx = match self.scorch_type {
            ScorchType::Scorch1 => 0,
            ScorchType::Scorch2 => 1,
            ScorchType::Scorch3 => 2,
            ScorchType::Scorch4 => 3,
            ScorchType::ShadowScorch => 4,
            ScorchType::Random => crate::terrain::scorch_mesh::resolve_scorch_type(-1),
        };
        let _ = crate::terrain::scorch_mesh::add_terrain_scorch(
            [primary.x, primary.y, primary.z],
            self.radius,
            scorch_idx,
        );
    }
}

/// FXList at bone position nugget (matches C++ FXListAtBonePosFXNugget)
pub struct FXListAtBonePosFXNugget {
    pub fx_list: Option<Arc<FXList>>,
    pub bone_name: String,
    pub orient_to_bone: bool,
}

impl FXListAtBonePosFXNugget {
    const MAX_BONE_POINTS: usize = 40;

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
        let Some(fx_list) = &self.fx_list else {
            return;
        };

        if let Some(query) = context.bone_query {
            for start_index in [0, 1] {
                let bones = query.current_client_bone_positions(
                    &self.bone_name,
                    start_index,
                    Self::MAX_BONE_POINTS,
                );
                for bone in bones {
                    fx_list.execute_fx_pos(
                        bone.position,
                        Some(&bone.transform),
                        0.0,
                        None,
                        0.0,
                        context,
                    );
                }
            }
            return;
        }

        if let Some(pos) = primary_pos {
            fx_list.execute_fx_pos(pos, primary_mtx, 0.0, None, 0.0, context);
        }
    }
}

/// C++ `FXList::doFXPos` (FXList.cpp:784) plays only when
/// `ThePartitionManager->getShroudStatusForPlayer(localPlayer, primary) == CELLSHROUD_CLEAR`.
/// `PartitionManager.cpp:3017-3023` returns `CELLSHROUD_SHROUDED` for
/// `playerIndex < 0` or a missing cell (including an uninitialized grid).
fn fx_pos_cell_is_clear(primary: Point3<f32>, local_player_index: i32) -> bool {
    if local_player_index < 0 {
        return false;
    }
    let position = gamelogic::common::Coord3D {
        x: primary.x,
        y: primary.y,
        z: primary.z,
    };
    let Ok(shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() else {
        return false;
    };
    matches!(
        shroud.get_shroud_state(local_player_index as u32, &position),
        gamelogic::system::shroud_manager::ShroudState::Visible
    )
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

    /// Execute FX at a position (matches C++ FXList::doFXPos).
    ///
    /// C++ `FXList.cpp:784` skips every nugget unless
    /// `ThePartitionManager->getShroudStatusForPlayer(local, primary) == CELLSHROUD_CLEAR`.
    pub fn execute_fx_pos(
        &self,
        primary: Point3<f32>,
        primary_mtx: Option<&Matrix3<f32>>,
        primary_speed: f32,
        secondary: Option<Point3<f32>>,
        override_radius: f32,
        context: &mut FXContext,
    ) {
        if !fx_pos_cell_is_clear(primary, context.local_player_index) {
            return;
        }
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

    /// Execute FX on objects (matches C++ FXList::doFXObj).
    ///
    /// C++ `FXList.cpp:794-797` uses object `OBJECTSHROUD_PARTIAL_CLEAR`, not
    /// the cell `CELLSHROUD_CLEAR` gate. Invalid local player is fail-closed.
    pub fn execute_fx_obj(
        &self,
        primary_pos: Option<Point3<f32>>,
        primary_mtx: Option<&Matrix3<f32>>,
        secondary_pos: Option<Point3<f32>>,
        context: &mut FXContext,
    ) {
        if primary_pos.is_some() && context.local_player_index < 0 {
            return;
        }
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

        // C++ FXList.cpp:575-578 — only TheParticleSystemManager->findTemplate.
        let Some(template) = manager.find_template(&self.template_name) else {
            return created_systems;
        };

        // Apply offset with matrix transformation
        let mut offset = self.offset;
        if let Some(matrix) = mtx {
            offset = matrix * offset;
        }

        // Create multiple systems based on count
        for _ in 0..self.count {
            // C++ FXList.cpp:588-603 — radius draw, then angle draw; the
            // height draw happens only when the ground snap does NOT consume
            // the Z.
            let radius = self.radius.sample();
            let angle = crate::GameClientRandomValueReal!(0.0, 2.0 * std::f32::consts::PI);

            let mut spawn_pos = Point3::new(
                primary.x + offset.x + radius * angle.cos(),
                primary.y + offset.y + radius * angle.sin(),
                primary.z + offset.z,
            );

            let ground_z = if self.create_at_ground_height {
                gamelogic::helpers::TheTerrainLogic::get().map(|terrain| {
                    let dest = gamelogic::common::Coord3D {
                        x: spawn_pos.x,
                        y: spawn_pos.y,
                        z: spawn_pos.z,
                    };
                    let layer = terrain.get_layer_for_destination(&dest);
                    terrain.get_layer_height(spawn_pos.x, spawn_pos.y, layer)
                })
            } else {
                None
            };
            match ground_z {
                Some(z) => spawn_pos.z = z,
                // C++ FXList.cpp:603 — non-ground branch samples Height.
                None => spawn_pos.z += self.height.sample(),
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
                let attached = object_id.is_some() && self.attach_to_object;
                if let Some(system) = manager.find_particle_system_mut(system_id) {
                    // C++ FXList.cpp:617-621 — attachToObject replaces setPosition.
                    if !attached {
                        system.set_position(spawn_pos);
                    }

                    // C++: orientToObject then rotateLocalTransformX/Y/Z.
                    if self.orient_to_object {
                        if let Some(matrix) = mtx {
                            system.set_local_transform(*matrix);
                        }
                    }
                    if self.rotate_x != 0.0 {
                        system.rotate_local_transform_x(self.rotate_x);
                    }
                    if self.rotate_y != 0.0 {
                        system.rotate_local_transform_y(self.rotate_y);
                    }
                    if self.rotate_z != 0.0 {
                        system.rotate_local_transform_z(self.rotate_z);
                    }

                    // C++: delayInMsec >= 0 → setInitialDelay(ceil(msec→frames)).
                    // -1 leave-sentinel keeps the template default.
                    let delay_msec = self.delay.sample();
                    if delay_msec >= 0.0 {
                        let delay_frames = (delay_msec * 30.0 / 1000.0).ceil() as u32;
                        system.set_initial_delay(delay_frames);
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

impl FXNugget for ParticleSystemFXNugget {
    fn do_fx_pos(
        &self,
        primary: Point3<f32>,
        primary_mtx: Option<&Matrix3<f32>>,
        _primary_speed: f32,
        _secondary: Option<Point3<f32>>,
        override_radius: f32,
        context: &mut FXContext,
    ) {
        // C++ ParticleSystemFXNugget::doFXPos → reallyDoFX via TheParticleSystemManager.
        let _ = ParticleSystemFXNugget::do_fx_pos(
            self,
            primary,
            primary_mtx,
            override_radius,
            context.particle_manager,
        );
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
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingNugget {
        positions: Arc<Mutex<Vec<Point3<f32>>>>,
    }

    impl FXNugget for RecordingNugget {
        fn do_fx_pos(
            &self,
            primary: Point3<f32>,
            _primary_mtx: Option<&Matrix3<f32>>,
            _primary_speed: f32,
            _secondary: Option<Point3<f32>>,
            _override_radius: f32,
            _context: &mut FXContext,
        ) {
            self.positions.lock().unwrap().push(primary);
        }
    }

    struct TestBoneQuery;

    impl FXBoneQuery for TestBoneQuery {
        fn current_client_bone_positions(
            &self,
            bone_name_prefix: &str,
            start_index: usize,
            max_bones: usize,
        ) -> Vec<FXBoneTransform> {
            assert_eq!(bone_name_prefix, "MUZZLE");
            assert_eq!(max_bones, FXListAtBonePosFXNugget::MAX_BONE_POINTS);
            match start_index {
                0 => vec![FXBoneTransform {
                    position: Point3::new(1.0, 2.0, 3.0),
                    transform: Matrix3::identity(),
                }],
                1 => vec![
                    FXBoneTransform {
                        position: Point3::new(4.0, 5.0, 6.0),
                        transform: Matrix3::identity(),
                    },
                    FXBoneTransform {
                        position: Point3::new(7.0, 8.0, 9.0),
                        transform: Matrix3::identity(),
                    },
                ],
                _ => Vec::new(),
            }
        }
    }

    fn test_context<'a>(
        manager: &'a mut ParticleSystemManager,
        bone_query: Option<&'a dyn FXBoneQuery>,
    ) -> FXContext<'a> {
        FXContext {
            particle_manager: manager,
            ray_effect_manager: None,
            decal_manager: None,
            bone_query,
            current_frame: 0,
            local_player_index: 0,
        }
    }

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
    fn fx_list_at_bone_pos_uses_current_client_bones() {
        let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
        {
            let mut shroud = shroud_manager.lock().expect("shroud");
            *shroud = gamelogic::system::shroud_manager::ShroudManager::new();
            shroud.init_shroud_grid(500.0, 500.0);
            shroud.do_shroud_reveal(
                &gamelogic::common::Coord3D {
                    x: 4.0,
                    y: 5.0,
                    z: 6.0,
                },
                75.0,
                1,
            );
        }

        let positions = Arc::new(Mutex::new(Vec::new()));
        let mut nested = FXList::new();
        nested.add_nugget(Arc::new(RecordingNugget {
            positions: Arc::clone(&positions),
        }));
        let nugget = FXListAtBonePosFXNugget::new(Arc::new(nested), "MUZZLE".to_string());

        let mut manager = ParticleSystemManager::new();
        let query = TestBoneQuery;
        let mut context = test_context(&mut manager, Some(&query));

        nugget.do_fx_obj(None, None, None, &mut context);

        *shroud_manager.lock().expect("shroud") =
            gamelogic::system::shroud_manager::ShroudManager::new();

        let positions = positions.lock().unwrap();
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[0], Point3::new(1.0, 2.0, 3.0));
        assert_eq!(positions[1], Point3::new(4.0, 5.0, 6.0));
        assert_eq!(positions[2], Point3::new(7.0, 8.0, 9.0));
    }

    #[test]
    fn execute_fx_pos_skips_nuggets_unless_cellshroud_clear() {
        let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
        {
            let mut shroud = shroud_manager.lock().expect("shroud");
            *shroud = gamelogic::system::shroud_manager::ShroudManager::new();
            shroud.init_shroud_grid(500.0, 500.0);
        }

        let positions = Arc::new(Mutex::new(Vec::new()));
        let mut list = FXList::new();
        list.add_nugget(Arc::new(RecordingNugget {
            positions: Arc::clone(&positions),
        }));
        let mut manager = ParticleSystemManager::new();
        let mut context = test_context(&mut manager, None);
        let primary = Point3::new(100.0, 100.0, 0.0);

        list.execute_fx_pos(primary, None, 0.0, None, 0.0, &mut context);
        assert!(
            positions.lock().unwrap().is_empty(),
            "unexplored CELLSHROUD_SHROUDED must not leak FXList integration nuggets"
        );

        {
            let mut shroud = shroud_manager.lock().expect("shroud");
            shroud.do_shroud_reveal(
                &gamelogic::common::Coord3D {
                    x: 100.0,
                    y: 100.0,
                    z: 0.0,
                },
                75.0,
                1,
            );
        }
        list.execute_fx_pos(primary, None, 0.0, None, 0.0, &mut context);
        assert_eq!(
            positions.lock().unwrap().len(),
            1,
            "CELLSHROUD_CLEAR must play FXList integration nuggets"
        );

        *shroud_manager.lock().expect("shroud") =
            gamelogic::system::shroud_manager::ShroudManager::new();
    }

    #[test]
    fn execute_fx_obj_fail_closes_when_local_player_invalid() {
        let positions = Arc::new(Mutex::new(Vec::new()));
        let mut list = FXList::new();
        list.add_nugget(Arc::new(RecordingNugget {
            positions: Arc::clone(&positions),
        }));
        let mut manager = ParticleSystemManager::new();
        let mut context = test_context(&mut manager, None);
        context.local_player_index = -1;
        list.execute_fx_obj(Some(Point3::new(10.0, 10.0, 0.0)), None, None, &mut context);
        assert!(
            positions.lock().unwrap().is_empty(),
            "hq-nyjgg: invalid local player must fail-close doFXObj"
        );
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
