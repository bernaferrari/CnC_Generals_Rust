// FILE: particle_sys.rs
// Author: Ported from C++ (Michael S. Booth, November 2001)
// Desc: Particle System implementation - individual particles and particle info
//
// Ported from:
// - /GeneralsMD/Code/GameEngine/Include/GameClient/ParticleSys.h
// - /GeneralsMD/Code/GameEngine/Source/GameClient/System/ParticleSys.cpp

use std::f32::consts::{PI, TAU as TWO_PI};
use std::sync::{Arc, Mutex, Weak};

use crate::Common::{Coord2D, Coord3D, RGBColor};
use game_engine::common::system::{snapshot::Snapshotable, Xfer};
use game_engine::system::XferMode;
use gamelogic::common::ObjectID;
use gamelogic::helpers::TheGameLogic;
use gamelogic::system::shroud_manager::get_shroud_manager;
use gamelogic::terrain::get_terrain_logic;
use gamelogic::object::drawable::{DrawableArcExt, DrawableExt};
use glam::{Mat4, Vec3, Vec4, Quat};
use ww3d_renderer_3d::camera::CameraState;
use wwmath::matrix3d::Matrix3D;
use game_engine::common::ini::{INIError, INI};

fn pseudo_rand(seed: u32) -> f32 {
    // Simple xorshift32 to avoid external deps; returns [0,1).
    let mut x = seed.wrapping_add(0x9E3779B9);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    (x as f32 / u32::MAX as f32).abs()
}

const MAX_PARTICLES_PER_SYSTEM: usize = 2048;
const MAX_GLOBAL_PARTICLES: usize = 8192;

/// Maximum number of keyframes for particle animation
/// Matches C++ ParticleSys.h:47
pub const MAX_KEYFRAMES: usize = 8;

/// Maximum volume particle depth
/// Matches C++ ParticleSys.h:40-42
pub const MAX_VOLUME_PARTICLE_DEPTH: usize = 16;
pub const DEFAULT_VOLUME_PARTICLE_DEPTH: usize = 0;
pub const OPTIMUM_VOLUME_PARTICLE_DEPTH: usize = 6;

/// Particle system ID type
/// Matches C++ ParticleSys.h:35-38
pub type ParticleSystemID = u32;
pub const INVALID_PARTICLE_SYSTEM_ID: ParticleSystemID = 0;

/// Simple view frustum for culling
#[derive(Clone, Debug)]
pub struct SimpleFrustum {
    pub planes: [Vec4; 6],
}

impl SimpleFrustum {
    pub fn contains_point(&self, p: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            let d = plane.x * p.x + plane.y * p.y + plane.z * p.z + plane.w;
            if d < -radius {
                return false;
            }
        }
        true
    }
}

/// Attachment resolver used to fetch world transforms and shroud status for attached IDs.
pub trait AttachmentResolver: Send + Sync {
    fn resolve_object_world(&self, object_id: u32) -> Option<Coord3D>;
    fn resolve_drawable_world(&self, drawable_id: u32) -> Option<Coord3D>;
    fn resolve_object_transform(&self, object_id: u32) -> Option<Mat4>;
    fn resolve_drawable_transform(&self, drawable_id: u32) -> Option<Mat4>;
    fn object_shrouded(&self, object_id: u32, local_player_index: i32) -> Option<bool>;
    fn drawable_shrouded(&self, drawable_id: u32, local_player_index: i32) -> Option<bool> {
        let _ = (drawable_id, local_player_index);
        None
    }
}

/// Default resolver that queries GameLogic object registry and shroud manager.
#[derive(Clone, Default)]
struct GameLogicAttachmentResolver;

impl AttachmentResolver for GameLogicAttachmentResolver {
    fn resolve_object_world(&self, object_id: u32) -> Option<Coord3D> {
        TheGameLogic::find_object_by_id(object_id as ObjectID)?
            .read()
            .ok()
            .map(|obj| *obj.get_position())
    }

    fn resolve_drawable_world(&self, drawable_id: u32) -> Option<Coord3D> {
        // Attempt to find an object with matching drawable and use its position.
        let registry = gamelogic::object::registry::OBJECT_REGISTRY.get_all_objects();
        for obj_ref in registry {
            if let Ok(obj) = obj_ref.read() {
                if let Some(drawable) = obj.get_drawable() {
                    if let Ok(drawable_guard) = drawable.read() {
                        if drawable_guard.get_id() == drawable_id {
                            return Some(*obj.get_position());
                        }
                    }
                }
            }
        }
        None
    }

    fn resolve_object_transform(&self, object_id: u32) -> Option<Mat4> {
        TheGameLogic::find_object_by_id(object_id as ObjectID)?
            .read()
            .ok()
            .map(|obj| obj.get_transform_matrix())
    }

    fn resolve_drawable_transform(&self, drawable_id: u32) -> Option<Mat4> {
        // Prefer drawable manager path if exposed; fallback to object registry.
        let registry = gamelogic::object::registry::OBJECT_REGISTRY.get_all_objects();
        for obj_ref in registry {
            if let Ok(obj) = obj_ref.read() {
                if let Some(drawable) = obj.get_drawable() {
                    if let Ok(drawable_guard) = drawable.read() {
                        if drawable_guard.get_id() == drawable_id {
                            let m = drawable_guard.get_instance_matrix();
                            return Some(mat4_from_matrix3d(m));
                        }
                    }
                }
            }
        }
        None
    }

    fn object_shrouded(&self, object_id: u32, local_player_index: i32) -> Option<bool> {
        let mgr = get_shroud_manager().lock().ok()?;
        Some(!mgr.can_see_object(local_player_index.max(0) as u32, object_id))
    }

    fn drawable_shrouded(&self, drawable_id: u32, local_player_index: i32) -> Option<bool> {
        // If drawable is tied to an object, reuse object shroud status.
        let registry = gamelogic::object::registry::OBJECT_REGISTRY.get_all_objects();
        for obj_ref in registry {
            if let Ok(obj) = obj_ref.read() {
                if let Some(drawable) = obj.get_drawable() {
                    if let Ok(drawable_guard) = drawable.read() {
                        if drawable_guard.get_id() == drawable_id {
                            let mgr = get_shroud_manager().lock().ok()?;
                            return Some(!mgr.can_see_object(
                                local_player_index.max(0) as u32,
                                obj.get_id(),
                            ));
                        }
                    }
                }
            }
        }
        None
    }
}

/// Keyframe for a single value (e.g., alpha)
/// Matches C++ ParticleSys.h:49-53
#[derive(Clone, Copy, Debug)]
pub struct Keyframe {
    pub value: f32,
    pub frame: u32,
}

/// Keyframe for RGB color values
/// Matches C++ ParticleSys.h:55-59
#[derive(Clone, Copy, Debug)]
pub struct RGBColorKeyframe {
    pub color: RGBColor,
    pub frame: u32,
}

impl Default for Keyframe {
    fn default() -> Self {
        Self { value: 0.0, frame: 0 }
    }
}

impl Default for RGBColorKeyframe {
    fn default() -> Self {
        Self {
            color: RGBColor {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
            },
            frame: 0,
        }
    }
}

/// Particle priority levels
///
/// Higher priority particles are less likely to be culled when the system
/// is under load. ALWAYS_RENDER particles are never culled.
///
/// Matches C++ ParticleSys.h:61-88
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ParticlePriorityType {
    Invalid = 0,
    WeaponExplosion = 1,
    Scorchmark = 2,
    DustTrail = 3,
    Buildup = 4,
    DebrisTrail = 5,
    UnitDamageFx = 6,
    DeathExplosion = 7,
    SemiConstant = 8,
    Constant = 9,
    WeaponTrail = 10,
    AreaEffect = 11,
    Critical = 12,
    AlwaysRender = 13,
}

impl ParticlePriorityType {
    pub const PARTICLE_PRIORITY_LOWEST: ParticlePriorityType = ParticlePriorityType::WeaponExplosion;
    pub const PARTICLE_PRIORITY_HIGHEST: ParticlePriorityType = ParticlePriorityType::AlwaysRender;
    pub const NUM_PARTICLE_PRIORITIES: usize = 14;
}

/// Particle shader types
/// Matches C++ ParticleSys.h:281-284
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ParticleShaderType {
    InvalidShader = 0,
    Additive = 1,
    Alpha = 2,
    AlphaTest = 3,
    Multiply = 4,
}

/// Particle types
/// Matches C++ ParticleSys.h:287-290
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ParticleType {
    InvalidType = 0,
    Particle = 1,
    Drawable = 2,
    Streak = 3,
    VolumeParticle = 4,
    Smudge = 5,
}

/// Emission velocity types
/// Matches C++ ParticleSys.h:343-346
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EmissionVelocityType {
    InvalidVelocity = 0,
    Ortho = 1,
    Spherical = 2,
    Hemispherical = 3,
    Cylindrical = 4,
    Outward = 5,
}

/// Emission volume types
/// Matches C++ ParticleSys.h:386-389
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EmissionVolumeType {
    InvalidVolume = 0,
    Point = 1,
    Line = 2,
    Box = 3,
    Sphere = 4,
    Cylinder = 5,
}

/// Wind motion types
/// Matches C++ ParticleSys.h:432-437
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum WindMotion {
    Invalid = 0,
    NotUsed = 1,
    PingPong = 2,
    Circular = 3,
}

/// ParticleInfo structure filled out to initialize a Particle
///
/// This structure contains all the initial parameters for creating a particle.
/// Matches C++ ParticleSys.h:93-132
#[derive(Clone, Debug)]
pub struct ParticleInfo {
    pub vel: Coord3D,
    pub pos: Coord3D,
    pub emitter_pos: Coord3D,
    pub vel_damping: f32,
    pub system_under_control: Option<ParticleSystemID>,

    pub angle_z: f32,
    pub angular_rate_z: f32,
    pub angular_damping: f32,

    pub lifetime: u32,

    pub size: f32,
    pub size_rate: f32,
    pub size_rate_damping: f32,

    pub alpha_key: [Keyframe; MAX_KEYFRAMES],
    pub color_key: [RGBColorKeyframe; MAX_KEYFRAMES],

    pub color_scale: f32,
    pub wind_randomness: f32,

    /// If true, particle's 0.0 Z rotation corresponds to direction of emitter
    pub particle_up_towards_emitter: bool,
}

impl ParticleInfo {
    /// Create a new ParticleInfo with default values
    /// Matches C++ ParticleSys.cpp:49-71
    pub fn new() -> Self {
        Self {
            angle_z: 0.0,
            angular_damping: 0.0,
            angular_rate_z: 0.0,
            color_scale: 0.0,
            size: 0.0,
            size_rate: 0.0,
            size_rate_damping: 0.0,
            vel_damping: 0.0,
            wind_randomness: 0.0,
            emitter_pos: Coord3D::zero(),
            pos: Coord3D::zero(),
            vel: Coord3D::zero(),
            lifetime: 0,
            particle_up_towards_emitter: false,
            alpha_key: [Keyframe { value: 0.0, frame: 0 }; MAX_KEYFRAMES],
            color_key: [RGBColorKeyframe {
                color: RGBColor { red: 0.0, green: 0.0, blue: 0.0 },
                frame: 0,
            }; MAX_KEYFRAMES],
        }
    }
}

impl Default for ParticleInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// An individual particle created by a ParticleSystem.
///
/// NOTE: Particles cannot exist without a parent particle system.
/// Matches C++ ParticleSys.h:139-220
pub struct Particle {
    // Parent system (weak to avoid cycles)
    parent: Weak<Mutex<ParticleSystem>>,
    system_next: Option<Box<Particle>>,
    system_prev: Option<*mut Particle>,
    overall_next: Option<*mut Particle>,
    overall_prev: Option<*mut Particle>,

    personality: u32,

    // Base particle info
    vel: Coord3D,
    pos: Coord3D,
    emitter_pos: Coord3D,
    vel_damping: f32,
    angle_z: f32,
    angular_rate_z: f32,
    angular_damping: f32,
    lifetime: u32,
    size: f32,
    size_rate: f32,
    size_rate_damping: f32,
    color_scale: f32,
    wind_randomness: f32,
    particle_up_towards_emitter: bool,

    alpha_key: [Keyframe; MAX_KEYFRAMES],
    color_key: [RGBColorKeyframe; MAX_KEYFRAMES],

    // Current particle state
    accel: Coord3D,
    last_pos: Coord3D,
    lifetime_left: u32,
    create_timestamp: u32,

    alpha: f32,
    alpha_rate: f32,
    alpha_target_key: usize,

    color: RGBColor,
    color_rate: RGBColor,
    color_target_key: usize,

    is_culled: bool,
    in_system_list: bool,
    in_overall_list: bool,

    system_under_control: Option<ParticleSystemID>,
    controlled_system: Option<Arc<Mutex<ParticleSystem>>>,
}

