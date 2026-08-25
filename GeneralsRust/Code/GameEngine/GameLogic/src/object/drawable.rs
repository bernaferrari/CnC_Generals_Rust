//! Drawable class - Visual representation of objects
//!
//! Drawables are the client-side visual representation of game objects.
//! They handle rendering, animation, effects, and visual state management.

#[path = "drawable_core.rs"]
mod drawable_core;
#[path = "drawable_extensions.rs"]
mod drawable_extensions;
#[path = "drawable_physics_visual.rs"]
mod drawable_physics_visual;
#[path = "drawable_render.rs"]
mod drawable_render;
#[path = "drawable_snapshot.rs"]
mod drawable_snapshot;
#[path = "drawable_state.rs"]
mod drawable_state;
#[path = "drawable_transform.rs"]
mod drawable_transform;
#[path = "drawable_update.rs"]
mod drawable_update;

pub(crate) use drawable_extensions::DrawableThingHandle;
pub use drawable_extensions::{DrawableArcExt, DrawableExt, ProjectileLaunchOffset};

use crate::common::ObjectID;
use crate::common::audio::AudioEventRts;
use crate::common::audio::TimeOfDay;
use crate::common::*;
use crate::effects::FXList;
use crate::helpers::TheAudio;
use crate::helpers::{ModelDrawSourceIdentity, TheGameClient, TheGameLogic, TheGlobalData};
use crate::object::body::body_module::BodyDamageType;
use crate::object::draw::draw_module::{
    DebrisDrawInterface, DrawModule, ObjectDrawInterface, RGBColor, ShadowType,
};
use crate::object::draw::{
    TerrainDecalType, W3DDebrisDraw, W3DDebrisDrawModuleData, object_should_animate,
};
use crate::player::ThePlayerList;
use game_engine::System::{get_runtime_drawable_id_counter, set_runtime_drawable_id_counter};
use game_engine::bit_flags::create_model_condition_flags;
use game_engine::common::audio::AudioEventInfo;
use game_engine::common::audio::dynamic_audio_event_info::DynamicAudioEventInfo;
use game_engine::common::audio::game_audio::{
    get_global_audio_manager, initialize_global_audio_manager,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::thing::module::{
    Drawable as ModuleDrawableTrait, Module, ModuleData, ModuleInterfaceType,
    Object as ModuleObjectTrait, Thing as ModuleThing,
};
use glam::{EulerRot, Quat};
use log::warn;
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Types of drawable objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawableType {
    Static,    // Static model (buildings, props)
    Animated,  // Animated model (units, creatures)
    Particle,  // Particle system
    Beam,      // Beam/laser effect
    Decal,     // Ground decal
    Billboard, // Billboard sprite
    Composite, // Composite of multiple drawables
    Effect,    // Special effect
    UI,        // UI element
}

/// Level of detail for rendering optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LevelOfDetail {
    High = 0,     // Full detail
    Medium = 1,   // Reduced detail
    Low = 2,      // Low detail
    Impostor = 3, // Billboard impostor
}

/// Animation states for drawable objects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    Idle,
    Moving,
    Attacking,
    Dying,
    Dead,
    BeingBuilt,
    BeingRepaired,
    Damaged,
    Celebrating,
    Custom(u32), // Custom animation ID
}

/// Stealth look state, mirroring C++ Drawable::StealthLookType behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StealthLookType {
    None,
    Invisible,
    VisibleFriendly,
    VisibleDetected,
    VisibleFriendlyDetected,
    DisguisedEnemy,
    DisguisedFriendly,
    DisguisedNeutral,
}

// Rendering flags for special visual effects
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct RenderFlags: u32 {
        const NONE = 0;
        const CAST_SHADOW = 1 << 0;
        const RECEIVE_SHADOW = 1 << 1;
        const TRANSPARENT = 1 << 2;
        const ADDITIVE_BLEND = 1 << 3;
        const ALPHA_TEST = 1 << 4;
        const DEPTH_WRITE = 1 << 5;
        const WIREFRAME = 1 << 6;
        const NO_CULL = 1 << 7;
        const DOUBLE_SIDED = 1 << 8;
        const GLOW = 1 << 9;
        const REFLECTION = 1 << 10;
        const REFRACTION = 1 << 11;
        const ANIMATED_TEXTURE = 1 << 12;
        const ENVIRONMENT_MAP = 1 << 13;
        const BUMP_MAP = 1 << 14;
        const NORMAL_MAP = 1 << 15;
        const SPECULAR_MAP = 1 << 16;
        const EMISSIVE_MAP = 1 << 17;
        const CLIP_PLANE = 1 << 18;
        const OCCLUDE = 1 << 19;
        const DISTORTION = 1 << 20;
        const HEAT_SHIMMER = 1 << 21;
    }
}

/// Tint status bits (mirrors GameClient Drawable tint flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TintStatus(u32);

impl TintStatus {
    pub const NONE: Self = Self(0x00000000);
    pub const DISABLED: Self = Self(0x00000001);
    pub const IRRADIATED: Self = Self(0x00000002);
    pub const POISONED: Self = Self(0x00000004);
    pub const GAINING_SUBDUAL_DAMAGE: Self = Self(0x00000008);
    pub const FRENZY: Self = Self(0x00000010);

    pub fn is_set(&self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    pub fn set(&mut self, flag: Self) {
        self.0 |= flag.0;
    }

    pub fn clear(&mut self, flag: Self) {
        self.0 &= !flag.0;
    }
}

/// Material properties for rendering
#[derive(Debug, Clone)]
pub struct Material {
    pub diffuse_texture: Option<String>,
    pub normal_texture: Option<String>,
    pub specular_texture: Option<String>,
    pub emissive_texture: Option<String>,
    pub diffuse_color: Color,
    pub specular_color: Color,
    pub emissive_color: Color,
    pub shininess: Real,
    pub transparency: Real,
    pub reflectivity: Real,
    pub texture_scale: Coord2D,
    pub texture_offset: Coord2D,
    pub animation_rate: Real,
}

/// Bone data for skeletal animation
#[derive(Debug, Clone)]
pub struct BoneData {
    pub name: String,
    pub parent_index: i32,
    pub transform: Matrix3D,
    pub inverse_bind_pose: Matrix3D,
}

/// Animation clip data
#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name: String,
    pub duration: Real,
    pub loop_animation: bool,
    pub keyframes: Vec<AnimationKeyframe>,
    pub events: Vec<AnimationEvent>,
}

/// Animation keyframe
#[derive(Debug, Clone)]
pub struct AnimationKeyframe {
    pub time: Real,
    pub bone_transforms: Vec<Matrix3D>,
}

/// Animation event (for triggering sound effects, particles, etc.)
#[derive(Debug, Clone)]
pub struct AnimationEvent {
    pub time: Real,
    pub event_type: String,
    pub parameters: HashMap<String, String>,
}

struct DrawModuleEntry {
    name: AsciiString,
    tag: AsciiString,
    interface_mask: ModuleInterfaceType,
    module_data: Arc<dyn ModuleData>,
    module: Mutex<Box<dyn Module>>,
}

impl fmt::Debug for DrawModuleEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DrawModuleEntry")
            .field("name", &self.name)
            .field("tag", &self.tag)
            .field("interface_mask", &self.interface_mask)
            .finish()
    }
}

impl DrawModuleEntry {
    fn new(
        name: AsciiString,
        tag: AsciiString,
        interface_mask: ModuleInterfaceType,
        module_data: Arc<dyn ModuleData>,
        module: Box<dyn Module>,
    ) -> Self {
        Self {
            name,
            tag,
            interface_mask,
            module_data,
            module: Mutex::new(module),
        }
    }

    fn name(&self) -> &AsciiString {
        &self.name
    }

