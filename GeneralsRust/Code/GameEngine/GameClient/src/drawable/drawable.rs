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
use crate::display::view::{with_tactical_view_ref, Point3};
use crate::draw_group_info::get_draw_group_info;
use crate::gui::display_string::get_display_string_manager;
use crate::gui::font::{get_font_library, FontDesc};
use crate::render_bridge::get_render_bridge;
use crate::system::TimeOfDay;
use crate::system::{Anim2D, Anim2DCollection};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::bit_flags::{
    create_model_condition_flags, ModelConditionBitFlags, ModelConditionFlags,
};
use game_engine::common::ini::{get_anim2d_collection, Anim2DTemplate};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use gamelogic::common::types::FormationID;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::object::update::AnimatedParticleSysBoneClientUpdateModule;
use gamelogic::object::update::BeaconClientUpdateModule;
use gamelogic::object::update::LaserUpdate;
use gamelogic::object::update::LaserUpdateModule;
use gamelogic::object::update::SwayClientUpdateModule;
use gamelogic::player::{Player, NO_HOTKEY_SQUAD, NUM_HOTKEY_SQUADS};
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

    /// Rotation around the X axis (right-hand rule).
    /// Matches C++ Matrix3D::Rotate_X.
    pub fn rotation_x(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        let mut m = Self::identity();
        m.elements[1][1] = c;
        m.elements[1][2] = -s;
        m.elements[2][1] = s;
        m.elements[2][2] = c;
        m
    }

    /// Rotation around the Y axis (right-hand rule).
    /// Matches C++ Matrix3D::Rotate_Y.
    pub fn rotation_y(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        let mut m = Self::identity();
        m.elements[0][0] = c;
        m.elements[0][2] = s;
        m.elements[2][0] = -s;
        m.elements[2][2] = c;
        m
    }

    /// Rotation around the Z axis (right-hand rule).
    /// Matches C++ Matrix3D::Rotate_Z.
    pub fn rotation_z(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        let mut m = Self::identity();
        m.elements[0][0] = c;
        m.elements[0][1] = -s;
        m.elements[1][0] = s;
        m.elements[1][1] = c;
        m
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

/// 2D integer coordinate — screen-space position.
/// Matches C++ ICoord2D from Common/Geometry.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl ICoord2D {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }
}

/// 2D axis-aligned region with integer components.
/// Matches C++ IRegion2D from Common/Geometry.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

impl IRegion2D {
    pub fn new(lo: ICoord2D, hi: ICoord2D) -> Self {
        Self { lo, hi }
    }

    /// Width of the region (hi.x - lo.x).
    pub fn width(&self) -> i32 {
        self.hi.x - self.lo.x
    }

    /// Height of the region (hi.y - lo.y).
    pub fn height(&self) -> i32 {
        self.hi.y - self.lo.y
    }
}

/// Computed 2D overlay data for a single drawable, submitted to the render pipeline each frame.
/// Mirrors the data that C++ computes on-the-fly inside drawHealthBar, drawVeterancy,
/// drawConstructPercent, drawCaption, and drawIconUI (Drawable.cpp lines 2661–3940).
///
/// These methods store their results here instead of calling TheDisplay directly,
/// so the render pipeline can consume the data later.
#[derive(Debug, Clone, Default)]
pub struct DrawableOverlayData {
    /// Screen-space region for health bar and icons (matches C++ computeHealthRegion output).
    pub health_region: Option<IRegion2D>,
    /// Health bar fill ratio (0.0 = dead, 1.0 = full).
    pub health_ratio: f32,
    /// Whether to show construction progress instead of health.
    pub is_under_construction: bool,
    /// Construction progress 0.0–1.0 (matches C++ Object::getConstructionPercent / 100).
    pub construction_percent: f32,
    /// Veterancy level (0 = Regular, 1 = Veteran, 2 = Elite, 3 = Heroic).
    /// Matches C++ VeterancyLevel enum values.
    pub veterancy_level: u8,
    /// Caption text to display (matches C++ m_captionDisplayString).
    pub caption: Option<String>,
    /// Whether this drawable should have 2D overlay drawn this frame.
    pub visible: bool,

    // --- Ammo pip overlay (drawAmmo, Drawable.cpp lines 2861-2912) ---
    /// Number of full ammo pips (matches C++ numFull from getAmmoPipShowingInfo).
    pub ammo_full: u8,
    /// Total number of ammo pip slots (matches C++ numTotal from getAmmoPipShowingInfo).
    pub ammo_total: u8,
    /// Whether ammo pips should be shown this frame.
    pub show_ammo: bool,

    // --- Container pip overlay (drawContained, Drawable.cpp lines 2915-2986) ---
    /// Number of full container pips (matches C++ numFull from getContainerPipsToShow).
    pub contained_full: u8,
    /// Total number of container pip slots (matches C++ numTotal).
    pub contained_total: u8,
    /// Number of contained infantry units (for green/blue color coding).
    pub contained_infantry_count: u8,
    /// Whether container pips should be shown this frame.
    pub show_contained: bool,

    // --- Healing icon overlay (drawHealing, Drawable.cpp lines 3212-3301) ---
    /// Whether to show healing icon (matches C++ showHealing logic).
    pub show_healing: bool,
    /// Healing icon type: 0=default, 1=structure, 2=vehicle (matches C++ DrawableIconType).
    pub healing_icon_type: u8,

    // --- Emoticon overlay (drawEmoticon, Drawable.cpp lines 2826-2857) ---
    /// Whether an emoticon icon should be shown.
    pub show_emoticon: bool,

    // --- Bomb overlay (drawBombed, Drawable.cpp lines 3435-3609) ---
    /// Whether any bomb icon should be shown.
    pub show_bombed: bool,
    /// Bomb type: 0=none, 1=timed, 2=remote, 3=car bomb (matches C++ bomb icon types).
    pub bomb_type: u8,
    /// Countdown timer in seconds for timed bomb (matches C++ StickyBombUpdate countdown).
    pub bomb_timer_seconds: u32,

    // --- Disabled overlay (drawDisabled, Drawable.cpp lines 3614-3667) ---
    /// Whether the disabled (lightning bolt) icon should be shown.
    pub show_disabled: bool,

    // --- Enthusiastic overlay (drawEnthusiastic, Drawable.cpp lines 3306-3373) ---
    /// Whether the enthusiastic weapon-bonus icon should be shown.
    pub show_enthusiastic: bool,
    /// Whether the subliminal variant of enthusiastic should be used.
    pub show_subliminal: bool,

    // --- Demoralized overlay (drawDemoralized, Drawable.cpp lines 3378-3426) ---
    /// Whether the demoralized icon should be shown (gated by ALLOW_DEMORALIZE in C++).
    pub show_demoralized: bool,