impl Particle {
    /// Create a new particle from particle info
    /// Matches C++ ParticleSys.cpp:231-296
    pub fn new(parent: &Arc<Mutex<ParticleSystem>>, info: &ParticleInfo, current_frame: u32) -> Self {
        let mut particle = Self {
            parent: Arc::downgrade(parent),
            system_next: None,
            system_prev: None,
            overall_next: None,
            overall_prev: None,
            personality: 0,

            vel: info.vel,
            pos: info.pos,
            emitter_pos: info.emitter_pos,
            vel_damping: info.vel_damping,
            angle_z: info.angle_z,
            angular_rate_z: info.angular_rate_z,
            angular_damping: info.angular_damping,
            lifetime: info.lifetime,
            size: info.size,
            size_rate: info.size_rate,
            size_rate_damping: info.size_rate_damping,
            color_scale: info.color_scale,
            wind_randomness: info.wind_randomness,
            particle_up_towards_emitter: info.particle_up_towards_emitter,

            alpha_key: info.alpha_key,
            color_key: info.color_key,

            accel: Coord3D::zero(),
            last_pos: Coord3D::zero(),
            lifetime_left: info.lifetime,
            create_timestamp: current_frame,

            alpha: info.alpha_key[0].value,
            alpha_rate: 0.0,
            alpha_target_key: 1,

            color: info.color_key[0].color,
            color_rate: RGBColor { red: 0.0, green: 0.0, blue: 0.0 },
            color_target_key: 1,

            is_culled: false,
            in_system_list: false,
            in_overall_list: false,

            system_under_control: None,
            controlled_system: None,
        };

        particle.compute_alpha_rate();
        particle.compute_color_rate();

        particle
    }

    /// Create an unbound particle (used for load) without a parent link.
    pub fn new_unbound(info: &ParticleInfo, current_frame: u32) -> Self {
        let mut particle = Self {
            parent: Weak::new(),
            system_next: None,
            system_prev: None,
            overall_next: None,
            overall_prev: None,
            personality: 0,

            vel: info.vel,
            pos: info.pos,
            emitter_pos: info.emitter_pos,
            vel_damping: info.vel_damping,
            angle_z: info.angle_z,
            angular_rate_z: info.angular_rate_z,
            angular_damping: info.angular_damping,
            lifetime: info.lifetime,
            size: info.size,
            size_rate: info.size_rate,
            size_rate_damping: info.size_rate_damping,
            color_scale: info.color_scale,
            wind_randomness: info.wind_randomness,
            particle_up_towards_emitter: info.particle_up_towards_emitter,

            alpha_key: info.alpha_key,
            color_key: info.color_key,

            accel: Coord3D::zero(),
            last_pos: Coord3D::zero(),
            lifetime_left: info.lifetime,
            create_timestamp: current_frame,

            alpha: info.alpha_key[0].value,
            alpha_rate: 0.0,
            alpha_target_key: 1,

            color: info.color_key[0].color,
            color_rate: RGBColor { red: 0.0, green: 0.0, blue: 0.0 },
            color_target_key: 1,

            is_culled: false,
            in_system_list: false,
            in_overall_list: false,

            system_under_control: None,
            controlled_system: None,
        };

        particle.compute_alpha_rate();
        particle.compute_color_rate();
        particle
    }

    /// Compute alpha rate to get to next keyframe
    /// Matches C++ ParticleSys.cpp:190-202
    fn compute_alpha_rate(&mut self) {
        if self.alpha_key[self.alpha_target_key].frame == 0 {
            self.alpha_rate = 0.0;
            return;
        }

        let delta = self.alpha_key[self.alpha_target_key].value
            - self.alpha_key[self.alpha_target_key - 1].value;
        let time = self.alpha_key[self.alpha_target_key].frame
            - self.alpha_key[self.alpha_target_key - 1].frame;

        self.alpha_rate = delta / (time as f32);
    }

    /// Compute color rate to get to next keyframe
    /// Matches C++ ParticleSys.cpp:207-226
    fn compute_color_rate(&mut self) {
        if self.color_key[self.color_target_key].frame == 0 {
            self.color_rate = RGBColor { red: 0.0, green: 0.0, blue: 0.0 };
            return;
        }

        let time = (self.color_key[self.color_target_key].frame
            - self.color_key[self.color_target_key - 1].frame) as f32;

        let delta_red = self.color_key[self.color_target_key].color.red
            - self.color_key[self.color_target_key - 1].color.red;
        self.color_rate.red = delta_red / time;

        let delta_green = self.color_key[self.color_target_key].color.green
            - self.color_key[self.color_target_key - 1].color.green;
        self.color_rate.green = delta_green / time;

        let delta_blue = self.color_key[self.color_target_key].color.blue
            - self.color_key[self.color_target_key - 1].color.blue;
        self.color_rate.blue = delta_blue / time;
    }

    /// Update particle behavior - returns false if particle should be destroyed
    /// Matches C++ ParticleSys.cpp:333-461
    pub fn update(&mut self, current_frame: u32) -> bool {
        // Integrate acceleration into velocity
        self.vel.x += self.accel.x;
        self.vel.y += self.accel.y;
        self.vel.z += self.accel.z;

        self.vel.x *= self.vel_damping;
        self.vel.y *= self.vel_damping;
        self.vel.z *= self.vel_damping;

        // Integrate velocity into position
        let drift_vel = self
            .parent
            .upgrade()
            .and_then(|p| p.lock().ok())
            .map(|ps| ps.drift_velocity)
            .unwrap_or_else(Coord3D::zero);
        self.pos.x += self.vel.x + drift_vel.x;
        self.pos.y += self.vel.y + drift_vel.y;
        self.pos.z += self.vel.z + drift_vel.z;

        // Apply particle-type specific grounding/alignment
        if let Some(parent) = self.parent.upgrade().and_then(|p| p.lock().ok()) {
            match parent.particle_type() {
                ParticleType::Smudge => {
                    if let Ok(terrain) = get_terrain_logic().read() {
                        let ground = terrain.get_ground_height(self.pos.x, self.pos.y, None);
                        self.pos.z = ground;
                        self.vel.z = 0.0;
                        self.accel.z = 0.0;
                        self.angle_z = 0.0;
                    }
                }
                ParticleType::VolumeParticle => {
                    // Keep volume particles from rising above emitter if gravity drives them up
                    if self.vel.z > 0.0 {
                        self.vel.z *= 0.5;
                    }
                }
                ParticleType::Drawable => {
                    // Face parent transform orientation if available
                    let forward = parent.world_transform.transform_vector3(Vec3::Z);
                    if forward.length() > 1e-4 {
                        self.angle_z = forward.y.atan2(forward.x);
                    }
                }
                ParticleType::Streak => {
                    // Face camera for streak billboarding if camera is available
                    if let Some(cam) = parent.camera_pos {
                        let dir_x = cam.x - self.pos.x;
                        let dir_y = cam.y - self.pos.y;
                        if dir_x != 0.0 || dir_y != 0.0 {
                            self.angle_z = dir_y.atan2(dir_x);
                        }
                    }
                }
                _ => {}
            }
        }

        // Apply wind motion if enabled
        let wind_motion = self
            .parent
            .upgrade()
            .and_then(|p| p.lock().ok())
            .map(|ps| ps.wind_motion)
            .unwrap_or(WindMotion::NotUsed);
        if wind_motion != WindMotion::NotUsed {
            self.do_wind_motion();
        }

        // Update orientation
        self.angle_z += self.angular_rate_z;
        self.angular_rate_z *= self.angular_damping;

        // Adjust angle to point towards emitter if needed
        if self.particle_up_towards_emitter {
            let emitter_dir_x = self.pos.x - self.emitter_pos.x;
            let emitter_dir_y = self.pos.y - self.emitter_pos.y;
            self.angle_z = emitter_dir_y.atan2(emitter_dir_x) + PI;
        }

        // Streak alignment: face velocity direction
        if let Some(parent) = self.parent.upgrade().and_then(|p| p.lock().ok()) {
            if parent.particle_type() == ParticleType::Streak {
                let dir_x = self.vel.x;
                let dir_y = self.vel.y;
                if dir_x != 0.0 || dir_y != 0.0 {
                    self.angle_z = dir_y.atan2(dir_x);
                }
                // Stretch streak size based on speed to approximate trail length
                self.size = self.size.max(self.get_streak_length());
            }
        }

        // Update size
        self.size += self.size_rate;
        self.size_rate *= self.size_rate_damping;

        // Update alpha (if not additive shader)
        let shader_type = self
            .parent
            .upgrade()
            .and_then(|p| p.lock().ok())
            .map(|ps| ps.shader_type)
            .unwrap_or(ParticleShaderType::Additive);
        if shader_type != ParticleShaderType::Additive {
            self.alpha += self.alpha_rate;

            if self.alpha_target_key < MAX_KEYFRAMES
                && self.alpha_key[self.alpha_target_key].frame > 0
            {
                let elapsed_frames = current_frame.saturating_sub(self.create_timestamp);
                if elapsed_frames >= self.alpha_key[self.alpha_target_key].frame {
                    self.alpha = self.alpha_key[self.alpha_target_key].value;
                    self.alpha_target_key += 1;
                    self.compute_alpha_rate();
                }
            } else {
                self.alpha_rate = 0.0;
            }

            self.alpha = self.alpha.clamp(0.0, 1.0);
        }

        // Update color
        self.color.red += self.color_rate.red;
        self.color.green += self.color_rate.green;
        self.color.blue += self.color_rate.blue;

        if self.color_target_key < MAX_KEYFRAMES
            && self.color_key[self.color_target_key].frame > 0
        {
            let elapsed_frames = current_frame.saturating_sub(self.create_timestamp);
            if elapsed_frames >= self.color_key[self.color_target_key].frame {
                self.color_target_key += 1;
                self.compute_color_rate();
            }
        } else {
            self.color_rate = RGBColor { red: 0.0, green: 0.0, blue: 0.0 };
        }

        // Apply color scale
        self.color.red += self.color_scale;
        self.color.green += self.color_scale;
        self.color.blue += self.color_scale;

        // Camera-facing streaks: force opacity drop when facing edge-on to camera
        if let Some(cam) = self.parent.upgrade().and_then(|p| p.lock().ok()).and_then(|p| p.camera_pos) {
            if self.particle_up_towards_emitter && self.parent.upgrade().and_then(|p| p.lock().ok()).map(|p| p.particle_type()).unwrap_or(ParticleType::Particle) == ParticleType::Streak {
                let to_cam = Vec3::new(cam.x - self.pos.x, cam.y - self.pos.y, 0.0);
                let streak_dir = Vec3::new(self.vel.x, self.vel.y, 0.0);
                let dot = to_cam.normalize_or_zero().dot(streak_dir.normalize_or_zero()).abs();
                // more perpendicular => less alpha
                self.alpha *= dot;
            }
        }

        // Clamp colors
        self.color.red = self.color.red.clamp(0.0, 1.0);
        self.color.green = self.color.green.clamp(0.0, 1.0);
        self.color.blue = self.color.blue.clamp(0.0, 1.0);

        // Reset acceleration for next frame
        self.accel = Coord3D::zero();

        // Monitor lifetime
        if self.lifetime_left > 0 {
            self.lifetime_left -= 1;
            if self.lifetime_left == 0 {
                self.destroy_controlled_system();
                return false;
            }
        }

        // Check if invisible
        if self.is_invisible() {
            self.destroy_controlled_system();
            return false;
        }

        if let Some(ctrl_arc) = &self.controlled_system {
            if let Ok(mut ctrl) = ctrl_arc.lock() {
                ctrl.set_position(self.pos);
                if let Some(parent) = self.parent.upgrade().and_then(|p| p.lock().ok()) {
                    ctrl.set_shrouded(parent.shrouded);
                    ctrl.set_lod_scale(parent.lod_scale);
                    ctrl.set_fps_budget_factor(parent.fps_budget_factor);
                    // Compose full transform: parent world * child rotation (Z) * child scale * translation
                    let rot_z = Mat4::from_quat(Quat::from_rotation_z(self.angle_z));
                    let scl = Mat4::from_scale(Vec3::splat(self.local_scale.max(0.01)));
                    let translate = Mat4::from_translation(Vec3::new(self.pos.x, self.pos.y, self.pos.z));
                    ctrl.world_transform = parent.world_transform * rot_z * scl * translate;
                    ctrl.vel_coeff = parent.vel_coeff;
                    ctrl.size_coeff = parent.size_coeff;
                    ctrl.count_coeff = parent.count_coeff;
                    ctrl.delay_coeff = parent.delay_coeff;
                    ctrl.gravity = parent.gravity;
                }
            }
        }

        true
    }

    fn destroy_controlled_system(&mut self) {
        if let Some(ctrl_arc) = self.controlled_system.take() {
            if let Ok(mut ctrl) = ctrl_arc.lock() {
                ctrl.destroy();
            }
        }
        self.system_under_control = None;
    }

    /// Apply wind motion to particle
    /// Matches C++ ParticleSys.cpp:466-544
    fn do_wind_motion(&mut self) {
        let wind_angle = self
            .parent
            .upgrade()
            .and_then(|p| p.lock().ok())
            .map(|ps| ps.wind_angle)
            .unwrap_or(0.0);

        // Get system position (includes attachment offsets if any)
        let system_pos = self
            .parent
            .upgrade()
            .and_then(|p| p.lock().ok())
            .map(|ps| ps.get_world_position())
            .unwrap_or_else(Coord3D::zero);

        // Compute vector from system to particle
        let v_x = self.pos.x - system_pos.x;
        let v_y = self.pos.y - system_pos.y;
        let v_z = self.pos.z - system_pos.z;

        // Distance thresholds for wind force
        let full_force_distance = 75.0;
        let no_force_distance = 200.0;

        let dist_from_wind = (v_x * v_x + v_y * v_y + v_z * v_z).sqrt();

        if dist_from_wind < no_force_distance {
            let mut wind_force_strength = 2.0 * self.wind_randomness;

            // Reduce force at distance
            if dist_from_wind > full_force_distance {
                wind_force_strength *= 1.0
                    - ((dist_from_wind - full_force_distance)
                        / (no_force_distance - full_force_distance));
            }

            // Apply wind motion
            self.pos.x += wind_angle.cos() * wind_force_strength;
            self.pos.y += wind_angle.sin() * wind_force_strength;
        }
    }

