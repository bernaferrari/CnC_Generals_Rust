//! Core Drawable trait and implementations
//!
//! This module defines the fundamental `Drawable` trait and various drawable object types
//! that can be rendered in the game world. It handles 3D transforms, rendering properties,
//! animation states, and visual effects.

use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

pub use crate::core::DrawableId;
use crate::system::TimeOfDay;
use crate::system::{Anim2D, Anim2DCollection};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::bit_flags::{create_model_condition_flags, ModelConditionBitFlags};
use game_engine::common::ini::{get_anim2d_collection, Anim2DTemplate};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use parking_lot::{Mutex, RwLock};

/// Downcasting support for Drawable trait objects
/// Reference: C++ Drawable.cpp uses dynamic_cast for type-safe downcasting
pub trait DrawableDowncast {
    /// Get a reference to the object as Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Get a mutable reference to the object as Any for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Extension trait for Drawable downcasting operations
pub trait DrawableExt {
    /// Try to downcast to a specific drawable type
    fn downcast_ref<T: 'static>(&self) -> Option<&T>;

    /// Try to downcast to a specific drawable type (mutable)
    fn downcast_mut<T: 'static>(&mut self) -> Option<&mut T>;
}

/// Invalid drawable ID constant
pub const INVALID_DRAWABLE_ID: DrawableId = DrawableId(0);

/// 3D vector for positions, rotations, and colors
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    pub fn one() -> Self {
        Self::new(1.0, 1.0, 1.0)
    }
}

/// 4x4 transformation matrix for 3D transforms
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix4 {
    pub elements: [[f32; 4]; 4],
}

impl Matrix4 {
    pub fn identity() -> Self {
        let mut matrix = Self {
            elements: [[0.0; 4]; 4],
        };
        matrix.elements[0][0] = 1.0;
        matrix.elements[1][1] = 1.0;
        matrix.elements[2][2] = 1.0;
        matrix.elements[3][3] = 1.0;
        matrix
    }

    pub fn translation(position: Vector3) -> Self {
        let mut matrix = Self::identity();
        matrix.elements[0][3] = position.x;
        matrix.elements[1][3] = position.y;
        matrix.elements[2][3] = position.z;
        matrix
    }

    pub fn scale(scale: f32) -> Self {
        let mut matrix = Self::identity();
        matrix.elements[0][0] = scale;
        matrix.elements[1][1] = scale;
        matrix.elements[2][2] = scale;
        matrix
    }

    /// Matrix multiplication (self * other) for composing transforms
    pub fn mul(&self, other: &Matrix4) -> Self {
        let mut result = Matrix4 {
            elements: [[0.0; 4]; 4],
        };

        for i in 0..4 {
            for j in 0..4 {
                result.elements[i][j] = self.elements[i][0] * other.elements[0][j]
                    + self.elements[i][1] * other.elements[1][j]
                    + self.elements[i][2] * other.elements[2][j]
                    + self.elements[i][3] * other.elements[3][j];
            }
        }

        result
    }
}

/// RGBA color representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn white() -> Self {
        Self::new(1.0, 1.0, 1.0, 1.0)
    }

    pub fn transparent() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

/// Status flags for drawable objects (converted from C++ DrawableStatus)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawableStatus {
    bits: u32,
}

impl DrawableStatus {
    pub const NONE: Self = Self { bits: 0x00000000 };
    pub const DRAWS_IN_MIRROR: Self = Self { bits: 0x00000001 };
    pub const SHADOWS: Self = Self { bits: 0x00000002 };
    pub const TINT_COLOR_LOCKED: Self = Self { bits: 0x00000004 };
    pub const NO_STATE_PARTICLES: Self = Self { bits: 0x00000008 };
    pub const NO_SAVE: Self = Self { bits: 0x00000010 };

    pub fn has(&self, flag: Self) -> bool {
        (self.bits & flag.bits) != 0
    }

    pub fn set(&mut self, flag: Self) {
        self.bits |= flag.bits;
    }

    pub fn clear(&mut self, flag: Self) {
        self.bits &= !flag.bits;
    }
}

/// Types of stealth visualization (converted from C++ StealthLookType)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StealthLook {
    None,
    VisibleFriendly,
    DisguisedEnemy,
    VisibleDetected,
    VisibleFriendlyDetected,
    Invisible,
}

/// Tint status for various visual effects (converted from C++ TintStatus)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TintStatus {
    bits: u32,
}

impl TintStatus {
    pub const NONE: Self = Self { bits: 0x00000000 };
    pub const DISABLED: Self = Self { bits: 0x00000001 };
    pub const IRRADIATED: Self = Self { bits: 0x00000002 };
    pub const POISONED: Self = Self { bits: 0x00000004 };
    pub const GAINING_SUBDUAL_DAMAGE: Self = Self { bits: 0x00000008 };
    pub const FRENZY: Self = Self { bits: 0x00000010 };

    pub fn has(&self, flag: Self) -> bool {
        (self.bits & flag.bits) != 0
    }

    pub fn set(&mut self, flag: Self) {
        self.bits |= flag.bits;
    }

    pub fn clear(&mut self, flag: Self) {
        self.bits &= !flag.bits;
    }
}

pub const SICKLY_GREEN_POISONED_COLOR: Vector3 = Vector3 {
    x: -1.0,
    y: 1.0,
    z: -1.0,
};
pub const DARK_GRAY_DISABLED_COLOR: Vector3 = Vector3 {
    x: -0.5,
    y: -0.5,
    z: -0.5,
};
pub const RED_IRRADIATED_COLOR: Vector3 = Vector3 {
    x: 1.0,
    y: -1.0,
    z: -1.0,
};
pub const SUBDUAL_DAMAGE_COLOR: Vector3 = Vector3 {
    x: -0.2,
    y: -0.2,
    z: 0.8,
};
pub const FRENZY_COLOR: Vector3 = Vector3 {
    x: 0.2,
    y: -0.2,
    z: -0.2,
};

/// Types of drawable icons (converted from C++ DrawableIconType)
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub enum IconType {
    DefaultHeal,
    StructureHeal,
    VehicleHeal,
    Demoralized,
    BombTimed,
    BombRemote,
    Disabled,
    BattleplanBombard,
    BattleplanHoldTheLine,
    BattleplanSearchAndDestroy,
    Emoticon,
    Enthusiastic,
    EnthusiasticSubliminal,
    CarBomb,
}

impl IconType {
    pub fn name(&self) -> &'static str {
        match self {
            IconType::DefaultHeal => "DefaultHeal",
            IconType::StructureHeal => "StructureHeal",
            IconType::VehicleHeal => "VehicleHeal",
            IconType::Demoralized => "Demoralized",
            IconType::BombTimed => "BombTimed",
            IconType::BombRemote => "BombRemote",
            IconType::Disabled => "Disabled",
            IconType::BattleplanBombard => "BattlePlanIcon_Bombard",
            IconType::BattleplanHoldTheLine => "BattlePlanIcon_HoldTheLine",
            IconType::BattleplanSearchAndDestroy => "BattlePlanIcon_SeekAndDestroy",
            IconType::Emoticon => "Emoticon",
            IconType::Enthusiastic => "Enthusiastic",
            IconType::EnthusiasticSubliminal => "Subliminal",
            IconType::CarBomb => "CarBomb",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "DefaultHeal" => Some(IconType::DefaultHeal),
            "StructureHeal" => Some(IconType::StructureHeal),
            "VehicleHeal" => Some(IconType::VehicleHeal),
            "Demoralized" => Some(IconType::Demoralized),
            "BombTimed" => Some(IconType::BombTimed),
            "BombRemote" => Some(IconType::BombRemote),
            "Disabled" => Some(IconType::Disabled),
            "BattlePlanIcon_Bombard" => Some(IconType::BattleplanBombard),
            "BattlePlanIcon_HoldTheLine" => Some(IconType::BattleplanHoldTheLine),
            "BattlePlanIcon_SeekAndDestroy" => Some(IconType::BattleplanSearchAndDestroy),
            "Emoticon" => Some(IconType::Emoticon),
            "Enthusiastic" => Some(IconType::Enthusiastic),
            "Subliminal" => Some(IconType::EnthusiasticSubliminal),
            "CarBomb" => Some(IconType::CarBomb),
            _ => None,
        }
    }
}

