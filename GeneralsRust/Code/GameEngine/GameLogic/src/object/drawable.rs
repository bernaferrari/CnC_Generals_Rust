//! Drawable class - Visual representation of objects
//!
//! Drawables are the client-side visual representation of game objects.
//! They handle rendering, animation, effects, and visual state management.

use crate::common::audio::AudioEventRts;
use crate::common::audio::TimeOfDay;
use crate::common::ObjectID;
use crate::common::*;
use crate::helpers::TheAudio;
use crate::helpers::{TheGameLogic, TheGlobalData};
use crate::object::body::body_module::BodyDamageType;
use crate::object::draw::draw_module::{DrawModule, ObjectDrawInterface, RGBColor};
use crate::object::draw::TerrainDecalType;
use crate::player::ThePlayerList;
use game_engine::bit_flags::create_model_condition_flags;
use game_engine::common::audio::dynamic_audio_event_info::DynamicAudioEventInfo;
use game_engine::common::audio::game_audio::{
    get_global_audio_manager, initialize_global_audio_manager,
};
use game_engine::common::audio::AudioEventInfo;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::thing::module::{
    Drawable as ModuleDrawableTrait, Module, ModuleData, ModuleInterfaceType,
    Object as ModuleObjectTrait, Thing as ModuleThing,
};
use game_engine::System::{get_runtime_drawable_id_counter, set_runtime_drawable_id_counter};
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
    OverlordTank(&'a mut crate::object::draw::W3DOverlordTankDraw),
    Tracer(&'a mut crate::object::draw::W3DTracerDraw),
    Laser(&'a mut crate::object::draw::W3DLaserDraw),
    Rope(&'a mut crate::object::draw::W3DRopeDraw),
    Projectile(&'a mut crate::object::draw::W3DProjectileDraw),
    ProjectileStream(&'a mut crate::object::draw::W3DProjectileStreamDraw),
    Tree(&'a mut crate::object::draw::W3DTreeDraw),
    Debris(&'a mut crate::object::draw::W3DDebrisDraw),
}

impl<'a> DrawModuleKindMut<'a> {
    fn into_draw_module(self) -> &'a mut dyn DrawModule {
        match self {
            Self::Model(draw) => draw,
            Self::Tank(draw) => draw,
            Self::OverlordTank(draw) => draw,
            Self::Tracer(draw) => draw,
            Self::Laser(draw) => draw,
            Self::Rope(draw) => draw,
            Self::Projectile(draw) => draw,
            Self::ProjectileStream(draw) => draw,
            Self::Tree(draw) => draw,
            Self::Debris(draw) => draw,
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
            Self::OverlordTank(draw) => draw.set_terrain_decal(decal_type),
            Self::Tracer(draw) => draw.set_terrain_decal(decal_type),
            Self::Laser(draw) => draw.set_terrain_decal(decal_type),
            Self::Rope(draw) => draw.set_terrain_decal(decal_type),
            Self::Projectile(draw) => draw.set_terrain_decal(decal_type),
            Self::ProjectileStream(draw) => draw.set_terrain_decal(decal_type),
            Self::Tree(draw) => draw.set_terrain_decal(decal_type),
            Self::Debris(draw) => draw.set_terrain_decal(decal_type),
        }
    }

    fn bind_owner_id(self, object_id: ObjectID) {
        match self {
            Self::OverlordTank(draw) => draw.bind_owner_id(object_id),
            Self::Model(draw) => draw.bind_owner_id(object_id),
            Self::Tank(draw) => draw.bind_owner_id(object_id),
            Self::Laser(draw) => draw.bind_owner_id(object_id),
            Self::ProjectileStream(draw) => draw.bind_owner_id(object_id),
            Self::Debris(draw) => draw.bind_owner_id(object_id),
            Self::Tracer(_) | Self::Rope(_) | Self::Projectile(_) | Self::Tree(_) => {}
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
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DOverlordTankDraw>()
    {
        func(DrawModuleKindMut::OverlordTank(module));
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
        (module as &mut dyn Any).downcast_mut::<crate::object::draw::W3DDebrisDraw>()
    {
        func(DrawModuleKindMut::Debris(module));
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

const AC_LOOP: u32 = 0x00000004;
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
    if id == 0 {
        1
    } else {
        id
    }
}

fn next_drawable_id_value(current: DrawableID) -> DrawableID {
    let next = current.wrapping_add(1);
    if next == 0 {
        1
    } else {
        next
    }
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

impl Drawable {
    /// Create a new Drawable
    pub fn new(
        drawable_id: DrawableID,
        object_id: ObjectID,
        model_name: String,
        drawable_type: DrawableType,
    ) -> Self {
        Drawable {
            drawable_id: normalize_drawable_id(drawable_id),
            object_id,
            object_ref: None,
            drawable_type,

            transform: Matrix3D::IDENTITY,
            instance_matrix: None,
            instance_scale: 1.0,
            world_position: Coord3D::new(0.0, 0.0, 0.0),
            world_rotation: Coord3D::new(0.0, 0.0, 0.0),
            world_scale: Coord3D::new(1.0, 1.0, 1.0),

            is_visible: true,
            hidden: false,
            hidden_by_stealth: false,
            always_visible: false,
            frustum_culled: false,
            occlusion_culled: false,
            distance_culled: false,
            current_lod: LevelOfDetail::High,
            lod_distances: [50.0, 100.0, 200.0, 400.0],

            model_name,
            submesh_names: Vec::new(),
            materials: Vec::new(),
            bounding_box: BoundingBox {
                min: Coord3D::new(-1.0, -1.0, -1.0),
                max: Coord3D::new(1.0, 1.0, 1.0),
            },
            bounding_sphere: BoundingSphere {
                center: Coord3D::new(0.0, 0.0, 0.0),
                radius: 1.0,
            },

            skeleton: Vec::new(),
            animation_clips: HashMap::new(),
            current_animation: None,
            animation_time: 0.0,
            animation_speed: 1.0,
            animation_state: AnimationState::Idle,
            blend_animations: Vec::new(),
            bone_transforms: Vec::new(),
            swaying_enabled: true,

            model_conditions: ModelConditionFlags::empty(),
            conditional_models: HashMap::new(),

            render_flags: RenderFlags::CAST_SHADOW | RenderFlags::RECEIVE_SHADOW,
            draw_priority: 0,
            alpha: 1.0,
            color_tint: Color::white(),
            indicator_color: Color::black(),
            selection_flash_envelope: None,
            color_tint_envelope: None,
            drawable_status_bits: 0x00000002, // DRAWABLE_STATUS_SHADOWS
            tint_status: TintStatus::NONE,
            prev_tint_status: TintStatus::NONE,
            fade_mode: 0,
            time_elapsed_fade: 0,
            time_to_fade: 0,
            loco_info: None,
            flash_count: 0,
            flash_color: Color::white(),
            shroud_status_object_id: object_id,
            expiration_date: 0,
            legacy_icons: Vec::new(),

            receives_lighting: true,
            casts_shadows: true,
            receives_shadows: true,
            self_illuminated: 0.0,

            particle_systems: Vec::new(),
            attachments: HashMap::new(),
            damage_states: Vec::new(),
            current_damage_state: 0,

            is_selected: false,
            selection_circle: None,
            health_bar: None,
            terrain_decal: TerrainDecalType::None,
            decal_opacity: 0.0,
            decal_opacity_fade_target: 0.0,
            decal_opacity_fade_rate: 0.0,
            drawable_fully_obscured_by_shroud: false,

            active_effects: Vec::new(),
            timed_effects: Vec::new(),

            modules: Vec::new(),

            last_update_frame: 0,
            update_frequency: 1,
            frozen: false,

            stealth_factor: 1.0,
            effective_stealth_opacity: 1.0,
            stealth_look: StealthLookType::None,
            second_material_pass_opacity: 0.0,
            cloak_texture: None,
            distortion_amount: 0.0,

            weather_affected: true,
            wetness_factor: 0.0,
            snow_accumulation: 0.0,

            attached_sounds: Vec::new(),
            ambient_sound_handle: 0,
            ambient_sound_enabled: true,
            ambient_sound_enabled_from_script: true,
            custom_sound_ambient_off: false,
            custom_sound_ambient_info: None,
            custom_sound_ambient_dynamic_info: None,

            terrain_following: false,
            ground_offset: 0.0,
            slope_adaptation: 0.0,

            screen_effects: Vec::new(),
        }
    }

    /// Allocate a drawable ID with save/load counter parity when GameClient hooks exist.
    pub fn allocate_drawable_id() -> DrawableID {
        if let Some(counter) = get_runtime_drawable_id_counter() {
            let id = normalize_drawable_id(counter);
            let next = next_drawable_id_value(id);
            set_runtime_drawable_id_counter(next);
            LOCAL_NEXT_DRAWABLE_ID.store(next, Ordering::Relaxed);
            return id;
        }

        allocate_local_drawable_id()
    }

    /// Get the next drawable-id counter value.
    pub fn get_drawable_id_counter() -> DrawableID {
        if let Some(counter) = get_runtime_drawable_id_counter() {
            let normalized = normalize_drawable_id(counter);
            LOCAL_NEXT_DRAWABLE_ID.store(normalized, Ordering::Relaxed);
            return normalized;
        }
        normalize_drawable_id(LOCAL_NEXT_DRAWABLE_ID.load(Ordering::Relaxed))
    }

    /// Set the next drawable-id counter value.
    pub fn set_drawable_id_counter(next_drawable_id: DrawableID) {
        let normalized = normalize_drawable_id(next_drawable_id);
        LOCAL_NEXT_DRAWABLE_ID.store(normalized, Ordering::Relaxed);
        set_runtime_drawable_id_counter(normalized);
    }

    pub fn get_drawable_id(&self) -> DrawableID {
        self.drawable_id
    }

    pub fn set_drawable_id(&mut self, drawable_id: DrawableID) {
        self.drawable_id = normalize_drawable_id(drawable_id);
    }

    pub fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    /// Register a draw module instance with the drawable.
    pub fn add_module(
        &mut self,
        interface_mask: ModuleInterfaceType,
        name: AsciiString,
        tag: AsciiString,
        module_data: Arc<dyn ModuleData>,
        mut module: Box<dyn Module>,
    ) -> DrawableModuleHandle {
        let _ = with_draw_module_kind(module.as_mut(), |draw| {
            draw.bind_owner_id(self.object_id);
        });
        module.on_drawable_bound_to_object();
        let entry = Arc::new(DrawModuleEntry::new(
            name,
            tag,
            interface_mask,
            module_data,
            module,
        ));
        self.modules.push(Arc::clone(&entry));
        DrawableModuleHandle::new(entry)
    }

    /// Enable or disable sway effects (used by topple logic).
    pub fn set_swaying_enabled(&mut self, enabled: bool) {
        self.swaying_enabled = enabled;
    }

    /// Set tint status bit(s) on this drawable.
    pub fn set_tint_status(&mut self, status_bits: TintStatus) {
        self.tint_status.set(status_bits);
    }

    /// Replace the current tint status with an exact value.
    pub fn set_tint_status_exact(&mut self, status: TintStatus) {
        self.tint_status = status;
    }

    /// Get current tint status bitmask.
    pub fn get_tint_status(&self) -> TintStatus {
        self.tint_status
    }

    /// Get current color tint.
    pub fn get_tint_color(&self) -> Color {
        self.color_tint
    }

    /// Set color tint explicitly.
    pub fn set_color_tint(&mut self, color: Color) {
        self.color_tint = color;
    }

    /// Clear color tint back to default (white).
    pub fn clear_color_tint(&mut self) {
        self.color_tint = Color::white();
    }

    pub fn set_time_of_day(&mut self, time_of_day: TimeOfDay) {
        match time_of_day {
            TimeOfDay::Night => self.set_model_condition_state(ModelConditionFlags::NIGHT),
            _ => self.clear_model_condition_state(ModelConditionFlags::NIGHT),
        }

        if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(obj_guard) = object.read() {
                self.start_ambient_sound(&obj_guard, time_of_day);
            }
        }
    }

    pub fn set_indicator_color(&mut self, color: Color) {
        self.indicator_color = color;
    }

    pub fn get_indicator_color(&self) -> Color {
        self.indicator_color
    }

    /// Clear tint status bit(s) on this drawable.
    pub fn clear_tint_status(&mut self, status_bits: TintStatus) {
        self.tint_status.clear(status_bits);
    }

    /// Test tint status bit(s).
    pub fn test_tint_status(&self, status_bits: TintStatus) -> bool {
        self.tint_status.is_set(status_bits)
    }

    /// Return draw modules that advertise the requested interface.
    pub fn modules_with_interface(
        &self,
        interface: ModuleInterfaceType,
    ) -> Vec<DrawableModuleHandle> {
        self.modules
            .iter()
            .filter(|entry| (entry.mask().0 & interface.0) != 0)
            .map(|entry| DrawableModuleHandle::new(Arc::clone(entry)))
            .collect()
    }

    fn get_draw_modules_with_interface(
        &self,
        interface: ModuleInterfaceType,
    ) -> Vec<DrawableModuleHandle> {
        self.modules_with_interface(interface)
    }

    fn xfer_drawable_modules(&mut self, xfer: &mut dyn Xfer) {
        let current_version: u8 = 1;
        let mut version = current_version;
        let _ = xfer.xfer_version(&mut version, current_version);

        let xfer_mode = xfer.get_xfer_mode();
        let is_saving = matches!(
            xfer_mode,
            game_engine::system::XferMode::Save | game_engine::system::XferMode::Crc
        );

        let mut module_types = 2u16;
        let _ = xfer.xfer_unsigned_short(&mut module_types);

        for module_type in 0..module_types {
            let interface = match module_type {
                0 => ModuleInterfaceType::DRAW,
                1 => ModuleInterfaceType::CLIENT_UPDATE,
                _ => ModuleInterfaceType::NONE,
            };

            if interface == ModuleInterfaceType::NONE {
                warn!(
                    "Drawable::xfer_drawable_modules encountered unsupported module type bucket {} on drawable {}",
                    module_type, self.drawable_id
                );
                let mut unknown_count = 0u16;
                let _ = xfer.xfer_unsigned_short(&mut unknown_count);
                for _ in 0..unknown_count {
                    let mut ignored = String::new();
                    let _ = xfer.xfer_ascii_string(&mut ignored);
                    let block_size = xfer.begin_block().unwrap_or(0);
                    if block_size > 0 {
                        let _ = xfer.skip(block_size);
                    }
                    let _ = xfer.end_block();
                }
                continue;
            }

            if is_saving {
                let modules_for_type: Vec<&Arc<DrawModuleEntry>> = self
                    .modules
                    .iter()
                    .filter(|entry| (entry.mask().0 & interface.0) != 0)
                    .collect();

                let mut module_count = modules_for_type.len().min(u16::MAX as usize) as u16;
                let _ = xfer.xfer_unsigned_short(&mut module_count);

                for entry in modules_for_type.into_iter().take(module_count as usize) {
                    let mut module_identifier = entry
                        .with_module(|module| {
                            NameKeyGenerator::key_to_name(module.get_module_tag_name_key())
                        })
                        .unwrap_or_default();
                    if module_identifier.is_empty() {
                        panic!(
                            "Drawable::xfer_drawable_modules unresolved module identifier for tag '{}' on drawable {}",
                            entry.tag(),
                            self.drawable_id
                        );
                    }
                    let _ = xfer.xfer_ascii_string(&mut module_identifier);

                    let _ = xfer.begin_block();
                    entry.with_module(|module| {
                        if let Err(err) = module.xfer(xfer) {
                            panic!(
                                "Drawable::xfer_drawable_modules failed for '{}' on drawable {}: {}",
                                module_identifier, self.drawable_id, err
                            );
                        };
                    });
                    let _ = xfer.end_block();
                }
            } else {
                let mut module_count = 0u16;
                let _ = xfer.xfer_unsigned_short(&mut module_count);

                for _ in 0..module_count {
                    let mut module_identifier = String::new();
                    let _ = xfer.xfer_ascii_string(&mut module_identifier);
                    let module_identifier_key = NameKeyGenerator::name_to_key(&module_identifier);

                    let module_index = self.modules.iter().position(|entry| {
                        (entry.mask().0 & interface.0) != 0
                            && entry.with_module(|module| {
                                module.get_module_tag_name_key() == module_identifier_key
                            })
                    });

                    let data_size = xfer.begin_block().unwrap_or(0);
                    if let Some(index) = module_index {
                        let entry = &self.modules[index];
                        entry.with_module(|module| {
                            if let Err(err) = module.xfer(xfer) {
                                panic!(
                                    "Drawable::xfer_drawable_modules load failed for '{}' on drawable {}: {}",
                                    module_identifier, self.drawable_id, err
                                );
                            }
                        });
                    } else if data_size > 0 {
                        panic!(
                            "Drawable::xfer_drawable_modules skipping missing module '{}' on drawable {}",
                            module_identifier, self.drawable_id
                        );
                    }
                    let _ = xfer.end_block();
                }
            }
        }
    }

    /// Retrieve all registered drawable modules.
    pub fn modules(&self) -> Vec<DrawableModuleHandle> {
        self.modules
            .iter()
            .cloned()
            .map(DrawableModuleHandle::new)
            .collect()
    }

    /// Retrieve a draw module by its logical name.
    pub fn module_by_name(&self, name: &AsciiString) -> Option<DrawableModuleHandle> {
        self.modules
            .iter()
            .find(|entry| entry.name() == name)
            .cloned()
            .map(DrawableModuleHandle::new)
    }

    /// Retrieve a draw module by its tag identifier.
    pub fn module_by_tag(&self, tag: &AsciiString) -> Option<DrawableModuleHandle> {
        self.modules
            .iter()
            .find(|entry| entry.tag() == tag)
            .cloned()
            .map(DrawableModuleHandle::new)
    }

    /// Remove all registered modules, invoking their delete hooks.
    pub fn clear_modules(&mut self) {
        for entry in &self.modules {
            entry.with_module(|module| module.on_delete());
        }
        self.modules.clear();
    }

    /// Update drawable for one frame
    pub fn update(
        &mut self,
        delta_time: Real,
        frame_number: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Skip update if frozen or not time to update
        if self.frozen || (frame_number - self.last_update_frame) < self.update_frequency {
            return Ok(());
        }

        self.last_update_frame = frame_number;

        // Update animations
        self.update_animation(delta_time)?;

        // Update particle systems
        self.update_particle_systems(delta_time)?;

        // Update visual effects
        self.update_visual_effects(delta_time)?;

        // Update terrain decal opacity fade
        if self.terrain_decal != TerrainDecalType::None {
            if self.decal_opacity_fade_rate != 0.0 {
                self.decal_opacity += self.decal_opacity_fade_rate;
                if let Some(first) = self
                    .get_draw_modules_with_interface(ModuleInterfaceType::DRAW)
                    .first()
                    .cloned()
                {
                    first.with_module(|module| {
                        with_draw_module_mut(module, |draw| {
                            draw.set_terrain_decal_opacity(self.decal_opacity)
                        });
                    });
                }

                if self.decal_opacity_fade_rate < 0.0 && self.decal_opacity <= 0.0 {
                    self.decal_opacity_fade_rate = 0.0;
                    self.decal_opacity = 0.0;
                    self.terrain_decal = TerrainDecalType::None;
                } else if self.decal_opacity_fade_rate > 0.0 && self.decal_opacity >= 1.0 {
                    self.decal_opacity = 1.0;
                    self.decal_opacity_fade_rate = 0.0;
                }
            }
        } else {
            self.decal_opacity = 0.0;
        }

        // Update damage visualization
        self.update_damage_state()?;

        // Update level of detail
        self.update_level_of_detail()?;

        // Update stealth effects
        self.update_stealth_effects(delta_time)?;

        // Update environmental effects
        self.update_environmental_effects(delta_time)?;

        // Update attachments
        self.update_attachments(delta_time)?;

        Ok(())
    }

    /// Set the world transform
    pub fn set_transform(&mut self, transform: Matrix3D) {
        let old_mtx = self.transform;
        let old_pos = self.world_position;
        let old_angle = self.world_rotation.y;

        self.transform = transform;
        let (scale, rotation, translation) = transform.to_scale_rotation_translation();
        self.world_position = translation;
        self.world_scale = scale;
        let (rx, ry, rz) = rotation.to_euler(EulerRot::XYZ);
        self.world_rotation = Coord3D::new(rx, ry, rz);

        // Update bounding volumes
        self.update_bounding_volumes();

        self.react_to_transform_change(&old_mtx, &old_pos, old_angle);
    }

    /// Notify draw modules that the world transform changed.
    ///
    /// Mirrors C++ `Drawable::reactToTransformChange`.
    pub fn react_to_transform_change(
        &mut self,
        old_mtx: &Matrix3D,
        old_pos: &Coord3D,
        old_angle: Real,
    ) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| {
                    draw.react_to_transform_change(old_mtx, old_pos, old_angle);
                });
            });
        }
    }

    /// Notify draw modules that geometry changed.
    ///
    /// Mirrors C++ `Drawable::reactToGeometryChange`.
    pub fn react_to_geometry_change(&mut self) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| draw.react_to_geometry_change());
            });
        }
    }

    /// Play animation
    pub fn play_animation(
        &mut self,
        animation_name: &str,
        _loop_animation: bool,
        blend_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.animation_clips.contains_key(animation_name) {
            if let Some(current) = &self.current_animation {
                if current == animation_name {
                    return Ok(()); // Already playing this animation
                }

                // Start blending from current animation
                if blend_time > 0.0 {
                    self.blend_animations.push(AnimationBlend {
                        animation_name: current.clone(),
                        weight: 1.0,
                        fade_time: blend_time,
                        current_fade: 0.0,
                    });
                }
            }

            self.current_animation = Some(animation_name.to_string());
            self.animation_time = 0.0;
        }

        Ok(())
    }

    /// Stop current animation
    pub fn stop_animation(&mut self) {
        self.current_animation = None;
        self.animation_time = 0.0;
        self.blend_animations.clear();
    }

    /// Set model condition flags (for conditional model switching)
    pub fn set_model_conditions(&mut self, conditions: ModelConditionFlags) {
        self.model_conditions = conditions;

        // Check if we need to switch models based on conditions
        for (flags, model_name) in &self.conditional_models {
            if self.model_conditions.intersects(*flags) {
                self.model_name = model_name.clone();
                break;
            }
        }
    }

    /// Set visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.is_visible = visible;
        self.update_hidden_status();
    }

    /// Check if currently visible (not culled)
    pub fn is_currently_visible(&self) -> bool {
        self.is_visible
            && !self.hidden
            && !self.hidden_by_stealth
            && !self.frustum_culled
            && !self.occlusion_culled
            && !self.distance_culled
    }

    /// Set selection state
    /// Flash this drawable as if selected (short-lived visual cue).
    pub fn flash_as_selected(&mut self) {
        let effect = VisualEffect {
            effect_type: "SelectionFlash".to_string(),
            bone_attachment: None,
            offset: Coord3D::new(0.0, 0.0, 0.0),
            scale: 1.0,
            color: Color::new(255, 255, 255, 255),
            parameters: HashMap::new(),
        };

        // Short flash; rendering layer can interpret this effect as a selection blink.
        self.add_effect(effect, Some(0.25));
    }

    /// Flash this drawable with a script-defined color for a duration.
    /// C++ parity path: ScriptActions::doNamedFlash/doTeamFlash.
    pub fn script_flash(&mut self, color: Color, duration_seconds: Real) {
        if duration_seconds <= 0.0 {
            return;
        }

        let effect = VisualEffect {
            effect_type: "ScriptFlash".to_string(),
            bone_attachment: None,
            offset: Coord3D::new(0.0, 0.0, 0.0),
            scale: 1.0,
            color,
            parameters: HashMap::new(),
        };

        self.add_effect(effect, Some(duration_seconds.max(0.1)));
    }

    /// Set a script-controlled emoticon above this drawable.
    /// C++ parity path: ScriptActions::doNamedEmoticon/doTeamEmoticon.
    pub fn script_set_emoticon(&mut self, emoticon_name: &str, duration_frames: i32) {
        if emoticon_name.is_empty() || duration_frames <= 0 {
            return;
        }

        // Keep only one script emoticon active at a time, matching set/replace behavior.
        self.active_effects
            .retain(|e| !e.effect_type.starts_with("ScriptEmoticon:"));
        self.timed_effects
            .retain(|e| !e.effect.effect_type.starts_with("ScriptEmoticon:"));

        let effect = VisualEffect {
            effect_type: format!("ScriptEmoticon:{}", emoticon_name),
            bone_attachment: None,
            offset: Coord3D::new(0.0, 0.0, 0.0),
            scale: 1.0,
            color: Color::white(),
            parameters: HashMap::new(),
        };
        let seconds = (duration_frames as Real / LOGICFRAMES_PER_SECOND as Real)
            .max(1.0 / LOGICFRAMES_PER_SECOND as Real);
        self.add_effect(effect, Some(seconds));
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.is_selected = selected;

        if selected && self.selection_circle.is_none() {
            // Create default selection circle
            self.selection_circle = Some(SelectionCircle {
                radius: self.bounding_sphere.radius * 1.2,
                color: Color::new(0, 255, 0, 204),
                texture: "SelectionRing.tga".to_string(),
                animation_speed: 2.0,
            });
        } else if !selected {
            self.selection_circle = None;
        }
    }

    /// Add visual effect
    pub fn add_effect(&mut self, effect: VisualEffect, duration: Option<Real>) {
        if let Some(dur) = duration {
            self.timed_effects.push(TimedEffect {
                effect,
                duration: dur,
                elapsed_time: 0.0,
                fade_in_time: 0.2,
                fade_out_time: 0.2,
            });
        } else {
            self.active_effects.push(effect);
        }
    }

    /// Remove visual effect
    pub fn remove_effect(&mut self, effect_type: &str) {
        self.active_effects.retain(|e| e.effect_type != effect_type);
        self.timed_effects
            .retain(|e| e.effect.effect_type != effect_type);
    }

    /// Set stealth minimum opacity floor (C++ m_stealthOpacity).
    pub fn set_stealth_factor(&mut self, factor: Real) {
        self.stealth_factor = factor.clamp(0.0, 1.0);
        self.effective_stealth_opacity = self.stealth_factor;

        // Enable distortion effect when partially stealthed
        let stealth_blend = (1.0 - self.effective_stealth_opacity).clamp(0.0, 1.0);
        if stealth_blend > 0.0 && !self.hidden_by_stealth {
            self.distortion_amount = stealth_blend * 0.5;
            self.render_flags |= RenderFlags::DISTORTION;
        } else {
            self.distortion_amount = 0.0;
            self.render_flags &= !RenderFlags::DISTORTION;
        }
    }

    /// Update the stealth-opacity floor without changing pulse output.
    pub fn set_stealth_min_opacity(&mut self, min_opacity: Real) {
        self.stealth_factor = min_opacity.clamp(0.0, 1.0);
    }

    /// Attach another drawable (for weapons, effects, etc.)
    pub fn attach_drawable(
        &mut self,
        name: String,
        drawable: Arc<RwLock<Drawable>>,
        bone_name: String,
        offset: Coord3D,
    ) {
        let attachment = Attachment {
            drawable,
            bone_name,
            offset,
            rotation: Coord3D::new(0.0, 0.0, 0.0),
            scale: Coord3D::new(1.0, 1.0, 1.0),
        };

        self.attachments.insert(name, attachment);
    }

    /// Detach drawable
    pub fn detach_drawable(&mut self, name: &str) -> Option<Attachment> {
        self.attachments.remove(name)
    }

    /// Get bone world transform by name
    pub fn get_bone_transform(&self, bone_name: &str) -> Option<Matrix3D> {
        // Find bone index
        for (index, bone) in self.skeleton.iter().enumerate() {
            if bone.name == bone_name {
                if index < self.bone_transforms.len() {
                    let local_transform = self.bone_transforms[index];
                    return Some(self.transform * local_transform);
                }
                break;
            }
        }
        None
    }

    /// Get bone local transform by name (without applying object transform).
    pub fn get_bone_local_transform(&self, bone_name: &str) -> Option<Matrix3D> {
        for (index, bone) in self.skeleton.iter().enumerate() {
            if bone.name == bone_name {
                if index < self.bone_transforms.len() {
                    return Some(self.bone_transforms[index]);
                }
                break;
            }
        }
        None
    }

    /// Get pristine bone transforms by prefix (approximation of C++ getPristineBonePositions).
    pub fn get_pristine_bone_transforms(
        &self,
        bone_name_prefix: &str,
        start_index: usize,
        max_bones: usize,
    ) -> Vec<Matrix3D> {
        use crate::object::draw::draw_module::ObjectDrawInterface;

        let condition = self.model_conditions;
        let mut positions = vec![Coord3D::origin(); max_bones];
        let mut transforms = vec![Matrix3D::IDENTITY; max_bones];
        for module_handle in self.modules() {
            let count = module_handle
                .with_module_downcast::<crate::object::draw::w3d_model_draw::W3DModelDraw, _, _>(
                    |draw_module| {
                        draw_module.get_pristine_bone_positions(
                            &condition,
                            bone_name_prefix,
                            start_index as i32,
                            &mut positions,
                            &mut transforms,
                            max_bones,
                        )
                    },
                )
                .unwrap_or(0);

            if count > 0 {
                return transforms.into_iter().take(count).collect();
            }
        }

        let mut matches: Vec<&BoneData> = if start_index == 0 {
            self.skeleton
                .iter()
                .filter(|bone| bone.name == bone_name_prefix)
                .collect()
        } else {
            self.skeleton
                .iter()
                .filter(|bone| bone.name.starts_with(bone_name_prefix))
                .collect()
        };

        matches.sort_by(|a, b| a.name.cmp(&b.name));

        let skip = start_index.saturating_sub(1);
        matches
            .into_iter()
            .skip(skip)
            .take(max_bones)
            .filter_map(|bone| self.get_bone_transform(&bone.name))
            .collect()
    }

    /// Get pristine bone positions (local space) by prefix.
    pub fn get_pristine_bone_positions(
        &self,
        bone_name_prefix: &str,
        start_index: usize,
        max_bones: usize,
    ) -> Vec<Coord3D> {
        use crate::object::draw::draw_module::ObjectDrawInterface;

        let condition = self.model_conditions;
        let mut positions = vec![Coord3D::origin(); max_bones];
        let mut transforms = vec![Matrix3D::IDENTITY; max_bones];
        for module_handle in self.modules() {
            let count = module_handle
                .with_module_downcast::<crate::object::draw::w3d_model_draw::W3DModelDraw, _, _>(
                    |draw_module| {
                        draw_module.get_pristine_bone_positions(
                            &condition,
                            bone_name_prefix,
                            start_index as i32,
                            &mut positions,
                            &mut transforms,
                            max_bones,
                        )
                    },
                )
                .unwrap_or(0);

            if count > 0 {
                return positions.into_iter().take(count).collect();
            }
        }

        let mut matches: Vec<(usize, &BoneData)> = if start_index == 0 {
            self.skeleton
                .iter()
                .enumerate()
                .filter(|(_, bone)| bone.name == bone_name_prefix)
                .collect()
        } else {
            self.skeleton
                .iter()
                .enumerate()
                .filter(|(_, bone)| bone.name.starts_with(bone_name_prefix))
                .collect()
        };

        matches.sort_by(|a, b| a.1.name.cmp(&b.1.name));

        let skip = start_index.saturating_sub(1);
        matches
            .into_iter()
            .skip(skip)
            .take(max_bones)
            .filter_map(|(index, _)| self.bone_transforms.get(index).copied())
            .map(|transform| {
                let (_, _, translation) = transform.to_scale_rotation_translation();
                translation
            })
            .collect()
    }

    /// Update damage state based on health percentage
    pub fn update_damage_state_for_health(&mut self, health_percentage: Real) {
        let mut new_state = 0;

        for (index, damage_state) in self.damage_states.iter().enumerate() {
            if health_percentage <= damage_state.health_threshold {
                new_state = index;
                break;
            }
        }

        if new_state != self.current_damage_state {
            self.current_damage_state = new_state;

            if let Some(damage_state) = self.damage_states.get(new_state) {
                // Apply damage state effects
                self.color_tint = damage_state.color_tint;

                if let Some(alpha) = damage_state.alpha_override {
                    self.alpha = alpha;
                }

                // Start damage particles
                let particle_effects = damage_state.particle_effects.clone();
                for particle_name in &particle_effects {
                    self.start_particle_system(particle_name);
                }
            }
        }
    }

    // Private helper methods

    fn update_animation(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(anim_name) = &self.current_animation.clone() {
            if let Some(clip) = self.animation_clips.get(anim_name).cloned() {
                self.animation_time += delta_time * self.animation_speed;

                // Handle looping
                if self.animation_time >= clip.duration {
                    if clip.loop_animation {
                        self.animation_time = self.animation_time % clip.duration;
                    } else {
                        self.animation_time = clip.duration;
                        // Animation finished - could trigger callback here
                    }
                }

                // Update bone transforms based on current animation time
                self.update_bone_transforms(&clip)?;
            }
        }

        // Update animation blends
        self.blend_animations.retain_mut(|blend| {
            blend.current_fade += delta_time;
            blend.weight = 1.0 - (blend.current_fade / blend.fade_time);
            blend.weight > 0.0
        });

        Ok(())
    }

    fn update_bone_transforms(
        &mut self,
        clip: &AnimationClip,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Find the appropriate keyframes for current time
        for keyframe in &clip.keyframes {
            if keyframe.time <= self.animation_time {
                // This is a simplified version - real implementation would interpolate between keyframes
                self.bone_transforms = keyframe.bone_transforms.clone();
                break;
            }
        }
        Ok(())
    }

    fn update_particle_systems(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for particle_system in &mut self.particle_systems {
            if particle_system.is_active {
                // Update particle system parameters
                // Real implementation would update particle positions, spawn new particles, etc.
            }
        }
        Ok(())
    }

    fn update_visual_effects(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update timed effects
        self.timed_effects.retain_mut(|effect| {
            effect.elapsed_time += delta_time;
            effect.elapsed_time < effect.duration
        });

        Ok(())
    }

    fn update_damage_state(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // This would be called by the associated Object when health changes
        Ok(())
    }

    fn update_level_of_detail(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Calculate distance to camera and select appropriate LOD.
        // Until the camera system is exposed in GameLogic, use origin as a stable reference.
        let distance_to_camera = {
            let pos = self.world_position;
            (pos.x * pos.x + pos.y * pos.y + pos.z * pos.z).sqrt()
        };

        self.current_lod = if distance_to_camera < self.lod_distances[0] {
            LevelOfDetail::High
        } else if distance_to_camera < self.lod_distances[1] {
            LevelOfDetail::Medium
        } else if distance_to_camera < self.lod_distances[2] {
            LevelOfDetail::Low
        } else {
            LevelOfDetail::Impostor
        };

        Ok(())
    }

    fn update_stealth_effects(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update stealth visual effects like shimmering
        let stealth_blend = (1.0 - self.effective_stealth_opacity).clamp(0.0, 1.0);
        if stealth_blend > 0.0 && !self.hidden_by_stealth {
            // Add subtle animation to distortion
            self.distortion_amount += (delta_time * 2.0).sin() * 0.01;
        }

        Ok(())
    }

    fn update_environmental_effects(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update weather effects
        if self.weather_affected {
            // This would query the weather system
            // self.wetness_factor = weather_system.get_rain_intensity();
            // self.snow_accumulation += weather_system.get_snow_rate() * delta_time;
        }

        Ok(())
    }

    fn update_attachments(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let attachment_keys: Vec<String> = self.attachments.keys().cloned().collect();
        for key in attachment_keys {
            // Update attachment position based on bone transform
            let bone_name = if let Some(attachment) = self.attachments.get(&key) {
                attachment.bone_name.clone()
            } else {
                continue;
            };

            if let Some(bone_transform) = self.get_bone_transform(&bone_name) {
                if let Some(attachment) = self.attachments.get_mut(&key) {
                    let attachment_transform =
                        bone_transform * Matrix3D::from_translation(attachment.offset);

                    if let Ok(mut attached_drawable) = attachment.drawable.write() {
                        attached_drawable.set_transform(attachment_transform);
                        attached_drawable.update(delta_time, self.last_update_frame)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn update_bounding_volumes(&mut self) {
        // Update bounding sphere center
        self.bounding_sphere.center = self.world_position;

        // Update bounding box (simplified)
        let half_size = Coord3D::new(1.0, 1.0, 1.0); // Would be calculated from model
        self.bounding_box.min = self.world_position - half_size;
        self.bounding_box.max = self.world_position + half_size;
    }

    fn start_particle_system(&mut self, particle_name: &str) {
        for particle_system in &mut self.particle_systems {
            if particle_system.name == particle_name {
                particle_system.is_active = true;
                break;
            }
        }
    }

    pub fn is_selected(&self) -> bool {
        self.is_selected
    }

    pub fn set_terrain_decal(&mut self, decal_type: TerrainDecalType) {
        self.terrain_decal = decal_type;
        for entry in &self.modules {
            if (entry.mask().0 & ModuleInterfaceType::DRAW.0) == 0 {
                continue;
            }
            entry.with_module(|module| set_decal_on_draw_module(module, decal_type));
        }
    }

    pub fn set_terrain_decal_size(&mut self, x: Real, y: Real) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| draw.set_terrain_decal_size(x, y))
            });
        }
    }

    pub fn set_terrain_decal_fade_target(&mut self, target: Real, rate: Real) {
        if (self.decal_opacity_fade_target - target).abs() > f32::EPSILON {
            self.decal_opacity_fade_target = target;
            self.decal_opacity_fade_rate = rate;
        }
    }

    pub fn get_terrain_decal(&self) -> TerrainDecalType {
        self.terrain_decal
    }

    pub fn set_shadows_enabled(&mut self, enabled: bool) {
        if enabled {
            self.drawable_status_bits |= 0x00000002; // DRAWABLE_STATUS_SHADOWS
        } else {
            self.drawable_status_bits &= !0x00000002;
        }
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| draw.set_shadows_enabled(enabled))
            });
        }
    }

    pub fn release_shadows(&mut self) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle
                .with_module(|module| with_draw_module_mut(module, |draw| draw.release_shadows()));
        }
    }

    pub fn allocate_shadows(&mut self) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle
                .with_module(|module| with_draw_module_mut(module, |draw| draw.allocate_shadows()));
        }
    }

    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        if self.drawable_fully_obscured_by_shroud != fully_obscured {
            for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
                module_handle.with_module(|module| {
                    with_draw_module_mut(module, |draw| {
                        draw.set_fully_obscured_by_shroud(fully_obscured)
                    })
                });
            }
            self.drawable_fully_obscured_by_shroud = fully_obscured;
        }
    }

    /// Mirror C++ Drawable::changedTeam.
    pub fn changed_team(&mut self, object: &crate::object::Object) {
        let time_of_day = TheGlobalData::get()
            .map(|data| data.get_time_of_day())
            .unwrap_or(TimeOfDay::Day);
        let indicator = match time_of_day {
            TimeOfDay::Night => object.get_night_indicator_color(),
            _ => object.get_indicator_color(),
        };
        self.set_indicator_color(indicator);

        if object.is_kind_of(KindOf::FSFake) {
            let relationship = ThePlayerList()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|player| {
                    let guard = player.read().ok()?;
                    let team = object.get_team()?;
                    let team_guard = team.read().ok()?;
                    Some(guard.get_relationship_with_team(&team_guard))
                })
                .unwrap_or(Relationship::Enemies);

            if matches!(relationship, Relationship::Allies | Relationship::Neutral) {
                self.set_terrain_decal(TerrainDecalType::ShadowTexture);
            } else {
                self.set_terrain_decal(TerrainDecalType::None);
            }
        }
    }

    pub fn enable_ambient_sound_from_script(&mut self, enabled: Bool) {
        self.ambient_sound_enabled_from_script = enabled;
        if !enabled {
            self.stop_ambient_sound();
        } else if self.ambient_sound_enabled {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(obj_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound(&obj_guard, time_of_day);
                }
            }
        }
    }

    pub fn enable_ambient_sound(&mut self, enabled: Bool) {
        if self.ambient_sound_enabled == enabled {
            return;
        }
        self.ambient_sound_enabled = enabled;
        if enabled {
            if self.ambient_sound_enabled_from_script {
                if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                    if let Ok(obj_guard) = object.read() {
                        let time_of_day = TheGlobalData::get()
                            .map(|data| data.get_time_of_day())
                            .unwrap_or(TimeOfDay::Day);
                        self.start_ambient_sound(&obj_guard, time_of_day);
                    }
                }
            }
        } else {
            self.stop_ambient_sound();
        }
    }

    pub fn is_ambient_sound_enabled(&self) -> Bool {
        self.ambient_sound_enabled
    }

    pub fn is_ambient_sound_enabled_from_script(&self) -> Bool {
        self.ambient_sound_enabled_from_script
    }

    pub fn is_ambient_sound_enabled_effective(&self) -> Bool {
        self.ambient_sound_enabled && self.ambient_sound_enabled_from_script
    }

    fn mangle_custom_audio_name(&self, base_name: &str) -> String {
        // C++ parity: leading space avoids colliding with INI-defined names.
        format!(" CUSTOM {} {}", self.drawable_id, base_name)
    }

    fn set_custom_sound_ambient_dynamic_info_internal(
        &mut self,
        custom_info: DynamicAudioEventInfo,
        restart_sound: bool,
    ) {
        self.clear_custom_sound_ambient(false);

        let info_name = custom_info.audio_event_info.audio_name.clone();
        let info_copy = custom_info.audio_event_info.clone();
        let registered_info = {
            let manager =
                get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
            manager.lock().ok().and_then(|mut guard| {
                guard.register_audio_event_info(info_copy.clone());
                guard.find_audio_event_info(&info_name)
            })
        };

        self.custom_sound_ambient_off = false;
        self.custom_sound_ambient_dynamic_info = Some(custom_info);
        self.custom_sound_ambient_info = registered_info.or_else(|| Some(Arc::new(info_copy)));

        if restart_sound && self.is_ambient_sound_enabled_effective() {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(obj_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound(&obj_guard, time_of_day);
                }
            }
        }
    }

    pub fn set_custom_sound_ambient_dynamic_info(
        &mut self,
        mut custom_info: DynamicAudioEventInfo,
    ) {
        let custom_name = self.mangle_custom_audio_name(&custom_info.audio_event_info.audio_name);
        custom_info.override_audio_name(&custom_name);
        self.set_custom_sound_ambient_dynamic_info_internal(custom_info, true);
    }

    pub fn set_custom_sound_ambient_off(&mut self) {
        self.clear_custom_sound_ambient(false);
        self.custom_sound_ambient_off = true;
    }

    pub fn is_custom_sound_ambient_off(&self) -> Bool {
        self.custom_sound_ambient_off
    }

    pub fn set_custom_sound_ambient_info(&mut self, info: Arc<AudioEventInfo>) {
        self.clear_custom_sound_ambient(false);
        self.custom_sound_ambient_off = false;
        self.custom_sound_ambient_info = Some(info);
        self.custom_sound_ambient_dynamic_info = None;
        if self.is_ambient_sound_enabled_effective() {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(obj_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound(&obj_guard, time_of_day);
                }
            }
        }
    }

    fn is_permanent_ambient_sound(info: &AudioEventInfo) -> bool {
        (info.control & AC_LOOP) != 0 || info.loop_count != 1
    }

    fn find_or_create_audio_event_info(event_name: &str) -> Option<Arc<AudioEventInfo>> {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let mut manager = manager.lock().ok()?;
        if let Some(info) = manager.find_audio_event_info(event_name) {
            Some(info)
        } else {
            manager.new_audio_event_info(event_name.to_string())
        }
    }

    fn get_ambient_sound_for_damage(
        object: &crate::object::Object,
        damage_state: BodyDamageType,
    ) -> Option<AudioEventRts> {
        let template = object.get_template();
        match damage_state {
            BodyDamageType::Rubble => template.get_sound_ambient_rubble(),
            BodyDamageType::ReallyDamaged => template
                .get_sound_ambient_really_damaged()
                .or_else(|| template.get_sound_ambient()),
            BodyDamageType::Damaged => template
                .get_sound_ambient_damaged()
                .or_else(|| template.get_sound_ambient()),
            _ => template.get_sound_ambient(),
        }
    }

    fn start_ambient_sound_internal(
        &mut self,
        object: &crate::object::Object,
        time_of_day: TimeOfDay,
        only_if_permanent: bool,
    ) {
        if !self.is_ambient_sound_enabled_effective() {
            self.stop_ambient_sound();
            return;
        }

        let damage_state = object
            .get_body_module()
            .and_then(|body| body.lock().ok().map(|guard| guard.get_damage_state()))
            .unwrap_or(BodyDamageType::Pristine);

        if self.custom_sound_ambient_off && damage_state != BodyDamageType::Rubble {
            self.stop_ambient_sound();
            return;
        }

        let (event_name, event_info) = if damage_state != BodyDamageType::Rubble {
            if let Some(custom_info) = &self.custom_sound_ambient_info {
                (
                    custom_info.audio_name.clone(),
                    Some(Arc::clone(custom_info)),
                )
            } else {
                let Some(event) = Self::get_ambient_sound_for_damage(object, damage_state) else {
                    self.stop_ambient_sound();
                    return;
                };
                let name = event.get_event_name().to_string();
                let info = if name.is_empty() {
                    None
                } else {
                    Self::find_or_create_audio_event_info(&name)
                };
                (name, info)
            }
        } else {
            let Some(event) = Self::get_ambient_sound_for_damage(object, damage_state) else {
                self.stop_ambient_sound();
                return;
            };
            let name = event.get_event_name().to_string();
            let info = if name.is_empty() {
                None
            } else {
                Self::find_or_create_audio_event_info(&name)
            };
            (name, info)
        };

        if event_name.is_empty() {
            self.stop_ambient_sound();
            return;
        }

        let Some(info) = event_info else {
            self.stop_ambient_sound();
            return;
        };

        if only_if_permanent && !Self::is_permanent_ambient_sound(&info) {
            self.stop_ambient_sound();
            return;
        }

        self.stop_ambient_sound();

        let mut audio_event = AudioEventRts::new(event_name);
        audio_event.set_drawable_id(self.drawable_id);
        audio_event.set_object_id(object.get_id());
        audio_event.set_time_of_day(time_of_day);

        if let Some(audio) = TheAudio::get() {
            self.ambient_sound_handle = audio.add_audio_event(&audio_event);
        }
    }

    pub fn clear_custom_sound_ambient(&mut self, restart_sound: bool) {
        self.custom_sound_ambient_info = None;
        self.custom_sound_ambient_dynamic_info = None;
        self.custom_sound_ambient_off = false;
        if restart_sound && self.is_ambient_sound_enabled_effective() {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(obj_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound(&obj_guard, time_of_day);
                }
            }
        }
    }

    pub fn stop_ambient_sound(&mut self) {
        if self.ambient_sound_handle == 0 {
            return;
        }
        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(self.ambient_sound_handle);
        }
        self.ambient_sound_handle = 0;
    }

    pub fn start_ambient_sound(&mut self, object: &crate::object::Object, time_of_day: TimeOfDay) {
        self.start_ambient_sound_internal(object, time_of_day, false);
    }

    /// Set whether the drawable is hidden
    pub fn set_drawable_hidden(&mut self, hidden: bool) -> Result<(), GameError> {
        self.hidden = hidden;
        self.is_visible = !hidden;
        self.update_hidden_status();
        Ok(())
    }

    /// Clear pending drawable dependency state before an explicit draw.
    /// Matches C++ Drawable::notifyDrawableDependencyCleared used by W3DOverlordTankDraw.
    pub fn notify_drawable_dependency_cleared(&mut self) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_object_draw_interface_mut(module, |draw| {
                    draw.notify_draw_module_dependency_cleared();
                })
            });
        }
    }

    /// Check if the drawable is effectively hidden (by explicit hide or stealth)
    /// Matches C++ Drawable.h line 305: isDrawableEffectivelyHidden()
    /// Returns true if hidden via setDrawableHidden OR fully stealthed
    pub fn is_drawable_effectively_hidden(&self) -> bool {
        self.hidden || !self.is_visible || self.hidden_by_stealth
    }

    /// Update hidden state on draw modules and selection data.
    fn update_hidden_status(&mut self) {
        let hidden = self.hidden || self.hidden_by_stealth;
        if hidden {
            self.set_selected(false);
        }
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| draw.set_hidden(hidden));
            });
        }
    }

    /// Set a specific model condition state flag
    /// Updates the model conditions by setting the specified flag
    pub fn set_model_condition_state(&mut self, state: ModelConditionFlags) {
        self.model_conditions |= state;
        self.update_conditional_model();
        self.propagate_model_condition_state_to_draw_modules();
    }

    /// Clear a specific model condition state flag
    /// Updates the model conditions by clearing the specified flag
    pub fn clear_model_condition_state(&mut self, state: ModelConditionFlags) {
        self.model_conditions &= !state;
        self.update_conditional_model();
        self.propagate_model_condition_state_to_draw_modules();
    }

    /// Clear one set of flags and set another atomically
    /// This is used to transition between states cleanly
    pub fn clear_and_set_model_condition_state(
        &mut self,
        clear: ModelConditionFlags,
        set: ModelConditionFlags,
    ) {
        self.model_conditions &= !clear;
        self.model_conditions |= set;
        self.update_conditional_model();
        self.propagate_model_condition_state_to_draw_modules();
    }

    /// Set effective stealth opacity using C++ pulse semantics.
    /// `pulse_factor` is clamped [0..1], and `explicit_opacity` updates the stealth floor when set.
    pub fn set_effective_opacity(&mut self, pulse_factor: Real, explicit_opacity: Option<Real>) {
        if let Some(opacity) = explicit_opacity {
            self.stealth_factor = opacity.clamp(0.0, 1.0);
        }

        let pf = pulse_factor.clamp(0.0, 1.0);
        let pulse_margin = 1.0 - self.stealth_factor;
        let pulse_amount = pulse_margin * pf;
        self.effective_stealth_opacity = (self.stealth_factor + pulse_amount).clamp(0.0, 1.0);
    }

    /// Set stealth appearance mode and hidden state (C++ Drawable::setStealthLook parity).
    pub fn set_stealth_look(&mut self, look: StealthLookType) {
        if look == self.stealth_look {
            return;
        }

        // C++ parity: reset stealth floor before applying look-specific behavior.
        self.stealth_factor = 1.0;

        let is_mine = self
            .object_ref
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|object| {
                object
                    .read()
                    .ok()
                    .map(|guard| guard.is_kind_of(KindOf::Mine))
            })
            .unwrap_or(false);

        self.stealth_look = look;
        self.hidden_by_stealth = matches!(look, StealthLookType::Invisible);
        self.second_material_pass_opacity = match look {
            StealthLookType::VisibleDetected if !is_mine => 1.0,
            StealthLookType::VisibleFriendlyDetected if !is_mine => 1.0,
            _ => 0.0,
        };

        // C++ parity: disable shadows while in globally detected visualization state.
        self.set_shadows_enabled(!matches!(look, StealthLookType::VisibleDetected));

        self.update_hidden_status();
    }

    pub fn get_stealth_look(&self) -> StealthLookType {
        self.stealth_look
    }

    /// Helper to update the model based on conditional model settings
    fn update_conditional_model(&mut self) {
        // Check if we need to switch models based on conditions
        for (flags, model_name) in &self.conditional_models {
            if self.model_conditions.intersects(*flags) {
                self.model_name = model_name.clone();
                return;
            }
        }
    }

    fn propagate_model_condition_state_to_draw_modules(&mut self) {
        let conditions = self.model_conditions;
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_object_draw_interface_mut(module, |draw| {
                    draw.replace_model_condition_state(&conditions);
                });
            });
        }
    }

    /// Clear model condition flags
    /// Updates the model conditions by clearing the specified flags
    pub fn clear_model_condition_flags(&mut self, clear: ModelConditionFlags) {
        self.model_conditions &= !clear;
        self.update_conditional_model();
    }

    /// Clear garrisoned model condition
    pub fn clear_model_condition_garrisoned(&mut self) -> Result<(), String> {
        self.model_conditions &= !ModelConditionFlags::GARRISONED;
        self.update_conditional_model();
        Ok(())
    }

    /// Set the orientation (rotation) of the drawable
    /// Updates the world rotation to reflect the new angle
    pub fn set_orientation(&mut self, angle: Real) {
        // Update the Y-axis rotation (yaw) and rebuild world transform.
        self.world_rotation.y = angle;
        let rotation = Quat::from_euler(
            EulerRot::XYZ,
            self.world_rotation.x,
            self.world_rotation.y,
            self.world_rotation.z,
        );
        let new_transform = Matrix3D::from_scale_rotation_translation(
            self.world_scale,
            rotation,
            self.world_position,
        );
        self.set_transform(new_transform);
    }

    /// Get the current world position of the drawable
    pub fn get_position(&self) -> Coord3D {
        self.world_position
    }

    /// Get the current world transform matrix.
    pub fn get_transform_matrix(&self) -> Matrix3D {
        self.transform
    }

    /// Get current world-space bounding box.
    pub fn get_bounding_box(&self) -> BoundingBox {
        self.bounding_box.clone()
    }

    /// Get current world-space bounding sphere radius.
    pub fn get_bounding_sphere_radius(&self) -> Real {
        self.bounding_sphere.radius
    }

    /// Get decomposed world scale used by bone-space conversions.
    pub fn get_world_scale(&self) -> Coord3D {
        self.world_scale
    }

    /// Get the object associated with this drawable
    pub fn get_object(&self) -> Option<Arc<rhai::Locked<crate::object::Object>>> {
        self.object_ref.as_ref().and_then(|weak| weak.upgrade())
    }

    pub(crate) fn bind_object_ref(&mut self, object: &Arc<RwLock<crate::object::Object>>) {
        self.object_ref = Some(Arc::downgrade(object));
    }

    /// Get current worldspace client bone positions
    /// Returns the transform matrix for a specific bone in worldspace
    pub fn get_current_worldspace_client_bone_positions(
        &self,
        bone_name: &str,
    ) -> Option<Matrix3D> {
        let bone_name_ascii = AsciiString::from(bone_name);
        for module_handle in self.modules() {
            let mut world_bone = Matrix3D::IDENTITY;
            let found = module_handle.with_module(|module| {
                let mut found = false;
                with_draw_module_mut(module, |draw| {
                    if let Some(interface) = draw.get_object_draw_interface_mut() {
                        found = interface.client_only_get_render_obj_bone_transform(
                            &bone_name_ascii,
                            &mut world_bone,
                        );
                    }
                });
                found
            });

            if found {
                return Some(world_bone);
            }
        }

        // Fallback for partially ported draw modules that expose skeleton data directly.
        self.get_bone_transform(bone_name)
    }

    /// Set animation to loop in N frames
    ///
    /// This call says, "I want the current animation (if any) to take n frames to complete a single cycle".
    /// If it's a looping anim, each loop will take n frames.
    /// Note that you must call this AFTER setting the condition codes.
    ///
    /// Reference: C++ Drawable.h:469 - setAnimationLoopDuration
    pub fn set_animation_loop_duration(&mut self, num_frames: u32) {
        for module_handle in self.modules() {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| {
                    if let DrawModuleKindMut::Model(w3d_draw) = draw {
                        w3d_draw.set_animation_loop_duration(num_frames);
                    }
                });
            });
        }
    }

    /// Set animation completion time
    ///
    /// Similar to setAnimationLoopDuration, but assumes that the current state is a "ONCE",
    /// and is smart about transition states... if there is a transition state "inbetween",
    /// it is included in the completion time.
    ///
    /// Reference: C++ Drawable.h:475 - setAnimationCompletionTime
    pub fn set_animation_completion_time(&mut self, num_frames: u32) {
        for module_handle in self.modules() {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| {
                    if let DrawModuleKindMut::Model(w3d_draw) = draw {
                        w3d_draw.set_animation_completion_time(num_frames);
                    }
                });
            });
        }
    }

    /// Set animation frame manually
    ///
    /// Manually set a drawable's current animation to a specific frame.
    ///
    /// Reference: C++ Drawable.h:478 - setAnimationFrame
    pub fn set_animation_frame(&mut self, frame: i32) {
        for module_handle in self.modules() {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| {
                    if let DrawModuleKindMut::Model(w3d_draw) = draw {
                        w3d_draw.set_animation_frame(frame);
                    }
                });
            });
        }
    }

    /// Show or hide a named sub-object on the drawable.
    /// Mirrors C++ Drawable::showSubObject.
    pub fn show_sub_object(&mut self, name: &str, show: bool) {
        for module_handle in self.modules() {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| {
                    if let DrawModuleKindMut::Model(w3d_draw) = draw {
                        w3d_draw.show_sub_object(name, show);
                    }
                });
            });
        }
    }

    /// Update supply crate visual status on draw modules.
    /// Matches C++ Drawable::updateDrawableSupplyStatus.
    pub fn update_supply_status(&mut self, max_supply: i32, current_supply: i32) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_object_draw_interface_mut(module, |draw| {
                    draw.update_supply_status(max_supply, current_supply);
                });
            });
        }
    }

    /// Update projectile clip status for draw modules.
    /// Mirrors C++ Drawable::updateDrawableClipStatus.
    pub fn update_drawable_clip_status(
        &mut self,
        shots_remaining: u32,
        max_shots: u32,
        weapon_slot: WeaponSlotType,
    ) {
        let slot_index = weapon_slot as usize;
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_object_draw_interface_mut(module, |draw| {
                    draw.update_projectile_clip_status(shots_remaining, max_shots, slot_index);
                });
            });
        }
    }

    /// Route weapon-fire FX handling through draw modules.
    /// Mirrors C++ `Drawable::handleWeaponFireFX`.
    pub fn handle_weapon_fire_fx(
        &mut self,
        weapon_slot: WeaponSlotType,
        barrel_index: i32,
        victim_pos: &Coord3D,
    ) -> bool {
        let slot_index = weapon_slot as usize;
        let mut handled = false;
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_object_draw_interface_mut(module, |draw| {
                    if draw.handle_weapon_fire_fx(slot_index, barrel_index, victim_pos) {
                        handled = true;
                    }
                });
            });
        }
        handled
    }

    /// Apply pending sub-object visibility changes.
    /// Mirrors C++ Drawable::updateSubObjects.
    pub fn update_sub_objects(&mut self) {
        for module_handle in self.modules() {
            module_handle.with_module(|module| {
                let _ = with_draw_module_kind(module, |draw| {
                    if let DrawModuleKindMut::Model(w3d_draw) = draw {
                        w3d_draw.update_sub_objects();
                    }
                });
            });
        }
    }
}