    /// Apply force (acceleration) to particle
    /// Matches C++ ParticleSys.cpp:323-328
    pub fn apply_force(&mut self, force: &Coord3D) {
        self.accel.x += force.x;
        self.accel.y += force.y;
        self.accel.z += force.z;
    }

    /// Compute approximate streak direction (for stub behavior)
    pub fn get_streak_dir(&self) -> Coord3D {
        let v = self.vel;
        let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt().max(1e-3);
        Coord3D::new(v.x / len, v.y / len, v.z / len)
    }

    /// Streak length helper based on velocity magnitude (for visuals)
    pub fn get_streak_length(&self) -> f32 {
        let speed = (self.vel.x * self.vel.x + self.vel.y * self.vel.y + self.vel.z * self.vel.z).sqrt();
        // Clamp to reasonable range; C++ uses renderer streak settings; approximate here
        speed.clamp(0.5, 20.0)
    }

    /// Volume particle depth helper
    pub fn get_volume_depth(&self, depth: u32) -> u32 {
        depth.min(MAX_VOLUME_PARTICLE_DEPTH as u32)
    }

    /// Check if particle is invisible based on shader type
    /// Matches C++ ParticleSys.cpp:557-596
    pub fn is_invisible(&self) -> bool {
        let shader = self
            .parent
            .upgrade()
            .and_then(|p| p.lock().ok())
            .map(|ps| ps.shader_type)
            .unwrap_or(ParticleShaderType::InvalidShader);
        match shader {
            ParticleShaderType::Additive => {
                // If color is black, particle is invisible
                if self.color_key[self.color_target_key].frame == 0 {
                    if (self.color.red + self.color.green + self.color.blue) <= 0.06 {
                        return true;
                    }
                }
                false
            }
            ParticleShaderType::Alpha => {
                // If alpha is zero, particle is invisible
                self.alpha < 0.02
            }
            ParticleShaderType::AlphaTest => {
                // Assume these are never invisible
                false
            }
            ParticleShaderType::Multiply => {
                // If color is white, particle is invisible
                if self.color_key[self.color_target_key].frame == 0 {
                    if (self.color.red * self.color.green * self.color.blue) > 0.95 {
                        return true;
                    }
                }
                false
            }
            _ => true,
        }
    }

    /// Get particle priority (from parent system)
    /// Matches C++ ParticleSys.cpp:549-552
    pub fn get_priority(&self) -> ParticlePriorityType {
        self.system.get_priority()
    }

    // Accessor methods
    pub fn get_position(&self) -> &Coord3D {
        &self.pos
    }

    pub fn get_size(&self) -> f32 {
        self.size
    }

    pub fn get_angle(&self) -> f32 {
        self.angle_z
    }

    pub fn get_alpha(&self) -> f32 {
        self.alpha
    }

    pub fn get_color(&self) -> &RGBColor {
        &self.color
    }

    pub fn is_culled(&self) -> bool {
        self.is_culled
    }

    pub fn set_is_culled(&mut self, culled: bool) {
        self.is_culled = culled;
    }

    pub fn get_personality(&self) -> u32 {
        self.personality
    }

    pub fn set_personality(&mut self, p: u32) {
        self.personality = p;
    }

    pub fn control_particle_system(&mut self, sys_id: ParticleSystemID) {
        self.system_under_control = Some(sys_id);
    }

    pub fn detach_controlled_particle_system(&mut self) {
        self.system_under_control = None;
        self.controlled_system = None;
    }
}

/// Emission velocity parameters aligned with C++ union layout.
#[derive(Clone, Copy, Debug)]
pub struct EmissionVelocityParams {
    pub ortho: Coord3D,
    pub spherical_speed: f32,
    pub hemispherical_speed: f32,
    pub cylindrical_radial: f32,
    pub cylindrical_normal: f32,
    pub outward_speed: f32,
    pub outward_other_speed: f32,
}

impl Default for EmissionVelocityParams {
    fn default() -> Self {
        Self {
            ortho: Coord3D::new(0.0, 0.0, 1.0),
            spherical_speed: 1.0,
            hemispherical_speed: 1.0,
            cylindrical_radial: 1.0,
            cylindrical_normal: 0.0,
            outward_speed: 1.0,
            outward_other_speed: 0.0,
        }
    }
}

/// Emission volume parameters aligned with C++ union layout.
#[derive(Clone, Copy, Debug)]
pub struct EmissionVolumeParams {
    pub line_start: Coord3D,
    pub line_end: Coord3D,
    pub box_half_size: Coord3D,
    pub sphere_radius: f32,
    pub cylinder_radius: f32,
    pub cylinder_length: f32,
}

impl Default for EmissionVolumeParams {
    fn default() -> Self {
        Self {
            line_start: Coord3D::zero(),
            line_end: Coord3D::zero(),
            box_half_size: Coord3D::zero(),
            sphere_radius: 0.0,
            cylinder_radius: 0.0,
            cylinder_length: 0.0,
        }
    }
}

/// Particle system definition
/// Matches core behavior of C++ ParticleSystem/ParticleSystemTemplate, closer to parity.
#[derive(Clone)]
pub struct ParticleSystemTemplate {
    pub priority: ParticlePriorityType,
    pub shader_type: ParticleShaderType,
    pub particle_type: ParticleType,
    pub particle_type_name: Option<String>,
    pub wind_motion: WindMotion,
    pub wind_angle: f32,
    pub drift_velocity: Coord3D,
    pub gravity: f32,
    pub is_one_shot: bool,
    pub burst_count: u32,
    pub burst_delay: u32,
    pub initial_delay: u32,
    pub system_lifetime: u32,
    pub emission_velocity_type: EmissionVelocityType,
    pub emission_velocity: EmissionVelocityParams,
    pub emission_volume_type: EmissionVolumeType,
    pub emission_volume: EmissionVolumeParams,
    pub is_emission_volume_hollow: bool,
    pub is_ground_aligned: bool,
    pub is_emit_above_ground_only: bool,
    pub is_particle_up_towards_emitter: bool,
    pub start_size_min: f32,
    pub start_size_max: f32,
    pub start_size_rate: f32,
    pub size_rate: f32,
    pub size_rate_damping: f32,
    pub lifetime_frames: u32,
    pub volume_particle_depth: u32,
    pub color_scale: f32,
    pub wind_randomness: f32,
    pub alpha_keys: [Keyframe; MAX_KEYFRAMES],
    pub color_keys: [RGBColorKeyframe; MAX_KEYFRAMES],
    pub wind_angle_change_min: f32,
    pub wind_angle_change_max: f32,
    pub wind_pingpong_start_angle_min: f32,
    pub wind_pingpong_start_angle_max: f32,
    pub wind_pingpong_end_angle_min: f32,
    pub wind_pingpong_end_angle_max: f32,
    pub attached_system_template: Option<String>,
    pub slave_system_template: Option<String>,
}

impl Default for ParticleSystemTemplate {
    fn default() -> Self {
        Self {
            priority: ParticlePriorityType::WeaponExplosion,
            shader_type: ParticleShaderType::Additive,
            particle_type: ParticleType::Particle,
            particle_type_name: None,
            wind_motion: WindMotion::NotUsed,
            wind_angle: 0.0,
            drift_velocity: Coord3D::zero(),
            gravity: 0.0,
            is_one_shot: false,
            burst_count: 1,
            burst_delay: 0,
            initial_delay: 0,
            system_lifetime: 0,
            emission_velocity_type: EmissionVelocityType::Ortho,
            emission_velocity: EmissionVelocityParams::default(),
            emission_volume_type: EmissionVolumeType::Point,
            emission_volume: EmissionVolumeParams::default(),
            is_emission_volume_hollow: false,
            is_ground_aligned: false,
            is_emit_above_ground_only: false,
            is_particle_up_towards_emitter: false,
            start_size_min: 0.5,
            start_size_max: 1.0,
            start_size_rate: 0.0,
            size_rate: 0.0,
            size_rate_damping: 1.0,
            lifetime_frames: 60,
            volume_particle_depth: DEFAULT_VOLUME_PARTICLE_DEPTH as u32,
            color_scale: 0.0,
            wind_randomness: 1.0,
            alpha_keys: [Keyframe { value: 1.0, frame: 0 }; MAX_KEYFRAMES],
            color_keys: [RGBColorKeyframe {
                color: RGBColor { red: 1.0, green: 1.0, blue: 1.0 },
                frame: 0,
            }; MAX_KEYFRAMES],
            wind_angle_change_min: 0.05,
            wind_angle_change_max: 0.15,
            wind_pingpong_start_angle_min: 0.0,
            wind_pingpong_start_angle_max: PI / 4.0,
            wind_pingpong_end_angle_min: TWO_PI - (PI / 4.0),
            wind_pingpong_end_angle_max: TWO_PI,
            attached_system_template: None,
            slave_system_template: None,
        }
    }
}

impl Snapshotable for ParticleSystemTemplate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u32 = 2;
        xfer.xfer_version(&mut version, 2).map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut (self.priority as i32))
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut (self.shader_type as i32))
            .map_err(|e| e.to_string())?;
        let mut pt = self.particle_type as i32;
        xfer.xfer_int(&mut pt).map_err(|e| e.to_string())?;
        self.particle_type = match pt {
            1 => ParticleType::Particle,
            2 => ParticleType::Drawable,
            3 => ParticleType::Streak,
            4 => ParticleType::VolumeParticle,
            5 => ParticleType::Smudge,
            _ => ParticleType::Particle,
        };
        if version >= 2 {
            let mut name = self.particle_type_name.clone().unwrap_or_default();
            xfer.xfer_ascii_string(&mut name).map_err(|e| e.to_string())?;
            self.particle_type_name = if name.is_empty() { None } else { Some(name) };
        }
        xfer.xfer_int(&mut (self.wind_motion as i32))
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_angle).map_err(|e| e.to_string())?;
        xfer_coord3d(xfer, &mut self.drift_velocity)?;
        if version >= 2 {
            xfer.xfer_real(&mut self.gravity).map_err(|e| e.to_string())?;
        } else {
            // old saves had no gravity; keep default 0
        }
        xfer.xfer_bool(&mut self.is_one_shot).map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.burst_count)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.burst_delay)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.initial_delay)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.system_lifetime)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.lifetime_frames)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.start_size_min)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.start_size_max)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.start_size_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.size_rate).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.size_rate_damping)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.volume_particle_depth)
            .map_err(|e| e.to_string())?;
        for k in self.alpha_keys.iter_mut() {
            xfer.xfer_real(&mut k.value).map_err(|e| e.to_string())?;
            let mut frame = k.frame as i32;
            xfer.xfer_int(&mut frame).map_err(|e| e.to_string())?;
            k.frame = frame.max(0) as u32;
        }
        for k in self.color_keys.iter_mut() {
            xfer.xfer_real(&mut k.color.red).map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut k.color.green).map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut k.color.blue).map_err(|e| e.to_string())?;
            let mut frame = k.frame as i32;
            xfer.xfer_int(&mut frame).map_err(|e| e.to_string())?;
            k.frame = frame.max(0) as u32;
        }
        xfer.xfer_real(&mut self.color_scale)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_randomness)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_angle_change_min)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_angle_change_max)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_pingpong_start_angle_min)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_pingpong_start_angle_max)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_pingpong_end_angle_min)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_pingpong_end_angle_max)
            .map_err(|e| e.to_string())?;

        let mut emission_vel = self.emission_velocity_type as i32;
        xfer.xfer_int(&mut emission_vel).map_err(|e| e.to_string())?;
        self.emission_velocity_type = match emission_vel {
            0 => EmissionVelocityType::Ortho,
            1 => EmissionVelocityType::Spherical,
            2 => EmissionVelocityType::Hemispherical,
            3 => EmissionVelocityType::Cylindrical,
            4 => EmissionVelocityType::Outward,
            _ => EmissionVelocityType::InvalidVelocity,
        };
        if version >= 2 {
            xfer_coord3d(xfer, &mut self.emission_velocity.ortho)?;
            xfer.xfer_real(&mut self.emission_velocity.spherical_speed)
                .map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut self.emission_velocity.hemispherical_speed)
                .map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut self.emission_velocity.cylindrical_radial)
                .map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut self.emission_velocity.cylindrical_normal)
                .map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut self.emission_velocity.outward_speed)
                .map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut self.emission_velocity.outward_other_speed)
                .map_err(|e| e.to_string())?;
        } else {
            // legacy: single coord3d velocity
            let mut legacy = Coord3D::new(0.0, 0.0, 0.0);
            xfer_coord3d(xfer, &mut legacy)?;
            self.emission_velocity.ortho = legacy;
            self.emission_velocity.spherical_speed = legacy.x;
            self.emission_velocity.hemispherical_speed = legacy.x;
            self.emission_velocity.cylindrical_radial = legacy.x;
            self.emission_velocity.cylindrical_normal = legacy.z;
            self.emission_velocity.outward_speed = legacy.x;
            self.emission_velocity.outward_other_speed = legacy.z;
        }

        let mut emission_vol = self.emission_volume_type as i32;
        xfer.xfer_int(&mut emission_vol).map_err(|e| e.to_string())?;
        self.emission_volume_type = match emission_vol {
            0 => EmissionVolumeType::Point,
            1 => EmissionVolumeType::Box,
            2 => EmissionVolumeType::Sphere,
            3 => EmissionVolumeType::Cylinder,
            4 => EmissionVolumeType::Line,
            _ => EmissionVolumeType::InvalidVolume,
        };
        if version >= 2 {
            xfer_coord3d(xfer, &mut self.emission_volume.line_start)?;
            xfer_coord3d(xfer, &mut self.emission_volume.line_end)?;
            xfer_coord3d(xfer, &mut self.emission_volume.box_half_size)?;
            xfer.xfer_real(&mut self.emission_volume.sphere_radius)
                .map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut self.emission_volume.cylinder_radius)
                .map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut self.emission_volume.cylinder_length)
                .map_err(|e| e.to_string())?;
        } else {
            // legacy: box half size + radius/length
            xfer_coord3d(xfer, &mut self.emission_volume.box_half_size)?;
            xfer.xfer_real(&mut self.emission_volume.sphere_radius)
                .map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut self.emission_volume.cylinder_length)
                .map_err(|e| e.to_string())?;
            self.emission_volume.cylinder_radius = self.emission_volume.sphere_radius;
            // line defaults remain zero
        }
        xfer.xfer_bool(&mut self.is_emission_volume_hollow)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_ground_aligned)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_emit_above_ground_only)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_particle_up_towards_emitter)
            .map_err(|e| e.to_string())?;

        let mut has_attached = self.attached_system_template.is_some();
        xfer.xfer_bool(&mut has_attached).map_err(|e| e.to_string())?;
        if has_attached {
            let mut name = self
                .attached_system_template
                .clone()
                .unwrap_or_else(String::new);
            xfer.xfer_ascii_string(&mut name).map_err(|e| e.to_string())?;
            self.attached_system_template = Some(name);
        } else {
            self.attached_system_template = None;
        }
        let mut has_slave = self.slave_system_template.is_some();
        xfer.xfer_bool(&mut has_slave).map_err(|e| e.to_string())?;
        if has_slave {
            let mut name =
                self.slave_system_template.clone().unwrap_or_else(String::new);
            xfer.xfer_ascii_string(&mut name).map_err(|e| e.to_string())?;
            self.slave_system_template = Some(name);
        } else {
            self.slave_system_template = None;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Recompute next personality to avoid collisions after load.
        let max_pers = self
            .particles
            .iter()
            .map(|p| p.get_personality())
            .max()
            .unwrap_or(0);
        self.next_personality = max_pers.wrapping_add(1);
        Ok(())
    }
}