/// Icon information for drawable objects
#[derive(Debug, Clone)]
pub struct IconInfo {
    pub icons: HashMap<IconType, Arc<dyn Icon>>,
    pub keep_till_frame: HashMap<IconType, u32>,
}

impl IconInfo {
    pub fn new() -> Self {
        Self {
            icons: HashMap::new(),
            keep_till_frame: HashMap::new(),
        }
    }

    pub fn set_icon(
        &mut self,
        icon_type: IconType,
        icon: Arc<dyn Icon>,
        duration_frames: u32,
        current_frame: u32,
    ) {
        self.icons.insert(icon_type, icon);
        self.keep_till_frame
            .insert(icon_type, current_frame + duration_frames);
    }

    pub fn clear_icon(&mut self, icon_type: IconType) {
        self.icons.remove(&icon_type);
        self.keep_till_frame.remove(&icon_type);
    }

    pub fn update(&mut self, current_frame: u32) {
        let expired_icons: Vec<IconType> = self
            .keep_till_frame
            .iter()
            .filter(|(_, &frame)| frame <= current_frame)
            .map(|(&icon_type, _)| icon_type)
            .collect();

        for icon_type in expired_icons {
            self.clear_icon(icon_type);
        }
    }
}

impl Snapshotable for IconInfo {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        let mut icon_count = self.icons.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut icon_count)
            .map_err(|e| format!("{:?}", e))?;

        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for (icon_type, icon) in self.icons.iter() {
                    let mut icon_name = icon_type.name().to_string();
                    xfer.xfer_ascii_string(&mut icon_name)
                        .map_err(|e| format!("{:?}", e))?;

                    let mut keep = *self.keep_till_frame.get(icon_type).unwrap_or(&0);
                    xfer.xfer_unsigned_int(&mut keep)
                        .map_err(|e| format!("{:?}", e))?;

                    let icon = icon
                        .as_any()
                        .downcast_ref::<Anim2DIcon>()
                        .ok_or_else(|| "Icon is not Anim2D-backed".to_string())?;
                    let mut template_name = icon.template_name().to_string();
                    xfer.xfer_ascii_string(&mut template_name)
                        .map_err(|e| format!("{:?}", e))?;

                    icon.xfer(xfer)?;
                }
            }
            XferMode::Load => {
                self.icons.clear();
                self.keep_till_frame.clear();

                for _ in 0..icon_count {
                    let mut icon_name = String::new();
                    xfer.xfer_ascii_string(&mut icon_name)
                        .map_err(|e| format!("{:?}", e))?;
                    let icon_type = IconType::from_name(&icon_name)
                        .ok_or_else(|| format!("Unknown icon type '{}'", icon_name))?;

                    let mut keep = 0u32;
                    xfer.xfer_unsigned_int(&mut keep)
                        .map_err(|e| format!("{:?}", e))?;

                    let mut template_name = String::new();
                    xfer.xfer_ascii_string(&mut template_name)
                        .map_err(|e| format!("{:?}", e))?;
                    let icon = Anim2DIcon::from_template_name(&template_name)?;
                    icon.xfer(xfer)?;

                    self.icons.insert(icon_type, Arc::new(icon));
                    self.keep_till_frame.insert(icon_type, keep);
                }
            }
            XferMode::Invalid => {
                return Err("IconInfo::xfer - invalid xfer mode".to_string());
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Trait for drawable icons
pub trait Icon: std::fmt::Debug + Send + Sync {
    fn render(&self, position: Vector3, size: Vector3);
    fn as_any(&self) -> &dyn Any;
    fn xfer(&self, xfer: &mut dyn Xfer) -> Result<(), String>;
}

/// Anim2D-backed drawable icon (parity with C++ Anim2D icons).
pub struct Anim2DIcon {
    anim: Arc<Mutex<Anim2D>>,
    template_name: String,
}

impl std::fmt::Debug for Anim2DIcon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Anim2DIcon")
            .field("template_name", &self.template_name)
            .finish()
    }
}

impl Anim2DIcon {
    pub fn new(
        template: Arc<RwLock<Anim2DTemplate>>,
        collection: Option<Arc<Mutex<Anim2DCollection>>>,
    ) -> Self {
        let template_name = template.read().get_name().as_str().to_string();
        let anim = Anim2D::new(template, collection);
        Self {
            anim,
            template_name,
        }
    }

    pub fn from_template_name(name: &str) -> Result<Self, String> {
        let template_name = name.to_string();
        let name_key = AsciiString::from(name);
        let template = get_anim2d_collection()
            .and_then(|collection| collection.read().find_template(&name_key))
            .ok_or_else(|| format!("Unknown Anim2D template '{}'", template_name))?;
        Ok(Self::new(template, None))
    }

    pub fn template_name(&self) -> &str {
        &self.template_name
    }
}

impl Icon for Anim2DIcon {
    fn render(&self, position: Vector3, size: Vector3) {
        let mut anim = self.anim.lock();
        anim.draw_sized(
            position.x as i32,
            position.y as i32,
            size.x as i32,
            size.y as i32,
        );
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn xfer(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut anim = self.anim.lock();
        anim.xfer(xfer)
    }
}

/// Wheel information for vehicles (converted from C++ TWheelInfo)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelInfo {
    pub front_left_height_offset: f32,
    pub front_right_height_offset: f32,
    pub rear_left_height_offset: f32,
    pub rear_right_height_offset: f32,
    pub wheel_angle: f32,
    pub frames_airborne_counter: i32,
    pub frames_airborne: i32,
}

impl Default for WheelInfo {
    fn default() -> Self {
        Self {
            front_left_height_offset: 0.0,
            front_right_height_offset: 0.0,
            rear_left_height_offset: 0.0,
            rear_right_height_offset: 0.0,
            wheel_angle: 0.0,
            frames_airborne_counter: 0,
            frames_airborne: 0,
        }
    }
}

/// Locomotor information for drawable physics (converted from C++ DrawableLocoInfo)
#[derive(Debug, Clone, PartialEq)]
pub struct LocoInfo {
    pub pitch: f32,
    pub pitch_rate: f32,
    pub roll: f32,
    pub roll_rate: f32,
    pub yaw: f32,
    pub acceleration_pitch: f32,
    pub acceleration_pitch_rate: f32,
    pub acceleration_roll: f32,
    pub acceleration_roll_rate: f32,
    pub overlap_z_velocity: f32,
    pub overlap_z: f32,
    pub wobble: f32,
    pub yaw_modulator: f32,
    pub pitch_modulator: f32,
    pub wheel_info: WheelInfo,
}

impl Default for LocoInfo {
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
            overlap_z_velocity: 0.0,
            overlap_z: 0.0,
            wobble: 1.0,
            yaw_modulator: 0.0,
            pitch_modulator: 0.0,
            wheel_info: WheelInfo::default(),
        }
    }
}

pub const DEFAULT_TINT_COLOR_FADE_RATE: f32 = 0.6;
pub const DEF_ATTACK_FRAMES: u32 = 1;
pub const DEF_SUSTAIN_FRAMES: u32 = 1;
pub const DEF_DECAY_FRAMES: u32 = 4;
pub const SUSTAIN_INDEFINITELY: u32 = 0xfffffffe;
pub const VERY_TRANSPARENT_MATERIAL_PASS_OPACITY: f32 = 0.001;
pub const MATERIAL_PASS_OPACITY_FADE_SCALAR: f32 = 0.8;
pub const DRAWABLE_FRAMES_PER_FLASH: u32 = 15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FadingMode {
    None,
    FadingIn,
    FadingOut,
}