impl Snapshot for Drawable {
    fn crc(&self, xfer: &mut dyn Xfer) {
        let mut drawable_id = self.drawable_id;
        let _ = xfer.xfer_unsigned_int(&mut drawable_id);

        let mut object_id = self.object_id;
        let _ = xfer.xfer_object_id(&mut object_id);

        let mut model_conditions = self.model_conditions.bits();
        xfer_u128_bits(xfer, &mut model_conditions);

        let mut hidden = self.hidden;
        let mut hidden_by_stealth = self.hidden_by_stealth;
        let _ = xfer.xfer_bool(&mut hidden);
        let _ = xfer.xfer_bool(&mut hidden_by_stealth);

        let mut tint_status = self.tint_status.0;
        let mut prev_tint_status = self.prev_tint_status.0;
        let _ = xfer.xfer_unsigned_int(&mut tint_status);
        let _ = xfer.xfer_unsigned_int(&mut prev_tint_status);
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let current_version: u8 = 7;
        let mut version = current_version;
        let _ = xfer.xfer_version(&mut version, current_version);

        let xfer_mode = xfer.get_xfer_mode();
        let is_loading = xfer_mode == game_engine::system::XferMode::Load;

        if is_loading {
            self.stop_ambient_sound();
        }

        let mut drawable_id = self.get_drawable_id();
        let _ = xfer.xfer_unsigned_int(&mut drawable_id);
        self.set_drawable_id(drawable_id);

        if version >= 2 {
            let mut condition_state = self.model_conditions;
            xfer_model_condition_flags_legacy(xfer, &mut condition_state);
            self.model_conditions = condition_state;
            if is_loading {
                self.update_conditional_model();
            }
        }

        if version >= 3 {
            if version >= 5 {
                let mut transform = self.transform;
                xfer_matrix3d_legacy(xfer, &mut transform);
                self.set_transform(transform);
            } else {
                let mut position = self.get_position();
                xfer.xfer_coord3d(&mut position);

                let mut orientation = self.world_rotation.y;
                let _ = xfer.xfer_real(&mut orientation);

                if is_loading {
                    let rotation = Quat::from_euler(
                        EulerRot::XYZ,
                        self.world_rotation.x,
                        orientation,
                        self.world_rotation.z,
                    );
                    let transform = Matrix3D::from_scale_rotation_translation(
                        self.world_scale,
                        rotation,
                        position,
                    );
                    self.set_transform(transform);
                }
            }
        }

        let mut has_selection_flash = self.selection_flash_envelope.is_some();
        let _ = xfer.xfer_bool(&mut has_selection_flash);
        if has_selection_flash {
            if self.selection_flash_envelope.is_none() {
                self.selection_flash_envelope = Some(LegacyTintEnvelope::default());
            }
            if let Some(envelope) = self.selection_flash_envelope.as_mut() {
                envelope.xfer(xfer);
            }
        } else if is_loading {
            self.selection_flash_envelope = None;
        }

        let mut has_color_tint = self.color_tint_envelope.is_some();
        let _ = xfer.xfer_bool(&mut has_color_tint);
        if has_color_tint {
            if self.color_tint_envelope.is_none() {
                self.color_tint_envelope = Some(LegacyTintEnvelope::default());
            }
            if let Some(envelope) = self.color_tint_envelope.as_mut() {
                envelope.xfer(xfer);
            }
        } else if is_loading {
            self.color_tint_envelope = None;
        }

        let mut decal_type = terrain_decal_type_to_u32(self.terrain_decal);
        let _ = xfer.xfer_unsigned_int(&mut decal_type);
        if is_loading {
            self.set_terrain_decal(terrain_decal_type_from_u32(decal_type));
        }

        let _ = xfer.xfer_real(&mut self.alpha);
        let _ = xfer.xfer_real(&mut self.stealth_factor);

        let mut effective_stealth_opacity = self.effective_stealth_opacity;
        let _ = xfer.xfer_real(&mut effective_stealth_opacity);
        if is_loading {
            self.effective_stealth_opacity = effective_stealth_opacity.clamp(0.0, 1.0);
        }

        let _ = xfer.xfer_real(&mut self.decal_opacity_fade_target);
        let _ = xfer.xfer_real(&mut self.decal_opacity_fade_rate);
        let _ = xfer.xfer_real(&mut self.decal_opacity);

        let mut object_id = self
            .object_ref
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|object| object.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(self.object_id);
        let _ = xfer.xfer_object_id(&mut object_id);

        if is_loading {
            if let Some(bound_object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(bound_guard) = bound_object.read() {
                    if object_id != bound_guard.get_id() {
                        warn!(
                            "Drawable::xfer object link mismatch for drawable {}: stream object {} != bound object {}",
                            self.drawable_id,
                            object_id,
                            bound_guard.get_id()
                        );
                    }
                }
            }

            self.object_id = object_id;
            if self.object_ref.is_none() && object_id != INVALID_ID {
                self.object_ref = TheGameLogic::find_object_by_id(object_id)
                    .map(|object| Arc::downgrade(&object));
            }
        }

        let mut status_bits = self.drawable_status_bits;
        let _ = xfer.xfer_unsigned_int(&mut status_bits);
        self.drawable_status_bits = status_bits;

        let mut tint_status = self.tint_status.0;
        let _ = xfer.xfer_unsigned_int(&mut tint_status);
        self.tint_status = TintStatus(tint_status);

        let mut prev_tint_status = self.prev_tint_status.0;
        let _ = xfer.xfer_unsigned_int(&mut prev_tint_status);
        self.prev_tint_status = TintStatus(prev_tint_status);

        let _ = xfer.xfer_unsigned_int(&mut self.fade_mode);
        let _ = xfer.xfer_unsigned_int(&mut self.time_elapsed_fade);
        let _ = xfer.xfer_unsigned_int(&mut self.time_to_fade);

        let mut has_loco_info = self.loco_info.is_some();
        let _ = xfer.xfer_bool(&mut has_loco_info);
        if has_loco_info {
            if self.loco_info.is_none() {
                self.loco_info = Some(LegacyDrawableLocoInfo::default());
            }
            if let Some(loco) = self.loco_info.as_mut() {
                loco.xfer(xfer);
            }
        } else if is_loading {
            self.loco_info = None;
        }

        self.xfer_drawable_modules(xfer);

        let mut stealth_look = stealth_look_to_u32(self.stealth_look);
        let _ = xfer.xfer_unsigned_int(&mut stealth_look);
        if is_loading {
            self.stealth_look = stealth_look_from_u32(stealth_look);
        }

        let _ = xfer.xfer_int(&mut self.flash_count);
        let mut flash_color = self.flash_color.to_argb_u32() as i32;
        let _ = xfer.xfer_color(&mut flash_color);
        if is_loading {
            self.flash_color = color_from_argb_u32(flash_color as u32);
        }

        let _ = xfer.xfer_bool(&mut self.hidden);
        let _ = xfer.xfer_bool(&mut self.hidden_by_stealth);

        let _ = xfer.xfer_real(&mut self.second_material_pass_opacity);

        let mut instance_is_identity = self.instance_matrix.is_none();
        let _ = xfer.xfer_bool(&mut instance_is_identity);

        let mut instance_matrix = self.instance_matrix.unwrap_or(Matrix3D::IDENTITY);
        xfer_matrix3d_user_legacy(xfer, &mut instance_matrix);

        let mut instance_scale = self.instance_scale;
        let _ = xfer.xfer_real(&mut instance_scale);

        if is_loading {
            self.instance_matrix = if instance_is_identity {
                None
            } else {
                Some(instance_matrix)
            };
            self.instance_scale = instance_scale;
        }

        let _ = xfer.xfer_object_id(&mut self.shroud_status_object_id);

        if version < 2 {
            let mut condition_state = self.model_conditions;
            xfer_model_condition_flags_legacy(xfer, &mut condition_state);
            self.model_conditions = condition_state;
            if is_loading {
                self.update_conditional_model();
            }
        }

        let _ = xfer.xfer_unsigned_int(&mut self.expiration_date);

        let mut icon_count = self.legacy_icons.len().min(u8::MAX as usize) as u8;
        let _ = xfer.xfer_unsigned_byte(&mut icon_count);
        if xfer_mode == game_engine::system::XferMode::Load {
            self.legacy_icons.clear();
            for _ in 0..icon_count {
                let mut icon = LegacyDrawableIcon::default();
                let _ = xfer.xfer_ascii_string(&mut icon.icon_index_name);
                let _ = xfer.xfer_unsigned_int(&mut icon.keep_till_frame);
                let _ = xfer.xfer_ascii_string(&mut icon.icon_template_name);
                icon.icon_state.xfer(xfer);
                self.legacy_icons.push(icon);
            }
        } else {
            for icon in self.legacy_icons.iter_mut().take(icon_count as usize) {
                let _ = xfer.xfer_ascii_string(&mut icon.icon_index_name);
                let _ = xfer.xfer_unsigned_int(&mut icon.keep_till_frame);
                let _ = xfer.xfer_ascii_string(&mut icon.icon_template_name);
                icon.icon_state.xfer(xfer);
            }
        }

        if version >= 4 {
            let _ = xfer.xfer_bool(&mut self.ambient_sound_enabled);
        }

        if version >= 6 {
            let _ = xfer.xfer_bool(&mut self.ambient_sound_enabled_from_script);
        }

        if version >= 7 {
            let mut customized = self.custom_sound_ambient_info.is_some()
                || self.custom_sound_ambient_dynamic_info.is_some()
                || self.custom_sound_ambient_off;
            let _ = xfer.xfer_bool(&mut customized);

            if customized {
                let mut customized_to_silence = self.custom_sound_ambient_off;
                let _ = xfer.xfer_bool(&mut customized_to_silence);

                if is_loading {
                    if customized_to_silence {
                        self.set_custom_sound_ambient_off();
                    } else {
                        let mut base_info_name = String::new();
                        let _ = xfer.xfer_ascii_string(&mut base_info_name);

                        let manager = get_global_audio_manager()
                            .unwrap_or_else(initialize_global_audio_manager);
                        let (mut customized_info, successful_load) = match manager.lock() {
                            Ok(guard) => {
                                if let Some(base_info) =
                                    guard.find_audio_event_info(&base_info_name)
                                {
                                    (DynamicAudioEventInfo::from_base_info(&base_info), true)
                                } else {
                                    warn!(
                                        "Drawable load: missing base ambient sound '{}'; discarding custom overrides",
                                        base_info_name
                                    );
                                    (DynamicAudioEventInfo::new(), false)
                                }
                            }
                            Err(_) => (DynamicAudioEventInfo::new(), false),
                        };

                        let custom_name = self
                            .mangle_custom_audio_name(&customized_info.audio_event_info.audio_name);
                        customized_info.override_audio_name(&custom_name);
                        let _ = customized_info.xfer_no_name(xfer);

                        if successful_load {
                            self.set_custom_sound_ambient_dynamic_info_internal(
                                customized_info,
                                false,
                            );
                        } else {
                            self.clear_custom_sound_ambient(false);
                            self.custom_sound_ambient_off = false;
                        }
                    }
                } else if !customized_to_silence {
                    let mut base_info_name = self
                        .custom_sound_ambient_dynamic_info
                        .as_ref()
                        .map(|info| info.get_original_name().to_string())
                        .or_else(|| {
                            self.custom_sound_ambient_info
                                .as_ref()
                                .map(|info| info.audio_name.clone())
                        })
                        .unwrap_or_default();
                    let _ = xfer.xfer_ascii_string(&mut base_info_name);

                    if let Some(customized_info) = self.custom_sound_ambient_dynamic_info.as_mut() {
                        let _ = customized_info.xfer_no_name(xfer);
                    } else if let Some(info) = &self.custom_sound_ambient_info {
                        let mut fallback = DynamicAudioEventInfo::from_base_info(info.as_ref());
                        let _ = fallback.xfer_no_name(xfer);
                    }
                }
            } else if is_loading {
                self.custom_sound_ambient_off = false;
                self.custom_sound_ambient_info = None;
                self.custom_sound_ambient_dynamic_info = None;
            }
        }

        if is_loading {
            // C++ parity: do not trust serialized stealth look; StealthUpdate will
            // re-drive the correct state on the next logic update.
            self.stealth_look = StealthLookType::None;
            if self.hidden || self.hidden_by_stealth {
                self.update_hidden_status();
            }
        }
    }