impl ParticleSystemTemplate {
    pub fn parse_from_ini(ini: &mut INI, section: &str) -> Result<Self, INIError> {
        let mut template = ParticleSystemTemplate::default();
        let table = ini.get_section(section).ok_or(INIError::InvalidData)?;

        if let Some(val) = table.get("Priority") {
            template.priority = match val.as_str() {
                "ALWAYS_RENDER" => ParticlePriorityType::AlwaysRender,
                "CRITICAL" => ParticlePriorityType::Critical,
                "AREA_EFFECT" => ParticlePriorityType::AreaEffect,
                _ => ParticlePriorityType::WeaponExplosion,
            };
        }
        if let Some(val) = table.get("ParticleType") {
            template.particle_type = match val.as_str() {
                "DRAWABLE" => ParticleType::Drawable,
                "STREAK" => ParticleType::Streak,
                "VOLUME_PARTICLE" => ParticleType::VolumeParticle,
                "SMUDGE" => ParticleType::Smudge,
                _ => ParticleType::Particle,
            };
        }
        if let Some(val) = table.get("IsOneShot") {
            template.is_one_shot = val == "yes" || val == "true" || val == "1";
        }
        if let Some(val) = table.get("ParticleTypeName") {
            template.particle_type_name = Some(val.to_string());
        }
        if let Some(val) = table.get("Shader") {
            template.shader_type = match val.as_str() {
                "ADDITIVE" => ParticleShaderType::Additive,
                "ALPHA" => ParticleShaderType::Alpha,
                "ALPHA_TEST" => ParticleShaderType::AlphaTest,
                "MULTIPLY" => ParticleShaderType::Multiply,
                _ => ParticleShaderType::Additive,
            };
        }
        if let Some(val) = table.get("Gravity") {
            template.gravity = val.parse().unwrap_or(template.gravity);
        }
        if let Some(val) = table.get("WindMotion") {
            template.wind_motion = match val.as_str() {
                "PING_PONG" => WindMotion::PingPong,
                "CIRCULAR" => WindMotion::Circular,
                "NOT_USED" => WindMotion::NotUsed,
                _ => WindMotion::NotUsed,
            };
        }
        if let Some(val) = table.get("WindAngle") {
            template.wind_angle = val.parse().unwrap_or(0.0);
        }
        if let Some(val) = table.get("WindAngleChangeMin") {
            template.wind_angle_change_min = val.parse().unwrap_or(template.wind_angle_change_min);
        }
        if let Some(val) = table.get("WindAngleChangeMax") {
            template.wind_angle_change_max = val.parse().unwrap_or(template.wind_angle_change_max);
        }
        if let Some(val) = table.get("WindPingPongStartAngleMin") {
            template.wind_pingpong_start_angle_min = val.parse().unwrap_or(template.wind_pingpong_start_angle_min);
        }
        if let Some(val) = table.get("WindPingPongStartAngleMax") {
            template.wind_pingpong_start_angle_max = val.parse().unwrap_or(template.wind_pingpong_start_angle_max);
        }
        if let Some(val) = table.get("WindPingPongEndAngleMin") {
            template.wind_pingpong_end_angle_min = val.parse().unwrap_or(template.wind_pingpong_end_angle_min);
        }
        if let Some(val) = table.get("WindPingPongEndAngleMax") {
            template.wind_pingpong_end_angle_max = val.parse().unwrap_or(template.wind_pingpong_end_angle_max);
        }

        if let Some(val) = table.get("BurstCount") {
            template.burst_count = val.parse().unwrap_or(template.burst_count);
        }
        if let Some(val) = table.get("BurstDelay") {
            template.burst_delay = val.parse().unwrap_or(template.burst_delay);
        }
        if let Some(val) = table.get("InitialDelay") {
            template.initial_delay = val.parse().unwrap_or(template.initial_delay);
        }
        if let Some(val) = table.get("SystemLifetime") {
            template.system_lifetime = val.parse().unwrap_or(template.system_lifetime);
        }
        if let Some(val) = table.get("DriftVelocity") {
            let parts: Vec<&str> = val.split_whitespace().collect();
            if parts.len() == 3 {
                template.drift_velocity.x = parts[0].parse().unwrap_or(template.drift_velocity.x);
                template.drift_velocity.y = parts[1].parse().unwrap_or(template.drift_velocity.y);
                template.drift_velocity.z = parts[2].parse().unwrap_or(template.drift_velocity.z);
            }
        }

        if let Some(val) = table.get("EmitVelType").or_else(|| table.get("VelocityType")) {
            template.emission_velocity_type = match val.as_str() {
                "ORTHO" => EmissionVelocityType::Ortho,
                "SPHERICAL" => EmissionVelocityType::Spherical,
                "HEMISPHERICAL" => EmissionVelocityType::Hemispherical,
                "CYLINDRICAL" => EmissionVelocityType::Cylindrical,
                "OUTWARD" => EmissionVelocityType::Outward,
                _ => EmissionVelocityType::Ortho,
            };
        }
        if let Some(val) = table.get("VelOrthoX") {
            template.emission_velocity.ortho.x =
                val.parse().unwrap_or(template.emission_velocity.ortho.x);
        }
        if let Some(val) = table.get("VelOrthoY") {
            template.emission_velocity.ortho.y =
                val.parse().unwrap_or(template.emission_velocity.ortho.y);
        }
        if let Some(val) = table.get("VelOrthoZ") {
            template.emission_velocity.ortho.z =
                val.parse().unwrap_or(template.emission_velocity.ortho.z);
        }
        if let Some(val) = table.get("VelSpherical") {
            template.emission_velocity.spherical_speed =
                val.parse().unwrap_or(template.emission_velocity.spherical_speed);
        }
        if let Some(val) = table.get("VelHemispherical") {
            template.emission_velocity.hemispherical_speed =
                val.parse().unwrap_or(template.emission_velocity.hemispherical_speed);
        }
        if let Some(val) = table.get("VelCylindricalRadial") {
            template.emission_velocity.cylindrical_radial =
                val.parse().unwrap_or(template.emission_velocity.cylindrical_radial);
        }
        if let Some(val) = table.get("VelCylindricalNormal") {
            template.emission_velocity.cylindrical_normal =
                val.parse().unwrap_or(template.emission_velocity.cylindrical_normal);
        }
        if let Some(val) = table.get("VelOutward") {
            template.emission_velocity.outward_speed =
                val.parse().unwrap_or(template.emission_velocity.outward_speed);
        }
        if let Some(val) = table.get("VelOutwardOther") {
            template.emission_velocity.outward_other_speed =
                val.parse().unwrap_or(template.emission_velocity.outward_other_speed);
        }

        if let Some(val) = table.get("VolumeType") {
            template.emission_volume_type = match val.as_str() {
                "POINT" => EmissionVolumeType::Point,
                "LINE" => EmissionVolumeType::Line,
                "BOX" => EmissionVolumeType::Box,
                "SPHERE" => EmissionVolumeType::Sphere,
                "CYLINDER" => EmissionVolumeType::Cylinder,
                _ => EmissionVolumeType::Point,
            };
        }
        if let Some(val) = table.get("VolLineStart") {
            let parts: Vec<&str> = val.split_whitespace().collect();
            if parts.len() == 3 {
                template.emission_volume.line_start.x =
                    parts[0].parse().unwrap_or(template.emission_volume.line_start.x);
                template.emission_volume.line_start.y =
                    parts[1].parse().unwrap_or(template.emission_volume.line_start.y);
                template.emission_volume.line_start.z =
                    parts[2].parse().unwrap_or(template.emission_volume.line_start.z);
            }
        }
        if let Some(val) = table.get("VolLineEnd") {
            let parts: Vec<&str> = val.split_whitespace().collect();
            if parts.len() == 3 {
                template.emission_volume.line_end.x =
                    parts[0].parse().unwrap_or(template.emission_volume.line_end.x);
                template.emission_volume.line_end.y =
                    parts[1].parse().unwrap_or(template.emission_volume.line_end.y);
                template.emission_volume.line_end.z =
                    parts[2].parse().unwrap_or(template.emission_volume.line_end.z);
            }
        }
        if let Some(val) = table.get("VolBoxHalfSizeX") {
            template.emission_volume.box_half_size.x =
                val.parse().unwrap_or(template.emission_volume.box_half_size.x);
        }
        if let Some(val) = table.get("VolBoxHalfSizeY") {
            template.emission_volume.box_half_size.y =
                val.parse().unwrap_or(template.emission_volume.box_half_size.y);
        }
        if let Some(val) = table.get("VolBoxHalfSizeZ") {
            template.emission_volume.box_half_size.z =
                val.parse().unwrap_or(template.emission_volume.box_half_size.z);
        }
        if let Some(val) = table.get("VolBoxHalfSize") {
            let parts: Vec<&str> = val.split_whitespace().collect();
            if parts.len() == 3 {
                template.emission_volume.box_half_size.x =
                    parts[0].parse().unwrap_or(template.emission_volume.box_half_size.x);
                template.emission_volume.box_half_size.y =
                    parts[1].parse().unwrap_or(template.emission_volume.box_half_size.y);
                template.emission_volume.box_half_size.z =
                    parts[2].parse().unwrap_or(template.emission_volume.box_half_size.z);
            }
        }
        if let Some(val) = table.get("VolSphereRadius") {
            template.emission_volume.sphere_radius =
                val.parse().unwrap_or(template.emission_volume.sphere_radius);
        }
        if let Some(val) = table.get("VolCylinderRadius") {
            template.emission_volume.cylinder_radius =
                val.parse().unwrap_or(template.emission_volume.cylinder_radius);
        }
        if let Some(val) = table.get("VolCylinderLength") {
            template.emission_volume.cylinder_length =
                val.parse().unwrap_or(template.emission_volume.cylinder_length);
        }
        if let Some(val) = table.get("IsHollow") {
            template.is_emission_volume_hollow = val == "yes" || val == "true" || val == "1";
        }
        if let Some(val) = table.get("IsGroundAligned") {
            template.is_ground_aligned = val == "yes" || val == "true" || val == "1";
        }
        if let Some(val) = table.get("IsEmitAboveGroundOnly") {
            template.is_emit_above_ground_only = val == "yes" || val == "true" || val == "1";
        }
        if let Some(val) = table.get("IsParticleUpTowardsEmitter") {
            template.is_particle_up_towards_emitter = val == "yes" || val == "true" || val == "1";
        }

        if let Some(val) = table.get("StartSizeMin") {
            template.start_size_min = val.parse().unwrap_or(template.start_size_min);
        }
        if let Some(val) = table.get("StartSizeMax") {
            template.start_size_max = val.parse().unwrap_or(template.start_size_max);
        }
        if let Some(val) = table.get("StartSizeRate") {
            template.start_size_rate = val.parse().unwrap_or(template.start_size_rate);
        }
        if let Some(val) = table.get("SizeRate") {
            template.size_rate = val.parse().unwrap_or(template.size_rate);
        }
        if let Some(val) = table.get("SizeRateDamping") {
            template.size_rate_damping = val.parse().unwrap_or(template.size_rate_damping);
        }
        if let Some(val) = table.get("Lifetime") {
            template.lifetime_frames = val.parse().unwrap_or(template.lifetime_frames);
        }
        if let Some(val) = table.get("VolumeParticleDepth") {
            template.volume_particle_depth =
                val.parse().unwrap_or(template.volume_particle_depth);
        }
        if let Some(val) = table.get("ColorScale") {
            template.color_scale = val.parse().unwrap_or(template.color_scale);
        }
        if let Some(val) = table.get("WindRandomness") {
            template.wind_randomness = val.parse().unwrap_or(template.wind_randomness);
        }
        if let Some(val) = table.get("AttachedSystemName") {
            template.attached_system_template = Some(val.to_string());
        }
        if let Some(val) = table.get("SlaveSystemName") {
            template.slave_system_template = Some(val.to_string());
        }

        Ok(template)
    }
}