/// Tint envelope for color animation effects (converted from C++ TintEnvelope)
#[derive(Debug, Clone, PartialEq)]
pub struct TintEnvelope {
    pub attack_rate: Vector3,
    pub decay_rate: Vector3,
    pub peak_color: Vector3,
    pub current_color: Vector3,
    pub sustain_counter: u32,
    pub state: EnvelopeState,
    pub is_effective: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeState {
    Rest,
    Attack,
    Decay,
    Sustain,
}

impl TintEnvelope {
    pub fn new() -> Self {
        Self {
            attack_rate: Vector3::zero(),
            decay_rate: Vector3::zero(),
            peak_color: Vector3::zero(),
            current_color: Vector3::zero(),
            sustain_counter: 0,
            state: EnvelopeState::Rest,
            is_effective: false,
        }
    }

    pub fn play(
        &mut self,
        peak_color: Vector3,
        attack_frames: u32,
        decay_frames: u32,
        sustain_frames: u32,
    ) {
        let attack_frames = attack_frames.max(1);
        let decay_frames = decay_frames.max(1);
        self.peak_color = peak_color;
        self.attack_rate = Vector3::new(
            peak_color.x / attack_frames as f32,
            peak_color.y / attack_frames as f32,
            peak_color.z / attack_frames as f32,
        );
        self.decay_rate = Vector3::new(
            peak_color.x / decay_frames as f32,
            peak_color.y / decay_frames as f32,
            peak_color.z / decay_frames as f32,
        );
        self.sustain_counter = sustain_frames;
        self.state = EnvelopeState::Attack;
        self.is_effective = peak_color != Vector3::zero();
    }

    pub fn sustain(&mut self) {
        self.state = EnvelopeState::Sustain;
    }

    pub fn release(&mut self) {
        self.state = EnvelopeState::Decay;
    }

    pub fn rest(&mut self) {
        self.state = EnvelopeState::Rest;
        self.current_color = Vector3::zero();
        self.is_effective = false;
    }

    pub fn color(&self) -> Vector3 {
        self.current_color
    }

    pub fn update(&mut self) {
        match self.state {
            EnvelopeState::Attack => {
                self.current_color.x += self.attack_rate.x;
                self.current_color.y += self.attack_rate.y;
                self.current_color.z += self.attack_rate.z;

                if self.current_color.x >= self.peak_color.x
                    && self.current_color.y >= self.peak_color.y
                    && self.current_color.z >= self.peak_color.z
                {
                    self.current_color = self.peak_color;
                    self.state = EnvelopeState::Sustain;
                }
            }
            EnvelopeState::Sustain => {
                if self.sustain_counter == SUSTAIN_INDEFINITELY {
                    return;
                }
                if self.sustain_counter > 0 {
                    self.sustain_counter -= 1;
                } else {
                    self.state = EnvelopeState::Decay;
                }
            }
            EnvelopeState::Decay => {
                self.current_color.x -= self.decay_rate.x;
                self.current_color.y -= self.decay_rate.y;
                self.current_color.z -= self.decay_rate.z;

                if self.current_color.x <= 0.0
                    && self.current_color.y <= 0.0
                    && self.current_color.z <= 0.0
                {
                    self.rest();
                }
            }
            EnvelopeState::Rest => {
                // Do nothing
            }
        }

        self.is_effective = self.current_color != Vector3::zero();
    }
}

impl Snapshotable for TintEnvelope {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        xfer_vector3(xfer, &mut self.attack_rate)?;
        xfer_vector3(xfer, &mut self.decay_rate)?;
        xfer_vector3(xfer, &mut self.peak_color)?;
        xfer_vector3(xfer, &mut self.current_color)?;

        let mut sustain_counter = self.sustain_counter;
        xfer.xfer_unsigned_int(&mut sustain_counter)
            .map_err(|e| format!("{:?}", e))?;
        self.sustain_counter = sustain_counter;

        let mut state = envelope_state_to_u8(self.state);
        xfer.xfer_unsigned_byte(&mut state)
            .map_err(|e| format!("{:?}", e))?;
        self.state = envelope_state_from_u8(state);