    fn load_post_process(&mut self) {
        if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(object_guard) = object.read() {
                self.object_id = object_guard.get_id();
                self.set_transform(object_guard.get_transform_matrix());
            }
        }

        if self.ambient_sound_enabled && self.ambient_sound_enabled_from_script {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(object_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound_internal(&object_guard, time_of_day, true);
                } else {
                    self.stop_ambient_sound();
                }
            } else {
                self.stop_ambient_sound();
            }
        } else {
            self.stop_ambient_sound();
        }
    }
}

impl crate::drawable::Drawable for Drawable {
    /// Draw the drawable at a specific position
    /// Reference: C++ Drawable.cpp - rendering is delegated to draw modules
    fn draw(&mut self, transform: Option<&Matrix3D>) {
        let object_effectively_dead = self
            .object_ref
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|object| object.read().ok().map(|guard| guard.is_effectively_dead()))
            .unwrap_or(false);

        // C++ Drawable::draw parity: fade thermal/second pass unless frenzy tint is active.
        if !self.test_tint_status(TintStatus::FRENZY) {
            if object_effectively_dead {
                self.second_material_pass_opacity = 0.0;
            } else if self.second_material_pass_opacity > VERY_TRANSPARENT_MATERIAL_PASS_OPACITY {
                self.second_material_pass_opacity *= MATERIAL_PASS_OPACITY_FADE_SCALAR;
            } else {
                self.second_material_pass_opacity = 0.0;
            }
        }