pub struct ParticleSystem {
    template: ParticleSystemTemplate,
    system_id: ParticleSystemID,
    particles: Vec<Particle>,
    is_destroyed: bool,
    is_stopped: bool,
    burst_delay_left: u32,
    delay_left: u32,
    system_lifetime_left: u32,
    is_first_pos: bool,
    last_pos: Coord3D,
    pos: Coord3D,
    wind_angle_current: f32,
    wind_motion_moving_to_end: bool,
    wind_motion_start_angle: f32,
    wind_motion_end_angle: f32,
    wind_angle_change: f32,
    attached_object_id: Option<u32>,
    attached_drawable_id: Option<u32>,
    master_system_id: Option<ParticleSystemID>,
    slave_system_id: Option<ParticleSystemID>,
    attached_world_pos: Option<Coord3D>,
    shrouded: bool,
    lod_scale: f32,
    fps_budget_factor: f32,
    gravity: f32,
    vel_coeff: f32,
    count_coeff: f32,
    delay_coeff: f32,
    size_coeff: f32,
    start_timestamp: u32,
    next_personality: u32,
    is_saveable: bool,
    local_offset: Coord3D,
    local_scale: f32,
    skip_parent_transform: bool,
    world_transform: Mat4,
    camera_pos: Option<Vec3>,
}

impl ParticleSystem {
    pub fn new(template: ParticleSystemTemplate, system_id: ParticleSystemID) -> Self {
        Self {
            burst_delay_left: template.burst_delay,
            delay_left: template.initial_delay,
            system_lifetime_left: template.system_lifetime,
            template,
            system_id,
            particles: Vec::new(),
            is_destroyed: false,
            is_stopped: false,
            is_first_pos: true,
            last_pos: Coord3D::zero(),
            pos: Coord3D::zero(),
            wind_angle_current: 0.0,
            wind_motion_moving_to_end: true,
            wind_motion_start_angle: 0.0,
            wind_motion_end_angle: 0.0,
            wind_angle_change: 0.0,
            attached_object_id: None,
            attached_drawable_id: None,
            master_system_id: None,
            slave_system_id: None,
            attached_world_pos: None,
            shrouded: false,
            lod_scale: 1.0,
            fps_budget_factor: 1.0,
            gravity: template.gravity,
            vel_coeff: 1.0,
            count_coeff: 1.0,
            delay_coeff: 1.0,
            size_coeff: 1.0,
            start_timestamp: 0,
            next_personality: 1,
            is_saveable: true,
            local_offset: Coord3D::zero(),
            local_scale: 1.0,
            skip_parent_transform: false,
            world_transform: Mat4::IDENTITY,
            camera_pos: None,
        }
    }

    fn particle_type(&self) -> ParticleType {
        self.template.particle_type
    }

    pub fn get_priority(&self) -> ParticlePriorityType {
        self.template.priority
    }

    pub fn get_shader_type(&self) -> ParticleShaderType {
        self.template.shader_type
    }

    pub fn get_wind_motion(&self) -> WindMotion {
        self.template.wind_motion
    }

    pub fn get_wind_angle(&self) -> f32 {
        self.wind_angle_current
    }

    pub fn get_drift_velocity(&self) -> Coord3D {
        self.template.drift_velocity
    }

    pub fn is_destroyed(&self) -> bool {
        self.is_destroyed
    }

    pub fn stop(&mut self) {
        self.is_stopped = true;
    }

    pub fn start(&mut self) {
        self.is_stopped = false;
    }

    pub fn destroy(&mut self) {
        self.is_destroyed = true;
    }

    pub fn set_position(&mut self, pos: Coord3D) {
        self.pos = pos;
    }

    pub fn set_local_scale(&mut self, scale: f32) {
        self.local_scale = scale.max(0.01);
    }

    fn resolve_attachment(
        &mut self,
        resolver: Option<&Arc<dyn AttachmentResolver>>,
        local_player_index: i32,
    ) {
        self.shrouded = false;
        let mut updated = false;

        let mut parent_transform = Mat4::IDENTITY;

        if let Some(obj_id) = self.attached_object_id {
            if let Some(res) = resolver {
                if let Some(world) = res.resolve_object_world(obj_id) {
                    self.last_pos = self.pos;
                    self.pos = world;
                    updated = true;
                }
                if let Some(shr) = res.object_shrouded(obj_id, local_player_index) {
                    self.shrouded = shr;
                }
                if let Some(xf) = res.resolve_object_transform(obj_id) {
                    parent_transform = xf;
                }
            }
            if !updated {
                if let Some(fallback) = self.attached_world_pos {
                    self.last_pos = self.pos;
                    self.pos = fallback;
                    updated = true;
                } else {
                    self.is_destroyed = true;
                }
            }
        } else if let Some(draw_id) = self.attached_drawable_id {
            if let Some(res) = resolver {
                if let Some(world) = res.resolve_drawable_world(draw_id) {
                    self.last_pos = self.pos;
                    self.pos = world;
                    updated = true;
                }
                if let Some(shr) = res.drawable_shrouded(draw_id, local_player_index) {
                    self.shrouded = shr;
                }
                if let Some(xf) = res.resolve_drawable_transform(draw_id) {
                    parent_transform = xf;
                }
            }
            if !updated {
                if let Some(fallback) = self.attached_world_pos {
                    self.last_pos = self.pos;
                    self.pos = fallback;
                    updated = true;
                } else {
                    self.is_destroyed = true;
                }
            }
        } else if let Some(world) = self.attached_world_pos {
            self.last_pos = self.pos;
            self.pos = world;
            updated = true;
        }

        if !updated {
            self.last_pos = self.pos;
        }

        // Base translation from current pos (resolved above)
        let base_translation =
            Mat4::from_translation(Vec3::new(self.pos.x, self.pos.y, self.pos.z));

        // Apply parent transform unless skipped
        let mut xf = if self.skip_parent_transform {
            Mat4::IDENTITY
        } else {
            parent_transform * base_translation
        };

        // Apply local offset (as translation) on top
        xf = xf * Mat4::from_translation(Vec3::new(
            self.local_offset.x,
            self.local_offset.y,
            self.local_offset.z,
        ));

        // Apply local scale
        xf = xf * Mat4::from_scale(Vec3::splat(self.local_scale.max(0.01)));

        // Cache world transform and position from matrix
        let t = xf.transform_point3(Vec3::ZERO);
        self.world_transform = xf;
        self.last_pos = self.pos;
        self.pos = Coord3D::new(t.x, t.y, t.z);
    }

    fn get_world_position(&self) -> Coord3D {
        self.pos
    }

    pub fn set_master(&mut self, master_id: ParticleSystemID) {
        self.master_system_id = Some(master_id);
    }

    pub fn set_slave(&mut self, slave_id: ParticleSystemID) {
        self.slave_system_id = Some(slave_id);
    }

    pub fn attach_to_object(&mut self, object_id: u32) {
        self.attached_object_id = Some(object_id);
        self.attached_drawable_id = None;
    }

    pub fn attach_to_drawable(&mut self, drawable_id: u32) {
        self.attached_drawable_id = Some(drawable_id);
        self.attached_object_id = None;
    }

    pub fn set_attached_world_pos(&mut self, pos: Coord3D) {
        self.attached_world_pos = Some(pos);
    }

    pub fn set_shrouded(&mut self, shrouded: bool) {
        self.shrouded = shrouded;
    }

    pub fn set_lod_scale(&mut self, scale: f32) {
        self.lod_scale = scale.clamp(0.0, 1.0);
    }

    pub fn set_fps_budget_factor(&mut self, factor: f32) {
        self.fps_budget_factor = factor.clamp(0.0, 1.0);
    }

    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    fn discard_oldest_particle(&mut self) {
        if !self.particles.is_empty() {
            self.particles.remove(0);
        }
    }

    pub fn create_particle(
        &mut self,
        info: &ParticleInfo,
        parent: &Arc<Mutex<ParticleSystem>>,
        current_frame: u32,
        force: bool,
        manager: Option<&mut ParticleSystemManager>,
    ) {
        if self.is_destroyed || self.is_stopped {
            return;
        }
        if !force && self.template.is_one_shot && self.particle_count() >= self.template.burst_count as usize {
            return;
        }
        if self.particle_count() >= MAX_PARTICLES_PER_SYSTEM {
            return;
        }
        let mut p = Particle::new(parent, info, current_frame);
        p.set_personality(self.next_personality);
        self.next_personality = self.next_personality.wrapping_add(1);

        if let (Some(mgr), Some(attached_name)) =
            (manager, self.template.attached_system_template.clone())
        {
            if let Some(attached_sys) = mgr.create_from_template(&attached_name) {
                if let Ok(mut child) = attached_sys.lock() {
                    child.set_position(p.pos);
                    child.set_master(self.system_id);
                    child.set_shrouded(self.shrouded);
                    child.set_lod_scale(self.lod_scale);
                    child.set_fps_budget_factor(self.fps_budget_factor);
                }
                if let Ok(id) = attached_sys.lock().map(|s| s.get_system_id()) {
                    p.control_particle_system(id);
                }
                p.controlled_system = Some(attached_sys);
            }
        }

        self.particles.push(p);
    }

    fn compute_point_on_unit_sphere(&self) -> Coord3D {
        // Simple deterministic pseudo-random spread using system_id and particle count
        let seed = self.system_id ^ (self.particle_count() as u32);
        let theta = pseudo_rand(seed.wrapping_mul(31)) * TWO_PI;
        let z = pseudo_rand(seed.wrapping_mul(17)) * 2.0 - 1.0;
        let r = (1.0 - z * z).sqrt();
        Coord3D::new(r * theta.cos(), r * theta.sin(), z)
    }

    fn compute_particle_position(&mut self) -> Coord3D {
        let seed = self.system_id ^ (self.particle_count() as u32);
        let mut local = match self.template.emission_volume_type {
            EmissionVolumeType::Point | EmissionVolumeType::InvalidVolume => Coord3D::zero(),
            EmissionVolumeType::Line => {
                let t = pseudo_rand(seed.wrapping_mul(19));
                let start = self.template.emission_volume.line_start;
                let end = self.template.emission_volume.line_end;
                Coord3D::new(
                    start.x + (end.x - start.x) * t,
                    start.y + (end.y - start.y) * t,
                    start.z + (end.z - start.z) * t,
                )
            }
            EmissionVolumeType::Box => {
                let h = self.template.emission_volume.box_half_size;
                if self.template.is_emission_volume_hollow {
                    // Choose a face uniformly, then random point on that face
                    let side = (pseudo_rand(seed.wrapping_mul(31)) * 6.0).floor() as i32;
                    let rx = pseudo_rand(seed.wrapping_mul(3)) * 2.0 - 1.0;
                    let ry = pseudo_rand(seed.wrapping_mul(5)) * 2.0 - 1.0;
                    let rz = pseudo_rand(seed.wrapping_mul(7)) * 2.0 - 1.0;
                    match side.rem_euclid(6) {
                        0 => Coord3D::new(h.x, h.y * rx, h.z * ry),
                        1 => Coord3D::new(-h.x, h.y * rx, h.z * ry),
                        2 => Coord3D::new(h.x * rx, h.y, h.z * ry),
                        3 => Coord3D::new(h.x * rx, -h.y, h.z * ry),
                        4 => Coord3D::new(h.x * rx, h.y * ry, h.z),
                        _ => Coord3D::new(h.x * rx, h.y * ry, -h.z),
                    }
                } else {
                    let rx = pseudo_rand(seed.wrapping_mul(3)) * 2.0 - 1.0;
                    let ry = pseudo_rand(seed.wrapping_mul(5)) * 2.0 - 1.0;
                    let rz = pseudo_rand(seed.wrapping_mul(7)) * 2.0 - 1.0;
                    Coord3D::new(h.x * rx, h.y * ry, h.z * rz)
                }
            }
            EmissionVolumeType::Sphere => {
                let mut dir = self.compute_point_on_unit_sphere();
                let radius = if self.template.is_emission_volume_hollow {
                    self.template.emission_volume.sphere_radius
                } else {
                    self.template.emission_volume.sphere_radius
                        * pseudo_rand(seed.wrapping_mul(11)).clamp(0.0, 1.0)
                };
                dir.x *= radius;
                dir.y *= radius;
                dir.z *= radius;
                dir
            }
            EmissionVolumeType::Cylinder => {
                let theta = pseudo_rand(seed.wrapping_mul(13)) * TWO_PI;
                let radius = if self.template.is_emission_volume_hollow {
                    self.template.emission_volume.cylinder_radius
                } else {
                    self.template.emission_volume.cylinder_radius
                        * pseudo_rand(seed.wrapping_mul(7)).sqrt()
                };
                let half_len = 0.5 * self.template.emission_volume.cylinder_length;
                let z = -half_len + pseudo_rand(seed.wrapping_mul(17)) * (half_len * 2.0);
                Coord3D::new(radius * theta.cos(), radius * theta.sin(), z)
            }
        };

        // Transform local offset by system/world transform
        let local_vec = Vec3::new(local.x, local.y, local.z);
        let world = self.world_transform.transform_point3(local_vec);
        // If transform is identity, fall back to current system position to avoid NaNs
        if self.world_transform == Mat4::IDENTITY {
            Coord3D::new(self.pos.x + local.x, self.pos.y + local.y, self.pos.z + local.z)
        } else {
            Coord3D::new(world.x, world.y, world.z)
        }
    }