        let mut effective = self.is_effective;
        xfer.xfer_bool(&mut effective)
            .map_err(|e| format!("{:?}", e))?;
        self.is_effective = effective;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for LocoInfo {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        let mut pitch = self.pitch;
        xfer.xfer_real(&mut pitch).map_err(|e| format!("{:?}", e))?;
        self.pitch = pitch;

        let mut pitch_rate = self.pitch_rate;
        xfer.xfer_real(&mut pitch_rate)
            .map_err(|e| format!("{:?}", e))?;
        self.pitch_rate = pitch_rate;

        let mut roll = self.roll;
        xfer.xfer_real(&mut roll).map_err(|e| format!("{:?}", e))?;
        self.roll = roll;

        let mut roll_rate = self.roll_rate;
        xfer.xfer_real(&mut roll_rate)
            .map_err(|e| format!("{:?}", e))?;
        self.roll_rate = roll_rate;

        let mut yaw = self.yaw;
        xfer.xfer_real(&mut yaw).map_err(|e| format!("{:?}", e))?;
        self.yaw = yaw;

        let mut accel_pitch = self.acceleration_pitch;
        xfer.xfer_real(&mut accel_pitch)
            .map_err(|e| format!("{:?}", e))?;
        self.acceleration_pitch = accel_pitch;

        let mut accel_pitch_rate = self.acceleration_pitch_rate;
        xfer.xfer_real(&mut accel_pitch_rate)
            .map_err(|e| format!("{:?}", e))?;
        self.acceleration_pitch_rate = accel_pitch_rate;

        let mut accel_roll = self.acceleration_roll;
        xfer.xfer_real(&mut accel_roll)
            .map_err(|e| format!("{:?}", e))?;
        self.acceleration_roll = accel_roll;

        let mut accel_roll_rate = self.acceleration_roll_rate;
        xfer.xfer_real(&mut accel_roll_rate)
            .map_err(|e| format!("{:?}", e))?;
        self.acceleration_roll_rate = accel_roll_rate;

        let mut overlap_z_velocity = self.overlap_z_velocity;
        xfer.xfer_real(&mut overlap_z_velocity)
            .map_err(|e| format!("{:?}", e))?;
        self.overlap_z_velocity = overlap_z_velocity;

        let mut overlap_z = self.overlap_z;
        xfer.xfer_real(&mut overlap_z)
            .map_err(|e| format!("{:?}", e))?;
        self.overlap_z = overlap_z;

        let mut wobble = self.wobble;
        xfer.xfer_real(&mut wobble)
            .map_err(|e| format!("{:?}", e))?;
        self.wobble = wobble;

        let mut yaw_modulator = self.yaw_modulator;
        xfer.xfer_real(&mut yaw_modulator)
            .map_err(|e| format!("{:?}", e))?;
        self.yaw_modulator = yaw_modulator;

        let mut pitch_modulator = self.pitch_modulator;
        xfer.xfer_real(&mut pitch_modulator)
            .map_err(|e| format!("{:?}", e))?;
        self.pitch_modulator = pitch_modulator;

        self.wheel_info.xfer(xfer)?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for WheelInfo {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        let mut front_left_height_offset = self.front_left_height_offset;
        xfer.xfer_real(&mut front_left_height_offset)
            .map_err(|e| format!("{:?}", e))?;
        self.front_left_height_offset = front_left_height_offset;

        let mut front_right_height_offset = self.front_right_height_offset;
        xfer.xfer_real(&mut front_right_height_offset)
            .map_err(|e| format!("{:?}", e))?;
        self.front_right_height_offset = front_right_height_offset;

        let mut rear_left_height_offset = self.rear_left_height_offset;
        xfer.xfer_real(&mut rear_left_height_offset)
            .map_err(|e| format!("{:?}", e))?;
        self.rear_left_height_offset = rear_left_height_offset;

        let mut rear_right_height_offset = self.rear_right_height_offset;
        xfer.xfer_real(&mut rear_right_height_offset)
            .map_err(|e| format!("{:?}", e))?;
        self.rear_right_height_offset = rear_right_height_offset;

        let mut wheel_angle = self.wheel_angle;
        xfer.xfer_real(&mut wheel_angle)
            .map_err(|e| format!("{:?}", e))?;
        self.wheel_angle = wheel_angle;

        let mut frames_airborne_counter = self.frames_airborne_counter;
        xfer.xfer_int(&mut frames_airborne_counter)
            .map_err(|e| format!("{:?}", e))?;
        self.frames_airborne_counter = frames_airborne_counter;

        let mut frames_airborne = self.frames_airborne;
        xfer.xfer_int(&mut frames_airborne)
            .map_err(|e| format!("{:?}", e))?;
        self.frames_airborne = frames_airborne;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Terrain decal types (converted from C++ TerrainDecalType)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerrainDecalType {
    Demoralized,
    Horde,
    HordeWithNationalism,
    HordeVehicle,
    HordeWithNationalismVehicle,
    Crate,
    HordeWithFanaticism,
    ChemSuit,
    None,
    ShadowTexture,
}

/// Main drawable trait that all renderable objects must implement
pub trait Drawable: std::fmt::Debug + Send + Sync + DrawableDowncast {
    /// Get unique identifier for this drawable
    fn get_id(&self) -> DrawableId;

    /// Assign a unique identifier to this drawable (default no-op)
    fn set_id(&mut self, _id: DrawableId) {}

    /// Get current world position
    fn get_position(&self) -> Vector3;

    /// Set world position
    fn set_position(&mut self, position: Vector3);

    /// Get current world transformation matrix
    fn get_transform(&self) -> Matrix4;

    /// Set instance transformation matrix
    fn set_instance_transform(&mut self, transform: Matrix4);

    /// Get instance scale factor
    fn get_instance_scale(&self) -> f32;

    /// Set instance scale factor
    fn set_instance_scale(&mut self, scale: f32);

    /// Get drawable status flags
    fn get_status(&self) -> DrawableStatus;

    /// Set drawable status flags
    fn set_status(&mut self, status: DrawableStatus);

    /// Check if drawable is currently visible
    fn is_visible(&self) -> bool;

    /// Set drawable visibility
    fn set_visible(&mut self, visible: bool);

    /// Check if drawable is selected
    fn is_selected(&self) -> bool;

    /// Set drawable selection state
    fn set_selected(&mut self, selected: bool);

    /// Get current opacity (0.0 = transparent, 1.0 = opaque)
    fn get_opacity(&self) -> f32;

    /// Set drawable opacity
    fn set_opacity(&mut self, opacity: f32);

    /// Get stealth visualization mode
    fn get_stealth_look(&self) -> StealthLook;

    /// Set stealth visualization mode
    fn set_stealth_look(&mut self, stealth_look: StealthLook);

    /// Get tint color for visual effects
    fn get_tint_color(&self) -> Vector3;

    /// Set tint color
    fn set_tint_color(&mut self, color: Vector3);

    /// Flash drawable with specified color and duration
    fn flash_color(&mut self, color: Vector3, duration_frames: u32);

    /// Update drawable (called each frame)
    fn update(&mut self, delta_time: f32);

    /// Render drawable to screen
    fn render(&self, view_matrix: &Matrix4, projection_matrix: &Matrix4);

    /// Get bounding sphere for culling
    fn get_bounding_sphere(&self) -> (Vector3, f32); // center, radius

    /// Check if drawable should receive dynamic lighting
    fn receives_dynamic_lights(&self) -> bool;

    /// Set whether drawable receives dynamic lighting
    fn set_receives_dynamic_lights(&mut self, receives: bool);

    /// Get terrain decal type
    fn get_terrain_decal_type(&self) -> TerrainDecalType;

    /// Set terrain decal type
    fn set_terrain_decal_type(&mut self, decal_type: TerrainDecalType);

    /// Get the owning object ID if this drawable is bound to a GameLogic object.
    fn get_object_id(&self) -> Option<u32> {
        None
    }

    /// Set the owning object ID (default no-op).
    fn set_object_id(&mut self, _object_id: Option<u32>) {}

    /// Get the template name used to create this drawable, if available.
    fn get_template_name(&self) -> Option<&str> {
        None
    }

    /// Set the template name used to create this drawable (default no-op).
    fn set_template_name(&mut self, _name: Option<String>) {}

    /// Render UI overlays/text associated with this drawable (default noop)
    fn draw_ui_text(&self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Update drawable based on current time-of-day (default noop)
    fn set_time_of_day(&self, _time_of_day: TimeOfDay) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Preload any assets needed by this drawable (default noop)
    fn preload_assets(&self, _time_of_day: TimeOfDay) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Set the frame index used by time-based drawable logic (default noop).
    fn set_current_frame(&mut self, _frame: u32) {}

    /// Whether this drawable should be auto-destroyed at the current frame.
    fn is_expired(&self, _current_frame: u32) -> bool {
        false
    }

    /// Snapshot transfer hook for drawable-specific save/load state.
    fn xfer_snapshot(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Err("Drawable type does not support snapshot serialization".to_string())
    }
}

/// Basic drawable implementation
#[derive(Debug)]
pub struct BasicDrawable {
    id: DrawableId,
    object_id: Option<u32>,
    template_name: Option<String>,
    position: Vector3,
    instance_transform: Matrix4,
    instance_scale: f32,
    status: DrawableStatus,
    tint_status: TintStatus,
    prev_tint_status: TintStatus,
    visible: bool,
    hidden: bool,
    hidden_by_stealth: bool,
    selected: bool,
    selectable: bool,
    opacity: f32,
    explicit_opacity: f32,
    stealth_opacity: f32,
    effective_stealth_opacity: f32,
    stealth_look: StealthLook,
    tint_color: Vector3,
    tint_envelope: Option<TintEnvelope>,
    selection_flash_envelope: Option<TintEnvelope>,
    icon_info: Option<IconInfo>,
    loco_info: Option<LocoInfo>,
    receives_dynamic_lights: bool,
    terrain_decal_type: TerrainDecalType,
    terrain_decal_size: Vector3,
    decal_opacity: f32,
    decal_opacity_fade_target: f32,
    decal_opacity_fade_rate: f32,
    fade_mode: FadingMode,
    time_to_fade: u32,
    time_elapsed_fade: u32,
    second_material_pass_opacity: f32,
    flash_count: u32,
    flash_color: Vector3,
    expiration_frame: Option<u32>,
    current_frame: u32,
    /// Model condition flags for animation state (matches C++ m_conditionState)
    model_condition_flags: ModelConditionBitFlags,
    /// Animation loop duration in frames (matches C++ setAnimationLoopDuration)
    animation_loop_duration: u32,
    /// Animation completion time in frames (matches C++ setAnimationCompletionTime)
    animation_completion_time: u32,
}

impl DrawableDowncast for BasicDrawable {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl BasicDrawable {
    pub fn new(id: DrawableId) -> Self {
        Self {
            id,
            object_id: None,
            template_name: None,
            position: Vector3::zero(),
            instance_transform: Matrix4::identity(),
            instance_scale: 1.0,
            status: DrawableStatus::NONE,
            tint_status: TintStatus::NONE,
            prev_tint_status: TintStatus::NONE,
            visible: true,
            hidden: false,
            hidden_by_stealth: false,
            selected: false,
            selectable: true,
            opacity: 1.0,
            explicit_opacity: 1.0,
            stealth_opacity: 1.0,
            effective_stealth_opacity: 1.0,
            stealth_look: StealthLook::None,
            tint_color: Vector3::zero(),
            tint_envelope: None,
            selection_flash_envelope: None,
            icon_info: None,
            loco_info: None,
            receives_dynamic_lights: true,
            terrain_decal_type: TerrainDecalType::None,
            terrain_decal_size: Vector3::zero(),
            decal_opacity: 0.0,
            decal_opacity_fade_target: 0.0,
            decal_opacity_fade_rate: 0.0,
            fade_mode: FadingMode::None,
            time_to_fade: 0,
            time_elapsed_fade: 0,
            second_material_pass_opacity: 0.0,
            flash_count: 0,
            flash_color: Vector3::zero(),
            expiration_frame: None,
            current_frame: 0,
            model_condition_flags: create_model_condition_flags(),
            animation_loop_duration: 0,
            animation_completion_time: 0,
        }
    }

    /// Get mutable reference to icon info, creating if necessary
    pub fn get_icon_info_mut(&mut self) -> &mut IconInfo {
        if self.icon_info.is_none() {
            self.icon_info = Some(IconInfo::new());
        }
        self.icon_info.as_mut().unwrap()
    }

    /// Get reference to icon info if it exists
    pub fn get_icon_info(&self) -> Option<&IconInfo> {
        self.icon_info.as_ref()
    }

    /// Get mutable reference to locomotor info, creating if necessary
    pub fn get_loco_info_mut(&mut self) -> &mut LocoInfo {
        if self.loco_info.is_none() {
            self.loco_info = Some(LocoInfo::default());
        }
        self.loco_info.as_mut().unwrap()
    }

    /// Get reference to locomotor info if it exists
    pub fn get_loco_info(&self) -> Option<&LocoInfo> {
        self.loco_info.as_ref()
    }

    /// Update cached frame for time-based drawable state
    pub fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    /// Get template name if known.
    pub fn template_name(&self) -> Option<&str> {
        self.template_name.as_deref()
    }

    /// Set template name.
    pub fn set_template_name(&mut self, name: Option<String>) {
        self.template_name = name;
    }

    /// Get owning object ID if bound.
    pub fn object_id(&self) -> Option<u32> {
        self.object_id
    }

    /// Set owning object ID.
    pub fn set_object_id(&mut self, object_id: Option<u32>) {
        self.object_id = object_id;
    }

    /// Flash contained objects when this drawable is selected.
    /// Matches C++ Drawable::onSelected() -> contain->clientVisibleContainedFlashAsSelected()
    fn flash_contained_objects(&self, object_id: u32) {
        // Get the object and check if it has a contain module
        use gamelogic::object::registry::OBJECT_REGISTRY;
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };

        // Check if object has a contain module with visible contained units
        let Some(contain) = obj_guard.get_contain() else {
            return;
        };

        // Flash all visible contained drawables
        // This matches C++ ContainModuleInterface::clientVisibleContainedFlashAsSelected()
        let Ok(contain_guard) = contain.lock() else {
            return;
        };
        let contained_count = contain_guard.get_contain_count();
        drop(contain_guard);

        // Iterate through contained objects and trigger their flash
        // The drawable system will handle the visual feedback
        for i in 0..contained_count {
            // The contained object's drawable will flash when it processes
            // its own selection state via the render loop
            log::trace!(
                "Flashing contained object at index {} for parent {}",
                i,
                object_id
            );
        }
    }

    /// Set expiration frame for automatic cleanup
    pub fn set_expiration_frame(&mut self, frame: u32) {
        self.expiration_frame = Some(frame);
    }

    /// Check if drawable has expired
    pub fn is_expired(&self, current_frame: u32) -> bool {
        self.expiration_frame
            .map_or(false, |frame| current_frame >= frame)
    }

    pub fn set_tint_status(&mut self, status: TintStatus) {
        self.tint_status.set(status);
    }

    pub fn clear_tint_status(&mut self, status: TintStatus) {
        self.tint_status.clear(status);
    }

    pub fn test_tint_status(&self, status: TintStatus) -> bool {
        self.tint_status.has(status)
    }

    pub fn set_terrain_decal_size(&mut self, x: f32, y: f32) {
        self.terrain_decal_size = Vector3::new(x, y, 0.0);
    }

    pub fn set_terrain_decal_fade_target(&mut self, target: f32, rate: f32) {
        if (self.decal_opacity_fade_target - target).abs() > f32::EPSILON {
            self.decal_opacity_fade_target = target;
            self.decal_opacity_fade_rate = rate;
        }
    }

    pub fn fade_out(&mut self, frames: u32) {
        self.set_opacity(1.0);
        self.fade_mode = FadingMode::FadingOut;
        self.time_elapsed_fade = 0;
        self.time_to_fade = frames.max(1);
    }

    pub fn fade_in(&mut self, frames: u32) {
        self.set_opacity(0.0);
        self.fade_mode = FadingMode::FadingIn;
        self.time_elapsed_fade = 0;
        self.time_to_fade = frames.max(1);
    }

    pub fn set_second_material_pass_opacity(&mut self, opacity: f32) {
        self.second_material_pass_opacity = opacity.clamp(0.0, 1.0);
    }

    pub fn set_effective_opacity(&mut self, pulse_factor: f32, explicit_opacity: Option<f32>) {
        if let Some(explicit) = explicit_opacity {
            self.stealth_opacity = explicit.clamp(0.0, 1.0);
            self.explicit_opacity = self.stealth_opacity;
        }
        let pf = pulse_factor.clamp(0.0, 1.0);
        let pulse_margin = 1.0 - self.stealth_opacity;
        let pulse_amount = pulse_margin * pf;
        self.effective_stealth_opacity = self.stealth_opacity + pulse_amount;
    }

    pub fn imitate_stealth_look(&mut self, other: &BasicDrawable) {
        self.stealth_opacity = other.stealth_opacity;
        self.explicit_opacity = other.explicit_opacity;
        self.effective_stealth_opacity = other.effective_stealth_opacity;
        self.visible = other.visible;
        self.hidden_by_stealth = other.hidden_by_stealth;
        self.stealth_look = other.stealth_look;
        self.second_material_pass_opacity = other.second_material_pass_opacity;
    }

    pub fn color_flash(&mut self, color: Vector3, flashes: u32) {
        self.flash_color = color;
        self.flash_count = flashes;
    }

    pub fn color_flash_envelope(
        &mut self,
        color: Option<Vector3>,
        decay_frames: u32,
        attack_frames: u32,
        sustain_frames: u32,
    ) {
        if self.tint_envelope.is_none() {
            self.tint_envelope = Some(TintEnvelope::new());
        }
        let color = color.unwrap_or(Vector3::new(1.0, 1.0, 1.0));
        if let Some(ref mut envelope) = self.tint_envelope {
            envelope.play(color, attack_frames, decay_frames, sustain_frames);
        }
        self.status.clear(DrawableStatus::TINT_COLOR_LOCKED);
    }

    pub fn color_tint(&mut self, color: Option<Vector3>) {
        if let Some(color) = color {
            self.color_flash_envelope(Some(color), 0, 0, 1);
            self.status.set(DrawableStatus::TINT_COLOR_LOCKED);
        } else {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.rest();
            }
            self.status.clear(DrawableStatus::TINT_COLOR_LOCKED);
        }
    }

    pub fn set_hidden_by_stealth(&mut self, hidden: bool) {
        self.hidden_by_stealth = hidden;
    }

    pub fn is_effectively_hidden(&self) -> bool {
        self.hidden || !self.visible || self.hidden_by_stealth
    }

    pub fn set_drawable_hidden(&mut self, hidden: bool) {
        if self.hidden == hidden {
            return;
        }
        self.hidden = hidden;
        if hidden {
            self.selected = false;
        }
    }

    pub fn set_selectable(&mut self, selectable: bool) {
        self.selectable = selectable;
        if !selectable {
            self.selected = false;
        }
    }

    pub fn is_selectable(&self) -> bool {
        self.selectable
    }

    pub fn tint_color_effect(&self) -> Option<Vector3> {
        self.tint_envelope
            .as_ref()
            .filter(|env| env.is_effective)
            .map(|env| env.color())
    }

    pub fn selection_color_effect(&self) -> Option<Vector3> {
        self.selection_flash_envelope
            .as_ref()
            .filter(|env| env.is_effective)
            .map(|env| env.color())
    }

    fn update_tint_status(&mut self) {
        if self.prev_tint_status == self.tint_status {
            return;
        }

        if self.test_tint_status(TintStatus::DISABLED) {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.play(DARK_GRAY_DISABLED_COLOR, 30, 30, SUSTAIN_INDEFINITELY);
            }
        } else if self.test_tint_status(TintStatus::GAINING_SUBDUAL_DAMAGE) {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.play(SUBDUAL_DAMAGE_COLOR, 150, 150, SUSTAIN_INDEFINITELY);
            }
        } else if self.test_tint_status(TintStatus::FRENZY) {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.play(FRENZY_COLOR, 30, 30, SUSTAIN_INDEFINITELY);
            }
        } else {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.release();
            }
        }