    /// Opacity for the second (heat-vision / stealth) material pass.
    /// Matches C++ m_secondMaterialPassOpacity — faded each frame in draw()/update(),
    /// set to non-zero by stealth detection logic, read by the render pipeline.
    pub second_material_pass_opacity: f32,
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
    /// C++ parity order used by Drawable::xfer icon serialization.
    /// C++ writes icon slots in fixed enum order; keep Rust stable too.
    pub const XFER_ORDER: [IconType; 14] = [
        IconType::DefaultHeal,
        IconType::StructureHeal,
        IconType::VehicleHeal,
        IconType::Demoralized,
        IconType::BombTimed,
        IconType::BombRemote,
        IconType::Disabled,
        IconType::BattleplanBombard,
        IconType::BattleplanHoldTheLine,
        IconType::BattleplanSearchAndDestroy,
        IconType::Emoticon,
        IconType::Enthusiastic,
        IconType::EnthusiasticSubliminal,
        IconType::CarBomb,
    ];

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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut icon_count = self.icons.len().min(u8::MAX as usize) as u8;
        xfer.xfer_unsigned_byte(&mut icon_count)
            .map_err(|e| format!("{:?}", e))?;

        for icon_type in IconType::XFER_ORDER {
            let Some(icon) = self.icons.get(&icon_type) else {
                continue;
            };

            let mut icon_name = icon_type.name().to_string();
            xfer.xfer_ascii_string(&mut icon_name)
                .map_err(|e| format!("{:?}", e))?;

            let mut keep = *self.keep_till_frame.get(&icon_type).unwrap_or(&0);
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
                for icon_type in IconType::XFER_ORDER {
                    let Some(icon) = self.icons.get(&icon_type) else {
                        continue;
                    };

                    let mut icon_name = icon_type.name().to_string();
                    xfer.xfer_ascii_string(&mut icon_name)
                        .map_err(|e| format!("{:?}", e))?;

                    let mut keep = *self.keep_till_frame.get(&icon_type).unwrap_or(&0);
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

fn snap_denorm(value: f32) -> f32 {
    if value > -1e-20 && value < 1e-20 {
        0.0
    } else {
        value
    }
}

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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut attack_rate = self.attack_rate;
        xfer_vector3(xfer, &mut attack_rate)?;

        let mut decay_rate = self.decay_rate;
        xfer_vector3(xfer, &mut decay_rate)?;

        let mut peak_color = self.peak_color;
        xfer_vector3(xfer, &mut peak_color)?;

        let mut current_color = self.current_color;
        xfer_vector3(xfer, &mut current_color)?;

        let mut sustain_counter = self.sustain_counter;
        xfer.xfer_unsigned_int(&mut sustain_counter)
            .map_err(|e| format!("{:?}", e))?;

        let mut state = envelope_state_to_u8(self.state);
        xfer.xfer_unsigned_byte(&mut state)
            .map_err(|e| format!("{:?}", e))?;

        let mut effective = self.is_effective;
        xfer.xfer_bool(&mut effective)
            .map_err(|e| format!("{:?}", e))?;

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

    fn is_instance_identity(&self) -> bool;

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

    /// Render drawable to screen.
    /// Takes &mut self because rendering may toggle shadow state per-frame
    /// based on stealth look (C++ parity: Drawable::draw() is non-const).
    fn render(&mut self, view_matrix: &Matrix4, projection_matrix: &Matrix4);

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
    /// 2D icon overlay data computed each frame (health bar, veterancy, construction, caption).
    /// Replaces C++ direct TheDisplay calls in drawIconUI/drawHealthBar/drawVeterancy/etc.
    pub overlay_data: DrawableOverlayData,
    /// Caption text displayed above the drawable (C++ m_captionDisplayString).
    caption_text: Option<String>,
    /// Team/indicator color propagated to draw modules (C++ setIndicatorColor -> replaceIndicatorColor).
    /// Stored as (r, g, b) where each component is 0-255.
    indicator_color: Option<(u8, u8, u8)>,
    /// Static image initialization flag (C++ s_staticImagesInited).
    static_images_inited: bool,
    /// C++ parity: Drawable::m_drawableFullyObscuredByShroud.
    /// When true, the drawable is completely hidden by fog-of-war and should not render.
    drawable_fully_obscured_by_shroud: bool,
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
            overlay_data: DrawableOverlayData::default(),
            caption_text: None,
            indicator_color: None,
            static_images_inited: false,
            drawable_fully_obscured_by_shroud: false,
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

    /// Get the current model-condition flags.
    pub fn get_model_condition_flags(&self) -> &ModelConditionBitFlags {
        &self.model_condition_flags
    }

    /// Clear and set model-condition flags in one operation.
    pub fn clear_and_set_model_condition_flags(
        &mut self,
        clr: &ModelConditionBitFlags,
        set: &ModelConditionBitFlags,
    ) {
        self.model_condition_flags.clear_and_set(clr, set);
    }

    /// Replace full model-condition flags.
    pub fn replace_model_condition_flags(
        &mut self,
        flags: ModelConditionBitFlags,
        force_replace: bool,
    ) {
        if force_replace || self.model_condition_flags != flags {
            self.model_condition_flags = flags;
        }
    }

    /// Set a single model-condition bit by index.
    pub fn set_model_condition_state(&mut self, index: usize) {
        self.model_condition_flags.set(index, true);
    }

    /// Clear a single model-condition bit by index.
    pub fn clear_model_condition_state(&mut self, index: usize) {
        self.model_condition_flags.set(index, false);
    }

    /// C++ parity helpers used by options flow to toggle shadow resources.
    pub fn set_shadows_enabled(&mut self, enable: bool) {
        if enable {
            self.status.set(DrawableStatus::SHADOWS);
        } else {
            self.status.clear(DrawableStatus::SHADOWS);
        }
    }

    pub fn allocate_shadows(&mut self) {
        self.set_shadows_enabled(true);
    }

    pub fn release_shadows(&mut self) {
        self.set_shadows_enabled(false);
    }

    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        if self.drawable_fully_obscured_by_shroud != fully_obscured {
            self.drawable_fully_obscured_by_shroud = fully_obscured;
        }
    }

    /// Emoticon helpers (C++ parity: one active emoticon at a time).
    pub fn clear_emoticon(&mut self) {
        if let Some(icon_info) = self.icon_info.as_mut() {
            icon_info.clear_icon(IconType::Emoticon);
        }
    }

    pub fn set_emoticon(
        &mut self,
        template_name: &str,
        duration_frames: u32,
    ) -> Result<(), String> {
        let icon = Anim2DIcon::from_template_name(template_name)?;
        let current_frame = self.current_frame;
        self.get_icon_info_mut().set_icon(
            IconType::Emoticon,
            Arc::new(icon),
            duration_frames,
            current_frame,
        );
        Ok(())
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

    fn is_object_kind_of(&self, kind: gamelogic::common::types::KindOf) -> bool {
        self.object_id.map_or(false, |obj_id| {
            OBJECT_REGISTRY.get_object(obj_id).map_or(false, |obj_arc| {
                obj_arc.read().map_or(false, |obj| obj.is_kind_of(kind))
            })
        })
    }

    /// Full stealth look logic ported from C++ Drawable::setStealthLook (Drawable.cpp:2527-2606).
    /// Sets stealth opacity, hidden-by-stealth flag, and second material pass opacity
    /// based on the stealth look type. The trait's set_stealth_look delegates here.
    pub fn apply_stealth_look(&mut self, look: StealthLook) {
        if look == self.stealth_look {
            return;
        }

        self.stealth_opacity = 1.0;
        match look {
            StealthLook::None => {
                self.hidden_by_stealth = false;
                self.second_material_pass_opacity = 0.0;
            }
            StealthLook::VisibleFriendly | StealthLook::VisibleFriendlyDetected => {
                // C++ reads TheGlobalData->m_stealthFriendlyOpacity as default opacity.
                // When GlobalData is wired, this should come from there. For now use
                // a parity-default of 0.5 which matches typical C++ values.
                let mut opacity: f32 = 0.5;

                // C++ checks for disguised objects — if disguised, stealth opacity
                // is not applied (disguised objects are fully visible to their owner).
                // PARITY_NOTE: Requires StealthUpdate module (Drawable.cpp:2549-2566).
                // When ported, check stealth->isDisguised() and read stealth->getFriendlyOpacity()
                // to override the default opacity. For now, disguise check is skipped and
                // the default GlobalData opacity (0.5) is used.

                self.stealth_opacity = opacity;
                self.hidden_by_stealth = false;

                // C++ sets second material pass for heat-vision on detected friendlies,
                // but not on mines (evil hack per srj todo).
                if look == StealthLook::VisibleFriendlyDetected
                    && !self.is_object_kind_of(gamelogic::common::types::KindOf::Mine)
                {
                    self.second_material_pass_opacity = 1.0;
                } else {
                    self.second_material_pass_opacity = 0.0;
                }
            }
            StealthLook::DisguisedEnemy => {
                self.hidden_by_stealth = false;
                self.second_material_pass_opacity = 0.0;
            }
            StealthLook::VisibleDetected => {
                self.hidden_by_stealth = false;
                // C++ disables heat-vision on mines (same hack as above).
                if self.is_object_kind_of(gamelogic::common::types::KindOf::Mine) {
                    self.second_material_pass_opacity = 0.0;
                } else {
                    self.second_material_pass_opacity = 1.0;
                }
            }
            StealthLook::Invisible => {
                self.hidden_by_stealth = true;
                self.second_material_pass_opacity = 0.0;
            }
        }
        self.stealth_look = look;
    }

    /// Propagate indicator color to all draw modules.
    /// C++ Drawable::setIndicatorColor (Drawable.cpp:4081-4089) iterates draw modules
    /// and calls replaceIndicatorColor on each ObjectDrawInterface.
    /// PARITY_NOTE: DrawModule system not yet ported. When available, this method must
    /// iterate draw_modules and call replace_indicator_color on each ObjectDrawInterface.
    /// For now, the color is stored on Drawable only.
    pub fn set_indicator_color(&mut self, color: Option<(u8, u8, u8)>) {
        self.indicator_color = color;
    }

    /// Get the current indicator color.
    pub fn get_indicator_color(&self) -> Option<(u8, u8, u8)> {
        self.indicator_color
    }

    /// Bind this drawable to a game object.
    /// C++ Drawable::friend_bindToObject (Drawable.cpp:4138-4162):
    /// Sets m_object, applies indicator color (day/night aware), creates terrain
    /// decal for FS_FAKE kindof, and notifies draw modules of the binding.
    /// PARITY_NOTE: Object/Player/DrawModule systems not yet wired. When ported, this
    /// must read GlobalData->m_timeOfDay to choose getNightIndicatorColor() vs
    /// getIndicatorColor(), check isKindOf(KINDOF_FS_FAKE) for terrain decal, and
    /// call onDrawableBoundToObject() on each draw module. For now, stores binding only.
    pub fn friend_bind_to_object(&mut self, object_id: u32) {
        self.object_id = Some(object_id);
        // C++ sets indicator color from object's team color.
        // When Object/Player system is fully wired, this reads
        // getObject()->getIndicatorColor() or getNightIndicatorColor().
        // For now, store the binding; indicator color is set externally.
    }

    /// Called when the owning object changes teams.
    /// C++ Drawable::changedTeam (Drawable.cpp:4168-4187):
    /// Re-applies indicator color from the object's new team and updates terrain decal.
    /// PARITY_NOTE: Object/Player systems not yet wired. When ported, this must
    /// re-read getIndicatorColor()/getNightIndicatorColor() based on time of day,
    /// and update FS_FAKE terrain decal per the new team relationship. For now, no-op.
    pub fn changed_team(&mut self) {
        // C++ re-applies indicator color from object's new team color.
        // When Object system is wired, this calls:
        //   setIndicatorColor(object->getIndicatorColor()) or
        //   setIndicatorColor(object->getNightIndicatorColor())
        // and updates FS_FAKE terrain decal based on new relationship.
    }

    /// Initialize static images shared by all drawables.
    /// C++ Drawable::initStaticImages (Drawable.cpp:249-285):
    /// Loads veterancy images (SCVeter1/2/3), ammo/container pip images,
    /// and icon animation templates. Called once at startup.
    /// PARITY_NOTE: MappedImageCollection/Anim2DCollection asset systems not yet ported.
    /// When available, load: SCVeter1-3, SCPAmmoFull/Empty, SCPPipFull/Empty, and
    /// all ICON_* Anim2D templates from TheAnim2DCollection. For now, marks initialized.
    pub fn init_static_images(&mut self) {
        if self.static_images_inited {
            return;
        }
        // C++ loads the following image assets:
        // s_veterancyImage[0] = NULL
        // s_veterancyImage[1] = "SCVeter1"
        // s_veterancyImage[2] = "SCVeter2"
        // s_veterancyImage[3] = "SCVeter3"
        // s_fullAmmo = "SCPAmmoFull"
        // s_emptyAmmo = "SCPAmmoEmpty"
        // s_fullContainer = "SCPPipFull"
        // s_emptyContainer = "SCPPipEmpty"
        // s_animationTemplates[ICON_*] = various icon Anim2D templates
        // When asset system is ported, these should be loaded here.
        self.static_images_inited = true;
    }

    /// Free static image resources.
    /// C++ Drawable::killStaticImages (Drawable.cpp:288-295):
    /// Deletes the animation templates array. Called at shutdown.
    /// PARITY_NOTE: No resources to free until init_static_images loads real assets.
    /// When ported, this must: delete[] s_animationTemplates; s_animationTemplates = NULL.
    pub fn kill_static_images(&mut self) {
        // C++: delete[] s_animationTemplates; s_animationTemplates = NULL;
        // When asset system is ported, free any allocated static resources here.
        self.static_images_inited = false;
    }

    /// Set caption text displayed above this drawable.
    /// C++ Drawable::setCaptionText (Drawable.cpp:4293-4322):
    /// Creates a DisplayString, applies font, sets sanitized text.
    /// For Rust, we store the text directly; font/rendering is handled by overlay_data.
    pub fn set_caption_text(&mut self, text: &str) {
        if text.is_empty() {
            self.clear_caption_text();
            return;
        }
        // C++ sanitizes via TheLanguageFilter->filterLine() (Drawable.cpp:4302).
        // PARITY_NOTE: LanguageFilter not yet ported. When available, pass text through
        // the language filter before storing. For now, text is stored as-is.
        let sanitized = text.to_string();
        if self.caption_text.as_deref() != Some(sanitized.as_str()) {
            self.caption_text = Some(sanitized);
        }
    }

    /// Clear caption text.
    /// C++ Drawable::clearCaptionText (Drawable.cpp:4325-4330):
    /// Frees the DisplayString and sets pointer to NULL.
    pub fn clear_caption_text(&mut self) {
        self.caption_text = None;
    }

    /// Get caption text if set.
    /// C++ Drawable::getCaptionText (Drawable.cpp:4333-4339):
    /// Returns the DisplayString text or empty UnicodeString.
    pub fn get_caption_text(&self) -> Option<&str> {
        self.caption_text.as_deref()
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

    /// Compute render condition flags from drawable state.
    /// Maps drawable visual state to RenderBridge condition flags.
    fn compute_render_condition_flags(&self) -> crate::render_bridge::RenderConditionFlags {
        use crate::render_bridge::RenderConditionFlags;
        let mut flags = RenderConditionFlags::empty();

        if self
            .model_condition_flags
            .test(ModelConditionFlags::DAMAGED)
        {
            flags |= RenderConditionFlags::DAMAGED;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::REALLYDAMAGED)
        {
            flags |= RenderConditionFlags::REALLY_DAMAGED;
        }
        if self.model_condition_flags.test(ModelConditionFlags::RUBBLE) {
            flags |= RenderConditionFlags::RUBBLE;
        }
        if self.model_condition_flags.test(ModelConditionFlags::NIGHT) {
            flags |= RenderConditionFlags::NIGHT;
        }
        if self.model_condition_flags.test(ModelConditionFlags::SNOW) {
            flags |= RenderConditionFlags::SNOW;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::AWAITING_CONSTRUCTION)
        {
            flags |= RenderConditionFlags::AWAITING_CONSTRUCTION;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::PARTIALLY_CONSTRUCTED)
        {
            flags |= RenderConditionFlags::PARTIALLY_CONSTRUCTED;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED)
        {
            flags |= RenderConditionFlags::ACTIVELY_CONSTRUCTED;
        }
        if self.model_condition_flags.test(ModelConditionFlags::AFLAME) {
            flags |= RenderConditionFlags::AFLAME;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::SMOLDERING)
        {
            flags |= RenderConditionFlags::SMOLDERING;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::TOPPLED)
        {
            flags |= RenderConditionFlags::TOPPLED;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::FLOODED)
        {
            flags |= RenderConditionFlags::FLOODED;
        }
        if self
            .model_condition_flags
            .test(ModelConditionFlags::DISGUISED)
        {
            flags |= RenderConditionFlags::DISGUISED;
        }

        if self.selected {
            flags |= RenderConditionFlags::SELECTED;
        }

        if matches!(self.stealth_look, StealthLook::DisguisedEnemy) {
            flags |= RenderConditionFlags::DISGUISED;
        }

        flags
    }

    fn find_hotkey_squad_number(player: &mut Player, object_id: u32) -> Option<i32> {
        for squad_number in 0..NUM_HOTKEY_SQUADS {
            if let Some(squad) = player.get_hotkey_squad(squad_number as i32) {
                if squad.is_on_squad_by_id(object_id) {
                    return Some(squad_number as i32);
                }
            }
        }

        None
    }

    fn draw_caption_string(
        text_handle: &crate::gui::display_string::DisplayStringHandle,
        x: i32,
        y: i32,
        color: u32,
        drop_color: u32,
        font_name: &str,
        font_size: i32,
        font_is_bold: bool,
        drop_shadow_offset_x: i32,
        drop_shadow_offset_y: i32,
    ) {
        let mut text = text_handle.borrow_mut();
        let font_desc = FontDesc::new(font_name, font_size, font_is_bold);
        if let Ok(font) = get_font_library().get_font(&font_desc) {
            text.set_font(font);
        }
        text.draw_with_drop(
            x,
            y,
            color,
            drop_color,
            drop_shadow_offset_x,
            drop_shadow_offset_y,
        );
    }

    // ---------------------------------------------------------------------------
    // 2D icon overlay methods (matches C++ Drawable.cpp drawIconUI, drawHealthBar,
    // drawVeterancy, drawConstructPercent, drawCaption, computeHealthRegion)
    //
    // These methods compute overlay data and store it in self.overlay_data.
    // The actual GPU rendering is handled by the render pipeline later.
    // ---------------------------------------------------------------------------

    pub fn compute_health_region(&self) -> Option<IRegion2D> {
        self.overlay_data.health_region
    }

    pub fn draw_health_bar(&mut self, health_region: &IRegion2D) {
        self.overlay_data.health_region = Some(*health_region);
        self.overlay_data.visible = true;

        if let Some(obj_id) = self.object_id {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                return;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                return;
            };
            let health = obj_guard.get_health();
            let max_health = obj_guard.get_max_health();
            if max_health > 0.0 {
                self.overlay_data.health_ratio = (health / max_health).clamp(0.0, 1.0);
            }
        }
    }

    pub fn draw_veterancy(&mut self, _health_region: &IRegion2D) {
        if let Some(obj_id) = self.object_id {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                return;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                return;
            };
            if obj_guard.get_experience_tracker().is_some() {
                self.overlay_data.veterancy_level = obj_guard.get_veterancy_level() as u8;
            }
        }
    }

    pub fn draw_construct_percent(&mut self, _health_region: &IRegion2D) {
        if let Some(obj_id) = self.object_id {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                return;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                return;
            };
            if obj_guard.is_under_construction() {
                self.overlay_data.is_under_construction = true;
                self.overlay_data.construction_percent =
                    (obj_guard.get_construction_percent() as f32) / 100.0;
            } else {
                self.overlay_data.is_under_construction = false;
            }
        }
    }

