//! Core drawable value types: math, color, status, overlay, stealth.

pub use crate::core::DrawableId;

pub const INVALID_DRAWABLE_ID: DrawableId = DrawableId(0);

/// C++ `Drawable::xfer` visual subset used by the live-host persist tail.
#[derive(Debug, Clone, Default)]
pub struct DrawableXferVisualSnapshot {
    pub explicit_opacity: f32,
    pub stealth_opacity: f32,
    pub effective_stealth_opacity: f32,
    pub instance_scale: f32,
    pub heat_vision_opacity: f32,
    pub tint_status: u32,
    pub prev_tint_status: u32,
    pub hidden: bool,
    pub hidden_by_stealth: bool,
    pub expiration_date: u32,
    pub has_loco: bool,
    pub loco_pitch: f32,
    pub loco_pitch_rate: f32,
    pub loco_roll: f32,
    pub loco_roll_rate: f32,
    pub loco_yaw: f32,
    pub loco_accel_pitch: f32,
    pub loco_accel_pitch_rate: f32,
    pub loco_accel_roll: f32,
    pub loco_accel_roll_rate: f32,
    pub overlay_icons: Vec<(String, u32, String, u32)>,
}

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

    /// Convert this legacy row-major, column-vector affine matrix into the
    /// column-major storage expected by `glam::Mat4` / the WGPU bridge.
    ///
    /// C++ `Matrix3D` and this port's `Matrix4` store translation at
    /// `[row][3]`; handing `elements` directly to
    /// `Mat4::from_cols_array_2d` transposes the semantic transform and puts
    /// translation in the homogeneous row instead. Keep this conversion at
    /// the explicit renderer boundary rather than changing the legacy Xfer
    /// layout or all C++-style matrix users.
    pub(crate) fn to_glam(self) -> glam::Mat4 {
        glam::Mat4::from_cols_array_2d(&[
            [
                self.elements[0][0],
                self.elements[1][0],
                self.elements[2][0],
                self.elements[3][0],
            ],
            [
                self.elements[0][1],
                self.elements[1][1],
                self.elements[2][1],
                self.elements[3][1],
            ],
            [
                self.elements[0][2],
                self.elements[1][2],
                self.elements[2][2],
                self.elements[3][2],
            ],
            [
                self.elements[0][3],
                self.elements[1][3],
                self.elements[2][3],
                self.elements[3][3],
            ],
        ])
    }

    /// Inverse of [`Self::to_glam`] for legacy code that receives a WGPU
    /// transform and must retain the C++ Matrix3D/Xfer convention.
    pub(crate) fn from_glam(matrix: glam::Mat4) -> Self {
        let columns = matrix.to_cols_array_2d();
        Self {
            elements: [
                [columns[0][0], columns[1][0], columns[2][0], columns[3][0]],
                [columns[0][1], columns[1][1], columns[2][1], columns[3][1]],
                [columns[0][2], columns[1][2], columns[2][2], columns[3][2]],
                [columns[0][3], columns[1][3], columns[2][3], columns[3][3]],
            ],
        }
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
    /// World pose for C++ `Drawable::drawCaption` worldToScreen center.
    pub caption_world: Option<[f32; 3]>,
    /// Whether this drawable should have 2D overlay drawn this frame.
    pub visible: bool,
    /// C++ `drawHealthBar` actually drew this frame (selected/hover + ShowObjectHealth).
    pub health_bar_visible: bool,
    /// Fill RGBA from `Drawable.cpp:3871-3916` (cyan construction/disabled or red↔green).
    pub health_fill: [f32; 4],
    /// Outline RGBA companion to `health_fill`.
    pub health_outline: [f32; 4],
    /// `CONTROLBAR:UnderConstructionDesc` formatted with construction percent.
    pub construct_text: Option<String>,
    /// Hotkey-squad numeral for `drawUIText` / live HUD submit.
    pub group_numeral: Option<String>,
    /// Formation `F` letter for `drawUIText`.
    pub formation_letter: Option<String>,
    /// C++ `drawsAnyUIText()` — GameClient should `addTextBearingDrawable`.
    pub queue_ui_text: bool,

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

/// C++ `TheGameText->fetch("CONTROLBAR:UnderConstructionDesc")` + `format(%d)`.
/// `Drawable.cpp:3707`.
pub fn format_under_construction_desc(construction_percent_0_1: f32) -> String {
    let pct = (construction_percent_0_1 * 100.0).round() as i32;
    let tmpl = crate::game_text::GameText::fetch("CONTROLBAR:UnderConstructionDesc");
    if tmpl.starts_with("MISSING:") {
        return format!("Under Construction: {pct}%");
    }
    format_cpp_int_percent(&tmpl, pct)
}

fn format_cpp_int_percent(tmpl: &str, value: i32) -> String {
    let bytes = tmpl.as_bytes();
    let mut out = String::with_capacity(tmpl.len() + 4);
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'%' => {
                    out.push('%');
                    i += 2;
                    continue;
                }
                b'd' | b'i' => {
                    out.push_str(&value.to_string());
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// C++ `Drawable::drawHealthBar` color (`Drawable.cpp:3871-3916`).
pub fn health_bar_colors(
    health_ratio: f32,
    under_construction_or_disabled: bool,
    really_damaged: bool,
    damaged: bool,
) -> ([f32; 4], [f32; 4]) {
    let ratio = health_ratio.clamp(0.0, 1.0);
    if under_construction_or_disabled {
        return ([0.0, ratio, 1.0, 1.0], [0.0, ratio * 0.5, 0.5, 1.0]);
    }
    let (mut red, mut green) = if ratio >= 0.5 {
        (1.0 - ((ratio - 0.5) / 0.5), 1.0)
    } else {
        (1.0, 1.0 - ((0.5 - ratio) / 0.5))
    };
    let outline = [red * 0.5, green * 0.5, 0.0, 1.0];
    if really_damaged {
        red = (1.0 + red) * 0.5;
        green *= 0.5;
    } else if !damaged {
        green = (1.0 + green) * 0.5;
        red *= 0.5;
    }
    ([red, green, 0.0, 1.0], outline)
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
    pub(crate) bits: u32,
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
    pub(crate) bits: u32,
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
pub const FRENZY_COLOR_INFANTRY: Vector3 = Vector3 {
    x: 0.0,
    y: -0.7,
    z: -0.7,
};

pub(crate) const DEFAULT_STEALTH_FRIENDLY_OPACITY: f32 = 0.5;

pub(crate) fn xfer_vector3(
    xfer: &mut dyn game_engine::common::system::Xfer,
    value: &mut Vector3,
) -> Result<(), String> {
    xfer.xfer_real(&mut value.x)
        .map_err(|e| format!("{:?}", e))?;
    xfer.xfer_real(&mut value.y)
        .map_err(|e| format!("{:?}", e))?;
    xfer.xfer_real(&mut value.z)
        .map_err(|e| format!("{:?}", e))?;
    Ok(())
}