    fn compute_particle_velocity(&self, pos: &Coord3D) -> Coord3D {
        let seed = self.system_id ^ (self.particle_count() as u32) ^ 0xDEADBEEF;
        let mut vel = match self.template.emission_velocity_type {
            EmissionVelocityType::Ortho => self.template.emission_velocity.ortho,
            EmissionVelocityType::Spherical => {
                let dir = self.compute_point_on_unit_sphere();
                Coord3D::new(
                    dir.x * self.template.emission_velocity.spherical_speed,
                    dir.y * self.template.emission_velocity.spherical_speed,
                    dir.z * self.template.emission_velocity.spherical_speed,
                )
            }
            EmissionVelocityType::Hemispherical => {
                let mut dir = Coord3D::zero();
                // biased to upper hemisphere
                while dir.x == 0.0 && dir.y == 0.0 && dir.z == 0.0 {
                    dir.x = pseudo_rand(seed.wrapping_mul(41)) * 2.0 - 1.0;
                    dir.y = pseudo_rand(seed.wrapping_mul(43)) * 2.0 - 1.0;
                    dir.z = pseudo_rand(seed.wrapping_mul(47));
                }
                let len = (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt().max(1e-5);
                dir.x /= len;
                dir.y /= len;
                dir.z /= len;
                Coord3D::new(
                    dir.x * self.template.emission_velocity.hemispherical_speed,
                    dir.y * self.template.emission_velocity.hemispherical_speed,
                    dir.z * self.template.emission_velocity.hemispherical_speed,
                )
            }
            EmissionVelocityType::Cylindrical => {
                let angle = pseudo_rand(seed.wrapping_mul(23)) * TWO_PI;
                let radial = self.template.emission_velocity.cylindrical_radial;
                let normal = self.template.emission_velocity.cylindrical_normal;
                Coord3D::new(radial * angle.cos(), radial * angle.sin(), normal)
            }
            EmissionVelocityType::Outward => {
                // Use emission volume normal to push outward
                match self.template.emission_volume_type {
                    EmissionVolumeType::Cylinder => {
                        let sys_pos = self.pos;
                        let mut dir = Coord3D::new(pos.x - sys_pos.x, pos.y - sys_pos.y, 0.0);
                        let len = (dir.x * dir.x + dir.y * dir.y).sqrt().max(1e-5);
                        dir.x /= len;
                        dir.y /= len;
                        Coord3D::new(
                            dir.x * self.template.emission_velocity.outward_speed,
                            dir.y * self.template.emission_velocity.outward_speed,
                            self.template.emission_velocity.outward_other_speed,
                        )
                    }
                    EmissionVolumeType::Line => {
                        // Build perpendicular to line + up component
                        let start = self.template.emission_volume.line_start;
                        let end = self.template.emission_volume.line_end;
                        let along = Coord3D::new(end.x - start.x, end.y - start.y, end.z - start.z);
                        let mut up = Coord3D::new(0.0, 0.0, 1.0);
                        let mut perp = Coord3D::new(
                            up.y * along.z - up.z * along.y,
                            up.z * along.x - up.x * along.z,
                            up.x * along.y - up.y * along.x,
                        );
                        let mut len = (perp.x * perp.x + perp.y * perp.y + perp.z * perp.z).sqrt();
                        if len < 1e-5 {
                            perp = Coord3D::new(1.0, 0.0, 0.0);
                            len = 1.0;
                        }
                        perp.x /= len;
                        perp.y /= len;
                        perp.z /= len;
                        Coord3D::new(
                            perp.x * self.template.emission_velocity.outward_speed,
                            perp.y * self.template.emission_velocity.outward_speed,
                            self.template.emission_velocity.outward_other_speed,
                        )
                    }
                    _ => {
                        let sys_pos = self.pos;
                        let mut dir =
                            Coord3D::new(pos.x - sys_pos.x, pos.y - sys_pos.y, pos.z - sys_pos.z);
                        let len = (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z)
                            .sqrt()
                            .max(1e-5);
                        dir.x /= len;
                        dir.y /= len;
                        dir.z /= len;
                        Coord3D::new(
                            dir.x * self.template.emission_velocity.outward_speed,
                            dir.y * self.template.emission_velocity.outward_speed,
                            dir.z * self.template.emission_velocity.outward_speed,
                        )
                    }
                }
            }
            EmissionVelocityType::InvalidVelocity => Coord3D::zero(),
        };

        // Rotate velocity by world transform basis (ignores translation)
        let rot = self
            .world_transform
            .transform_vector3(Vec3::new(vel.x, vel.y, vel.z));
        vel.x = rot.x;
        vel.y = rot.y;
        vel.z = rot.z;
        vel
    }

    fn generate_particle_info(&mut self) -> ParticleInfo {
        let mut info = ParticleInfo::new();
        let pos = self.compute_particle_position();
        let vel = self.compute_particle_velocity(&pos);
        info.pos = pos;
        info.vel = Coord3D::new(
            vel.x * self.vel_coeff,
            vel.y * self.vel_coeff,
            vel.z * self.vel_coeff,
        );
        info.emitter_pos = self.pos;
        info.lifetime = self.template.lifetime_frames.max(1);
        // Volume particle depth offset
        if self.template.particle_type == ParticleType::VolumeParticle {
            let depth = self.template.volume_particle_depth as f32;
            let offset = pseudo_rand(seed.wrapping_mul(37)) * depth;
            info.pos.z -= offset;
            // Slight alpha attenuation by depth
            let atten = 1.0 / (1.0 + depth.max(1.0));
            info.alpha_key[0].value *= atten;
        }
        if self.template.is_ground_aligned {
            if let Ok(terrain) = get_terrain_logic().read() {
                let ground = terrain.get_ground_height(info.pos.x, info.pos.y, None);
                info.pos.z = ground;
            }
        }
        // Size and rates with simple random selection
        let seed = self.system_id ^ (self.particle_count() as u32) ^ 0xA5A5_5A5A;
        let size_r = pseudo_rand(seed);
        info.size = (self.template.start_size_min
            + (self.template.start_size_max - self.template.start_size_min) * size_r)
            * self.size_coeff;
        info.size_rate = self.template.size_rate;
        info.size_rate_damping = self.template.size_rate_damping;
        // Start size rate is treated as an initial additive rate
        info.size += self.template.start_size_rate * self.size_coeff;
        if self.template.particle_type == ParticleType::Smudge {
            info.size_rate = 0.0;
            info.size_rate_damping = 1.0;
        }
        info.color_scale = self.template.color_scale;
        info.wind_randomness = self.template.wind_randomness;
        info.particle_up_towards_emitter = self.template.is_particle_up_towards_emitter;
        info.alpha_key = self.template.alpha_keys;
        info.color_key = self.template.color_keys;
        info.system_under_control = None;
        info
    }

    fn update_wind(&mut self) {
        match self.template.wind_motion {
            WindMotion::PingPong => {
                if self.wind_angle_current == 0.0 {
                    // initialize
                    self.wind_angle_current = self.template.wind_angle;
                    let r1 = pseudo_rand(self.system_id ^ 0x1111);
                    let r2 = pseudo_rand(self.system_id ^ 0x2222);
                    self.wind_motion_start_angle = self.template.wind_pingpong_start_angle_min
                        + (self.template.wind_pingpong_start_angle_max
                            - self.template.wind_pingpong_start_angle_min)
                            * r1;
                    self.wind_motion_end_angle = self.template.wind_pingpong_end_angle_min
                        + (self.template.wind_pingpong_end_angle_max
                            - self.template.wind_pingpong_end_angle_min)
                            * r2;
                    self.wind_angle_change = self.template.wind_angle_change_min
                        + (self.template.wind_angle_change_max
                            - self.template.wind_angle_change_min)
                            * pseudo_rand(self.system_id ^ 0x3333);
                }
                let start_angle = self.wind_motion_start_angle;
                let end_angle = self.wind_motion_end_angle;
                let total_span = (end_angle - start_angle).abs().max(0.001);
                let half_span = total_span * 0.5;
                let diff_from_center =
                    (half_span - (self.wind_angle_current - start_angle)).abs();
                let mut change = (1.0 - (diff_from_center / half_span)) * self.wind_angle_change;
                if change < 0.005 {
                    change = 0.005;
                }
                if self.wind_motion_moving_to_end {
                    self.wind_angle_current += change;
                    if self.wind_angle_current >= end_angle {
                        self.wind_motion_moving_to_end = false;
                        self.wind_angle_change = self.template.wind_angle_change_min
                            + (self.template.wind_angle_change_max
                                - self.template.wind_angle_change_min)
                                * pseudo_rand(self.system_id ^ 0x4444);
                        self.wind_motion_start_angle = self.template.wind_pingpong_start_angle_min;
                        self.wind_motion_end_angle = self.template.wind_pingpong_end_angle_max;
                    }
                } else {
                    self.wind_angle_current -= change;
                    if self.wind_angle_current <= start_angle {
                        self.wind_motion_moving_to_end = true;
                        self.wind_angle_change = self.template.wind_angle_change_min
                            + (self.template.wind_angle_change_max
                                - self.template.wind_angle_change_min)
                                * pseudo_rand(self.system_id ^ 0x5555);
                        self.wind_motion_start_angle = self.template.wind_pingpong_start_angle_min;
                        self.wind_motion_end_angle = self.template.wind_pingpong_end_angle_max;
                    }
                }
            }
            WindMotion::Circular => {
                if self.wind_angle_change == 0.0 {
                    self.wind_angle_change = self.template.wind_angle_change_min
                        + (self.template.wind_angle_change_max
                            - self.template.wind_angle_change_min)
                            * pseudo_rand(self.system_id ^ 0x6666);
                }
                self.wind_angle_current += self.wind_angle_change;
                if self.wind_angle_current > TWO_PI {
                    self.wind_angle_current -= TWO_PI;
                } else if self.wind_angle_current < 0.0 {
                    self.wind_angle_current += TWO_PI;
                }
            }
            WindMotion::NotUsed | WindMotion::Invalid => {
                self.wind_angle_current = self.template.wind_angle;
            }
        }
    }

    pub fn update_with_manager(
        &mut self,
        current_frame: u32,
        parent_arc: &Arc<Mutex<ParticleSystem>>,
        manager: &mut ParticleSystemManager,
    ) -> bool {
        if self.is_destroyed {
            return false;
        }

        // Resolve attachments (object/drawable) and shroud status.
        self.resolve_attachment(manager.attachment_resolver.as_ref(), manager.local_player_index);
        // If controlled by a particle, follow that transform (already placed via set_position).
        if let Some(master_id) = self.master_system_id {
            if let Some(master_arc) = manager.find_system(master_id) {
                if let Ok(master) = master_arc.lock() {
                    self.world_transform = master.world_transform;
                    self.camera_pos = master.camera_pos;
                }
            }
        } else {
            self.camera_pos = manager.camera_pos;
        }

        self.update_wind();

        // Update existing particles and cull dead ones.
        self.particles.retain_mut(|p| {
            // Apply drift/gravity as acceleration before update
            p.accel.x += self.template.drift_velocity.x;
            p.accel.y += self.template.drift_velocity.y;
            p.accel.z += self.template.drift_velocity.z + self.gravity;
            p.update(current_frame)
        });

        if self.is_destroyed && self.particles.is_empty() {
            return false;
        }

        // Propagate gating state to slave if present
        if let Some(slave_id) = self.slave_system_id {
            if let Some(slave_arc) = manager.find_system(slave_id) {
                if let Ok(mut slave) = slave_arc.lock() {
                    slave.set_shrouded(self.shrouded);
                    slave.set_lod_scale(self.lod_scale);
                    slave.set_fps_budget_factor(self.fps_budget_factor);
                    slave.set_position(self.pos);
                    slave.set_local_scale(self.local_scale);
                    slave.gravity = self.gravity;
                    slave.vel_coeff = self.vel_coeff;
                    slave.count_coeff = self.count_coeff;
                    slave.delay_coeff = self.delay_coeff;
                    slave.size_coeff = self.size_coeff;
                    slave.local_offset = self.local_offset;
                    slave.skip_parent_transform = self.skip_parent_transform;
                    slave.start_timestamp = self.start_timestamp;
                    slave.is_saveable = self.is_saveable;
                    slave.world_transform = self.world_transform;
                }
            }
        }

        // Lifetime handling for the system.
        if !self.template.is_one_shot && self.template.system_lifetime > 0 {
            if self.system_lifetime_left > 0 {
                self.system_lifetime_left = self.system_lifetime_left.saturating_sub(1);
            }
            if self.system_lifetime_left == 0 && self.particles.is_empty() {
                return false;
            }
        }

        // Handle initial delay
        if self.delay_left > 0 {
            self.delay_left = self.delay_left.saturating_sub(1);
            if self.delay_left == 0 {
                self.start_timestamp = current_frame;
            }
            return true;
        }

        // Budget/LOD/shroud gating
        let budget_scale = (self.lod_scale * self.fps_budget_factor).clamp(0.0, 1.0);
        let can_emit = !self.is_stopped
            && !self.shrouded
            && !self.is_destroyed
            && self.master_system_id.is_none()
            && budget_scale > 0.0;

        if can_emit {
            self.is_stopped = false;
            if self.burst_delay_left == 0 {
                let base_count = self.template.burst_count.max(1);
                let scaled_count =
                    ((base_count as f32) * budget_scale * self.count_coeff).ceil().max(1.0) as u32;
                let emit_count = scaled_count.min(MAX_PARTICLES_PER_SYSTEM as u32);

                for _ in 0..emit_count {
                    let mut info = self.generate_particle_info();
                    if self.template.is_emit_above_ground_only {
                        if let Ok(terrain) = get_terrain_logic().read() {
                            let ground = terrain.get_ground_height(info.pos.x, info.pos.y, None);
                            if info.pos.z < ground {
                                continue;
                            }
                        }
                    }
                    info.lifetime = if self.template.system_lifetime > 0 {
                        self.template.system_lifetime
                    } else {
                        self.template.lifetime_frames.max(1)
                    };
                    self.create_particle(&info, parent_arc, current_frame, true, Some(manager));

                    // Mirror into slave system if linked
                    if let Some(slave_id) = self.slave_system_id {
                        if let Some(slave_arc) = manager.find_system(slave_id) {
                            if let Ok(mut slave) = slave_arc.lock() {
                                slave.delay_left = 0;
                                slave.burst_delay_left = self.burst_delay_left;
                                slave.system_lifetime_left = self.system_lifetime_left;
                                slave.set_shrouded(self.shrouded);
                                slave.set_lod_scale(self.lod_scale);
                                slave.set_fps_budget_factor(self.fps_budget_factor);
                                slave.set_position(self.pos);
                                let mut slave_info = slave.generate_particle_info();
                                if slave.template.is_emit_above_ground_only {
                                    if let Ok(terrain) = get_terrain_logic().read() {
                                        let ground =
                                            terrain.get_ground_height(slave_info.pos.x, slave_info.pos.y, None);
                                        if slave_info.pos.z < ground {
                                            continue;
                                        }
                                    }
                                }
                                slave_info.lifetime = if slave.template.system_lifetime > 0 {
                                    slave.template.system_lifetime
                                } else {
                                    slave.template.lifetime_frames.max(1)
                                };
                                slave.create_particle(
                                    &slave_info,
                                    &slave_arc,
                                    current_frame,
                                    true,
                                    Some(manager),
                                );
                            }
                        }
                    }
                }

                let delay_scale = (1.0 / budget_scale.max(0.25)).clamp(1.0, 4.0) * self.delay_coeff;
                let mut next_delay =
                    (self.template.burst_delay as f32 * delay_scale).ceil() as u32;
                if next_delay == 0 {
                    next_delay = 1;
                }
                self.burst_delay_left = next_delay;
            } else {
                self.burst_delay_left = self.burst_delay_left.saturating_sub(1);
            }
        } else {
            // If budget or shroud disallow emission, pause bursts but keep particles alive.
            self.is_stopped = self.shrouded || budget_scale <= 0.05;
        }

        true
    }

    pub fn clear_particles(&mut self) {
        self.particles.clear();
    }

    pub fn get_system_id(&self) -> ParticleSystemID {
        self.system_id
    }
}

impl Snapshotable for ParticleSystem {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u32 = 2;
        xfer.xfer_version(&mut version, 2).map_err(|e| e.to_string())?;