    fn tag(&self) -> &AsciiString {
        &self.tag
    }

    fn mask(&self) -> ModuleInterfaceType {
        self.interface_mask
    }

    fn data(&self) -> &Arc<dyn ModuleData> {
        &self.module_data
    }

    fn with_module<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&mut dyn Module) -> R,
    {
        let mut guard = self.module.lock().expect("draw module lock poisoned");
        func(guard.as_mut())
    }
}

#[derive(Debug, Clone)]
struct LegacyTintEnvelope {
    attack_rate: Coord3D,
    decay_rate: Coord3D,
    peak_color: Coord3D,
    current_color: Coord3D,
    sustain_counter: u32,
    affect: bool,
    env_state: i8,
}

impl Default for LegacyTintEnvelope {
    fn default() -> Self {
        Self {
            attack_rate: Coord3D::new(0.0, 0.0, 0.0),
            decay_rate: Coord3D::new(0.0, 0.0, 0.0),
            peak_color: Coord3D::new(0.0, 0.0, 0.0),
            current_color: Coord3D::new(0.0, 0.0, 0.0),
            sustain_counter: 0,
            affect: false,
            env_state: 0,
        }
    }
}

impl LegacyTintEnvelope {
    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let current_version: u8 = 1;
        let mut version = current_version;
        let _ = xfer.xfer_version(&mut version, current_version);
        xfer.xfer_coord3d(&mut self.attack_rate);
        xfer.xfer_coord3d(&mut self.decay_rate);
        xfer.xfer_coord3d(&mut self.peak_color);
        xfer.xfer_coord3d(&mut self.current_color);
        let _ = xfer.xfer_unsigned_int(&mut self.sustain_counter);
        let _ = xfer.xfer_bool(&mut self.affect);
        let _ = xfer.xfer_byte(&mut self.env_state);
    }

    fn play(
        &mut self,
        color: Coord3D,
        attack_frames: u32,
        decay_frames: u32,
        sustain_at_peak: bool,
    ) {
        self.peak_color = color;
        self.current_color = color;
        self.affect = true;
        self.sustain_counter = if sustain_at_peak { 1 } else { 0 };
        self.env_state = if attack_frames > 0 { 1 } else { 2 };
        let attack = attack_frames.max(1) as f32;
        let decay = decay_frames.max(1) as f32;
        self.attack_rate = Coord3D::new(color.x / attack, color.y / attack, color.z / attack);
        self.decay_rate = Coord3D::new(color.x / decay, color.y / decay, color.z / decay);
    }

    fn rest(&mut self) {
        self.affect = false;
        self.sustain_counter = 0;
        self.env_state = 0;
        self.current_color = Coord3D::new(0.0, 0.0, 0.0);
        self.peak_color = Coord3D::new(0.0, 0.0, 0.0);
    }
}

#[derive(Debug, Clone)]
struct LegacyDrawableLocoInfo {
    pitch: Real,
    pitch_rate: Real,
    roll: Real,
    roll_rate: Real,
    yaw: Real,
    acceleration_pitch: Real,
    acceleration_pitch_rate: Real,
    acceleration_roll: Real,
    acceleration_roll_rate: Real,
    overlap_z_vel: Real,
    overlap_z: Real,
    wobble: Real,
    wheel_front_left_height_offset: Real,
    wheel_front_right_height_offset: Real,
    wheel_rear_left_height_offset: Real,
    wheel_rear_right_height_offset: Real,
    wheel_angle: Real,
    wheel_frames_airborne_counter: i32,
    wheel_frames_airborne: i32,
}

impl Default for LegacyDrawableLocoInfo {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            pitch_rate: 0.0,
            roll: 0.0,
            roll_rate: 0.0,
            yaw: 0.0,
            acceleration_pitch: 0.0,
            acceleration_pitch_rate: 0.0,
            acceleration_roll: 0.0,
            acceleration_roll_rate: 0.0,
            overlap_z_vel: 0.0,
            overlap_z: 0.0,
            wobble: 0.0,
            wheel_front_left_height_offset: 0.0,
            wheel_front_right_height_offset: 0.0,
            wheel_rear_left_height_offset: 0.0,
            wheel_rear_right_height_offset: 0.0,
            wheel_angle: 0.0,
            wheel_frames_airborne_counter: 0,
            wheel_frames_airborne: 0,
        }
    }
}

impl LegacyDrawableLocoInfo {
    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let _ = xfer.xfer_real(&mut self.pitch);
        let _ = xfer.xfer_real(&mut self.pitch_rate);
        let _ = xfer.xfer_real(&mut self.roll);
        let _ = xfer.xfer_real(&mut self.roll_rate);
        let _ = xfer.xfer_real(&mut self.yaw);
        let _ = xfer.xfer_real(&mut self.acceleration_pitch);
        let _ = xfer.xfer_real(&mut self.acceleration_pitch_rate);
        let _ = xfer.xfer_real(&mut self.acceleration_roll);
        let _ = xfer.xfer_real(&mut self.acceleration_roll_rate);
        let _ = xfer.xfer_real(&mut self.overlap_z_vel);
        let _ = xfer.xfer_real(&mut self.overlap_z);
        let _ = xfer.xfer_real(&mut self.wobble);
        let _ = xfer.xfer_real(&mut self.wheel_front_left_height_offset);
        let _ = xfer.xfer_real(&mut self.wheel_front_right_height_offset);
        let _ = xfer.xfer_real(&mut self.wheel_rear_left_height_offset);
        let _ = xfer.xfer_real(&mut self.wheel_rear_right_height_offset);
        let _ = xfer.xfer_real(&mut self.wheel_angle);
        let _ = xfer.xfer_int(&mut self.wheel_frames_airborne_counter);
        let _ = xfer.xfer_int(&mut self.wheel_frames_airborne);
    }
}

#[derive(Debug, Clone, Default)]
struct LegacyAnim2DState {
    current_frame: u16,
    last_update_frame: u32,
    status_bits: u8,
    min_frame: u16,
    max_frame: u16,
    frames_between_updates: u32,
    alpha: Real,
}

impl LegacyAnim2DState {
    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let current_version: u8 = 1;
        let mut version = current_version;
        let _ = xfer.xfer_version(&mut version, current_version);
        let _ = xfer.xfer_unsigned_short(&mut self.current_frame);
        let _ = xfer.xfer_unsigned_int(&mut self.last_update_frame);
        let _ = xfer.xfer_unsigned_byte(&mut self.status_bits);
        let _ = xfer.xfer_unsigned_short(&mut self.min_frame);
        let _ = xfer.xfer_unsigned_short(&mut self.max_frame);
        let _ = xfer.xfer_unsigned_int(&mut self.frames_between_updates);
        let _ = xfer.xfer_real(&mut self.alpha);
    }
}

#[derive(Debug, Clone, Default)]
struct LegacyDrawableIcon {
    icon_index_name: String,
    keep_till_frame: u32,
    icon_template_name: String,
    icon_state: LegacyAnim2DState,
}