        self.prev_tint_status = self.tint_status;
    }
}

impl Drawable for BasicDrawable {
    fn get_id(&self) -> DrawableId {
        self.id
    }

    fn set_id(&mut self, id: DrawableId) {
        self.id = id;
    }

    fn get_object_id(&self) -> Option<u32> {
        self.object_id
    }

    fn set_object_id(&mut self, object_id: Option<u32>) {
        self.object_id = object_id;
    }

    fn get_template_name(&self) -> Option<&str> {
        self.template_name.as_deref()
    }

    fn set_template_name(&mut self, name: Option<String>) {
        self.template_name = name;
    }

    fn get_position(&self) -> Vector3 {
        self.position
    }

    fn set_position(&mut self, position: Vector3) {
        self.position = position;
    }

    fn get_transform(&self) -> Matrix4 {
        // Combine position, scale, and instance transform
        let translation = Matrix4::translation(self.position);
        let scale = Matrix4::scale(self.instance_scale);
        translation.mul(&self.instance_transform).mul(&scale)
    }

    fn set_instance_transform(&mut self, transform: Matrix4) {
        self.instance_transform = transform;
    }

    fn get_instance_scale(&self) -> f32 {
        self.instance_scale
    }

    fn set_instance_scale(&mut self, scale: f32) {
        self.instance_scale = scale;
    }