        if self.hidden || self.hidden_by_stealth || self.drawable_fully_obscured_by_shroud {
            return;
        }

        if self.object_ref.is_some() && !object_effectively_dead {
            self.set_shadows_enabled(!matches!(
                self.stealth_look,
                StealthLookType::VisibleDetected
            ));
        }

        let mut transform_mtx = transform.copied().unwrap_or(self.transform);
        if let Some(instance_mtx) = self.instance_matrix {
            transform_mtx = transform_mtx * instance_mtx;
        }

        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| {
                    draw.do_draw_module(&transform_mtx);
                });
            });
        }
    }

    fn is_visible(&self) -> bool {
        self.is_visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.is_visible = visible;
    }

    /// Get current world transform
    fn get_transform(&self) -> Matrix3D {
        self.transform
    }
}

impl Material {
    /// Create a default material
    pub fn default() -> Self {
        Material {
            diffuse_texture: None,
            normal_texture: None,
            specular_texture: None,
            emissive_texture: None,
            diffuse_color: Color::white(),
            specular_color: Color::white(),
            emissive_color: Color::black(),
            shininess: 32.0,
            transparency: 0.0,
            reflectivity: 0.0,
            texture_scale: Coord2D::new(1.0, 1.0),
            texture_offset: Coord2D::ZERO,
            animation_rate: 0.0,
        }
    }
}