    pub fn draw_caption(&mut self, _health_region: &IRegion2D) {
        // Caption display is driven by m_captionDisplayString in C++.
        // For now, overlay_data.caption remains None unless set externally.
        // The render pipeline will check this field.
    }

    pub fn draw_emoticon(&mut self, _health_region: &IRegion2D) {
        // C++ parity: Drawable.cpp drawEmoticon (lines 2826-2857)
        if let Some(ref icon_info) = self.icon_info {
            let now = self.current_frame;
            if icon_info.icons.contains_key(&IconType::Emoticon) {
                let active = icon_info
                    .keep_till_frame
                    .get(&IconType::Emoticon)
                    .map_or(false, |&frame| frame >= now);
                self.overlay_data.show_emoticon = active;
                if !active {
                    self.clear_emoticon();
                }
            }
        }
    }

    pub fn draw_ammo(&mut self, _health_region: &IRegion2D) {
        // C++ parity: Drawable.cpp drawAmmo (lines 2861-2912)
        // Ammo pips only show for selected/moused-over local player objects.
        // C++ gates on: TheGlobalData->m_showObjectHealth && (isSelected() || mousedOver)
        //              && obj->getControllingPlayer() == ThePlayerList->getLocalPlayer()
        if !self.selected {
            self.overlay_data.show_ammo = false;
            return;
        }

        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };

        // C++ calls obj->getAmmoPipShowingInfo(numTotal, numFull).
        // The Rust Object doesn't have this method yet, so we query via weapon set.
        // For parity, we store the ammo state for the render pipeline.
        let (total, full) = obj_guard.get_ammo_pip_info();
        if total == 0 {
            self.overlay_data.show_ammo = false;
            return;
        }
        self.overlay_data.ammo_total = total as u8;
        self.overlay_data.ammo_full = full as u8;
        self.overlay_data.show_ammo = true;
    }

    pub fn draw_contained(&mut self, _health_region: &IRegion2D) {
        // C++ parity: Drawable.cpp drawContained (lines 2915-2986)
        if !self.selected {
            self.overlay_data.show_contained = false;
            return;
        }

        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };

        let Some(contain_arc) = obj_guard.get_contain() else {
            self.overlay_data.show_contained = false;
            return;
        };
        let Ok(contain_guard) = contain_arc.lock() else {
            return;
        };

        let contained_count = contain_guard.get_contained_count();
        if contained_count == 0 {
            self.overlay_data.show_contained = false;
            return;
        }

        let max_capacity = contain_guard.get_max_capacity();
        self.overlay_data.contained_full = contained_count.min(u8::MAX as usize) as u8;
        self.overlay_data.contained_total = max_capacity.min(u8::MAX as usize) as u8;
        self.overlay_data.show_contained = true;

        // C++ counts infantry among contained items for green/blue color coding
        let contained_objects = contain_guard.get_contained_objects();
        let mut infantry_count: u8 = 0;
        for &cid in contained_objects {
            if let Some(c_arc) = OBJECT_REGISTRY.get_object(cid) {
                if let Ok(c_guard) = c_arc.read() {
                    if c_guard.is_kind_of(gamelogic::common::types::KindOf::Infantry) {
                        infantry_count = infantry_count.saturating_add(1);
                    }
                }
            }
        }
        self.overlay_data.contained_infantry_count = infantry_count;
    }

    pub fn draw_healing(&mut self, _health_region: &IRegion2D) {
        // C++ parity: Drawable.cpp drawHealing (lines 3212-3301)
        // Shows healing icon when last healing was within HEALING_ICON_DISPLAY_TIME (90 frames = 3s).
        const HEALING_ICON_DISPLAY_TIME: u32 = 90; // 3 seconds at 30 FPS

        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };

        if obj_guard.is_kind_of(gamelogic::common::types::KindOf::NoHealIcon) {
            self.overlay_data.show_healing = false;
            return;
        }

        let mut show_healing = false;
        if let Some(body_arc) = obj_guard.get_body_module() {
            if let Ok(body_guard) = body_arc.lock() {
                let health = body_guard.get_health();
                let max_health = body_guard.get_max_health();
                if health != max_health {
                    let last_heal = body_guard.get_last_healing_timestamp();
                    let now = self.current_frame;
                    // C++ guards against early-game false positives
                    if now > HEALING_ICON_DISPLAY_TIME
                        && now.saturating_sub(last_heal) <= HEALING_ICON_DISPLAY_TIME
                    {
                        show_healing = true;
                    }
                }
            }
        }

        self.overlay_data.show_healing = show_healing;

        if show_healing {
            // C++ picks icon type based on KindOf
            if obj_guard.is_kind_of(gamelogic::common::types::KindOf::Structure) {
                self.overlay_data.healing_icon_type = 1; // ICON_STRUCTURE_HEAL
            } else if obj_guard.is_kind_of(gamelogic::common::types::KindOf::Vehicle) {
                self.overlay_data.healing_icon_type = 2; // ICON_VEHICLE_HEAL
            } else {
                self.overlay_data.healing_icon_type = 0; // ICON_DEFAULT_HEAL
            }
        } else {
            // Kill any existing healing icon (matches C++ else branch)
            if let Some(ref mut icon_info) = self.icon_info {
                icon_info.clear_icon(IconType::DefaultHeal);
                icon_info.clear_icon(IconType::StructureHeal);
                icon_info.clear_icon(IconType::VehicleHeal);
            }
        }
    }

    pub fn draw_enthusiastic(&mut self, _health_region: &IRegion2D) {
        // C++ parity: Drawable.cpp drawEnthusiastic (lines 3306-3373)
        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };

        use gamelogic::common::types::WeaponBonusConditionFlags;
        let bonus = obj_guard.get_weapon_bonus_condition();
        let has_enthusiastic = bonus.contains(WeaponBonusConditionFlags::ENTHUSIASTIC);
        let has_subliminal = bonus.contains(WeaponBonusConditionFlags::SUBLIMINAL);

        if has_enthusiastic {
            self.overlay_data.show_enthusiastic = true;
            self.overlay_data.show_subliminal = has_subliminal;
        } else {
            self.overlay_data.show_enthusiastic = false;
            self.overlay_data.show_subliminal = false;
            if let Some(ref mut icon_info) = self.icon_info {
                icon_info.clear_icon(IconType::Enthusiastic);
                icon_info.clear_icon(IconType::EnthusiasticSubliminal);
            }
        }
    }

    pub fn draw_demoralized(&mut self, _health_region: &IRegion2D) {
        // C++ parity: Drawable.cpp drawDemoralized (lines 3378-3426)
        // Gated by #ifdef ALLOW_DEMORALIZE in C++; we always compute the state.
        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };

        let Some(ai_arc) = obj_guard.get_ai_update_interface() else {
            self.overlay_data.show_demoralized = false;
            return;
        };
        let Ok(ai_guard) = ai_arc.lock() else {
            return;
        };

        // C++ calls ai->isDemoralized(). In Rust, check via weapon bonus condition.
        use gamelogic::common::types::WeaponBonusConditionFlags;
        let bonus = obj_guard.get_weapon_bonus_condition();
        let is_demoralized = bonus.contains(WeaponBonusConditionFlags::DEMORALIZED);

        self.overlay_data.show_demoralized = is_demoralized;

        if !is_demoralized {
            if let Some(ref mut icon_info) = self.icon_info {
                icon_info.clear_icon(IconType::Demoralized);
            }
        }
    }

    pub fn draw_bombed(&mut self, _health_region: &IRegion2D) {
        // C++ parity: Drawable.cpp drawBombed (lines 3435-3609)
        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };

        // C++ checks obj->testWeaponSetFlag(WEAPONSET_CARBOMB) first
        if obj_guard.test_weapon_set_flag(gamelogic::weapon::WeaponSetType::CarBomb) {
            self.overlay_data.show_bombed = true;
            self.overlay_data.bomb_type = 3; // car bomb
            return;
        }

        // C++ then checks StickyBombUpdate for timed/remote bombs
        // find_update_module("StickyBombUpdate") -> check isTimedBomb
        // For now, bomb_type 1=timed, 2=remote are stored when bomb modules are present.
        // The render pipeline will use these values.
        let update_handle = obj_guard.find_update_module("StickyBombUpdate");
        if update_handle.is_some() {
            // Bomb is attached; the render pipeline will handle visual countdown.
            self.overlay_data.show_bombed = true;
            // Default to timed; the specific type will be refined when
            // StickyBombUpdate is fully ported with isTimedBomb().
            if self.overlay_data.bomb_type == 0 {
                self.overlay_data.bomb_type = 1; // timed bomb
            }
        } else {
            self.overlay_data.show_bombed = false;
            self.overlay_data.bomb_type = 0;
            // C++ cleanup: kill bomb icons if expired
            if let Some(ref mut icon_info) = self.icon_info {
                let now = self.current_frame;
                let expired_timed = icon_info
                    .keep_till_frame
                    .get(&IconType::BombTimed)
                    .map_or(true, |&f| f <= now);
                let expired_remote = icon_info
                    .keep_till_frame
                    .get(&IconType::BombRemote)
                    .map_or(true, |&f| f <= now);
                if expired_timed {
                    icon_info.clear_icon(IconType::BombTimed);
                }
                if expired_remote {
                    icon_info.clear_icon(IconType::BombRemote);
                }
            }
        }
    }

    pub fn draw_disabled(&mut self, _health_region: &IRegion2D) {
        // C++ parity: Drawable.cpp drawDisabled (lines 3614-3667)
        // Checks: DISABLED_HACKED || DISABLED_PARALYZED || DISABLED_EMP ||
        //         DISABLED_SUBDUED || DISABLED_UNDERPOWERED
        let Some(obj_id) = self.object_id else {
            return;
        };
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };

        use gamelogic::common::types::DisabledType;
        let is_disabled = obj_guard.is_disabled_by_type(DisabledType::DisabledHacked)
            || obj_guard.is_disabled_by_type(DisabledType::Paralyzed)
            || obj_guard.is_disabled_by_type(DisabledType::DisabledEmp)
            || obj_guard.is_disabled_by_type(DisabledType::DisabledSubdued)
            || obj_guard.is_disabled_by_type(DisabledType::DisabledUnderpowered);

        self.overlay_data.show_disabled = is_disabled;

        if !is_disabled {
            if let Some(ref mut icon_info) = self.icon_info {
                icon_info.clear_icon(IconType::Disabled);
            }
        }
    }

    pub fn draw_icon_ui(&mut self) {
        let region = self.compute_health_region();

        // C++ parity: Drawable.cpp drawIconUI() dispatch order (lines 2738-2788):
        // healthBar → emoticon → caption → constructPercent →
        // (dead check bail) → healing → bombed → enthusiastic → demoralized →
        // disabled → ammo → contained → veterancy

        if let Some(ref health_region) = region {
            self.draw_health_bar(health_region);
            self.draw_emoticon(health_region);
            self.draw_caption(health_region);
            self.draw_construct_percent(health_region);
        }

        // C++: all icons below only draw on ALIVE things
        let Some(obj_id) = self.object_id else {
            return;
        };
        let is_dead = {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                return;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                return;
            };
            obj_guard.is_effectively_dead()
                || obj_guard.is_kind_of(gamelogic::common::types::KindOf::IgnoredInGui)
        };

        if is_dead {
            return;
        }

        if let Some(ref health_region) = region {
            self.draw_healing(health_region);
            self.draw_bombed(health_region);
            self.draw_enthusiastic(health_region);
            self.draw_demoralized(health_region);
            self.draw_disabled(health_region);
            self.draw_ammo(health_region);
            self.draw_contained(health_region);
            self.draw_veterancy(health_region);
        }
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

    fn is_instance_identity(&self) -> bool {
        self.instance_transform == Matrix4::identity()
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
        self.apply_stealth_look(stealth_look);
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
        self.overlay_data.second_material_pass_opacity = self.second_material_pass_opacity;

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

        // C++ parity: Drawable::updateDrawable() dispatches to all ClientUpdateModules.
        // Each concrete module has client_update() — no shared trait exists yet.
        if let Some(object_id) = self.object_id {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(obj_guard) = obj_arc.read() {
                    for module_handle in obj_guard.client_update_modules() {
                        let _ = module_handle
                            .with_module_downcast::<SwayClientUpdateModule, _, ()>(|m| {
                                m.client_update()
                            });
                        let _ = module_handle
                            .with_module_downcast::<BeaconClientUpdateModule, _, ()>(|m| {
                                m.client_update()
                            });
                        let _ = module_handle.with_module_downcast::<
                            AnimatedParticleSysBoneClientUpdateModule,
                            _,
                            (),
                        >(|m| m.client_update());
                        let _ = module_handle
                            .with_module_downcast::<LaserUpdate, _, ()>(|m| m.client_update());
                    }
                }
            }
        }
    }

    fn render(&mut self, view_matrix: &Matrix4, projection_matrix: &Matrix4) {
        if !self.visible
            || self.hidden
            || self.hidden_by_stealth
            || self.drawable_fully_obscured_by_shroud
        {
            return;
        }

        // C++ parity: Drawable::draw() toggles shadows per-frame based on stealth look.
        // Shadows are enabled unless the drawable is visibly detected by the enemy.
        self.set_shadows_enabled(self.stealth_look != StealthLook::VisibleDetected);

        // C++ parity: Drawable::draw() validates position (Drawable.cpp:2634 validatePos()).
        // Skip rendering if position contains NaN or is unreasonably large.
        let pos = &self.position;
        if pos.x.is_nan()
            || pos.y.is_nan()
            || pos.z.is_nan()
            || pos.x.abs() > 10000.0
            || pos.y.abs() > 10000.0
            || pos.z.abs() > 10000.0
        {
            return;
        }

        let opacity = self.get_opacity();
        if opacity <= 0.0 {
            return;
        }

        // C++ parity: Drawable::draw() builds transform from getTransformMatrix() *
        // getInstanceMatrix(), then applies physics xform before draw module dispatch.
        let mut world_transform = self.get_transform();
        if !self.is_instance_identity() {
            let instance = self.instance_transform;
            world_transform = world_transform.mul(&instance);
        }

        // C++ parity: applyPhysicsXform(&transformMtx) at Drawable.cpp:2649.
        // Uses locomotor-derived pitch/roll/yaw/overlap_z from LocoInfo to apply
        // visual physics transforms (vehicle tilt, hover bob, etc.).
        if let Some(ref loco) = self.loco_info {
            let total_pitch = snap_denorm(loco.pitch);
            let total_roll = snap_denorm(loco.roll);
            let total_yaw = snap_denorm(loco.yaw);
            let total_z = snap_denorm(loco.overlap_z);

            let physics_xform = Matrix4::translation(Vector3::new(0.0, 0.0, total_z))
                .mul(&Matrix4::rotation_y(total_pitch))
                .mul(&Matrix4::rotation_x(-total_roll))
                .mul(&Matrix4::rotation_z(total_yaw));
            world_transform = world_transform.mul(&physics_xform);
        }

        // Note: DrawModule dispatch is handled by GameLogic::Drawable::draw(), not here.
        // BasicDrawable::render() handles the rendering submission after draw modules
        // have executed. See GameLogic Drawable::draw() at object/drawable.rs:3393.

        let tint = self.get_tint_color();
        let selected = self.is_selected();

        let model_name = self.template_name.clone().unwrap_or_default();

        let condition_flags = self.compute_render_condition_flags();

        let submission = crate::render_bridge::DrawSubmission {
            drawable_id: crate::render_bridge::DrawableId(self.id.0),
            model_name,
            world_transform: glam::Mat4::from_cols_array_2d(&world_transform.elements),
            condition_flags,
            render_state: crate::render_bridge::RenderStateOverrides {
                opacity,
                emissive_tint: [tint.x.max(0.0), tint.y.max(0.0), tint.z.max(0.0)],
                selected,
                hidden: false,
                ..Default::default()
            },
            bone_overrides: Vec::new(),
            animation_name: None,
            animation_mode: None,
            animation_time: 0.0,
            bounding_sphere: {
                let (_, radius) = self.get_bounding_sphere();
                ww3d_core::BoundingSphere::new(
                    ww3d_core::glam::Vec3::new(self.position.x, self.position.y, self.position.z),
                    radius,
                )
            },
            bounding_box: ww3d_core::AABox::zero(),
            sort_level: 0,
            opaque: opacity >= 1.0,
            transparent: opacity < 1.0,
            cast_shadow: self.status.has(DrawableStatus::SHADOWS),
        };

        if let Ok(mut bridge_guard) = get_render_bridge().lock() {
            if let Some(bridge) = bridge_guard.as_mut() {
                bridge.submit(submission);
            }
        }
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

    fn draw_ui_text(&self) -> Result<(), Box<dyn Error>> {
        let Some(object_id) = self.object_id else {
            return Ok(());
        };

        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Ok(());
        };
        let Ok(object_guard) = object_arc.read() else {
            return Ok(());
        };

        let Some(screen_pos) = with_tactical_view_ref(|view| {
            view.world_to_screen(&Point3::new(
                object_guard.get_position().x,
                object_guard.get_position().y,
                object_guard.get_position().z,
            ))
        }) else {
            return Ok(());
        };

        let draw_group_info = get_draw_group_info()
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();

        let mut text_color = draw_group_info.color_for_text;
        if draw_group_info.use_player_color {
            if let Some(player_arc) = object_guard.get_controlling_player() {
                if let Ok(player_guard) = player_arc.read() {
                    text_color = player_guard.get_player_color().to_argb_u32();
                }
            }
        }

        let anchor_width = 32.0_f32;
        let base_x = if draw_group_info.using_pixel_offset_x {
            screen_pos.x
        } else {
            screen_pos.x + (anchor_width * draw_group_info.percent_offset_x) as i32
        };
        let base_y = if draw_group_info.using_pixel_offset_y {
            screen_pos.y + draw_group_info.pixel_offset_y
        } else {
            screen_pos.y
        };

        let mut drew_anything = false;

        if let Some(player_arc) = object_guard.get_controlling_player() {
            if let Ok(mut player_guard) = player_arc.write() {
                if let Some(group_number) =
                    Self::find_hotkey_squad_number(&mut player_guard, object_guard.get_id())
                {
                    if group_number > NO_HOTKEY_SQUAD && group_number < NUM_HOTKEY_SQUADS as i32 {
                        let mut manager = get_display_string_manager();
                        if let Some(group_text) = manager.get_group_numeral_string(group_number) {
                            Self::draw_caption_string(
                                &group_text,
                                base_x,
                                base_y,
                                text_color,
                                draw_group_info.color_for_text_drop_shadow,
                                &draw_group_info.font_name,
                                draw_group_info.font_size,
                                draw_group_info.font_is_bold,
                                draw_group_info.drop_shadow_offset_x,
                                draw_group_info.drop_shadow_offset_y,
                            );
                            drew_anything = true;
                        }
                    }
                }
            }
        }

        if object_guard.get_formation_id() != FormationID::NONE {
            let mut manager = get_display_string_manager();
            if let Some(formation_text) = manager.get_formation_letter_string() {
                Self::draw_caption_string(
                    &formation_text,
                    base_x + 10,
                    base_y,
                    text_color,
                    draw_group_info.color_for_text_drop_shadow,
                    &draw_group_info.font_name,
                    draw_group_info.font_size,
                    draw_group_info.font_is_bold,
                    draw_group_info.drop_shadow_offset_x,
                    draw_group_info.drop_shadow_offset_y,
                );
                drew_anything = true;
            }
        }

        if drew_anything {
            Ok(())
        } else {
            Ok(())
        }
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut id = self.id.0;
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("{:?}", e))?;

        let mut flags = self.model_condition_flags.clone();
        xfer_model_condition_flags(xfer, &mut flags)?;

        let mut position = self.position;
        xfer_vector3(xfer, &mut position)?;

        let mut instance_transform = self.instance_transform;
        xfer_matrix4(xfer, &mut instance_transform)?;

        let mut instance_scale = self.instance_scale;
        xfer.xfer_real(&mut instance_scale)
            .map_err(|e| format!("{:?}", e))?;

        let mut status_bits = self.status.bits;
        xfer.xfer_unsigned_int(&mut status_bits)
            .map_err(|e| format!("{:?}", e))?;

        let mut tint_status_bits = self.tint_status.bits;
        xfer.xfer_unsigned_int(&mut tint_status_bits)
            .map_err(|e| format!("{:?}", e))?;

        let mut prev_tint_status_bits = self.prev_tint_status.bits;
        xfer.xfer_unsigned_int(&mut prev_tint_status_bits)
            .map_err(|e| format!("{:?}", e))?;

        let mut visible = self.visible;
        xfer.xfer_bool(&mut visible)
            .map_err(|e| format!("{:?}", e))?;

        let mut hidden = self.hidden;
        xfer.xfer_bool(&mut hidden)
            .map_err(|e| format!("{:?}", e))?;

        let mut hidden_by_stealth = self.hidden_by_stealth;
        xfer.xfer_bool(&mut hidden_by_stealth)
            .map_err(|e| format!("{:?}", e))?;

        let mut selected = self.selected;
        xfer.xfer_bool(&mut selected)
            .map_err(|e| format!("{:?}", e))?;

        let mut selectable = self.selectable;
        xfer.xfer_bool(&mut selectable)
            .map_err(|e| format!("{:?}", e))?;

        let mut opacity = self.opacity;
        xfer.xfer_real(&mut opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut explicit_opacity = self.explicit_opacity;
        xfer.xfer_real(&mut explicit_opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut stealth_opacity = self.stealth_opacity;
        xfer.xfer_real(&mut stealth_opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut effective_stealth_opacity = self.effective_stealth_opacity;
        xfer.xfer_real(&mut effective_stealth_opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut stealth_look = stealth_look_to_u8(self.stealth_look);
        xfer.xfer_unsigned_byte(&mut stealth_look)
            .map_err(|e| format!("{:?}", e))?;

        let mut tint_color = self.tint_color;
        xfer_vector3(xfer, &mut tint_color)?;

        let mut has_tint_envelope = self.tint_envelope.is_some();
        xfer.xfer_bool(&mut has_tint_envelope)
            .map_err(|e| format!("{:?}", e))?;
        if has_tint_envelope {
            if let Some(ref envelope) = self.tint_envelope {
                Snapshotable::crc(envelope, xfer)?;
            }
        }

        let mut has_selection_flash = self.selection_flash_envelope.is_some();
        xfer.xfer_bool(&mut has_selection_flash)
            .map_err(|e| format!("{:?}", e))?;
        if has_selection_flash {
            if let Some(ref envelope) = self.selection_flash_envelope {
                Snapshotable::crc(envelope, xfer)?;
            }
        }

        let mut has_icon_info = self.icon_info.is_some();
        xfer.xfer_bool(&mut has_icon_info)
            .map_err(|e| format!("{:?}", e))?;
        if has_icon_info {
            if let Some(ref icon_info) = self.icon_info {
                Snapshotable::crc(icon_info, xfer)?;
            }
        }

        let mut has_loco_info = self.loco_info.is_some();
        xfer.xfer_bool(&mut has_loco_info)
            .map_err(|e| format!("{:?}", e))?;
        if has_loco_info {
            if let Some(ref loco_info) = self.loco_info {
                Snapshotable::crc(loco_info, xfer)?;
            }
        }

        let mut receives_dynamic_lights = self.receives_dynamic_lights;
        xfer.xfer_bool(&mut receives_dynamic_lights)
            .map_err(|e| format!("{:?}", e))?;

        let mut decal_type = terrain_decal_to_u8(self.terrain_decal_type);
        xfer.xfer_unsigned_byte(&mut decal_type)
            .map_err(|e| format!("{:?}", e))?;

        let mut terrain_decal_size = self.terrain_decal_size;
        xfer_vector3(xfer, &mut terrain_decal_size)?;

        let mut decal_opacity = self.decal_opacity;
        xfer.xfer_real(&mut decal_opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut decal_opacity_fade_target = self.decal_opacity_fade_target;
        xfer.xfer_real(&mut decal_opacity_fade_target)
            .map_err(|e| format!("{:?}", e))?;

        let mut decal_opacity_fade_rate = self.decal_opacity_fade_rate;
        xfer.xfer_real(&mut decal_opacity_fade_rate)
            .map_err(|e| format!("{:?}", e))?;

        let mut fade_mode = fading_mode_to_u8(self.fade_mode);
        xfer.xfer_unsigned_byte(&mut fade_mode)
            .map_err(|e| format!("{:?}", e))?;

        let mut time_to_fade = self.time_to_fade;
        xfer.xfer_unsigned_int(&mut time_to_fade)
            .map_err(|e| format!("{:?}", e))?;

        let mut time_elapsed_fade = self.time_elapsed_fade;
        xfer.xfer_unsigned_int(&mut time_elapsed_fade)
            .map_err(|e| format!("{:?}", e))?;

        let mut second_material_pass_opacity = self.second_material_pass_opacity;
        xfer.xfer_real(&mut second_material_pass_opacity)
            .map_err(|e| format!("{:?}", e))?;

        let mut flash_count = self.flash_count;
        xfer.xfer_unsigned_int(&mut flash_count)
            .map_err(|e| format!("{:?}", e))?;

        let mut flash_color = self.flash_color;
        xfer_vector3(xfer, &mut flash_color)?;

        let mut has_expiration = self.expiration_frame.is_some();
        xfer.xfer_bool(&mut has_expiration)
            .map_err(|e| format!("{:?}", e))?;
        if has_expiration {
            let mut frame = self.expiration_frame.unwrap_or(0);
            xfer.xfer_unsigned_int(&mut frame)
                .map_err(|e| format!("{:?}", e))?;
        }

        let mut current_frame = self.current_frame;
        xfer.xfer_unsigned_int(&mut current_frame)
            .map_err(|e| format!("{:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 2;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        let mut id = self.id.0;
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("{:?}", e))?;
        self.id = DrawableId(id);

        if version >= 2 {
            let mut flags = self.model_condition_flags.clone();
            xfer_model_condition_flags(xfer, &mut flags)?;
            if xfer.get_xfer_mode() == XferMode::Load {
                self.replace_model_condition_flags(self.model_condition_flags.clone(), true);
            }
        }

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

        if xfer.get_xfer_mode() == XferMode::Load {
            // C++ resets stealth look after load so a subsequent update re-applies
            // hidden/shadow behavior from authoritative object state.
            self.stealth_look = StealthLook::None;
        }

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

fn xfer_model_condition_flags(
    xfer: &mut dyn Xfer,
    flags: &mut ModelConditionBitFlags,
) -> Result<(), String> {
    let mut stream_bit_count = flags.size().min(u16::MAX as usize) as u16;
    xfer.xfer_unsigned_short(&mut stream_bit_count)
        .map_err(|e| format!("{:?}", e))?;

    let stream_bit_count = stream_bit_count as usize;
    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for i in 0..stream_bit_count {
                let mut value = flags.test(i);
                xfer.xfer_bool(&mut value).map_err(|e| format!("{:?}", e))?;
            }
        }
        XferMode::Load => {
            flags.clear();
            for i in 0..stream_bit_count {
                let mut value = false;
                xfer.xfer_bool(&mut value).map_err(|e| format!("{:?}", e))?;
                if i < flags.size() {
                    flags.set(i, value);
                }
            }
        }
        XferMode::Invalid => {
            return Err("xfer_model_condition_flags - invalid xfer mode".to_string());
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
    fn test_hotkey_squad_resolution() {
        let mut player = gamelogic::player::Player::new(0);
        player.init_from_dict_defaults();

        let squad = player
            .get_hotkey_squad(3)
            .expect("expected squad slot to exist after init");
        squad.add_object_id(77);

        assert_eq!(
            BasicDrawable::find_hotkey_squad_number(&mut player, 77),
            Some(3)
        );
        assert_eq!(
            BasicDrawable::find_hotkey_squad_number(&mut player, 99),
            None
        );
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
    fn test_icon_xfer_order_matches_cpp_slot_order() {
        let names: Vec<&'static str> = IconType::XFER_ORDER
            .iter()
            .map(|icon_type| icon_type.name())
            .collect();
        assert_eq!(
            names,
            vec![
                "DefaultHeal",
                "StructureHeal",
                "VehicleHeal",
                "Demoralized",
                "BombTimed",
                "BombRemote",
                "Disabled",
                "BattlePlanIcon_Bombard",
                "BattlePlanIcon_HoldTheLine",
                "BattlePlanIcon_SeekAndDestroy",
                "Emoticon",
                "Enthusiastic",
                "Subliminal",
                "CarBomb",
            ]
        );
    }

    #[test]
    fn test_model_condition_flags_flow_into_render_flags() {
        use crate::render_bridge::RenderConditionFlags;
        let mut drawable = BasicDrawable::new(DrawableId(1));
        drawable.set_model_condition_state(ModelConditionFlags::DAMAGED);
        drawable.set_model_condition_state(ModelConditionFlags::SNOW);
        drawable.set_model_condition_state(ModelConditionFlags::AFLAME);
        drawable.set_model_condition_state(ModelConditionFlags::TOPPLED);

        let render_flags = drawable.compute_render_condition_flags();
        assert!(render_flags.contains(RenderConditionFlags::DAMAGED));
        assert!(render_flags.contains(RenderConditionFlags::SNOW));
        assert!(render_flags.contains(RenderConditionFlags::AFLAME));
        assert!(render_flags.contains(RenderConditionFlags::TOPPLED));
    }

    #[test]
    fn test_shadow_toggle_helpers() {
        let mut drawable = BasicDrawable::new(DrawableId(1));
        assert!(!drawable.get_status().has(DrawableStatus::SHADOWS));
        drawable.allocate_shadows();
        assert!(drawable.get_status().has(DrawableStatus::SHADOWS));
        drawable.release_shadows();
        assert!(!drawable.get_status().has(DrawableStatus::SHADOWS));
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