enum DrawModuleKindMut<'a> {
    Model(&'a mut crate::object::draw::W3DModelDraw),
    Tank(&'a mut crate::object::draw::W3DTankDraw),
    TankTruck(&'a mut crate::object::draw::W3DTankTruckDraw),
    OverlordAircraft(&'a mut crate::object::draw::W3DOverlordAircraftDraw),
    OverlordTank(&'a mut crate::object::draw::W3DOverlordTankDraw),
    OverlordTruck(&'a mut crate::object::draw::W3DOverlordTruckDraw),
    Truck(&'a mut crate::object::draw::W3DTruckDraw),
    PoliceCar(&'a mut crate::object::draw::W3DPoliceCarDraw),
    ScienceModel(&'a mut crate::object::draw::W3DScienceModelDraw),
    DependencyModel(&'a mut crate::object::draw::W3DDependencyModelDraw),
    Tracer(&'a mut crate::object::draw::W3DTracerDraw),
    Laser(&'a mut crate::object::draw::W3DLaserDraw),
    Rope(&'a mut crate::object::draw::W3DRopeDraw),
    Projectile(&'a mut crate::object::draw::W3DProjectileDraw),
    ProjectileStream(&'a mut crate::object::draw::W3DProjectileStreamDraw),
    Tree(&'a mut crate::object::draw::W3DTreeDraw),
    Prop(&'a mut crate::object::draw::W3DPropDraw),
    Debris(&'a mut crate::object::draw::W3DDebrisDraw),
    Supply(&'a mut crate::object::draw::W3DSupplyDraw),
    Default(&'a mut crate::object::draw::W3DDefaultDraw),
}

impl<'a> DrawModuleKindMut<'a> {
    fn into_draw_module(self) -> &'a mut dyn DrawModule {
        match self {
            Self::Model(draw) => draw,
            Self::Tank(draw) => draw,
            Self::TankTruck(draw) => draw,
            Self::OverlordAircraft(draw) => draw,
            Self::OverlordTank(draw) => draw,
            Self::OverlordTruck(draw) => draw,
            Self::Truck(draw) => draw,
            Self::PoliceCar(draw) => draw,
            Self::ScienceModel(draw) => draw,
            Self::DependencyModel(draw) => draw,
            Self::Tracer(draw) => draw,
            Self::Laser(draw) => draw,
            Self::Rope(draw) => draw,
            Self::Projectile(draw) => draw,
            Self::ProjectileStream(draw) => draw,
            Self::Tree(draw) => draw,
            Self::Prop(draw) => draw,
            Self::Debris(draw) => draw,
            Self::Supply(draw) => draw,
            Self::Default(draw) => draw,
        }
    }

    fn into_laser_draw(self) -> Option<&'a mut crate::object::draw::W3DLaserDraw> {
        match self {
            Self::Laser(draw) => Some(draw),
            _ => None,
        }
    }

    fn set_terrain_decal(self, decal_type: TerrainDecalType) {
        match self {
            Self::Model(draw) => draw.set_terrain_decal(decal_type),
            Self::Tank(draw) => draw.set_terrain_decal(decal_type),
            Self::TankTruck(draw) => draw.set_terrain_decal(decal_type),
            Self::OverlordAircraft(draw) => draw.set_terrain_decal(decal_type),
            Self::OverlordTank(draw) => draw.set_terrain_decal(decal_type),
            Self::OverlordTruck(draw) => draw.set_terrain_decal(decal_type),
            Self::Truck(draw) => draw.set_terrain_decal(decal_type),
            Self::PoliceCar(draw) => draw.set_terrain_decal(decal_type),
            Self::ScienceModel(draw) => draw.set_terrain_decal(decal_type),
            Self::DependencyModel(draw) => draw.set_terrain_decal(decal_type),
            Self::Tracer(draw) => draw.set_terrain_decal(decal_type),
            Self::Laser(draw) => draw.set_terrain_decal(decal_type),
            Self::Rope(draw) => draw.set_terrain_decal(decal_type),
            Self::Projectile(draw) => draw.set_terrain_decal(decal_type),
            Self::ProjectileStream(draw) => draw.set_terrain_decal(decal_type),
            Self::Tree(draw) => draw.set_terrain_decal(decal_type),
            Self::Prop(draw) => draw.set_terrain_decal(decal_type),
            Self::Debris(draw) => draw.set_terrain_decal(decal_type),
            Self::Supply(draw) => draw.set_terrain_decal(decal_type),
            Self::Default(draw) => draw.set_terrain_decal(decal_type),
        }
    }

    fn bind_owner_id(self, object_id: ObjectID) {
        match self {
            Self::OverlordAircraft(draw) => draw.bind_owner_id(object_id),
            Self::OverlordTank(draw) => draw.bind_owner_id(object_id),
            Self::OverlordTruck(draw) => draw.bind_owner_id(object_id),
            Self::Truck(draw) => draw.bind_owner_id(object_id),
            Self::PoliceCar(draw) => draw.bind_owner_id(object_id),
            Self::ScienceModel(draw) => draw.bind_owner_id(object_id),
            Self::DependencyModel(draw) => draw.bind_owner_id(object_id),
            Self::Model(draw) => draw.bind_owner_id(object_id),
            Self::Tank(draw) => draw.bind_owner_id(object_id),
            Self::TankTruck(draw) => draw.bind_owner_id(object_id),
            Self::Laser(draw) => draw.bind_owner_id(object_id),
            Self::ProjectileStream(draw) => draw.bind_owner_id(object_id),
            Self::Debris(draw) => draw.bind_owner_id(object_id),
            Self::Supply(draw) => draw.bind_owner_id(object_id),
            Self::Projectile(draw) => draw.bind_owner_id(object_id),
            Self::Default(draw) => draw.bind_owner_id(object_id),
            Self::Prop(draw) => draw.bind_owner_id(object_id),
            Self::Tracer(_) | Self::Rope(_) | Self::Tree(_) => {}
        }
    }
}

fn with_draw_module_kind(
    module: &mut dyn Module,
    mut func: impl FnMut(DrawModuleKindMut<'_>),
) -> bool {
    if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DModelDraw>()
    {
        func(DrawModuleKindMut::Model(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DTankDraw>()
    {
        func(DrawModuleKindMut::Tank(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DTankTruckDraw>()
    {
        func(DrawModuleKindMut::TankTruck(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DOverlordAircraftDraw>()
    {
        func(DrawModuleKindMut::OverlordAircraft(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DOverlordTankDraw>()
    {
        func(DrawModuleKindMut::OverlordTank(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DOverlordTruckDraw>()
    {
        func(DrawModuleKindMut::OverlordTruck(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DTruckDraw>()
    {
        func(DrawModuleKindMut::Truck(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DPoliceCarDraw>()
    {
        func(DrawModuleKindMut::PoliceCar(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DScienceModelDraw>()
    {
        func(DrawModuleKindMut::ScienceModel(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DDependencyModelDraw>()
    {
        func(DrawModuleKindMut::DependencyModel(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DTracerDraw>()
    {
        func(DrawModuleKindMut::Tracer(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DLaserDraw>()
    {
        func(DrawModuleKindMut::Laser(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DRopeDraw>()
    {
        func(DrawModuleKindMut::Rope(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DProjectileDraw>()
    {
        func(DrawModuleKindMut::Projectile(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DProjectileStreamDraw>()
    {
        func(DrawModuleKindMut::ProjectileStream(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DTreeDraw>()
    {
        func(DrawModuleKindMut::Tree(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DPropDraw>()
    {
        func(DrawModuleKindMut::Prop(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DDebrisDraw>()
    {
        func(DrawModuleKindMut::Debris(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DSupplyDraw>()
    {
        func(DrawModuleKindMut::Supply(module));
        true
    } else if let Some(module) =
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DDefaultDraw>()
    {
        func(DrawModuleKindMut::Default(module));
        true
    } else {
        false
    }
}

fn set_decal_on_draw_module(module: &mut dyn Module, decal_type: TerrainDecalType) {
    let _ = with_draw_module_kind(module, |draw| {
        draw.set_terrain_decal(decal_type);
    });
}

fn with_draw_module_mut<F>(module: &mut dyn Module, func: F)
where
    F: FnOnce(&mut dyn DrawModule),
{
    let mut func = Some(func);
    let _ = with_draw_module_kind(module, |draw| {
        if let Some(func) = func.take() {
            func(draw.into_draw_module());
        }
    });
}

fn with_object_draw_interface_mut<F>(module: &mut dyn Module, func: F)
where
    F: FnOnce(&mut dyn ObjectDrawInterface),
{
    with_draw_module_mut(module, |draw| {
        if let Some(interface) = draw.get_object_draw_interface_mut() {
            func(interface);
        }
    });
}

fn with_rope_draw_interface_mut<F>(module: &mut dyn Module, func: F)
where
    F: FnOnce(&mut dyn crate::object::draw::draw_module::RopeDrawInterface),
{
    with_draw_module_mut(module, |draw| {
        if let Some(interface) = draw.get_rope_draw_interface_mut() {
            func(interface);
        }
    });
}

fn with_debris_draw_interface_mut<F>(module: &mut dyn Module, func: F)
where
    F: FnOnce(&mut dyn DebrisDrawInterface),
{
    with_draw_module_mut(module, |draw| {
        if let Some(interface) = draw.get_debris_draw_interface_mut() {
            func(interface);
        }
    });
}

/// Animation names applied by OCL `doStuffToObj` via `DebrisDrawInterface::setAnimNames`.
#[derive(Clone, Copy, Debug)]
pub struct DebrisDrawAnims<'a> {
    pub initial: &'a str,
    pub flying: &'a str,
    pub final_anim: &'a str,
    pub final_fx: Option<&'a FXList>,
}

fn packed_color_from_i32(value: i32) -> Color {
    let packed = value as u32;
    Color::new(
        (packed & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 24) & 0xFF) as u8,
    )
}

fn shadow_type_from_bits(bits: u32) -> ShadowType {
    if bits == 0 {
        ShadowType::None
    } else if (bits & SHADOW_VOLUME) != 0 {
        ShadowType::Volume
    } else if (bits & SHADOW_DECAL) != 0 {
        ShadowType::Decal
    } else {
        // Projection / alpha / additive still request a shadow; Decal is the closest GameLogic type.
        ShadowType::Decal
    }
}

/// Apply OCL debris model/anim configuration to a drawable.
///
/// Mirrors C++ `doStuffToObj` walking `obj->getDrawable()->getDrawModules()` and calling
/// `getDebrisDrawInterface()->setModelName` / `setAnimNames`. An empty draw-module list is a
/// no-op, matching C++ iterating a possibly empty NULL-terminated module array.
pub fn apply_debris_draw(
    drawable: &mut Drawable,
    model: &str,
    color: i32,
    shadow: u32,
    anims: Option<DebrisDrawAnims<'_>>,
) -> usize {
    drawable.apply_debris_draw(model, color, shadow, anims)
}

const AC_LOOP: u32 = 0x00000001;
const VERY_TRANSPARENT_MATERIAL_PASS_OPACITY: Real = 0.001;
const MATERIAL_PASS_OPACITY_FADE_SCALAR: Real = 0.8;

#[derive(Debug, Clone)]
pub struct DrawableModuleHandle {
    entry: Arc<DrawModuleEntry>,
}

impl DrawableModuleHandle {
    fn new(entry: Arc<DrawModuleEntry>) -> Self {
        Self { entry }
    }

    pub fn name(&self) -> &AsciiString {
        self.entry.name()
    }

    pub fn tag(&self) -> &AsciiString {
        self.entry.tag()
    }

    pub fn interface_mask(&self) -> ModuleInterfaceType {
        self.entry.mask()
    }

    /// Get laser draw interface when backed by a laser draw module.
    pub fn get_laser_draw_interface(&self) -> Option<Box<dyn std::any::Any>> {
        Some(Box::new(LaserDrawInterfaceHandle {
            entry: Arc::clone(&self.entry),
        }))
    }

    pub fn with_module<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&mut dyn Module) -> R,
    {
        self.entry.with_module(func)
    }

    pub fn with_module_data<F, R>(&self, func: F) -> R
    where
        F: FnOnce(&dyn ModuleData) -> R,
    {
        func(self.entry.data().as_ref())
    }

    pub fn with_object_draw_interface<F, R>(&self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn ObjectDrawInterface) -> R,
    {
        self.entry.with_module(|module| {
            let mut result = None;
            let mut func = Some(func);
            with_object_draw_interface_mut(module, |draw| {
                if let Some(func) = func.take() {
                    result = Some(func(draw));
                }
            });
            result
        })
    }

    pub fn with_debris_draw_interface<F, R>(&self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn DebrisDrawInterface) -> R,
    {
        self.entry.with_module(|module| {
            let mut result = None;
            let mut func = Some(func);
            with_debris_draw_interface_mut(module, |draw| {
                if let Some(func) = func.take() {
                    result = Some(func(draw));
                }
            });
            result
        })
    }

    pub fn module_name_key(&self) -> NameKeyType {
        self.entry
            .with_module(|module| module.get_module_name_key())
    }

    pub fn module_tag_key(&self) -> NameKeyType {
        self.entry
            .with_module(|module| module.get_module_tag_name_key())
    }

    pub fn module_data_arc(&self) -> Arc<dyn ModuleData> {
        Arc::clone(self.entry.data())
    }

    pub fn with_module_downcast<T: 'static, F, R>(&self, func: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.entry
            .with_module(|module| (module as &mut dyn Any).downcast_mut::<T>().map(func))
    }
}

struct LaserDrawInterfaceHandle {
    entry: Arc<DrawModuleEntry>,
}

impl crate::object::draw::draw_module::LaserDrawInterface for LaserDrawInterfaceHandle {
    fn get_laser_template_width(&self) -> Real {
        self.entry.with_module(|module| {
            let mut width = 0.0;
            let _ = with_draw_module_kind(module, |draw| {
                if let Some(laser) = draw.into_laser_draw() {
                    width = laser.get_laser_template_width();
                }
            });
            width
        })
    }
}

static LOCAL_NEXT_DRAWABLE_ID: AtomicU32 = AtomicU32::new(1);

fn normalize_drawable_id(id: DrawableID) -> DrawableID {
    if id == 0 { 1 } else { id }
}

fn next_drawable_id_value(current: DrawableID) -> DrawableID {
    let next = current.wrapping_add(1);
    if next == 0 { 1 } else { next }
}

fn allocate_local_drawable_id() -> DrawableID {
    loop {
        let observed = LOCAL_NEXT_DRAWABLE_ID.load(Ordering::Relaxed);
        let current = normalize_drawable_id(observed);
        let next = next_drawable_id_value(current);
        if LOCAL_NEXT_DRAWABLE_ID
            .compare_exchange(observed, next, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return current;
        }
    }
}

fn xfer_matrix3d_rows_legacy(xfer: &mut dyn Xfer, matrix: &mut Matrix3D) {
    let cols = matrix.to_cols_array();
    let mut row0 = [cols[0], cols[4], cols[8], cols[12]];
    let mut row1 = [cols[1], cols[5], cols[9], cols[13]];
    let mut row2 = [cols[2], cols[6], cols[10], cols[14]];

    for value in &mut row0 {
        let _ = xfer.xfer_real(value);
    }
    for value in &mut row1 {
        let _ = xfer.xfer_real(value);
    }
    for value in &mut row2 {
        let _ = xfer.xfer_real(value);
    }

    let rebuilt_cols = [
        row0[0], row1[0], row2[0], 0.0, row0[1], row1[1], row2[1], 0.0, row0[2], row1[2], row2[2],
        0.0, row0[3], row1[3], row2[3], 1.0,
    ];
    *matrix = Matrix3D::from_cols_array(&rebuilt_cols);
}

fn xfer_matrix3d_legacy(xfer: &mut dyn Xfer, matrix: &mut Matrix3D) {
    // C++ parity: Xfer::xferMatrix3D writes a version byte plus 3x4 matrix rows.
    let current_version: u8 = 1;
    let mut version = current_version;
    let _ = xfer.xfer_version(&mut version, current_version);
    xfer_matrix3d_rows_legacy(xfer, matrix);
}

fn xfer_matrix3d_user_legacy(xfer: &mut dyn Xfer, matrix: &mut Matrix3D) {
    // C++ parity: xferUser(&Matrix3D,sizeof(Matrix3D)) for instance matrices (3x4 rows, no version).
    xfer_matrix3d_rows_legacy(xfer, matrix);
}

fn xfer_model_condition_flags_legacy(xfer: &mut dyn Xfer, flags: &mut ModelConditionFlags) {
    // C++ parity: BitFlags::xfer saves named bits (versioned), not raw bitmasks.
    let current_version: u8 = 1;
    let mut version = current_version;
    let _ = xfer.xfer_version(&mut version, current_version);

    match xfer.get_xfer_mode() {
        game_engine::system::XferMode::Save => {
            let mut named = create_model_condition_flags();
            let bits = flags.bits();
            let max_bits = named.size().min(u128::BITS as usize);
            for i in 0..max_bits {
                if (bits & (1u128 << i)) != 0 {
                    named.set(i, true);
                }
            }

            let mut count = named.count().min(i32::MAX as usize) as i32;
            let _ = xfer.xfer_int(&mut count);
            for i in 0..named.size() {
                if let Some(bit_name) = named.get_bit_name_if_set(i) {
                    let mut token = bit_name.to_string();
                    let _ = xfer.xfer_ascii_string(&mut token);
                }
            }
        }
        game_engine::system::XferMode::Load => {
            let mut named = create_model_condition_flags();
            named.clear();

            let mut count = 0i32;
            let _ = xfer.xfer_int(&mut count);
            for _ in 0..count.max(0) {
                let mut token = String::new();
                let _ = xfer.xfer_ascii_string(&mut token);
                if !named.set_bit_by_name(&token) {
                    panic!(
                        "Drawable::xfer invalid ModelCondition flag token '{}'",
                        token
                    );
                }
            }

            let mut bits: u128 = 0;
            let max_bits = named.size().min(u128::BITS as usize);
            for i in 0..max_bits {
                if named.test(i) {
                    bits |= 1u128 << i;
                }
            }
            *flags = ModelConditionFlags::from_bits_retain(bits);
        }
        game_engine::system::XferMode::Crc => {
            let mut bits = flags.bits();
            xfer_u128_bits(xfer, &mut bits);
            *flags = ModelConditionFlags::from_bits_retain(bits);
        }
        _ => {}
    }
}

fn xfer_u128_bits(xfer: &mut dyn Xfer, value: &mut u128) {
    let mut lo = (*value & 0xFFFF_FFFF_FFFF_FFFF) as u64;
    let mut hi = (*value >> 64) as u64;
    let _ = xfer.xfer_u64(&mut lo);
    let _ = xfer.xfer_u64(&mut hi);
    *value = ((hi as u128) << 64) | (lo as u128);
}

fn color_from_argb_u32(packed: u32) -> Color {
    Color::new(
        (packed & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 24) & 0xFF) as u8,
    )
}

fn terrain_decal_type_to_u32(decal: TerrainDecalType) -> u32 {
    match decal {
        TerrainDecalType::Demoralized => 0,
        TerrainDecalType::Horde => 1,
        TerrainDecalType::HordeWithNationalismUpgrade => 2,
        TerrainDecalType::HordeVehicle => 3,
        TerrainDecalType::HordeWithNationalismUpgradeVehicle => 4,
        TerrainDecalType::Crate => 5,
        TerrainDecalType::HordeWithFanaticismUpgrade => 6,
        TerrainDecalType::ChemSuit => 7,
        TerrainDecalType::None => 8,
        TerrainDecalType::ShadowTexture => 9,
    }
}

fn terrain_decal_type_from_u32(value: u32) -> TerrainDecalType {
    match value {
        0 => TerrainDecalType::Demoralized,
        1 => TerrainDecalType::Horde,
        2 => TerrainDecalType::HordeWithNationalismUpgrade,
        3 => TerrainDecalType::HordeVehicle,
        4 => TerrainDecalType::HordeWithNationalismUpgradeVehicle,
        5 => TerrainDecalType::Crate,
        6 => TerrainDecalType::HordeWithFanaticismUpgrade,
        7 => TerrainDecalType::ChemSuit,
        9 => TerrainDecalType::ShadowTexture,
        _ => TerrainDecalType::None,
    }
}

fn stealth_look_to_u32(look: StealthLookType) -> u32 {
    // C++ Drawable.h ordering:
    // 0 None, 1 VisibleFriendly, 2 DisguisedEnemy, 3 VisibleDetected,
    // 4 VisibleFriendlyDetected, 5 Invisible.
    match look {
        StealthLookType::None => 0u32,
        StealthLookType::VisibleFriendly => 1u32,
        StealthLookType::DisguisedEnemy
        | StealthLookType::DisguisedFriendly
        | StealthLookType::DisguisedNeutral => 2u32,
        StealthLookType::VisibleDetected => 3u32,
        StealthLookType::VisibleFriendlyDetected => 4u32,
        StealthLookType::Invisible => 5u32,
    }
}

fn stealth_look_from_u32(value: u32) -> StealthLookType {
    match value {
        1 => StealthLookType::VisibleFriendly,
        2 => StealthLookType::DisguisedEnemy,
        3 => StealthLookType::VisibleDetected,
        4 => StealthLookType::VisibleFriendlyDetected,
        5 => StealthLookType::Invisible,
        _ => StealthLookType::None,
    }
}

/// Drawable object data and behavior
#[derive(Debug)]
#[allow(dead_code)]
pub struct Drawable {
    /// Unique drawable identifier (matches C++ Drawable::m_id semantics).
    drawable_id: DrawableID,

    /// Associated game object
    object_id: ObjectID,

    /// Back-reference to the owning object (for script/selection integrations).
    object_ref: Option<Weak<RwLock<crate::object::Object>>>,

    /// Drawable classification
    #[allow(dead_code)]
    drawable_type: DrawableType,

    /// Transform and positioning
    transform: Matrix3D,
    instance_matrix: Option<Matrix3D>,
    instance_scale: Real,
    world_position: Coord3D,
    world_rotation: Coord3D,
    world_scale: Coord3D,

    /// Visibility and culling
    is_visible: bool,
    hidden: bool,
    hidden_by_stealth: bool,
    always_visible: bool,   // Never culled
    frustum_culled: bool,   // Currently frustum culled
    occlusion_culled: bool, // Currently occlusion culled
    distance_culled: bool,  // Currently distance culled
    current_lod: LevelOfDetail,
    lod_distances: [Real; 4], // Distance thresholds for LOD levels

    /// Model and geometry
    model_name: String,
    submesh_names: Vec<String>,
    materials: Vec<Material>,
    bounding_box: BoundingBox,
    bounding_sphere: BoundingSphere,

    /// Animation system
    skeleton: Vec<BoneData>,
    animation_clips: HashMap<String, AnimationClip>,
    current_animation: Option<String>,
    animation_time: Real,
    animation_speed: Real,
    animation_state: AnimationState,
    blend_animations: Vec<AnimationBlend>,
    bone_transforms: Vec<Matrix3D>,
    swaying_enabled: bool,

    /// Model conditions (for state-based model switching)
    model_conditions: ModelConditionFlags,
    conditional_models: HashMap<ModelConditionFlags, String>,

    /// Rendering properties
    render_flags: RenderFlags,
    draw_priority: i32, // Render order priority
    alpha: Real,        // Overall transparency
    color_tint: Color,  // Color tinting
    indicator_color: Color,
    selection_flash_envelope: Option<LegacyTintEnvelope>,
    color_tint_envelope: Option<LegacyTintEnvelope>,
    drawable_status_bits: u32,
    tint_status: TintStatus,
    prev_tint_status: TintStatus,
    fade_mode: u32,
    time_elapsed_fade: u32,
    time_to_fade: u32,
    loco_info: Option<LegacyDrawableLocoInfo>,
    flash_count: i32,
    flash_color: Color,
    shroud_status_object_id: ObjectID,
    expiration_date: u32,
    legacy_icons: Vec<LegacyDrawableIcon>,

    /// Lighting
    receives_lighting: bool,
    casts_shadows: bool,
    receives_shadows: bool,
    self_illuminated: Real, // Self-illumination amount

    /// Particle systems
    particle_systems: Vec<ParticleSystem>,

    /// Attachments (weapons, effects, etc.)
    attachments: HashMap<String, Attachment>,

    /// Damage visualization
    damage_states: Vec<DamageState>,
    current_damage_state: usize,

    /// Selection and highlighting
    is_selected: bool,
    selection_circle: Option<SelectionCircle>,
    health_bar: Option<HealthBar>,
    terrain_decal: TerrainDecalType,
    decal_opacity: Real,
    decal_opacity_fade_target: Real,
    decal_opacity_fade_rate: Real,
    drawable_fully_obscured_by_shroud: bool,

    /// Special effects
    active_effects: Vec<VisualEffect>,
    timed_effects: Vec<TimedEffect>,

    /// Registered draw modules
    modules: Vec<Arc<DrawModuleEntry>>,

    /// Performance optimization
    last_update_frame: u32,
    update_frequency: u32, // Update every N frames
    frozen: bool,          // Completely frozen for optimization

    /// Stealth and cloaking
    stealth_factor: Real, // C++ m_stealthOpacity: minimum opacity floor while stealthed
    effective_stealth_opacity: Real, // C++ m_effectiveStealthOpacity
    stealth_look: StealthLookType,
    second_material_pass_opacity: Real,
    cloak_texture: Option<String>,
    distortion_amount: Real,

    /// Environmental effects
    weather_affected: bool,
    wetness_factor: Real,    // For rain effects
    snow_accumulation: Real, // For snow effects

    /// Audio integration
    attached_sounds: Vec<AttachedSound>,
    ambient_sound_handle: u32,
    ambient_sound_enabled: Bool,
    ambient_sound_enabled_from_script: Bool,
    custom_sound_ambient_off: Bool,
    custom_sound_ambient_info: Option<Arc<AudioEventInfo>>,
    custom_sound_ambient_dynamic_info: Option<DynamicAudioEventInfo>,

    /// Terrain adaptation
    terrain_following: bool,
    ground_offset: Real,
    slope_adaptation: Real, // How much to adapt to terrain slope

    /// Screen-space effects
    screen_effects: Vec<ScreenEffect>,
}

/// Animation blending information
#[derive(Debug, Clone)]
pub struct AnimationBlend {
    pub animation_name: String,
    pub weight: Real,
    pub fade_time: Real,
    pub current_fade: Real,
}

/// Bounding box for culling and collision
#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub min: Coord3D,
    pub max: Coord3D,
}

/// Bounding sphere for culling
#[derive(Debug, Clone)]
pub struct BoundingSphere {
    pub center: Coord3D,
    pub radius: Real,
}

/// Particle system attachment
#[derive(Debug, Clone)]
pub struct ParticleSystem {
    pub name: String,
    pub bone_attachment: Option<String>,
    pub offset: Coord3D,
    pub is_active: bool,
    pub parameters: HashMap<String, Real>,
}

/// Attachment for weapons, effects, etc.
#[derive(Debug, Clone)]
pub struct Attachment {
    pub drawable: Arc<RwLock<Drawable>>,
    pub bone_name: String,
    pub offset: Coord3D,
    pub rotation: Coord3D,
    pub scale: Coord3D,
}

/// Damage state visualization
#[derive(Debug, Clone)]
pub struct DamageState {
    pub health_threshold: Real,
    pub model_override: Option<String>,
    pub texture_overrides: HashMap<String, String>,
    pub particle_effects: Vec<String>,
    pub color_tint: Color,
    pub alpha_override: Option<Real>,
}

/// Selection circle visualization
#[derive(Debug, Clone)]
pub struct SelectionCircle {
    pub radius: Real,
    pub color: Color,
    pub texture: String,
    pub animation_speed: Real,
}

/// Health bar visualization
#[derive(Debug, Clone)]
pub struct HealthBar {
    pub offset: Coord3D,
    pub size: Coord2D,
    pub background_color: Color,
    pub health_color: Color,
    pub border_color: Color,
    pub always_visible: bool,
}

/// Visual effect instance
#[derive(Debug, Clone)]
pub struct VisualEffect {
    pub effect_type: String,
    pub bone_attachment: Option<String>,
    pub offset: Coord3D,
    pub scale: Real,
    pub color: Color,
    pub parameters: HashMap<String, Real>,
}

/// Timed visual effect
#[derive(Debug, Clone)]
pub struct TimedEffect {
    pub effect: VisualEffect,
    pub duration: Real,
    pub elapsed_time: Real,
    pub fade_in_time: Real,
    pub fade_out_time: Real,
}

/// Sound attached to the drawable
#[derive(Debug, Clone)]
pub struct AttachedSound {
    pub sound_name: String,
    pub bone_attachment: Option<String>,
    pub offset: Coord3D,
    pub volume: Real,
    pub pitch: Real,
    pub loop_sound: bool,
    pub is_playing: bool,
}

/// Screen-space effect
#[derive(Debug, Clone)]
pub struct ScreenEffect {
    pub effect_type: String,
    pub intensity: Real,
    pub duration: Real,
    pub parameters: HashMap<String, Real>,
}

impl ModuleDrawableTrait for Drawable {
    fn get_drawable_id(&self) -> u32 {
        self.drawable_id
    }
}

impl ModuleThing for Drawable {
    fn as_object(&self) -> Option<&dyn ModuleObjectTrait> {
        None
    }

    fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
        Some(self)
    }
}

fn interpolate_bone_transforms(previous: &[Matrix3D], next: &[Matrix3D], t: Real) -> Vec<Matrix3D> {
    let len = previous.len().max(next.len());
    let mut transforms = Vec::with_capacity(len);
    for index in 0..len {
        let prev = previous.get(index).copied();
        let next = next.get(index).copied();
        transforms.push(match (prev, next) {
            (Some(prev), Some(next)) => interpolate_transform(prev, next, t),
            (Some(prev), None) => prev,
            (None, Some(next)) => next,
            (None, None) => Matrix3D::IDENTITY,
        });
    }
    transforms
}

fn interpolate_transform(previous: Matrix3D, next: Matrix3D, t: Real) -> Matrix3D {
    let (prev_scale, prev_rotation, prev_translation) = previous.to_scale_rotation_translation();
    let (next_scale, next_rotation, next_translation) = next.to_scale_rotation_translation();

    let scale = prev_scale.lerp(next_scale, t);
    let rotation = normalized_slerp(prev_rotation, next_rotation, t);
    let translation = prev_translation.lerp(next_translation, t);
    Matrix3D::from_scale_rotation_translation(scale, rotation, translation)
}

fn normalized_slerp(previous: Quat, next: Quat, t: Real) -> Quat {
    let rotation = previous.slerp(next, t);
    if rotation.is_finite() {
        rotation
    } else {
        Quat::IDENTITY
    }
}
#[cfg(test)]
mod fade_in_tests {
    use super::*;
    use crate::common::INVALID_ID;

    #[test]
    fn fade_in_reaches_full_opacity_after_requested_frames() {
        let mut drawable = Drawable::new(
            42,
            INVALID_ID,
            "FadeUnit".to_string(),
            DrawableType::Animated,
        );
        drawable.fade_in(10);

        assert_eq!(drawable.fading_mode(), Drawable::FADING_IN);
        assert_eq!(drawable.time_to_fade(), 10);
        assert_eq!(drawable.time_elapsed_fade(), 0);
        assert!((drawable.get_explicit_opacity() - 0.0).abs() < 0.0001);
        assert!(drawable.is_fading());

        for i in 0..10 {
            drawable.update_fade();
            assert_eq!(drawable.fading_mode(), Drawable::FADING_IN);
            let expected = i as f32 / 10.0;
            assert!(
                (drawable.get_explicit_opacity() - expected).abs() < 0.0001,
                "tick {i}: opacity {} expected {expected}",
                drawable.get_explicit_opacity()
            );
        }

        drawable.update_fade();
        assert!((drawable.get_explicit_opacity() - 1.0).abs() < 0.0001);
        assert!((drawable.get_effective_opacity() - 1.0).abs() < 0.0001);
        assert_eq!(drawable.fading_mode(), Drawable::FADING_NONE);
        assert!(!drawable.is_fading());
    }

    #[test]
    fn fade_out_reaches_zero_opacity_after_requested_frames() {
        let mut drawable = Drawable::new(
            43,
            INVALID_ID,
            "FadeOutUnit".to_string(),
            DrawableType::Static,
        );
        drawable.fade_out(10);
        assert!((drawable.get_explicit_opacity() - 1.0).abs() < 0.0001);
        assert_eq!(drawable.fading_mode(), Drawable::FADING_OUT);

        for i in 0..10 {
            drawable.update_fade();
            let expected = (10 - i) as f32 / 10.0;
            assert!((drawable.get_explicit_opacity() - expected).abs() < 0.0001);
        }

        drawable.update_fade();
        assert!((drawable.get_explicit_opacity() - 0.0).abs() < 0.0001);
        assert_eq!(drawable.fading_mode(), Drawable::FADING_NONE);
    }
}

#[cfg(test)]
mod react_to_body_damage_tests {
    use super::*;
    use crate::object::body::body_module::BodyDamageType;

    fn damage_bits(state: BodyDamageType) -> ModelConditionFlags {
        match state {
            BodyDamageType::Pristine => ModelConditionFlags::empty(),
            BodyDamageType::Damaged => ModelConditionFlags::DAMAGED,
            BodyDamageType::ReallyDamaged => ModelConditionFlags::REALLYDAMAGED,
            BodyDamageType::Rubble => ModelConditionFlags::RUBBLE,
        }
    }

    #[test]
    fn react_to_body_damage_sets_and_clears_condition_bits() {
        let mut drawable = Drawable::new(
            1,
            INVALID_ID,
            "TestUnit".to_string(),
            DrawableType::Animated,
        );

        // Seed an unrelated flag that must survive damage transitions.
        drawable.set_model_condition_state(ModelConditionFlags::MOVING);
        // Seed a stale damage bit that must be cleared when state changes.
        drawable.set_model_condition_state(ModelConditionFlags::DAMAGED);

        drawable.react_to_body_damage_state_change(BodyDamageType::ReallyDamaged);
        let flags = drawable.get_model_conditions();
        assert!(
            flags.contains(ModelConditionFlags::REALLYDAMAGED),
            "ReallyDamaged must set REALLYDAMAGED"
        );
        assert!(
            !flags.contains(ModelConditionFlags::DAMAGED),
            "ReallyDamaged must clear DAMAGED"
        );
        assert!(
            !flags.contains(ModelConditionFlags::RUBBLE),
            "ReallyDamaged must clear RUBBLE"
        );
        assert!(
            flags.contains(ModelConditionFlags::MOVING),
            "non-damage flags must survive clear-and-set"
        );

        drawable.react_to_body_damage_state_change(BodyDamageType::Rubble);
        let flags = drawable.get_model_conditions();
        assert!(flags.contains(ModelConditionFlags::RUBBLE));
        assert!(!flags.contains(ModelConditionFlags::REALLYDAMAGED));
        assert!(flags.contains(ModelConditionFlags::MOVING));

        drawable.react_to_body_damage_state_change(BodyDamageType::Pristine);
        let flags = drawable.get_model_conditions();
        assert_eq!(
            flags
                & (ModelConditionFlags::DAMAGED
                    | ModelConditionFlags::REALLYDAMAGED
                    | ModelConditionFlags::RUBBLE),
            ModelConditionFlags::empty(),
            "Pristine clears all three damage condition bits"
        );
        assert!(flags.contains(ModelConditionFlags::MOVING));
    }

    #[test]
    fn react_to_body_damage_map_matches_cpp_damage_map() {
        for state in [
            BodyDamageType::Pristine,
            BodyDamageType::Damaged,
            BodyDamageType::ReallyDamaged,
            BodyDamageType::Rubble,
        ] {
            let mut drawable =
                Drawable::new(2, INVALID_ID, "DamageMap".to_string(), DrawableType::Static);
            // Pre-set all three so we can observe exclusive set.
            drawable.set_model_condition_state(
                ModelConditionFlags::DAMAGED
                    | ModelConditionFlags::REALLYDAMAGED
                    | ModelConditionFlags::RUBBLE,
            );
            drawable.react_to_body_damage_state_change(state);
            let flags = drawable.get_model_conditions();
            let expected = damage_bits(state);
            let damage_mask = ModelConditionFlags::DAMAGED
                | ModelConditionFlags::REALLYDAMAGED
                | ModelConditionFlags::RUBBLE;
            assert_eq!(
                flags & damage_mask,
                expected,
                "damage map mismatch for {:?}",
                state
            );
        }
    }
}

#[cfg(test)]
mod shadow_status_tests {
    use super::*;
    use crate::object::body::body_module::BodyDamageType;
    use crate::object::draw::draw_module::DrawModule as LogicDrawModule;
    use crate::object::draw::w3d_model_draw::{W3DModelDraw, W3DModelDrawModuleData};

    const DRAWABLE_STATUS_SHADOWS: u32 = 0x00000002;

    #[test]
    fn shadow_status_enabled_after_create() {
        let drawable = Drawable::new(
            10,
            INVALID_ID,
            "ShadowUnit".to_string(),
            DrawableType::Animated,
        );
        assert!(
            drawable.get_shadows_enabled(),
            "create seeds DRAWABLE_STATUS_SHADOWS for observability"
        );
        assert!(drawable.test_drawable_status(DRAWABLE_STATUS_SHADOWS));
        assert!(drawable.casts_shadows);
    }

    #[test]
    fn set_shadows_enabled_toggles_status_bit() {
        let mut drawable = Drawable::new(
            11,
            INVALID_ID,
            "ShadowToggle".to_string(),
            DrawableType::Static,
        );
        assert!(drawable.get_shadows_enabled());

        drawable.set_shadows_enabled(false);
        assert!(!drawable.get_shadows_enabled());
        assert!(!drawable.test_drawable_status(DRAWABLE_STATUS_SHADOWS));

        drawable.set_shadows_enabled(true);
        assert!(drawable.get_shadows_enabled());
    }

    #[test]
    fn allocate_release_shadows_do_not_flip_status() {
        let mut drawable = Drawable::new(
            12,
            INVALID_ID,
            "ShadowAlloc".to_string(),
            DrawableType::Static,
        );
        drawable.set_shadows_enabled(false);
        drawable.allocate_shadows();
        assert!(
            !drawable.get_shadows_enabled(),
            "allocate_shadows is fail-closed resource hook — must not set status"
        );

        drawable.set_shadows_enabled(true);
        drawable.release_shadows();
        assert!(
            drawable.get_shadows_enabled(),
            "release_shadows is fail-closed resource hook — must not clear status"
        );
    }

    #[test]
    fn model_condition_change_preserves_shadow_status() {
        let mut drawable = Drawable::new(
            13,
            INVALID_ID,
            "ShadowCondition".to_string(),
            DrawableType::Animated,
        );
        assert!(drawable.get_shadows_enabled());

        drawable.react_to_body_damage_state_change(BodyDamageType::Damaged);
        assert!(
            drawable.get_shadows_enabled(),
            "model condition damage bits must not clear SHADOWS status"
        );

        drawable.react_to_body_damage_state_change(BodyDamageType::Rubble);
        assert!(drawable.get_shadows_enabled());

        drawable.set_shadows_enabled(false);
        drawable.react_to_body_damage_state_change(BodyDamageType::Pristine);
        assert!(
            !drawable.get_shadows_enabled(),
            "explicit disable survives condition updates"
        );
    }

    #[test]
    fn w3d_model_draw_shadow_enabled_after_create() {
        // C++ W3DModelDraw ctor: m_shadowEnabled = TRUE.
        let mut draw = W3DModelDraw::new(W3DModelDrawModuleData::new());
        // Fail-closed residual: enable bookkeeping only (no GPU mesh).
        LogicDrawModule::set_shadows_enabled(&mut draw, false);
        LogicDrawModule::set_shadows_enabled(&mut draw, true);
        LogicDrawModule::allocate_shadows(&mut draw);
        LogicDrawModule::release_shadows(&mut draw);

        let drawable = Drawable::new(
            14,
            INVALID_ID,
            "ModelDrawShadow".to_string(),
            DrawableType::Animated,
        );
        assert!(drawable.get_shadows_enabled());
    }
}

#[cfg(test)]
mod debris_draw_tests {
    use super::*;
    use crate::common::INVALID_ID;
    use crate::object::draw::W3DDebrisDraw;

    fn make_drawable() -> Drawable {
        Drawable::new(
            77,
            INVALID_ID,
            "DebrisChunk".to_string(),
            DrawableType::Static,
        )
    }

    fn read_debris_model(drawable: &Drawable) -> Option<String> {
        drawable
            .module_by_name(&AsciiString::from("W3DDebrisDraw"))
            .and_then(|handle| {
                handle.with_module_downcast::<W3DDebrisDraw, _, _>(|draw| {
                    draw.model_name().as_str().to_string()
                })
            })
    }

    #[test]
    fn apply_debris_draw_is_noop_when_no_draw_modules() {
        let mut drawable = make_drawable();
        assert!(drawable.draw_modules().is_empty());
        let applied = apply_debris_draw(&mut drawable, "EXDebrisChunk", 0, SHADOW_DECAL, None);
        assert_eq!(applied, 0);
        assert!(read_debris_model(&drawable).is_none());
    }

    #[test]
    fn debris_draw_iteration_sets_model_name() {
        let mut drawable = make_drawable();
        drawable.attach_w3d_debris_draw();

        let color = Color::new(0x66, 0x33, 0xFF, 0x00);
        drawable.for_each_debris_draw_interface(|di| {
            di.set_model_name(AsciiString::from("EXDebrisChunk"), color, ShadowType::Decal);
        });

        let handle = drawable
            .module_by_name(&AsciiString::from("W3DDebrisDraw"))
            .expect("W3DDebrisDraw attached");
        handle
            .with_module_downcast::<W3DDebrisDraw, _, _>(|draw| {
                assert_eq!(draw.model_name().as_str(), "EXDebrisChunk");
                assert_eq!(draw.model_color(), color);
                assert_eq!(draw.shadow_type(), ShadowType::Decal);
            })
            .expect("downcast W3DDebrisDraw");
    }

    #[test]
    fn apply_debris_draw_sets_model_and_anims() {
        let mut drawable = make_drawable();
        drawable.attach_w3d_debris_draw();

        let color = Color::new(0x11, 0x22, 0x33, 0x44);
        let applied = apply_debris_draw(
            &mut drawable,
            "EXRockChunk",
            color.to_argb_u32() as i32,
            SHADOW_VOLUME,
            Some(DebrisDrawAnims {
                initial: "AnimInit",
                flying: "AnimFly",
                final_anim: "AnimLand",
                final_fx: None,
            }),
        );
        assert_eq!(applied, 1);
        assert_eq!(read_debris_model(&drawable).as_deref(), Some("EXRockChunk"));

        drawable
            .module_by_name(&AsciiString::from("W3DDebrisDraw"))
            .expect("W3DDebrisDraw attached")
            .with_module_downcast::<W3DDebrisDraw, _, _>(|draw| {
                assert_eq!(draw.model_name().as_str(), "EXRockChunk");
                assert_eq!(draw.model_color(), color);
                assert_eq!(draw.shadow_type(), ShadowType::Volume);
                assert_eq!(draw.anim_initial().as_str(), "AnimInit");
                assert_eq!(draw.anim_flying().as_str(), "AnimFly");
                assert_eq!(draw.anim_final().as_str(), "AnimLand");
            })
            .expect("downcast W3DDebrisDraw");
    }
}

#[cfg(test)]
mod draw_call_log {
    use crate::common::DrawableID;
    use std::cell::RefCell;

    thread_local! {
        static LOG: RefCell<Vec<(DrawableID, u32)>> = RefCell::new(Vec::new());
        static DEPTH: RefCell<u32> = RefCell::new(0);
    }

    pub struct DrawDepthGuard;

    impl Drop for DrawDepthGuard {
        fn drop(&mut self) {
            DEPTH.with(|d| {
                let mut depth = d.borrow_mut();
                *depth = depth.saturating_sub(1);
            });
        }
    }

    pub fn enter(id: DrawableID) -> DrawDepthGuard {
        let depth = DEPTH.with(|d| {
            let mut depth = d.borrow_mut();
            *depth += 1;
            *depth
        });
        LOG.with(|log| log.borrow_mut().push((id, depth)));
        DrawDepthGuard
    }

    pub fn take() -> Vec<(DrawableID, u32)> {
        LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
    }
}

#[cfg(test)]
mod draw_schedule_tests {
    use super::*;
    use crate::common::INVALID_ID;
    use crate::drawable::Drawable as DrawableTrait;

    #[test]
    fn rider_nested_draw_reenters_drawable_draw() {
        let tank = include_str!("draw/w3d_overlord_tank_draw.rs");
        let truck = include_str!("draw/w3d_overlord_truck_draw.rs");
        let air = include_str!("draw/w3d_overlord_aircraft_draw.rs");
        let rider = include_str!("draw/overlord_rider.rs");
        // The three C++ twin modules share the helper, which owns the actual
        // direct rider draw. Keep the source-level guard aligned with that
        // intentional deduplication rather than requiring duplicated calls.
        assert!(tank.contains("draw_overlord_rider"));
        assert!(truck.contains("draw_overlord_rider"));
        assert!(air.contains("draw_overlord_rider"));
        assert!(rider.contains("rider_guard.draw(None)"));

        let mut parent = Drawable::new(10, INVALID_ID, "Overlord".into(), DrawableType::Static);
        let mut rider = Drawable::new(11, INVALID_ID, "Helix".into(), DrawableType::Static);
        let _ = draw_call_log::take();
        parent.draw(None);
        rider.draw(None);
        let log = draw_call_log::take();
        assert_eq!(log, vec![(10, 1), (11, 1)]);

        let _ = draw_call_log::take();
        {
            let _outer = draw_call_log::enter(10);
            rider.draw(None);
        }
        let nested = draw_call_log::take();
        assert_eq!(nested, vec![(10, 1), (11, 2)]);
    }

    #[test]
    fn physics_seam_runs_once_per_drawable_draw() {
        let mut drawable = Drawable::new(21, INVALID_ID, "Tank".into(), DrawableType::Static);
        let _ = draw_call_log::take();
        drawable.draw(None);
        drawable.draw(None);
        let log = draw_call_log::take();
        assert_eq!(log, vec![(21, 1), (21, 1)]);
    }
}