/// Extension trait for Object to provide Drawable access
pub trait DrawableExt {
    /// Get drawable associated with this object
    fn get_drawable(&self) -> Option<Arc<RwLock<Drawable>>>;
    fn set_drawable(&mut self, drawable: Option<Arc<RwLock<Drawable>>>);
}

#[derive(Debug, Clone)]
pub(crate) struct DrawableThingHandle {
    drawable: Weak<RwLock<Drawable>>,
}

impl DrawableThingHandle {
    pub fn new(drawable: &Arc<RwLock<Drawable>>) -> Self {
        Self {
            drawable: Arc::downgrade(drawable),
        }
    }

    pub fn upgrade(&self) -> Option<Arc<RwLock<Drawable>>> {
        self.drawable.upgrade()
    }
}

impl ModuleDrawableTrait for DrawableThingHandle {
    fn get_drawable_id(&self) -> u32 {
        self.upgrade()
            .and_then(|drawable| drawable.read().ok().map(|guard| guard.drawable_id))
            .unwrap_or(0)
    }
}

impl ModuleThing for DrawableThingHandle {
    fn as_drawable(&self) -> Option<&dyn ModuleDrawableTrait> {
        Some(self)
    }
}

/// Extension trait for Arc<rhai::Locked<Drawable>> to provide helper methods
pub trait DrawableArcExt {
    fn get_id(&self) -> DrawableID;
    fn get_object_id(&self) -> ObjectID;
    fn get_model_condition_flags(&self) -> ModelConditionFlags;
    fn get_transform(&self) -> Matrix3D;
    fn get_instance_matrix(&self) -> Matrix3D;
    fn set_instance_matrix(&self, matrix: Option<&Matrix3D>);
    fn set_shadows_enabled(&self, enabled: bool);
    fn set_terrain_decal(&self, decal_type: TerrainDecalType);
    fn set_terrain_decal_size(&self, x: Real, y: Real);
    fn set_terrain_decal_fade_target(&self, target: Real, rate: Real);
    fn init_rope_draw_params(
        &self,
        length: Real,
        width: Real,
        color: RGBColor,
        wobble_len: Real,
        wobble_amp: Real,
        wobble_rate: Real,
    );
    fn set_rope_cur_len(&self, length: Real);
    fn set_rope_speed(&self, cur_speed: Real, max_speed: Real, accel: Real);
    fn update_bones_for_client_particle_systems(&self) -> bool;
    fn set_model_condition_state(&self, state: ModelConditionFlags);
    fn set_drawable_hidden(&self, hidden: bool);
    fn is_drawable_effectively_hidden(&self) -> bool;
    fn set_swaying_enabled(&self, enabled: bool);
    fn clear_model_condition_flags(&self, clear: ModelConditionFlags);
    fn clear_model_condition_state(&self, state: ModelConditionFlags);
    fn clear_and_set_model_condition_flags(
        &self,
        clear: &ModelConditionFlags,
        set: &ModelConditionFlags,
    );
    fn clear_and_set_model_condition_state(
        &self,
        clear: ModelConditionFlags,
        set: ModelConditionFlags,
    );
    fn get_projectile_launch_offset(
        &self,
        weapon_slot: WeaponSlotType,
        barrel_index: i32,
        turret_type: TurretType,
    ) -> Option<ProjectileLaunchOffset>;
    fn get_draw_modules(&self) -> Vec<DrawableModuleHandle>;
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectileLaunchOffset {
    pub transform: Matrix3D,
    pub turret_rot_pos: Coord3D,
    pub turret_pitch_pos: Coord3D,
}

impl DrawableArcExt for Arc<RwLock<Drawable>> {
    /// Get the drawable ID associated with this drawable
    fn get_id(&self) -> DrawableID {
        if let Ok(guard) = self.read() {
            guard.drawable_id
        } else {
            INVALID_ID
        }
    }