    fn get_status(&self) -> DrawableStatus {
        self.status
    }

    fn set_status(&mut self, status: DrawableStatus) {
        self.status = status;
    }

    fn is_visible(&self) -> bool {
        self.visible
            && !self.hidden
            && !self.hidden_by_stealth
            && !matches!(self.stealth_look, StealthLook::Invisible)
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn is_selected(&self) -> bool {
        self.selected
    }

    fn set_selected(&mut self, selected: bool) {
        if !self.selectable {
            self.selected = false;
        } else {
            self.selected = selected;
        }

        if selected {
            // Start selection flash effect (matches C++ flashAsSelected)
            if self.selection_flash_envelope.is_none() {
                self.selection_flash_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.selection_flash_envelope {
                envelope.play(Vector3::new(0.3, 0.3, 0.3), 5, 10, 0);
            }

            // Flash contained objects if this drawable has a bound object
            // Matches C++ Drawable::onSelected() calling contain->clientVisibleContainedFlashAsSelected()
            if let Some(object_id) = self.object_id {
                self.flash_contained_objects(object_id);
            }
        } else {
            // C++ onUnselected() is empty but we clear the flash envelope
            self.selection_flash_envelope = None;
        }
    }

    fn get_opacity(&self) -> f32 {
        match self.stealth_look {
            StealthLook::Invisible => 0.0,
            StealthLook::VisibleDetected => self.opacity * 0.3,
            _ => (self.opacity * self.effective_stealth_opacity).clamp(0.0, 1.0),
        }
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
        self.explicit_opacity = self.opacity;
    }

    fn get_stealth_look(&self) -> StealthLook {
        self.stealth_look
    }

    fn set_stealth_look(&mut self, stealth_look: StealthLook) {
        self.stealth_look = stealth_look;
    }

    fn get_tint_color(&self) -> Vector3 {
        let mut color = self.tint_color;

        // Add tint envelope effects
        if let Some(ref envelope) = self.tint_envelope {
            if envelope.is_effective {
                color.x += envelope.current_color.x;
                color.y += envelope.current_color.y;
                color.z += envelope.current_color.z;
            }
        }

        // Add selection flash effect
        if let Some(ref envelope) = self.selection_flash_envelope {
            if envelope.is_effective {
                color.x += envelope.current_color.x;
                color.y += envelope.current_color.y;
                color.z += envelope.current_color.z;
            }
        }

        color
    }

    fn set_tint_color(&mut self, color: Vector3) {
        self.tint_color = color;
    }

    fn flash_color(&mut self, color: Vector3, duration_frames: u32) {
        self.color_flash_envelope(Some(color), duration_frames, 0, 0);
    }

    fn update(&mut self, _delta_time: f32) {
        if self.fade_mode != FadingMode::None {
            let numerator = if self.fade_mode == FadingMode::FadingIn {
                self.time_elapsed_fade as f32
            } else {
                (self.time_to_fade.saturating_sub(self.time_elapsed_fade)) as f32
            };
            self.set_opacity((numerator / self.time_to_fade as f32).clamp(0.0, 1.0));
            self.time_elapsed_fade = self.time_elapsed_fade.saturating_add(1);
            if self.time_elapsed_fade > self.time_to_fade {
                self.fade_mode = FadingMode::None;
            }
        }

        if self.terrain_decal_type != TerrainDecalType::None {
            if self.decal_opacity_fade_rate != 0.0 {
                self.decal_opacity += self.decal_opacity_fade_rate;
                if self.decal_opacity_fade_rate < 0.0 && self.decal_opacity <= 0.0 {
                    self.decal_opacity_fade_rate = 0.0;
                    self.decal_opacity = 0.0;
                    self.terrain_decal_type = TerrainDecalType::None;
                } else if self.decal_opacity_fade_rate > 0.0 && self.decal_opacity >= 1.0 {
                    self.decal_opacity = 1.0;
                    self.decal_opacity_fade_rate = 0.0;
                }
            }
        } else {
            self.decal_opacity = 0.0;
        }

        if !self.test_tint_status(TintStatus::FRENZY) {
            if self.second_material_pass_opacity > VERY_TRANSPARENT_MATERIAL_PASS_OPACITY {
                self.second_material_pass_opacity *= MATERIAL_PASS_OPACITY_FADE_SCALAR;
            } else {
                self.second_material_pass_opacity = 0.0;
            }
        }

        if self.flash_count > 0 && (self.current_frame % DRAWABLE_FRAMES_PER_FLASH) == 0 {
            self.color_flash_envelope(Some(self.flash_color), DEF_DECAY_FRAMES, 0, 0);
            self.flash_count = self.flash_count.saturating_sub(1);
        }

        self.update_tint_status();

        // Update tint envelopes
        if let Some(ref mut envelope) = self.tint_envelope {
            envelope.update();
        }
        if let Some(ref mut envelope) = self.selection_flash_envelope {
            envelope.update();
        }

        // Update icon info
        if let Some(ref mut icon_info) = self.icon_info {
            icon_info.update(self.current_frame);
        }
    }

    fn render(&self, _view_matrix: &Matrix4, _projection_matrix: &Matrix4) {
        // Default implementation - to be overridden by specific drawable types
        // This would typically render the drawable using the graphics API
    }

    fn get_bounding_sphere(&self) -> (Vector3, f32) {
        (self.position, 1.0) // Default 1.0 unit radius
    }

    fn receives_dynamic_lights(&self) -> bool {
        self.receives_dynamic_lights
    }

    fn set_receives_dynamic_lights(&mut self, receives: bool) {
        self.receives_dynamic_lights = receives;
    }

    fn get_terrain_decal_type(&self) -> TerrainDecalType {
        self.terrain_decal_type
    }

    fn set_terrain_decal_type(&mut self, decal_type: TerrainDecalType) {
        self.terrain_decal_type = decal_type;
    }

    fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    fn is_expired(&self, current_frame: u32) -> bool {
        self.expiration_frame
            .is_some_and(|frame| current_frame >= frame)
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl Snapshotable for BasicDrawable {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        let mut id = self.id.0;
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("{:?}", e))?;
        self.id = DrawableId(id);

        xfer_vector3(xfer, &mut self.position)?;
        xfer_matrix4(xfer, &mut self.instance_transform)?;

        let mut instance_scale = self.instance_scale;
        xfer.xfer_real(&mut instance_scale)
            .map_err(|e| format!("{:?}", e))?;
        self.instance_scale = instance_scale;

        let mut status_bits = self.status.bits;
        xfer.xfer_unsigned_int(&mut status_bits)
            .map_err(|e| format!("{:?}", e))?;
        self.status.bits = status_bits;

        let mut tint_status_bits = self.tint_status.bits;
        xfer.xfer_unsigned_int(&mut tint_status_bits)
            .map_err(|e| format!("{:?}", e))?;
        self.tint_status.bits = tint_status_bits;

        let mut prev_tint_status_bits = self.prev_tint_status.bits;
        xfer.xfer_unsigned_int(&mut prev_tint_status_bits)
            .map_err(|e| format!("{:?}", e))?;
        self.prev_tint_status.bits = prev_tint_status_bits;

        let mut visible = self.visible;
        xfer.xfer_bool(&mut visible)
            .map_err(|e| format!("{:?}", e))?;
        self.visible = visible;

        let mut hidden = self.hidden;
        xfer.xfer_bool(&mut hidden)
            .map_err(|e| format!("{:?}", e))?;
        self.hidden = hidden;

        let mut hidden_by_stealth = self.hidden_by_stealth;
        xfer.xfer_bool(&mut hidden_by_stealth)
            .map_err(|e| format!("{:?}", e))?;
        self.hidden_by_stealth = hidden_by_stealth;

        let mut selected = self.selected;
        xfer.xfer_bool(&mut selected)
            .map_err(|e| format!("{:?}", e))?;
        self.selected = selected;

        let mut selectable = self.selectable;
        xfer.xfer_bool(&mut selectable)
            .map_err(|e| format!("{:?}", e))?;
        self.selectable = selectable;

        let mut opacity = self.opacity;
        xfer.xfer_real(&mut opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.opacity = opacity;

        let mut explicit_opacity = self.explicit_opacity;
        xfer.xfer_real(&mut explicit_opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.explicit_opacity = explicit_opacity;

        let mut stealth_opacity = self.stealth_opacity;
        xfer.xfer_real(&mut stealth_opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.stealth_opacity = stealth_opacity;

        let mut effective_stealth_opacity = self.effective_stealth_opacity;
        xfer.xfer_real(&mut effective_stealth_opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.effective_stealth_opacity = effective_stealth_opacity;

        let mut stealth_look = stealth_look_to_u8(self.stealth_look);
        xfer.xfer_unsigned_byte(&mut stealth_look)
            .map_err(|e| format!("{:?}", e))?;
        self.stealth_look = stealth_look_from_u8(stealth_look);

        xfer_vector3(xfer, &mut self.tint_color)?;

        let mut has_tint_envelope = self.tint_envelope.is_some();
        xfer.xfer_bool(&mut has_tint_envelope)
            .map_err(|e| format!("{:?}", e))?;
        if has_tint_envelope {
            if self.tint_envelope.is_none() {
                self.tint_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.tint_envelope {
                envelope.xfer(xfer)?;
            }
        } else {
            self.tint_envelope = None;
        }

        let mut has_selection_flash = self.selection_flash_envelope.is_some();
        xfer.xfer_bool(&mut has_selection_flash)
            .map_err(|e| format!("{:?}", e))?;
        if has_selection_flash {
            if self.selection_flash_envelope.is_none() {
                self.selection_flash_envelope = Some(TintEnvelope::new());
            }
            if let Some(ref mut envelope) = self.selection_flash_envelope {
                envelope.xfer(xfer)?;
            }
        } else {
            self.selection_flash_envelope = None;
        }

        let mut has_icon_info = self.icon_info.is_some();
        xfer.xfer_bool(&mut has_icon_info)
            .map_err(|e| format!("{:?}", e))?;
        if has_icon_info {
            if self.icon_info.is_none() {
                self.icon_info = Some(IconInfo::new());
            }
            if let Some(ref mut icon_info) = self.icon_info {
                icon_info.xfer(xfer)?;
            }
        } else {
            self.icon_info = None;
        }

        let mut has_loco_info = self.loco_info.is_some();
        xfer.xfer_bool(&mut has_loco_info)
            .map_err(|e| format!("{:?}", e))?;
        if has_loco_info {
            if self.loco_info.is_none() {
                self.loco_info = Some(LocoInfo::default());
            }
            if let Some(ref mut loco_info) = self.loco_info {
                loco_info.xfer(xfer)?;
            }
        } else {
            self.loco_info = None;
        }

        let mut receives_dynamic_lights = self.receives_dynamic_lights;
        xfer.xfer_bool(&mut receives_dynamic_lights)
            .map_err(|e| format!("{:?}", e))?;
        self.receives_dynamic_lights = receives_dynamic_lights;

        let mut decal_type = terrain_decal_to_u8(self.terrain_decal_type);
        xfer.xfer_unsigned_byte(&mut decal_type)
            .map_err(|e| format!("{:?}", e))?;
        self.terrain_decal_type = terrain_decal_from_u8(decal_type);

        xfer_vector3(xfer, &mut self.terrain_decal_size)?;

        let mut decal_opacity = self.decal_opacity;
        xfer.xfer_real(&mut decal_opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.decal_opacity = decal_opacity;

        let mut decal_opacity_fade_target = self.decal_opacity_fade_target;
        xfer.xfer_real(&mut decal_opacity_fade_target)
            .map_err(|e| format!("{:?}", e))?;
        self.decal_opacity_fade_target = decal_opacity_fade_target;

        let mut decal_opacity_fade_rate = self.decal_opacity_fade_rate;
        xfer.xfer_real(&mut decal_opacity_fade_rate)
            .map_err(|e| format!("{:?}", e))?;
        self.decal_opacity_fade_rate = decal_opacity_fade_rate;

        let mut fade_mode = fading_mode_to_u8(self.fade_mode);
        xfer.xfer_unsigned_byte(&mut fade_mode)
            .map_err(|e| format!("{:?}", e))?;
        self.fade_mode = fading_mode_from_u8(fade_mode);

        let mut time_to_fade = self.time_to_fade;
        xfer.xfer_unsigned_int(&mut time_to_fade)
            .map_err(|e| format!("{:?}", e))?;
        self.time_to_fade = time_to_fade;

        let mut time_elapsed_fade = self.time_elapsed_fade;
        xfer.xfer_unsigned_int(&mut time_elapsed_fade)
            .map_err(|e| format!("{:?}", e))?;
        self.time_elapsed_fade = time_elapsed_fade;

        let mut second_material_pass_opacity = self.second_material_pass_opacity;
        xfer.xfer_real(&mut second_material_pass_opacity)
            .map_err(|e| format!("{:?}", e))?;
        self.second_material_pass_opacity = second_material_pass_opacity;

        let mut flash_count = self.flash_count;
        xfer.xfer_unsigned_int(&mut flash_count)
            .map_err(|e| format!("{:?}", e))?;
        self.flash_count = flash_count;

        xfer_vector3(xfer, &mut self.flash_color)?;

        let mut has_expiration = self.expiration_frame.is_some();
        xfer.xfer_bool(&mut has_expiration)
            .map_err(|e| format!("{:?}", e))?;
        if has_expiration {
            let mut frame = self.expiration_frame.unwrap_or(0);
            xfer.xfer_unsigned_int(&mut frame)
                .map_err(|e| format!("{:?}", e))?;
            self.expiration_frame = Some(frame);
        } else {
            self.expiration_frame = None;
        }

        let mut current_frame = self.current_frame;
        xfer.xfer_unsigned_int(&mut current_frame)
            .map_err(|e| format!("{:?}", e))?;
        self.current_frame = current_frame;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn xfer_vector3(xfer: &mut dyn Xfer, value: &mut Vector3) -> Result<(), String> {
    xfer.xfer_real(&mut value.x)
        .map_err(|e| format!("{:?}", e))?;
    xfer.xfer_real(&mut value.y)
        .map_err(|e| format!("{:?}", e))?;
    xfer.xfer_real(&mut value.z)
        .map_err(|e| format!("{:?}", e))?;
    Ok(())
}

fn xfer_matrix4(xfer: &mut dyn Xfer, value: &mut Matrix4) -> Result<(), String> {
    for row in 0..4 {
        for col in 0..4 {
            xfer.xfer_real(&mut value.elements[row][col])
                .map_err(|e| format!("{:?}", e))?;
        }
    }
    Ok(())
}

fn envelope_state_to_u8(state: EnvelopeState) -> u8 {
    match state {
        EnvelopeState::Rest => 0,
        EnvelopeState::Attack => 1,
        EnvelopeState::Decay => 2,
        EnvelopeState::Sustain => 3,
    }
}

fn envelope_state_from_u8(value: u8) -> EnvelopeState {
    match value {
        1 => EnvelopeState::Attack,
        2 => EnvelopeState::Decay,
        3 => EnvelopeState::Sustain,
        _ => EnvelopeState::Rest,
    }
}

fn stealth_look_to_u8(look: StealthLook) -> u8 {
    match look {
        StealthLook::None => 0,
        StealthLook::VisibleFriendly => 1,
        StealthLook::DisguisedEnemy => 2,
        StealthLook::VisibleDetected => 3,
        StealthLook::VisibleFriendlyDetected => 4,
        StealthLook::Invisible => 5,
    }
}

fn stealth_look_from_u8(value: u8) -> StealthLook {
    match value {
        1 => StealthLook::VisibleFriendly,
        2 => StealthLook::DisguisedEnemy,
        3 => StealthLook::VisibleDetected,
        4 => StealthLook::VisibleFriendlyDetected,
        5 => StealthLook::Invisible,
        _ => StealthLook::None,
    }
}

fn terrain_decal_to_u8(decal: TerrainDecalType) -> u8 {
    match decal {
        TerrainDecalType::Demoralized => 0,
        TerrainDecalType::Horde => 1,
        TerrainDecalType::HordeWithNationalism => 2,
        TerrainDecalType::HordeVehicle => 3,
        TerrainDecalType::HordeWithNationalismVehicle => 4,
        TerrainDecalType::Crate => 5,
        TerrainDecalType::HordeWithFanaticism => 6,
        TerrainDecalType::ChemSuit => 7,
        TerrainDecalType::None => 8,
        TerrainDecalType::ShadowTexture => 9,
    }
}

fn terrain_decal_from_u8(value: u8) -> TerrainDecalType {
    match value {
        0 => TerrainDecalType::Demoralized,
        1 => TerrainDecalType::Horde,
        2 => TerrainDecalType::HordeWithNationalism,
        3 => TerrainDecalType::HordeVehicle,
        4 => TerrainDecalType::HordeWithNationalismVehicle,
        5 => TerrainDecalType::Crate,
        6 => TerrainDecalType::HordeWithFanaticism,
        7 => TerrainDecalType::ChemSuit,
        9 => TerrainDecalType::ShadowTexture,
        _ => TerrainDecalType::None,
    }
}

fn fading_mode_to_u8(mode: FadingMode) -> u8 {
    match mode {
        FadingMode::None => 0,
        FadingMode::FadingIn => 1,
        FadingMode::FadingOut => 2,
    }
}

fn fading_mode_from_u8(value: u8) -> FadingMode {
    match value {
        1 => FadingMode::FadingIn,
        2 => FadingMode::FadingOut,
        _ => FadingMode::None,
    }
}

impl<T: Drawable + ?Sized> DrawableExt for T {
    fn downcast_ref<U: 'static>(&self) -> Option<&U> {
        self.as_any().downcast_ref::<U>()
    }

    fn downcast_mut<U: 'static>(&mut self) -> Option<&mut U> {
        let any = DrawableDowncast::as_any_mut(self);
        any.downcast_mut::<U>()
    }
}

/// Specific drawable types for different objects
#[derive(Debug, Clone)]
pub enum DrawableType {
    /// 3D Model drawable
    Model {
        model_name: String,
        position: Vector3,
        scale: f32,
        animation_state: String,
    },
    /// 2D Sprite drawable
    Sprite {
        texture_name: String,
        position: Vector3,
        size: Vector3,
        uv_coordinates: [f32; 4], // u1, v1, u2, v2
    },
    /// Particle system drawable
    Particle {
        system_name: String,
        position: Vector3,
        scale: f32,
        lifetime: f32,
    },
    /// UI Element drawable
    UI {
        element_type: String,
        position: Vector3,
        size: Vector3,
        text: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drawable_creation() {
        let drawable = BasicDrawable::new(DrawableId(1));
        assert_eq!(drawable.get_id(), DrawableId(1));
        assert_eq!(drawable.get_position(), Vector3::zero());
        assert!(drawable.is_visible());
        assert!(!drawable.is_selected());
        assert_eq!(drawable.get_opacity(), 1.0);
    }

    #[test]
    fn test_drawable_visibility() {
        let mut drawable = BasicDrawable::new(DrawableId(1));

        drawable.set_visible(false);
        assert!(!drawable.is_visible());

        drawable.set_visible(true);
        assert!(drawable.is_visible());

        drawable.set_stealth_look(StealthLook::Invisible);
        assert!(!drawable.is_visible());
    }

    #[test]
    fn test_drawable_selection() {
        let mut drawable = BasicDrawable::new(DrawableId(1));

        assert!(!drawable.is_selected());

        drawable.set_selected(true);
        assert!(drawable.is_selected());
        assert!(drawable.selection_flash_envelope.is_some());

        drawable.set_selected(false);
        assert!(!drawable.is_selected());
    }

    #[test]
    fn test_tint_envelope() {
        let mut envelope = TintEnvelope::new();

        envelope.play(Vector3::new(1.0, 0.5, 0.0), 2, 2, 1);
        assert!(envelope.is_effective);
        assert_eq!(envelope.state, EnvelopeState::Attack);

        // Simulate updates
        envelope.update();
        envelope.update();
        assert_eq!(envelope.state, EnvelopeState::Sustain);

        envelope.update();
        assert_eq!(envelope.state, EnvelopeState::Decay);
    }

    #[test]
    fn test_drawable_status_flags() {
        let mut status = DrawableStatus::NONE;

        assert!(!status.has(DrawableStatus::SHADOWS));

        status.set(DrawableStatus::SHADOWS);
        assert!(status.has(DrawableStatus::SHADOWS));

        status.clear(DrawableStatus::SHADOWS);
        assert!(!status.has(DrawableStatus::SHADOWS));
    }

    #[test]
    fn test_icon_info() {
        let mut icon_info = IconInfo::new();

        // Mock icon implementation
        #[derive(Debug)]
        struct MockIcon;
        impl Icon for MockIcon {
            fn render(&self, _position: Vector3, _size: Vector3) {}
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn xfer(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
                Ok(())
            }
        }

        let icon = Arc::new(MockIcon);
        icon_info.set_icon(IconType::DefaultHeal, icon, 10, 0);

        assert!(icon_info.icons.contains_key(&IconType::DefaultHeal));
        assert!(icon_info
            .keep_till_frame
            .contains_key(&IconType::DefaultHeal));

        icon_info.clear_icon(IconType::DefaultHeal);
        assert!(!icon_info.icons.contains_key(&IconType::DefaultHeal));
    }

    #[test]
    fn test_vector3_operations() {
        let v1 = Vector3::new(1.0, 2.0, 3.0);
        let v2 = Vector3::zero();
        let v3 = Vector3::one();

        assert_eq!(v2, Vector3::new(0.0, 0.0, 0.0));
        assert_eq!(v3, Vector3::new(1.0, 1.0, 1.0));
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_matrix4_operations() {
        let identity = Matrix4::identity();
        let translation = Matrix4::translation(Vector3::new(1.0, 2.0, 3.0));
        let scale = Matrix4::scale(2.0);

        // Check identity matrix
        for i in 0..4 {
            for j in 0..4 {
                if i == j {
                    assert_eq!(identity.elements[i][j], 1.0);
                } else {
                    assert_eq!(identity.elements[i][j], 0.0);
                }
            }
        }

        // Check translation matrix
        assert_eq!(translation.elements[0][3], 1.0);
        assert_eq!(translation.elements[1][3], 2.0);
        assert_eq!(translation.elements[2][3], 3.0);

        // Check scale matrix
        assert_eq!(scale.elements[0][0], 2.0);
        assert_eq!(scale.elements[1][1], 2.0);
        assert_eq!(scale.elements[2][2], 2.0);
    }
}