        // Template
        self.template.xfer(xfer)?;

        // Identifiers and flags
        xfer.xfer_unsigned_int(&mut self.system_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_destroyed)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_stopped).map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_saveable).map_err(|e| e.to_string())?;

        xfer.xfer_unsigned_int(&mut self.burst_delay_left)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.delay_left)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.system_lifetime_left)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_first_pos)
            .map_err(|e| e.to_string())?;
        xfer_coord3d(xfer, &mut self.last_pos)?;
        xfer_coord3d(xfer, &mut self.pos)?;

        xfer.xfer_real(&mut self.wind_angle_current)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.wind_motion_moving_to_end)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_motion_start_angle)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_motion_end_angle)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_angle_change)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.gravity).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.vel_coeff).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.count_coeff).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.delay_coeff).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.size_coeff).map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.start_timestamp)
            .map_err(|e| e.to_string())?;
        if version >= 2 {
            xfer.xfer_unsigned_int(&mut self.next_personality)
                .map_err(|e| e.to_string())?;
        } else if xfer.get_xfer_mode() == XferMode::Load {
            self.next_personality = 1;
        }
        xfer.xfer_real(&mut self.local_offset.x)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.local_offset.y)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.local_offset.z)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.local_scale).map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.skip_parent_transform)
            .map_err(|e| e.to_string())?;

        let mut att_obj = self.attached_object_id.unwrap_or(0);
        let mut has_att_obj = self.attached_object_id.is_some();
        xfer.xfer_bool(&mut has_att_obj).map_err(|e| e.to_string())?;
        if has_att_obj {
            xfer.xfer_unsigned_int(&mut att_obj).map_err(|e| e.to_string())?;
            self.attached_object_id = Some(att_obj);
        } else {
            self.attached_object_id = None;
        }

        let mut att_draw = self.attached_drawable_id.unwrap_or(0);
        let mut has_att_draw = self.attached_drawable_id.is_some();
        xfer.xfer_bool(&mut has_att_draw).map_err(|e| e.to_string())?;
        if has_att_draw {
            xfer.xfer_unsigned_int(&mut att_draw).map_err(|e| e.to_string())?;
            self.attached_drawable_id = Some(att_draw);
        } else {
            self.attached_drawable_id = None;
        }

        let mut master = self.master_system_id.unwrap_or(INVALID_PARTICLE_SYSTEM_ID);
        let mut has_master = self.master_system_id.is_some();
        xfer.xfer_bool(&mut has_master).map_err(|e| e.to_string())?;
        if has_master {
            xfer.xfer_unsigned_int(&mut master).map_err(|e| e.to_string())?;
            self.master_system_id = Some(master);
        } else {
            self.master_system_id = None;
        }

        let mut slave = self.slave_system_id.unwrap_or(INVALID_PARTICLE_SYSTEM_ID);
        let mut has_slave = self.slave_system_id.is_some();
        xfer.xfer_bool(&mut has_slave).map_err(|e| e.to_string())?;
        if has_slave {
            xfer.xfer_unsigned_int(&mut slave).map_err(|e| e.to_string())?;
            self.slave_system_id = Some(slave);
        } else {
            self.slave_system_id = None;
        }

        let mut has_world_pos = self.attached_world_pos.is_some();
        xfer.xfer_bool(&mut has_world_pos).map_err(|e| e.to_string())?;
        if has_world_pos {
            let mut p = self.attached_world_pos.unwrap_or_default();
            xfer_coord3d(xfer, &mut p)?;
            self.attached_world_pos = Some(p);
        } else {
            self.attached_world_pos = None;
        }

        xfer.xfer_bool(&mut self.shrouded).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.lod_scale).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.fps_budget_factor)
            .map_err(|e| e.to_string())?;

        // Particles
        let mut count = self.particles.len() as u32;
        xfer.xfer_unsigned_int(&mut count).map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.particles.clear();
            for _ in 0..count {
                let mut p = Particle::new_unbound(&ParticleInfo::new(), 0);
                p.xfer(xfer)?;
                self.particles.push(p);
            }
        } else {
            for p in self.particles.iter_mut() {
                p.xfer(xfer)?;
            }
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.template.load_post_process()?;
        for p in self.particles.iter_mut() {
            p.load_post_process()?;
        }
        self.world_transform = Mat4::from_translation(glam::Vec3::new(self.pos.x, self.pos.y, self.pos.z));
        Ok(())
    }
}

/// Particle system manager - simplified
pub struct ParticleSystemManager {
    systems: Vec<Arc<Mutex<ParticleSystem>>>,
    unique_system_id: ParticleSystemID,
    templates: Vec<(String, ParticleSystemTemplate)>,
    attachment_resolver: Option<Arc<dyn AttachmentResolver>>,
    local_player_index: i32,
    on_screen_budget: Option<usize>,
    on_screen_budget_per_priority: Option<[usize; ParticlePriorityType::NUM_PARTICLE_PRIORITIES]>,
    frustum: Option<SimpleFrustum>,
    camera_pos: Option<Vec3>,
}