    fn get_object_id(&self) -> ObjectID {
        if let Ok(guard) = self.read() {
            guard.object_id
        } else {
            INVALID_ID
        }
    }

    /// Get the current model condition flags
    fn get_model_condition_flags(&self) -> ModelConditionFlags {
        if let Ok(guard) = self.read() {
            guard.model_conditions
        } else {
            ModelConditionFlags::empty()
        }
    }

    /// Get the current world transform
    fn get_transform(&self) -> Matrix3D {
        if let Ok(guard) = self.read() {
            guard.transform
        } else {
            Matrix3D::IDENTITY
        }
    }

    fn get_instance_matrix(&self) -> Matrix3D {
        if let Ok(guard) = self.read() {
            guard.instance_matrix.unwrap_or(Matrix3D::IDENTITY)
        } else {
            Matrix3D::IDENTITY
        }
    }

    /// Set the instance matrix for this drawable (used for jitter effects, rocking, etc.)
    fn set_instance_matrix(&self, matrix: Option<&Matrix3D>) {
        if let Ok(mut guard) = self.write() {
            guard.instance_matrix = matrix.cloned();
        }
    }

    /// Enable or disable shadow casting for this drawable
    fn set_shadows_enabled(&self, enabled: bool) {
        if let Ok(mut guard) = self.write() {
            guard.set_shadows_enabled(enabled);
        }
    }

    fn set_terrain_decal(&self, decal_type: TerrainDecalType) {
        if let Ok(mut guard) = self.write() {
            guard.set_terrain_decal(decal_type);
        }
    }

    fn set_terrain_decal_size(&self, x: Real, y: Real) {
        if let Ok(mut guard) = self.write() {
            guard.set_terrain_decal_size(x, y);
        }
    }

    /// Set terrain decal fade target and rate
    fn set_terrain_decal_fade_target(&self, target: Real, rate: Real) {
        if let Ok(mut guard) = self.write() {
            guard.set_terrain_decal_fade_target(target, rate);
        }
    }

    fn init_rope_draw_params(
        &self,
        length: Real,
        width: Real,
        color: RGBColor,
        wobble_len: Real,
        wobble_amp: Real,
        wobble_rate: Real,
    ) {
        if let Ok(guard) = self.read() {
            for module_handle in guard.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
                module_handle.with_module(|module| {
                    with_rope_draw_interface_mut(module, |rope| {
                        rope.init_rope_parms(
                            length,
                            width,
                            &color,
                            wobble_len,
                            wobble_amp,
                            wobble_rate,
                        );
                    });
                });
            }
        }
    }

    fn set_rope_cur_len(&self, length: Real) {
        if let Ok(guard) = self.read() {
            for module_handle in guard.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
                module_handle.with_module(|module| {
                    with_rope_draw_interface_mut(module, |rope| {
                        rope.set_rope_cur_len(length);
                    });
                });
            }
        }
    }

    fn set_rope_speed(&self, cur_speed: Real, max_speed: Real, accel: Real) {
        if let Ok(guard) = self.read() {
            for module_handle in guard.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
                module_handle.with_module(|module| {
                    with_rope_draw_interface_mut(module, |rope| {
                        rope.set_rope_speed(cur_speed, max_speed, accel);
                    });
                });
            }
        }
    }

    fn update_bones_for_client_particle_systems(&self) -> bool {
        if let Ok(guard) = self.read() {
            for module_handle in guard.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
                let updated = module_handle.with_module(|module| {
                    let mut result = false;
                    with_draw_module_mut(module, |draw| {
                        result = draw.update_bones_for_client_particle_systems();
                    });
                    result
                });
                if updated {
                    return true;
                }
            }
        }

        false
    }

    /// Set model condition state
    fn set_model_condition_state(&self, state: ModelConditionFlags) {
        if let Ok(mut guard) = self.write() {
            guard.set_model_condition_state(state);
        }
    }

    /// Set whether the drawable is hidden
    fn set_drawable_hidden(&self, hidden: bool) {
        if let Ok(mut guard) = self.write() {
            let _ = guard.set_drawable_hidden(hidden);
        }
    }

    fn set_swaying_enabled(&self, enabled: bool) {
        if let Ok(mut guard) = self.write() {
            guard.set_swaying_enabled(enabled);
        }
    }

    /// Check if the drawable is effectively hidden (by explicit hide or stealth)
    /// Matches C++ Drawable.h line 305: isDrawableEffectivelyHidden()
    fn is_drawable_effectively_hidden(&self) -> bool {
        if let Ok(guard) = self.read() {
            guard.is_drawable_effectively_hidden()
        } else {
            false
        }
    }

    /// Clear model condition flags
    fn clear_model_condition_flags(&self, clear: ModelConditionFlags) {
        if let Ok(mut guard) = self.write() {
            guard.clear_model_condition_flags(clear);
        }
    }

    fn clear_model_condition_state(&self, state: ModelConditionFlags) {
        if let Ok(mut guard) = self.write() {
            guard.clear_model_condition_state(state);
        }
    }

    /// Clear and set model condition flags atomically
    fn clear_and_set_model_condition_flags(
        &self,
        clear: &ModelConditionFlags,
        set: &ModelConditionFlags,
    ) {
        if let Ok(mut guard) = self.write() {
            guard.clear_and_set_model_condition_state(*clear, *set);
        }
    }

    /// Clear and set model condition state atomically (alias for clear_and_set_model_condition_flags)
    /// This method provides backward compatibility with code expecting this method name
    fn clear_and_set_model_condition_state(
        &self,
        clear: ModelConditionFlags,
        set: ModelConditionFlags,
    ) {
        if let Ok(mut guard) = self.write() {
            guard.clear_and_set_model_condition_state(clear, set);
        }
    }

    /// Get projectile launch offset for a specific weapon slot and barrel
    fn get_projectile_launch_offset(
        &self,
        weapon_slot: WeaponSlotType,
        barrel_index: i32,
        turret_type: TurretType,
    ) -> Option<ProjectileLaunchOffset> {
        use crate::object::draw::draw_module::ObjectDrawInterface;
        use crate::object::draw::w3d_model_draw::W3DModelDraw;

        if let Ok(guard) = self.read() {
            let condition = guard.model_conditions;
            let mut launch_pos = Matrix3D::IDENTITY;
            let mut turret_rot_pos = Coord3D::origin();
            let mut turret_pitch_pos = Coord3D::origin();

            // Iterate through all draw modules and find one that can provide the launch offset
            for module_handle in guard.modules() {
                // Try downcasting to W3DModelDraw which implements ObjectDrawInterface
                let found = module_handle
                    .with_module_downcast::<W3DModelDraw, _, _>(|draw_module| {
                        draw_module.get_projectile_launch_offset(
                            &condition,
                            weapon_slot as usize,
                            barrel_index,
                            &mut launch_pos,
                            turret_type,
                            &mut turret_rot_pos,
                            &mut turret_pitch_pos,
                        )
                    })
                    .unwrap_or(false);

                if found {
                    return Some(ProjectileLaunchOffset {
                        transform: launch_pos,
                        turret_rot_pos,
                        turret_pitch_pos,
                    });
                }
            }
        }

        None
    }

    /// Get all draw modules registered with this drawable
    fn get_draw_modules(&self) -> Vec<DrawableModuleHandle> {
        if let Ok(guard) = self.read() {
            guard.modules()
        } else {
            Vec::new()
        }
    }
}