impl ParticleSystemManager {
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
            unique_system_id: 1,
            templates: Vec::new(),
            attachment_resolver: Some(Arc::new(GameLogicAttachmentResolver::default())),
            local_player_index: 0,
            on_screen_budget: None,
            on_screen_budget_per_priority: None,
            frustum: None,
            camera_pos: None,
        }
    }

    pub fn set_attachment_resolver(
        &mut self,
        resolver: Option<Arc<dyn AttachmentResolver>>,
    ) {
        self.attachment_resolver = resolver;
    }

    pub fn set_local_player_index(&mut self, idx: i32) {
        self.local_player_index = idx;
    }

    pub fn set_on_screen_budget(&mut self, budget: Option<usize>) {
        self.on_screen_budget = budget;
    }

    pub fn set_frustum(&mut self, frustum: Option<SimpleFrustum>) {
        self.frustum = frustum;
    }

    pub fn set_camera_pos(&mut self, pos: Option<Vec3>) {
        self.camera_pos = pos;
    }

    pub fn update_camera_from_state(&mut self, cam: &CameraState) {
        // Build frustum planes from view-projection (matches DrawableManager approach)
        let vp = cam.projection * cam.view;
        let planes = [
            Vec4::new(vp.w.x + vp.x.x, vp.w.y + vp.x.y, vp.w.z + vp.x.z, vp.w.w + vp.x.w), // right
            Vec4::new(vp.w.x - vp.x.x, vp.w.y - vp.x.y, vp.w.z - vp.x.z, vp.w.w - vp.x.w), // left
            Vec4::new(vp.w.x + vp.y.x, vp.w.y + vp.y.y, vp.w.z + vp.y.z, vp.w.w + vp.y.w), // top
            Vec4::new(vp.w.x - vp.y.x, vp.w.y - vp.y.y, vp.w.z - vp.y.z, vp.w.w - vp.y.w), // bottom
            Vec4::new(vp.w.x + vp.z.x, vp.w.y + vp.z.y, vp.w.z + vp.z.z, vp.w.w + vp.z.w), // far
            Vec4::new(vp.w.x - vp.z.x, vp.w.y - vp.z.y, vp.w.z - vp.z.z, vp.w.w - vp.z.w), // near
        ];
        let norm_plane = |p: Vec4| {
            let len = (p.x * p.x + p.y * p.y + p.z * p.z).sqrt().max(1e-6);
            Vec4::new(p.x / len, p.y / len, p.z / len, p.w / len)
        };
        let frustum = SimpleFrustum {
            planes: [
                norm_plane(planes[0]),
                norm_plane(planes[1]),
                norm_plane(planes[2]),
                norm_plane(planes[3]),
                norm_plane(planes[4]),
                norm_plane(planes[5]),
            ],
        };
        self.set_frustum(Some(frustum));
        self.camera_pos = Some(cam.position);
    }

    pub fn set_on_screen_budget_per_priority(
        &mut self,
        budgets: Option<[usize; ParticlePriorityType::NUM_PARTICLE_PRIORITIES]>,
    ) {
        self.on_screen_budget_per_priority = budgets;
    }

    pub fn bootstrap_from_camera_and_drawable_manager(
        &mut self,
        camera: &CameraState,
    ) {
        self.update_camera_from_state(camera);
    }

    /// Convenience wiring for resolver/camera/budgets.
    pub fn bootstrap(
        &mut self,
        camera: Option<&CameraState>,
        resolver: Option<Arc<dyn AttachmentResolver>>,
        budget: Option<usize>,
        per_priority: Option<[usize; ParticlePriorityType::NUM_PARTICLE_PRIORITIES]>,
    ) {
        if let Some(cam) = camera {
            self.update_camera_from_state(cam);
        }
        if resolver.is_some() {
            self.attachment_resolver = resolver;
        }
        self.set_on_screen_budget(budget);
        self.set_on_screen_budget_per_priority(per_priority);
    }

    /// Apply defaults similar to device/LOD tuning for particle budgets.
    pub fn apply_device_lod_defaults(&mut self, lod_scale: f32) {
        let lod = lod_scale.clamp(0.25, 1.5);
        let global_cap = (MAX_GLOBAL_PARTICLES as f32 * lod) as usize;
        self.set_on_screen_budget(Some(global_cap));
        let mut caps = [usize::MAX; ParticlePriorityType::NUM_PARTICLE_PRIORITIES];
        for i in 0..ParticlePriorityType::NUM_PARTICLE_PRIORITIES {
            let pri = i as u32;
            let base = match pri {
                13 => usize::MAX,                           // AlwaysRender
                12 => (global_cap as f32 * 0.30) as usize,  // Critical
                11 => (global_cap as f32 * 0.22) as usize,  // AreaEffect
                10 => (global_cap as f32 * 0.18) as usize,  // WeaponTrail
                9 => (global_cap as f32 * 0.16) as usize,   // Constant
                _ => (global_cap as f32 * 0.12) as usize,
            };
            caps[i] = base.max(32);
        }
        self.set_on_screen_budget_per_priority(Some(caps));
    }

    /// Bootstrap defaults using provided camera, resolver, and LOD hints.
    pub fn bootstrap_defaults(
        &mut self,
        camera: Option<&CameraState>,
        resolver: Option<Arc<dyn AttachmentResolver>>,
        lod_scale: f32,
    ) {
        let resolver = resolver.unwrap_or_else(|| Arc::new(GameLogicAttachmentResolver::default()));
        self.bootstrap(camera, Some(resolver), None, None);
        self.apply_device_lod_defaults(lod_scale);
    }

    /// Convenience constructor that applies defaults immediately.
    pub fn new_with_defaults(
        camera: Option<&CameraState>,
        resolver: Option<Arc<dyn AttachmentResolver>>,
        lod_scale: f32,
    ) -> Self {
        let mut mgr = ParticleSystemManager::new();
        mgr.bootstrap_defaults(camera, resolver, lod_scale);
        mgr
    }

    fn next_system_id(&mut self) -> ParticleSystemID {
        let id = self.unique_system_id.wrapping_add(1);
        self.unique_system_id = id;
        id
    }

    fn create_system_internal(
        &mut self,
        template: ParticleSystemTemplate,
        create_slaves: bool,
    ) -> Arc<Mutex<ParticleSystem>> {
        let id = self.next_system_id();
        let sys = Arc::new(Mutex::new(ParticleSystem::new(template.clone(), id)));
        self.systems.push(sys.clone());

        if create_slaves {
            if let Some(slave_name) = template.slave_system_template.clone() {
                if let Some(slave_tmpl) = self
                    .templates
                    .iter()
                    .find(|(n, _)| n == &slave_name)
                    .map(|(_, t)| t.clone())
                {
                    let slave = self.create_system_internal(slave_tmpl, false);
                    if let (Ok(mut master), Ok(mut slave_ps)) = (sys.lock(), slave.lock()) {
                        master.slave_system_id = Some(slave_ps.get_system_id());
                        slave_ps.master_system_id = Some(master.get_system_id());
                    }
                }
            }
        }

        sys
    }

    pub fn create_system(&mut self, template: ParticleSystemTemplate) -> Arc<Mutex<ParticleSystem>> {
        self.create_system_internal(template, true)
    }

    pub fn register_template(&mut self, name: String, template: ParticleSystemTemplate) {
        self.templates.retain(|(n, _)| n != &name);
        self.templates.push((name, template));
    }

    pub fn create_from_template(&mut self, name: &str) -> Option<Arc<Mutex<ParticleSystem>>> {
        let template = self
            .templates
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())?;
        Some(self.create_system_internal(template, true))
    }

    pub fn destroy_system(&mut self, system_id: ParticleSystemID) {
        self.systems.retain(|s| s.lock().map(|ps| ps.get_system_id() != system_id).unwrap_or(false));
    }

    pub fn find_system(&self, system_id: ParticleSystemID) -> Option<Arc<Mutex<ParticleSystem>>> {
        self.systems
            .iter()
            .find(|s| s.lock().map(|ps| ps.get_system_id() == system_id).unwrap_or(false))
            .cloned()
    }

    pub fn update(&mut self, current_frame: u32) {
        let mut idx = 0;
        while idx < self.systems.len() {
            let sys_arc = self.systems[idx].clone();
            let keep = if let Ok(mut ps) = sys_arc.lock() {
                ps.update_with_manager(current_frame, &sys_arc, self)
            } else {
                false
            };

            if !keep {
                self.systems.remove(idx);
            } else {
                idx += 1;
            }
        }
        self.enforce_global_limit();

        // Optional frustum cull of particles
        if let Some(frustum) = &self.frustum {
            for sys in &self.systems {
                if let Ok(mut ps) = sys.lock() {
                    let sys_pos = ps.get_world_position();
                    let p_sys = Vec3::new(sys_pos.x, sys_pos.y, sys_pos.z);
                    ps.particles.iter_mut().for_each(|part| {
                        let radius = part.get_size().max(0.5);
                        let pos = part.get_position();
                        // Particle positions are already world-space; adjust by system origin if needed
                        let v = Vec3::new(pos.x, pos.y, pos.z);
                        let world = v + p_sys;
                        part.set_is_culled(!frustum.contains_point(world, radius));
                    });
                }
            }
        }
    }

    pub fn reset(&mut self) {
        self.systems.clear();
        self.unique_system_id = 1;
        self.templates.clear();
        self.attachment_resolver = Some(Arc::new(GameLogicAttachmentResolver::default()));
    }

    fn enforce_global_limit(&mut self) {
        let mut total: usize = self
            .systems
            .iter()
            .filter_map(|s| s.lock().ok().map(|ps| ps.particle_count()))
            .sum();
        let cap = self
            .on_screen_budget
            .map(|b| b.min(MAX_GLOBAL_PARTICLES))
            .unwrap_or(MAX_GLOBAL_PARTICLES);
        if total <= cap {
            return;
        }

        // Cull based on priority (lowest first), skipping ALWAYS_RENDER.
        let mut indices: Vec<usize> = (0..self.systems.len()).collect();
        indices.sort_by_key(|idx| {
            self.systems
                .get(*idx)
                .and_then(|s| s.lock().ok())
                .map(|ps| {
                    let pri = ps.get_priority() as u32;
                    let budget = (ps.lod_scale * ps.fps_budget_factor * 1000.0) as u32;
                    let dist = self.camera_pos.map(|cam| {
                        let p = ps.get_world_position();
                        let v = Vec3::new(p.x, p.y, p.z);
                        ((v - cam).length() * 100.0) as u32
                    }).unwrap_or(0);
                    (pri, budget, dist, ps.get_system_id())
                })
                .unwrap_or((ParticlePriorityType::AlwaysRender as u32, u32::MAX, 0, 0))
        });

        while total > cap {
            let mut culled = false;
            for idx in indices.iter().copied() {
                if let Some(sys) = self.systems.get(idx) {
                    if let Ok(mut ps) = sys.lock() {
                        if ps.get_priority() == ParticlePriorityType::AlwaysRender {
                            continue;
                        }
                        let pri_idx = ps.get_priority() as usize;
                        let per_pri_cap = self
                            .on_screen_budget_per_priority
                            .and_then(|arr| arr.get(pri_idx).copied());
                        if let Some(pri_cap) = per_pri_cap {
                            if ps.particle_count() as usize <= pri_cap {
                                continue;
                            }
                        }
                        if ps.particle_count() > 0 {
                            ps.discard_oldest_particle();
                            total = total.saturating_sub(1);
                            culled = true;
                            if total <= cap {
                                break;
                            }
                        }
                    }
                }
            }
            if !culled {
                break;
            }
        }
    }
}

impl Snapshotable for ParticleSystemManager {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u32 = 1;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;

        xfer.xfer_unsigned_int(&mut self.unique_system_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.local_player_index)
            .map_err(|e| e.to_string())?;
        let mut budget = self.on_screen_budget.unwrap_or(0) as u32;
        xfer.xfer_unsigned_int(&mut budget).map_err(|e| e.to_string())?;
        self.on_screen_budget = if budget == 0 { None } else { Some(budget as usize) };
        // frustum is runtime-only; not serialized

        // Templates
        let mut tmpl_count = self.templates.len() as u32;
        xfer.xfer_unsigned_int(&mut tmpl_count)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.templates.clear();
            for _ in 0..tmpl_count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name).map_err(|e| e.to_string())?;
                let mut tmpl = ParticleSystemTemplate::default();
                tmpl.xfer(xfer)?;
                self.templates.push((name, tmpl));
            }
        } else {
            for (name, tmpl) in self.templates.iter_mut() {
                let mut n = name.clone();
                xfer.xfer_ascii_string(&mut n).map_err(|e| e.to_string())?;
                tmpl.xfer(xfer)?;
            }
        }

        // Systems
        let mut sys_count = self.systems.len() as u32;
        xfer.xfer_unsigned_int(&mut sys_count)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.systems.clear();
            for _ in 0..sys_count {
                let mut ps = ParticleSystem::new(ParticleSystemTemplate::default(), 0);
                ps.xfer(xfer)?;
                self.systems.push(Arc::new(Mutex::new(ps)));
            }
        } else {
            for sys in self.systems.iter_mut() {
                if let Ok(mut ps) = sys.lock() {
                    ps.xfer(xfer)?;
                }
            }
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        for (_, tmpl) in self.templates.iter_mut() {
            tmpl.load_post_process()?;
        }
        for sys in self.systems.iter() {
            if let Ok(mut ps) = sys.lock() {
                ps.load_post_process()?;
            }
        }
        if self.attachment_resolver.is_none() {
            self.attachment_resolver = Some(Arc::new(GameLogicAttachmentResolver::default()));
        }
        Ok(())
    }
}

/// Compute angle between two 2D vectors
/// Helper function for particle orientation
fn angle_between(vec_a: &Coord2D, vec_b: &Coord2D) -> f32 {
    let dot = vec_a.x * vec_b.x + vec_a.y * vec_b.y;
    let det = vec_a.x * vec_b.y - vec_a.y * vec_b.x;
    det.atan2(dot)
}

fn xfer_coord3d(xfer: &mut dyn Xfer, c: &mut Coord3D) -> Result<(), String> {
    xfer.xfer_real(&mut c.x).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut c.y).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut c.z).map_err(|e| e.to_string())?;
    Ok(())
}

fn mat4_from_matrix3d(m: &Matrix3D) -> Mat4 {
    // Matrix3D stores rows; Mat4 expects column-major.
    let r0 = m.row[0];
    let r1 = m.row[1];
    let r2 = m.row[2];
        Mat4::from_cols_array(&[
            r0.x, r1.x, r2.x, 0.0, //
            r0.y, r1.y, r2.y, 0.0, //
            r0.z, r1.z, r2.z, 0.0, //
            r0.w, r1.w, r2.w, 1.0,
        ])
}

impl Snapshotable for Particle {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u32 = 1;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        xfer_coord3d(xfer, &mut self.pos)?;
        xfer_coord3d(xfer, &mut self.vel)?;
        xfer_coord3d(xfer, &mut self.emitter_pos)?;
        xfer.xfer_real(&mut self.vel_damping).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.angle_z).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.angular_rate_z).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.angular_damping).map_err(|e| e.to_string())?;
        let mut lt = self.lifetime as i32;
        xfer.xfer_int(&mut lt).map_err(|e| e.to_string())?;
        self.lifetime = lt.max(0) as u32;
        xfer.xfer_real(&mut self.size).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.size_rate).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.size_rate_damping)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.color_scale).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_randomness)
            .map_err(|e| e.to_string())?;
        let mut particle_up = self.particle_up_towards_emitter;
        xfer.xfer_bool(&mut particle_up).map_err(|e| e.to_string())?;
        self.particle_up_towards_emitter = particle_up;

        // alpha/color keyframes
        for k in self.alpha_key.iter_mut() {
            xfer.xfer_real(&mut k.value).map_err(|e| e.to_string())?;
            let mut frame = k.frame as i32;
            xfer.xfer_int(&mut frame).map_err(|e| e.to_string())?;
            k.frame = frame.max(0) as u32;
        }
        for k in self.color_key.iter_mut() {
            xfer.xfer_real(&mut k.color.red).map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut k.color.green).map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut k.color.blue).map_err(|e| e.to_string())?;
            let mut frame = k.frame as i32;
            xfer.xfer_int(&mut frame).map_err(|e| e.to_string())?;
            k.frame = frame.max(0) as u32;
        }

        // runtime state
        xfer_coord3d(xfer, &mut self.accel)?;
        xfer_coord3d(xfer, &mut self.last_pos)?;
        let mut lt_left = self.lifetime_left as i32;
        xfer.xfer_int(&mut lt_left).map_err(|e| e.to_string())?;
        self.lifetime_left = lt_left.max(0) as u32;
        let mut create_ts = self.create_timestamp as i32;
        xfer.xfer_int(&mut create_ts).map_err(|e| e.to_string())?;
        self.create_timestamp = create_ts.max(0) as u32;
        xfer.xfer_real(&mut self.alpha).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.alpha_rate).map_err(|e| e.to_string())?;
        let mut alpha_target = self.alpha_target_key as i32;
        xfer.xfer_int(&mut alpha_target).map_err(|e| e.to_string())?;
        self.alpha_target_key = alpha_target.max(0) as usize;
        xfer.xfer_real(&mut self.color.red).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.color.green).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.color.blue).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.color_rate.red).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.color_rate.green)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.color_rate.blue)
            .map_err(|e| e.to_string())?;
        let mut color_target = self.color_target_key as i32;
        xfer.xfer_int(&mut color_target).map_err(|e| e.to_string())?;
        self.color_target_key = color_target.max(0) as usize;
        let mut culled = self.is_culled;
        xfer.xfer_bool(&mut culled).map_err(|e| e.to_string())?;
        self.is_culled = culled;

        let mut has_ctrl = self.system_under_control.is_some();
        xfer.xfer_bool(&mut has_ctrl).map_err(|e| e.to_string())?;
        if has_ctrl {
            let mut ctrl = self
                .system_under_control
                .unwrap_or(INVALID_PARTICLE_SYSTEM_ID);
            xfer.xfer_unsigned_int(&mut ctrl).map_err(|e| e.to_string())?;
            self.system_under_control = Some(ctrl);
        } else {
            self.system_under_control = None;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Recompute derived state
        self.is_culled = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_info_creation() {
        let info = ParticleInfo::new();
        assert_eq!(info.lifetime, 0);
        assert_eq!(info.size, 0.0);
        assert_eq!(info.vel_damping, 0.0);
    }

    #[test]
    fn test_keyframe() {
        let keyframe = Keyframe {
            value: 1.0,
            frame: 10,
        };
        assert_eq!(keyframe.value, 1.0);
        assert_eq!(keyframe.frame, 10);
    }

    #[test]
    fn test_particle_priority_ordering() {
        assert!(ParticlePriorityType::AlwaysRender > ParticlePriorityType::Critical);
        assert!(ParticlePriorityType::Critical > ParticlePriorityType::WeaponExplosion);
    }

    #[test]
    fn test_angle_between() {
        let vec_a = Coord2D { x: 1.0, y: 0.0 };
        let vec_b = Coord2D { x: 0.0, y: 1.0 };
        let angle = angle_between(&vec_a, &vec_b);
        assert!((angle - PI / 2.0).abs() < 0.001);
    }
}

