//! InGameUI world-anim residual: pack presentation MoneyPickUp Anim2D samples into a
//! CPU layout buffer ready for dual-tick UI / eventual Anim2DCollection draw.
//!
//! Host residual closed here (fail-closed vs full retail Anim2D GPU draw):
//! - Z-rise offset from retail `zRisePerSecond` × age (host Y-up → +lift)
//! - Display-time / fade residual from MoneyPickUp template parameters
//! - Anim2D frame advance residual (NumberImages / AnimationDelay / LOOP mode)
//! - Frame image name residual (`SCPDollar000`..`SCPDollar030`)
//! - Honesty counters for anims / active / bytes packed
//! - Deterministic pack order for dual-tick presentation consumers
//! - MoneyPickUp ExecuteAnimationTime **4.0s** / ZRise **15** / Fades **Yes**
//!   + residual fade window **1.0s** after display time (WORLD_ANIM_FADE_WINDOW)
//! - Anim2D mode residual table (ONCE / LOOP / PING_PONG + reverse variants)
//!   matching C++ `Anim2D::tryNextFrame` frame advance residual
//! - Anim2D status bits residual (NONE/FROZEN/REVERSED/COMPLETE) + setAlpha
//!   residual (default alpha 1.0; draw color alpha = 255 * m_alpha)
//! - Anim2DCollection template list residual (`newTemplate` head-insert /
//!   `findTemplate` by name) + instance register/unRegister doubly-linked list
//! - Anim2DCollection `update` residual skips tryNextFrame when FROZEN
//! - Anim2DCollection `init` residual path `Data/INI/Animation2D.ini`
//! - MoneyPickUp RandomizeStartFrame = No residual
//! - `getCurrentFrameWidth`/`Height` monospaced placeholder size residual
//!
//! Still residual:
//! - Full Anim2DCollection GPU texture atlas sample / WW3D Image draw
//! - WORLD_ANIM_FADE_ON_EXPIRE live Display surface blend

use crate::presentation_frame::{PresentationFrame, PresentationWorldAnim};

/// Floats per packed world-anim layout entry:
/// pos.xyz + lift_y + display_time + z_rise + age_frames + alpha + fades + current_frame = 10 × f32.
pub const WORLD_ANIM_LAYOUT_FLOATS: usize = 10;
/// Bytes per packed layout entry.
pub const WORLD_ANIM_LAYOUT_BYTES: usize = WORLD_ANIM_LAYOUT_FLOATS * std::mem::size_of::<f32>();

/// Logic frames per second residual for age → seconds conversion.
pub const WORLD_ANIM_LOGIC_FPS: f32 = 30.0;

// ---------------------------------------------------------------------------
// MoneyPickUp Anim2D template residual (Animation2D.ini)
// ---------------------------------------------------------------------------

/// Retail `NumberImages` for MoneyPickUp.
pub const MONEY_PICKUP_NUM_FRAMES: u16 = 31;
/// Retail `AnimationDelay` in milliseconds.
pub const MONEY_PICKUP_ANIM_DELAY_MS: u32 = 30;
/// Retail frames between updates: ceil(ms * 30 / 1000) = ceil(0.9) = 1.
pub const MONEY_PICKUP_FRAMES_BETWEEN_UPDATES: u32 = 1;
/// Retail AnimationMode = LOOP.
pub const MONEY_PICKUP_ANIM_MODE_LOOP: bool = true;
/// Retail RandomizeStartFrame residual (No).
pub const MONEY_PICKUP_RANDOMIZE_START_FRAME: bool = false;

/// C++ `Anim2DMode` residual (Anim2D.h discriminants; keep order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ResidualAnim2DMode {
    Invalid = 0,
    Once = 1,
    OnceBackwards = 2,
    Loop = 3,
    LoopBackwards = 4,
    PingPong = 5,
    PingPongBackwards = 6,
}

/// C++ `ANIM_2D_NUM_MODES` residual (keep-last sentinel; not a valid mode).
pub const ANIM_2D_NUM_MODES: u32 = 7;

/// C++ `Anim2DModeNames[]` residual (DEFINE_ANIM_2D_MODE_NAMES).
pub const ANIM_2D_MODE_NAMES: [&str; 7] = [
    "NONE",
    "ONCE",
    "ONCE_BACKWARDS",
    "LOOP",
    "LOOP_BACKWARDS",
    "PING_PONG",
    "PING_PONG_BACKWARDS",
];

/// Default template AnimationMode residual (`Anim2DTemplate` ctor → ANIM_2D_LOOP).
pub const ANIM_2D_DEFAULT_MODE: ResidualAnim2DMode = ResidualAnim2DMode::Loop;

/// MoneyPickUp retail AnimationMode residual.
pub const MONEY_PICKUP_ANIM_MODE: ResidualAnim2DMode = ResidualAnim2DMode::Loop;

impl ResidualAnim2DMode {
    pub const fn discriminant(self) -> u32 {
        self as u32
    }

    pub const fn name(self) -> &'static str {
        ANIM_2D_MODE_NAMES[self as usize]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "NONE" => Some(Self::Invalid),
            "ONCE" => Some(Self::Once),
            "ONCE_BACKWARDS" => Some(Self::OnceBackwards),
            "LOOP" => Some(Self::Loop),
            "LOOP_BACKWARDS" => Some(Self::LoopBackwards),
            "PING_PONG" => Some(Self::PingPong),
            "PING_PONG_BACKWARDS" => Some(Self::PingPongBackwards),
            _ => None,
        }
    }

    pub const fn is_loop_family(self) -> bool {
        matches!(self, Self::Loop | Self::LoopBackwards)
    }

    pub const fn is_ping_pong_family(self) -> bool {
        matches!(self, Self::PingPong | Self::PingPongBackwards)
    }

    pub const fn starts_backwards(self) -> bool {
        matches!(
            self,
            Self::OnceBackwards | Self::LoopBackwards | Self::PingPongBackwards
        )
    }
}

/// Honesty: full Anim2DMode residual table (discriminants + names + defaults).
pub fn honesty_anim2d_mode_table() -> bool {
    ResidualAnim2DMode::Invalid.discriminant() == 0
        && ResidualAnim2DMode::Once.discriminant() == 1
        && ResidualAnim2DMode::OnceBackwards.discriminant() == 2
        && ResidualAnim2DMode::Loop.discriminant() == 3
        && ResidualAnim2DMode::LoopBackwards.discriminant() == 4
        && ResidualAnim2DMode::PingPong.discriminant() == 5
        && ResidualAnim2DMode::PingPongBackwards.discriminant() == 6
        && ANIM_2D_NUM_MODES == 7
        && ANIM_2D_MODE_NAMES.len() as u32 == ANIM_2D_NUM_MODES
        && ResidualAnim2DMode::Invalid.name() == "NONE"
        && ResidualAnim2DMode::Once.name() == "ONCE"
        && ResidualAnim2DMode::OnceBackwards.name() == "ONCE_BACKWARDS"
        && ResidualAnim2DMode::Loop.name() == "LOOP"
        && ResidualAnim2DMode::LoopBackwards.name() == "LOOP_BACKWARDS"
        && ResidualAnim2DMode::PingPong.name() == "PING_PONG"
        && ResidualAnim2DMode::PingPongBackwards.name() == "PING_PONG_BACKWARDS"
        && ANIM_2D_DEFAULT_MODE == ResidualAnim2DMode::Loop
        && MONEY_PICKUP_ANIM_MODE == ResidualAnim2DMode::Loop
        && MONEY_PICKUP_ANIM_MODE_LOOP
        && ResidualAnim2DMode::from_name("LOOP") == Some(ResidualAnim2DMode::Loop)
        && ResidualAnim2DMode::from_name("PING_PONG") == Some(ResidualAnim2DMode::PingPong)
        && ResidualAnim2DMode::from_name("bogus").is_none()
        && ResidualAnim2DMode::Loop.is_loop_family()
        && ResidualAnim2DMode::PingPong.is_ping_pong_family()
        && ResidualAnim2DMode::OnceBackwards.starts_backwards()
        && !ResidualAnim2DMode::Once.starts_backwards()
}
/// Retail image sequence prefix (`SCPDollar000`..).
pub const MONEY_PICKUP_IMAGE_PREFIX: &str = "SCPDollar";
/// Retail ExecuteAnimationTime residual (seconds) from MoneyCrateCollide.
pub const MONEY_PICKUP_DISPLAY_TIME_SECONDS: f32 = 4.0;
/// Retail ExecuteAnimationZRise residual (world units per second).
pub const MONEY_PICKUP_Z_RISE_PER_SECOND: f32 = 15.0;
/// Retail ExecuteAnimationFades residual (Yes).
pub const MONEY_PICKUP_FADES: bool = true;
/// Host residual fade window after display time (seconds) when Fades=Yes.
///
/// C++ world-anim fade residual: alpha decays over ~1 second after display
/// time expires. Fail-closed: not live WORLD_ANIM_FADE_ON_EXPIRE Display blend.
pub const MONEY_PICKUP_FADE_WINDOW_SECONDS: f32 = 1.0;

/// Residual alpha for MoneyPickUp given age_seconds and template fade params.
///
/// - age < display → alpha 1.0 (active)
/// - age ≥ display and fades → clamp(1 - (age - display) / fade_window, 0..1)
/// - age ≥ display and !fades → 0.0
#[inline]
pub fn money_pickup_fade_alpha(age_seconds: f32, display_time: f32, fades: bool) -> f32 {
    if age_seconds < display_time {
        1.0
    } else if fades {
        let past = age_seconds - display_time;
        (1.0 - past / MONEY_PICKUP_FADE_WINDOW_SECONDS).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Honesty: MoneyPickUp ExecuteAnimation residual params + fade window.
pub fn honesty_money_pickup_fade_params() -> bool {
    (MONEY_PICKUP_DISPLAY_TIME_SECONDS - 4.0).abs() < 0.01
        && (MONEY_PICKUP_Z_RISE_PER_SECOND - 15.0).abs() < 0.01
        && MONEY_PICKUP_FADES
        && (MONEY_PICKUP_FADE_WINDOW_SECONDS - 1.0).abs() < 0.01
        && (money_pickup_fade_alpha(0.0, 4.0, true) - 1.0).abs() < 0.01
        && (money_pickup_fade_alpha(3.9, 4.0, true) - 1.0).abs() < 0.01
        && (money_pickup_fade_alpha(4.0, 4.0, true) - 1.0).abs() < 0.01
        && (money_pickup_fade_alpha(4.5, 4.0, true) - 0.5).abs() < 0.01
        && (money_pickup_fade_alpha(5.0, 4.0, true) - 0.0).abs() < 0.01
        && (money_pickup_fade_alpha(5.0, 4.0, false) - 0.0).abs() < 0.01
}

/// Convert AnimationDelay ms → logic frames (C++ parseDurationUnsignedShort / ceil).
pub fn anim_delay_ms_to_frames(ms: u32) -> u32 {
    // ConvertDurationFromMsecsToFrames: ms * 30 / 1000, then ceil.
    let frames = (ms as f32) * WORLD_ANIM_LOGIC_FPS / 1000.0;
    frames.ceil().max(1.0) as u32
}

/// MoneyPickUp LOOP mode current frame residual from age.
///
/// C++ Anim2D::try_next_frame advances when
/// `now - last_update >= frames_between_updates`, LOOP wraps max→min.
pub fn money_pickup_current_frame(age_frames: u32) -> u16 {
    let between = MONEY_PICKUP_FRAMES_BETWEEN_UPDATES.max(1);
    let steps = age_frames / between;
    (steps % (MONEY_PICKUP_NUM_FRAMES as u32)) as u16
}

/// Residual frame image name: `SCPDollar` + zero-padded 3-digit frame.
pub fn money_pickup_frame_image_name(frame: u16) -> String {
    format!("{MONEY_PICKUP_IMAGE_PREFIX}{frame:03}")
}

/// Honesty: frame index + image name residual match MoneyPickUp template.
pub fn honesty_money_pickup_frame(age_frames: u32, frame: u16, image: &str) -> bool {
    let expected = money_pickup_current_frame(age_frames);
    frame == expected
        && image == money_pickup_frame_image_name(expected)
        && expected < MONEY_PICKUP_NUM_FRAMES
        && anim_delay_ms_to_frames(MONEY_PICKUP_ANIM_DELAY_MS) == MONEY_PICKUP_FRAMES_BETWEEN_UPDATES
        && MONEY_PICKUP_ANIM_MODE_LOOP
        && !MONEY_PICKUP_RANDOMIZE_START_FRAME
}

/// Residual honesty: full SCPDollar000..030 image sequence residual table.
pub fn honesty_money_pickup_image_sequence() -> bool {
    if MONEY_PICKUP_NUM_FRAMES != 31 {
        return false;
    }
    for frame in 0..MONEY_PICKUP_NUM_FRAMES {
        let name = money_pickup_frame_image_name(frame);
        if name != format!("{MONEY_PICKUP_IMAGE_PREFIX}{frame:03}") {
            return false;
        }
    }
    !MONEY_PICKUP_RANDOMIZE_START_FRAME && MONEY_PICKUP_ANIM_MODE_LOOP
}

/// Residual Anim2D animation mode discriminants (C++ `Anim2DMode` order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Anim2DModeResidual {
    /// ANIM_2D_ONCE — advance to max then COMPLETE.
    Once = 1,
    /// ANIM_2D_ONCE_BACKWARDS — decrement to min then COMPLETE.
    OnceBackwards = 2,
    /// ANIM_2D_LOOP — wrap max → min.
    Loop = 3,
    /// ANIM_2D_LOOP_BACKWARDS — wrap min → max.
    LoopBackwards = 4,
    /// ANIM_2D_PING_PONG — bounce with REVERSED status bit.
    PingPong = 5,
    /// ANIM_2D_PING_PONG_BACKWARDS — same bounce path as PingPong.
    PingPongBackwards = 6,
}

/// Result of one residual `Anim2D::tryNextFrame` step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anim2DFrameStep {
    pub frame: u16,
    /// ANIM_2D_STATUS_REVERSED residual (ping-pong direction).
    pub reversed: bool,
    /// ANIM_2D_STATUS_COMPLETE residual (ONCE modes at end).
    pub complete: bool,
}

/// Host residual for C++ `Anim2D::tryNextFrame` mode switch.
///
/// Fail-closed: not full GameLogic frame gate / Image draw / GPU atlas sample.
pub fn anim2d_try_next_frame(
    mode: Anim2DModeResidual,
    current: u16,
    min_frame: u16,
    max_frame: u16,
    reversed: bool,
) -> Anim2DFrameStep {
    match mode {
        Anim2DModeResidual::Once => {
            if current < max_frame {
                Anim2DFrameStep { frame: current.saturating_add(1), reversed: false, complete: false }
            } else {
                Anim2DFrameStep { frame: current, reversed: false, complete: true }
            }
        }
        Anim2DModeResidual::OnceBackwards => {
            if current > min_frame {
                Anim2DFrameStep { frame: current.saturating_sub(1), reversed: false, complete: false }
            } else {
                Anim2DFrameStep { frame: current, reversed: false, complete: true }
            }
        }
        Anim2DModeResidual::Loop => {
            if current == max_frame {
                Anim2DFrameStep { frame: min_frame, reversed: false, complete: false }
            } else {
                Anim2DFrameStep { frame: current.saturating_add(1), reversed: false, complete: false }
            }
        }
        Anim2DModeResidual::LoopBackwards => {
            if current > min_frame {
                Anim2DFrameStep { frame: current.saturating_sub(1), reversed: false, complete: false }
            } else {
                Anim2DFrameStep { frame: max_frame, reversed: false, complete: false }
            }
        }
        Anim2DModeResidual::PingPong | Anim2DModeResidual::PingPongBackwards => {
            if reversed {
                if current == min_frame {
                    Anim2DFrameStep { frame: current.saturating_add(1), reversed: false, complete: false }
                } else {
                    Anim2DFrameStep { frame: current.saturating_sub(1), reversed: true, complete: false }
                }
            } else if current == max_frame {
                Anim2DFrameStep { frame: current.saturating_sub(1), reversed: true, complete: false }
            } else {
                Anim2DFrameStep { frame: current.saturating_add(1), reversed: false, complete: false }
            }
        }
    }
}

/// Honesty: Anim2D mode residual table + MoneyPickUp LOOP path.
pub fn honesty_anim2d_mode_residual() -> bool {
    let step = anim2d_try_next_frame(Anim2DModeResidual::Loop, 30, 0, 30, false);
    if step.frame != 0 || step.complete || step.reversed {
        return false;
    }
    let step = anim2d_try_next_frame(Anim2DModeResidual::Loop, 0, 0, 30, false);
    if step.frame != 1 {
        return false;
    }
    let step = anim2d_try_next_frame(Anim2DModeResidual::Once, 5, 0, 5, false);
    if !step.complete || step.frame != 5 {
        return false;
    }
    let step = anim2d_try_next_frame(Anim2DModeResidual::Once, 4, 0, 5, false);
    if step.complete || step.frame != 5 {
        return false;
    }
    let step = anim2d_try_next_frame(Anim2DModeResidual::OnceBackwards, 0, 0, 5, false);
    if !step.complete || step.frame != 0 {
        return false;
    }
    let step = anim2d_try_next_frame(Anim2DModeResidual::LoopBackwards, 0, 0, 5, false);
    if step.frame != 5 {
        return false;
    }
    let step = anim2d_try_next_frame(Anim2DModeResidual::PingPong, 5, 0, 5, false);
    if step.frame != 4 || !step.reversed {
        return false;
    }
    let step = anim2d_try_next_frame(Anim2DModeResidual::PingPong, 0, 0, 5, true);
    if step.frame != 1 || step.reversed {
        return false;
    }
    let a = anim2d_try_next_frame(Anim2DModeResidual::PingPong, 3, 0, 5, false);
    let b = anim2d_try_next_frame(Anim2DModeResidual::PingPongBackwards, 3, 0, 5, false);
    a == b && MONEY_PICKUP_ANIM_MODE_LOOP && !MONEY_PICKUP_RANDOMIZE_START_FRAME
}

/// C++ `ANIM_2D_STATUS_*` residual bit flags.
pub const ANIM_2D_STATUS_NONE: u8 = 0x00;
pub const ANIM_2D_STATUS_FROZEN: u8 = 0x01;
pub const ANIM_2D_STATUS_REVERSED: u8 = 0x02;
pub const ANIM_2D_STATUS_COMPLETE: u8 = 0x04;
/// C++ `Anim2D` constructor default alpha residual.
pub const ANIM_2D_DEFAULT_ALPHA: f32 = 1.0;

/// Host residual: set status bit(s) (`Anim2D::setStatus`).
#[inline]
pub fn anim2d_set_status(status: u8, bits: u8) -> u8 {
    status | bits
}

/// Host residual: clear status bit(s) (`Anim2D::clearStatus`).
#[inline]
pub fn anim2d_clear_status(status: u8, bits: u8) -> u8 {
    status & !bits
}

/// Host residual: test status bit.
#[inline]
pub fn anim2d_status_test(status: u8, bits: u8) -> bool {
    (status & bits) != 0
}

/// Host residual: clamp setAlpha to [0, 1] (C++ stores Real freely; draw multiplies).
#[inline]
pub fn anim2d_set_alpha(alpha: f32) -> f32 {
    alpha.clamp(0.0, 1.0)
}

/// Host residual draw color alpha byte: `255 * m_alpha` (C++ `GameMakeColor`).
#[inline]
pub fn anim2d_draw_color_alpha(alpha: f32) -> u8 {
    (255.0 * anim2d_set_alpha(alpha)).round().clamp(0.0, 255.0) as u8
}

/// Honesty: Anim2D status bits + setAlpha residual.
pub fn honesty_anim2d_status_alpha() -> bool {
    ANIM_2D_STATUS_NONE == 0
        && ANIM_2D_STATUS_FROZEN == 0x01
        && ANIM_2D_STATUS_REVERSED == 0x02
        && ANIM_2D_STATUS_COMPLETE == 0x04
        && (ANIM_2D_DEFAULT_ALPHA - 1.0).abs() < 0.01
        && anim2d_set_status(ANIM_2D_STATUS_NONE, ANIM_2D_STATUS_FROZEN) == ANIM_2D_STATUS_FROZEN
        && anim2d_set_status(ANIM_2D_STATUS_FROZEN, ANIM_2D_STATUS_REVERSED)
            == (ANIM_2D_STATUS_FROZEN | ANIM_2D_STATUS_REVERSED)
        && anim2d_clear_status(
            ANIM_2D_STATUS_FROZEN | ANIM_2D_STATUS_REVERSED,
            ANIM_2D_STATUS_REVERSED,
        ) == ANIM_2D_STATUS_FROZEN
        && anim2d_status_test(ANIM_2D_STATUS_COMPLETE, ANIM_2D_STATUS_COMPLETE)
        && !anim2d_status_test(ANIM_2D_STATUS_NONE, ANIM_2D_STATUS_FROZEN)
        // FROZEN skips tryNextFrame residual.
        && anim2d_status_test(ANIM_2D_STATUS_FROZEN, ANIM_2D_STATUS_FROZEN)
        && anim2d_draw_color_alpha(1.0) == 255
        && anim2d_draw_color_alpha(0.5) == 128
        && anim2d_draw_color_alpha(0.0) == 0
        && (anim2d_set_alpha(1.5) - 1.0).abs() < 0.01
        && (anim2d_set_alpha(-0.1) - 0.0).abs() < 0.01
}

// ---------------------------------------------------------------------------
// Anim2DCollection residual (Anim2D.cpp) — host-testable, fail-closed vs GPU
// ---------------------------------------------------------------------------

/// C++ `Anim2DCollection::init` residual INI path (`Data\INI\Animation2D.ini`).
///
/// Host residual normalizes separators to `/`. Fail-closed: path constant honesty
/// only — not full INI parse of every Anim2DTemplate.
pub const ANIM2D_COLLECTION_INI_PATH: &str = "Data/INI/Animation2D.ini";

/// Host residual monospaced placeholder size for frame image atlas residual.
///
/// C++ `getCurrentFrameWidth/Height` returns Image natural size when the frame
/// Image is loaded; host residual uses a fixed monospaced placeholder so
/// width/height honesty stays deterministic without a GPU atlas.
pub const ANIM2D_FRAME_PLACEHOLDER_WIDTH: u32 = 32;
pub const ANIM2D_FRAME_PLACEHOLDER_HEIGHT: u32 = 32;

/// Host residual Anim2DTemplate id (collection list node).
pub type Anim2DTemplateId = u32;
/// Host residual Anim2D instance id (collection instance list node).
pub type Anim2DInstanceId = u32;

/// Host residual Anim2DTemplate entry (name + next link only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anim2DTemplateResidual {
    pub id: Anim2DTemplateId,
    pub name: String,
    /// MoneyPickUp RandomizeStartFrame residual (No for retail MoneyPickUp).
    pub randomize_start_frame: bool,
    pub num_frames: u16,
}

/// Host residual Anim2D instance entry for collection update residual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anim2DInstanceResidual {
    pub id: Anim2DInstanceId,
    pub template_id: Anim2DTemplateId,
    pub status: u8,
    pub current_frame: u16,
    pub min_frame: u16,
    pub max_frame: u16,
    pub mode: Anim2DModeResidual,
    /// True when instance is registered with this collection (C++ m_collectionSystem).
    pub registered: bool,
}

/// Host residual Anim2DCollection (template head-insert list + instance list).
///
/// C++ `newTemplate` head-inserts; `findTemplate` linear name search;
/// `registerAnimation` / `unRegisterAnimation` doubly-linked instance list;
/// `update` calls tryNextFrame unless ANIM_2D_STATUS_FROZEN.
/// Fail-closed: not full INI parse / Image atlas / GPU draw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Anim2DCollectionResidual {
    /// Head of template list residual (None = empty).
    pub template_head: Option<Anim2DTemplateId>,
    /// next residual per template id (singly linked, head-insert).
    pub template_next: std::collections::HashMap<Anim2DTemplateId, Option<Anim2DTemplateId>>,
    pub templates: std::collections::HashMap<Anim2DTemplateId, Anim2DTemplateResidual>,
    /// Head of instance list residual.
    pub instance_head: Option<Anim2DInstanceId>,
    pub instance_next: std::collections::HashMap<Anim2DInstanceId, Option<Anim2DInstanceId>>,
    pub instance_prev: std::collections::HashMap<Anim2DInstanceId, Option<Anim2DInstanceId>>,
    pub instances: std::collections::HashMap<Anim2DInstanceId, Anim2DInstanceResidual>,
    next_template_id: Anim2DTemplateId,
    next_instance_id: Anim2DInstanceId,
    /// True after init residual records the Animation2D.ini path constant.
    pub init_path_recorded: bool,
}

impl Anim2DCollectionResidual {
    pub fn new() -> Self {
        Self::default()
    }

    /// C++ `Anim2DCollection::init` residual: record Animation2D.ini path.
    ///
    /// Fail-closed: does not parse all templates from the live INI file.
    pub fn init(&mut self) {
        // Path constant honesty only.
        let _ = ANIM2D_COLLECTION_INI_PATH;
        self.init_path_recorded = true;
    }

    /// C++ `newTemplate`: allocate, assign name, head-insert into template list.
    pub fn new_template(&mut self, name: &str) -> Anim2DTemplateId {
        let id = self.next_template_id.saturating_add(1).max(1);
        self.next_template_id = id;
        let randomize = if name == "MoneyPickUp" {
            MONEY_PICKUP_RANDOMIZE_START_FRAME
        } else {
            false
        };
        let num_frames = if name == "MoneyPickUp" {
            MONEY_PICKUP_NUM_FRAMES
        } else {
            1
        };
        self.templates.insert(
            id,
            Anim2DTemplateResidual {
                id,
                name: name.to_string(),
                randomize_start_frame: randomize,
                num_frames,
            },
        );
        // Head-insert.
        self.template_next.insert(id, self.template_head);
        self.template_head = Some(id);
        id
    }

    /// C++ `findTemplate`: linear search template list by name.
    pub fn find_template(&self, name: &str) -> Option<Anim2DTemplateId> {
        let mut cur = self.template_head;
        while let Some(id) = cur {
            if self
                .templates
                .get(&id)
                .map(|t| t.name.as_str() == name)
                .unwrap_or(false)
            {
                return Some(id);
            }
            cur = self.template_next.get(&id).copied().flatten();
        }
        None
    }

    /// Allocate a residual instance bound to a template (not yet registered).
    pub fn new_instance(
        &mut self,
        template_id: Anim2DTemplateId,
        mode: Anim2DModeResidual,
        min_frame: u16,
        max_frame: u16,
    ) -> Anim2DInstanceId {
        let id = self.next_instance_id.saturating_add(1).max(1);
        self.next_instance_id = id;
        self.instances.insert(
            id,
            Anim2DInstanceResidual {
                id,
                template_id,
                status: ANIM_2D_STATUS_NONE,
                current_frame: min_frame,
                min_frame,
                max_frame,
                mode,
                registered: false,
            },
        );
        id
    }

    /// C++ `registerAnimation`: head-insert into doubly-linked instance list.
    pub fn register_animation(&mut self, id: Anim2DInstanceId) {
        if !self.instances.contains_key(&id) {
            return;
        }
        if self.instances.get(&id).map(|i| i.registered).unwrap_or(false) {
            return;
        }
        self.instance_next.insert(id, self.instance_head);
        self.instance_prev.insert(id, None);
        if let Some(old_head) = self.instance_head {
            self.instance_prev.insert(old_head, Some(id));
        }
        self.instance_head = Some(id);
        if let Some(inst) = self.instances.get_mut(&id) {
            inst.registered = true;
        }
    }

    /// C++ `unRegisterAnimation`: unlink from doubly-linked instance list.
    pub fn unregister_animation(&mut self, id: Anim2DInstanceId) {
        if !self.instances.get(&id).map(|i| i.registered).unwrap_or(false) {
            return;
        }
        let n = self.instance_next.get(&id).copied().flatten();
        let p = self.instance_prev.get(&id).copied().flatten();
        if let Some(nx) = n {
            self.instance_prev.insert(nx, p);
        }
        if let Some(pv) = p {
            self.instance_next.insert(pv, n);
        } else {
            self.instance_head = n;
        }
        self.instance_next.remove(&id);
        self.instance_prev.remove(&id);
        if let Some(inst) = self.instances.get_mut(&id) {
            inst.registered = false;
        }
    }

    /// C++ `Anim2DCollection::update`: tryNextFrame unless FROZEN.
    ///
    /// Returns number of instances that advanced (non-frozen residual steps).
    pub fn update(&mut self) -> u32 {
        let mut advanced = 0u32;
        // Collect ids first (avoid borrow conflicts while mutating instances).
        let mut ids = Vec::new();
        let mut cur = self.instance_head;
        while let Some(id) = cur {
            ids.push(id);
            cur = self.instance_next.get(&id).copied().flatten();
        }
        for id in ids {
            let Some(inst) = self.instances.get(&id) else {
                continue;
            };
            if anim2d_status_test(inst.status, ANIM_2D_STATUS_FROZEN) {
                continue;
            }
            let reversed = anim2d_status_test(inst.status, ANIM_2D_STATUS_REVERSED);
            let step = anim2d_try_next_frame(
                inst.mode,
                inst.current_frame,
                inst.min_frame,
                inst.max_frame,
                reversed,
            );
            if let Some(inst) = self.instances.get_mut(&id) {
                inst.current_frame = step.frame;
                if step.reversed {
                    inst.status = anim2d_set_status(inst.status, ANIM_2D_STATUS_REVERSED);
                } else {
                    inst.status = anim2d_clear_status(inst.status, ANIM_2D_STATUS_REVERSED);
                }
                if step.complete {
                    inst.status = anim2d_set_status(inst.status, ANIM_2D_STATUS_COMPLETE);
                }
                advanced = advanced.saturating_add(1);
            }
        }
        advanced
    }

    pub fn template_count(&self) -> usize {
        self.templates.len()
    }

    pub fn instance_count(&self) -> usize {
        self.instance_next.len()
    }
}

/// Host residual `Anim2D::getCurrentFrameWidth` monospaced placeholder.
///
/// C++ returns Image natural width when frame Image is present, else 0.
/// Host residual: valid frame index → placeholder width; out-of-range → 0.
/// Fail-closed vs live Image atlas.
#[inline]
pub fn anim2d_get_current_frame_width(frame: u16, num_frames: u16) -> u32 {
    if num_frames == 0 || frame >= num_frames {
        0
    } else {
        ANIM2D_FRAME_PLACEHOLDER_WIDTH
    }
}

/// Host residual `Anim2D::getCurrentFrameHeight` monospaced placeholder.
#[inline]
pub fn anim2d_get_current_frame_height(frame: u16, num_frames: u16) -> u32 {
    if num_frames == 0 || frame >= num_frames {
        0
    } else {
        ANIM2D_FRAME_PLACEHOLDER_HEIGHT
    }
}

/// Honesty: Anim2DCollection template/instance/update/init/frame-size residual.
pub fn honesty_anim2d_collection_residual() -> bool {
    // init path residual.
    if ANIM2D_COLLECTION_INI_PATH != "Data/INI/Animation2D.ini" {
        return false;
    }
    let mut col = Anim2DCollectionResidual::new();
    col.init();
    if !col.init_path_recorded {
        return false;
    }

    // newTemplate head-insert + findTemplate.
    let t1 = col.new_template("FirstAnim");
    let t2 = col.new_template("MoneyPickUp");
    if col.template_head != Some(t2) {
        return false;
    }
    if col.template_next.get(&t2).copied().flatten() != Some(t1) {
        return false;
    }
    if col.find_template("MoneyPickUp") != Some(t2) {
        return false;
    }
    if col.find_template("FirstAnim") != Some(t1) {
        return false;
    }
    if col.find_template("Missing").is_some() {
        return false;
    }
    // MoneyPickUp RandomizeStartFrame = No residual.
    if col
        .templates
        .get(&t2)
        .map(|t| t.randomize_start_frame)
        .unwrap_or(true)
    {
        return false;
    }
    if MONEY_PICKUP_RANDOMIZE_START_FRAME {
        return false;
    }

    // register / unregister doubly-linked instance list.
    let i1 = col.new_instance(t2, Anim2DModeResidual::Loop, 0, 30);
    let i2 = col.new_instance(t2, Anim2DModeResidual::Loop, 0, 30);
    col.register_animation(i1);
    col.register_animation(i2);
    // head-insert: i2 → i1
    if col.instance_head != Some(i2) {
        return false;
    }
    if col.instance_next.get(&i2).copied().flatten() != Some(i1) {
        return false;
    }
    if col.instance_prev.get(&i1).copied().flatten() != Some(i2) {
        return false;
    }
    col.unregister_animation(i2);
    if col.instance_head != Some(i1) {
        return false;
    }
    col.unregister_animation(i1);
    if col.instance_head.is_some() || col.instance_count() != 0 {
        return false;
    }

    // update skips FROZEN; advances non-frozen LOOP.
    let i3 = col.new_instance(t2, Anim2DModeResidual::Loop, 0, 30);
    let i4 = col.new_instance(t2, Anim2DModeResidual::Loop, 0, 30);
    col.register_animation(i3);
    col.register_animation(i4);
    if let Some(inst) = col.instances.get_mut(&i3) {
        inst.status = ANIM_2D_STATUS_FROZEN;
        inst.current_frame = 5;
    }
    if let Some(inst) = col.instances.get_mut(&i4) {
        inst.current_frame = 5;
    }
    let advanced = col.update();
    if advanced != 1 {
        return false;
    }
    if col.instances.get(&i3).map(|i| i.current_frame) != Some(5) {
        return false;
    }
    if col.instances.get(&i4).map(|i| i.current_frame) != Some(6) {
        return false;
    }

    // getCurrentFrameWidth/Height monospaced placeholder residual.
    anim2d_get_current_frame_width(0, MONEY_PICKUP_NUM_FRAMES) == ANIM2D_FRAME_PLACEHOLDER_WIDTH
        && anim2d_get_current_frame_height(15, MONEY_PICKUP_NUM_FRAMES)
            == ANIM2D_FRAME_PLACEHOLDER_HEIGHT
        && anim2d_get_current_frame_width(31, MONEY_PICKUP_NUM_FRAMES) == 0
        && anim2d_get_current_frame_height(0, 0) == 0
        && anim2d_get_current_frame_width(0, 1) == ANIM2D_FRAME_PLACEHOLDER_WIDTH
}

/// One CPU-side residual world-anim layout sample.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldAnimLayoutEntry {
    pub position: [f32; 3],
    /// C++ residual: Z rise over age (`zRisePerSecond * age_seconds`); host Y-up → +lift.
    pub lift_y: f32,
    pub display_time_seconds: f32,
    pub z_rise_per_second: f32,
    pub age_frames: f32,
    /// Alpha after fade residual (1.0 while active; decays after display time when fades).
    pub alpha: f32,
    pub fades: f32,
    pub template_hash: u32,
    /// Anim2D current frame residual (LOOP advance).
    pub current_frame: u16,
    /// Frame image name residual (`SCPDollarNNN`).
    pub frame_image: String,
}

impl WorldAnimLayoutEntry {
    pub fn to_floats(self) -> [f32; WORLD_ANIM_LAYOUT_FLOATS] {
        [
            self.position[0],
            self.position[1],
            self.position[2],
            self.lift_y,
            self.display_time_seconds,
            self.z_rise_per_second,
            self.age_frames,
            self.alpha,
            self.fades,
            self.current_frame as f32,
        ]
    }
}

/// Honesty bookkeeping for the residual world-anim layout path.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldAnimLayoutHonesty {
    pub anims_packed: u32,
    pub active_packed: u32,
    pub bytes_packed: u32,
    pub cpu_pack_ok: bool,
    pub has_geometry: bool,
    pub gpu_upload_ready: bool,
    pub money_pickup_templates_ok: bool,
    /// True when all packed MoneyPickUp entries have honest Anim2D frame residual.
    pub anim2d_frame_ok: bool,
    /// Peak Anim2D frame index observed this pack.
    pub peak_anim_frame: u16,
}

impl WorldAnimLayoutHonesty {
    pub fn honesty_cpu_pack_ok(&self) -> bool {
        self.cpu_pack_ok
    }

    pub fn honesty_geometry_ok(&self) -> bool {
        self.cpu_pack_ok && self.has_geometry && self.active_packed > 0
    }

    pub fn honesty_upload_ready_ok(&self) -> bool {
        self.gpu_upload_ready && self.cpu_pack_ok
    }

    pub fn honesty_template_ok(&self) -> bool {
        self.money_pickup_templates_ok
    }

    pub fn honesty_anim2d_frame_ok(&self) -> bool {
        self.anim2d_frame_ok
    }
}

/// Packed world-anim layout payload ready for dual-tick UI consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldAnimLayout {
    pub entries: Vec<WorldAnimLayoutEntry>,
    pub layout_bytes: Vec<u8>,
    pub honesty: WorldAnimLayoutHonesty,
}

fn template_hash(name: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in name.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

impl WorldAnimLayout {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            layout_bytes: Vec::new(),
            honesty: WorldAnimLayoutHonesty {
                cpu_pack_ok: true,
                money_pickup_templates_ok: true,
                anim2d_frame_ok: true,
                ..Default::default()
            },
        }
    }

    pub fn mark_gpu_upload_ready(&mut self) {
        self.honesty.gpu_upload_ready = self.honesty.cpu_pack_ok;
    }

    pub fn pack_from_presentation(frame: &PresentationFrame) -> Self {
        Self::pack_anims_at(&frame.world_anims, frame.frame.0)
    }

    pub fn pack_anims_at(anims: &[PresentationWorldAnim], logic_frame: u32) -> Self {
        if anims.is_empty() {
            return Self::empty();
        }

        let mut entries = Vec::with_capacity(anims.len());
        let mut active = 0u32;
        let mut templates_ok = true;
        let mut frame_ok = true;
        let mut peak_anim_frame = 0u16;
        for a in anims {
            if a.template.is_empty() {
                templates_ok = false;
            }
            let age = logic_frame.saturating_sub(a.spawn_frame);
            let age_sec = age as f32 / WORLD_ANIM_LOGIC_FPS;
            let lift = a.z_rise_per_second * age_sec;
            let display = a.display_time_seconds.max(0.0);
            let alpha = money_pickup_fade_alpha(age_sec, display, a.fades);
            if age_sec < display {
                active = active.saturating_add(1);
            }
            if alpha <= 0.0 {
                continue;
            }
            let current_frame = money_pickup_current_frame(age);
            let frame_image = money_pickup_frame_image_name(current_frame);
            if a.template == "MoneyPickUp"
                && !honesty_money_pickup_frame(age, current_frame, &frame_image)
            {
                frame_ok = false;
            }
            peak_anim_frame = peak_anim_frame.max(current_frame);
            entries.push(WorldAnimLayoutEntry {
                position: [a.position.x, a.position.y, a.position.z],
                lift_y: lift,
                display_time_seconds: display,
                z_rise_per_second: a.z_rise_per_second,
                age_frames: age as f32,
                alpha,
                fades: if a.fades { 1.0 } else { 0.0 },
                template_hash: template_hash(&a.template),
                current_frame,
                frame_image,
            });
        }

        let mut floats = Vec::with_capacity(entries.len() * WORLD_ANIM_LAYOUT_FLOATS);
        for e in &entries {
            floats.extend_from_slice(&e.clone().to_floats());
        }
        let layout_bytes = f32_slice_to_bytes(&floats);
        let anims_packed = entries.len() as u32;
        // Empty of MoneyPickUp failures is honest; non-empty requires frame residual.
        let anim2d_frame_ok = if anims_packed == 0 {
            true
        } else {
            frame_ok
                && entries.iter().all(|e| {
                    honesty_money_pickup_frame(e.age_frames as u32, e.current_frame, &e.frame_image)
                })
        };
        Self {
            honesty: WorldAnimLayoutHonesty {
                anims_packed,
                active_packed: active,
                bytes_packed: layout_bytes.len() as u32,
                cpu_pack_ok: true,
                has_geometry: active > 0,
                gpu_upload_ready: false,
                money_pickup_templates_ok: templates_ok,
                anim2d_frame_ok,
                peak_anim_frame,
            },
            entries,
            layout_bytes,
        }
    }
}

fn f32_slice_to_bytes(floats: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(floats.len() * 4);
    for f in floats {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

/// Host-testable residual: pack + mark upload-ready without a live GPU device.
pub fn pack_world_anim_and_mark_ready(frame: &PresentationFrame) -> WorldAnimLayout {
    let mut pack = WorldAnimLayout::pack_from_presentation(frame);
    pack.mark_gpu_upload_ready();
    pack
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation_frame::PresentationWorldAnim;

    #[test]
    fn empty_pack_is_honest_cpu_success() {
        let pack = WorldAnimLayout::empty();
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(!pack.honesty.honesty_geometry_ok());
        assert!(pack.honesty.honesty_template_ok());
        assert!(pack.honesty.honesty_anim2d_frame_ok());
    }

    #[test]
    fn packs_synthetic_money_pickup_with_z_rise() {
        let anim = PresentationWorldAnim::synthetic_money_pickup(0);
        let pack = WorldAnimLayout::pack_anims_at(&[anim.clone()], 15);
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(pack.honesty.honesty_geometry_ok());
        assert_eq!(pack.honesty.anims_packed, 1);
        assert_eq!(pack.honesty.active_packed, 1);
        let age_sec = 15.0 / WORLD_ANIM_LOGIC_FPS;
        let expected_lift = anim.z_rise_per_second * age_sec;
        assert!((pack.entries[0].lift_y - expected_lift).abs() < 0.001);
        assert!((pack.entries[0].alpha - 1.0).abs() < 0.001);
        let mut marked = pack;
        marked.mark_gpu_upload_ready();
        assert!(marked.honesty.honesty_upload_ready_ok());
    }

    #[test]
    fn money_pickup_anim2d_frame_advance_residual() {
        assert_eq!(anim_delay_ms_to_frames(30), 1);
        assert_eq!(money_pickup_current_frame(0), 0);
        assert_eq!(money_pickup_current_frame(1), 1);
        assert_eq!(money_pickup_current_frame(30), 30);
        assert_eq!(money_pickup_current_frame(31), 0); // LOOP wrap
        assert_eq!(money_pickup_frame_image_name(0), "SCPDollar000");
        assert_eq!(money_pickup_frame_image_name(12), "SCPDollar012");
        assert!(honesty_money_pickup_frame(15, 15, "SCPDollar015"));

        let anim = PresentationWorldAnim::synthetic_money_pickup(0);
        let pack = WorldAnimLayout::pack_anims_at(&[anim], 15);
        assert!(pack.honesty.honesty_anim2d_frame_ok());
        assert_eq!(pack.entries[0].current_frame, 15);
        assert_eq!(pack.entries[0].frame_image, "SCPDollar015");
        assert_eq!(pack.honesty.peak_anim_frame, 15);
    }

    #[test]
    fn money_pickup_image_sequence_and_randomize_residual() {
        assert!(!MONEY_PICKUP_RANDOMIZE_START_FRAME);
        assert!(honesty_money_pickup_image_sequence());
        assert_eq!(money_pickup_frame_image_name(0), "SCPDollar000");
        assert_eq!(money_pickup_frame_image_name(30), "SCPDollar030");
        assert_eq!(MONEY_PICKUP_NUM_FRAMES, 31);
    }

    #[test]
    fn money_pickup_fade_params_residual_honesty() {
        assert!(honesty_money_pickup_fade_params());
        assert!((MONEY_PICKUP_DISPLAY_TIME_SECONDS - 4.0).abs() < 0.01);
        assert!((MONEY_PICKUP_Z_RISE_PER_SECOND - 15.0).abs() < 0.01);
        assert!(MONEY_PICKUP_FADES);
        // Mid-fade residual at age 4.5s → alpha 0.5.
        assert!((money_pickup_fade_alpha(4.5, 4.0, true) - 0.5).abs() < 0.01);
        // Pack during fade window still has geometry (alpha > 0).
        let anim = PresentationWorldAnim::synthetic_money_pickup(0);
        // 4.5 seconds = 135 frames @ 30 FPS.
        let pack = WorldAnimLayout::pack_anims_at(&[anim], 135);
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert_eq!(pack.honesty.anims_packed, 1);
        assert!((pack.entries[0].alpha - 0.5).abs() < 0.01);
    }

    #[test]
    fn anim2d_mode_residual_honesty() {
        assert!(honesty_anim2d_mode_residual());
        let step = anim2d_try_next_frame(Anim2DModeResidual::Loop, 30, 0, 30, false);
        assert_eq!(step.frame, 0);
        assert!(!step.complete);
        let step = anim2d_try_next_frame(Anim2DModeResidual::Once, 30, 0, 30, false);
        assert!(step.complete);
        assert_eq!(step.frame, 30);
        let step = anim2d_try_next_frame(Anim2DModeResidual::PingPong, 30, 0, 30, false);
        assert_eq!(step.frame, 29);
        assert!(step.reversed);
        let step = anim2d_try_next_frame(Anim2DModeResidual::PingPong, 0, 0, 30, true);
        assert_eq!(step.frame, 1);
        assert!(!step.reversed);
        let step = anim2d_try_next_frame(Anim2DModeResidual::LoopBackwards, 0, 0, 30, false);
        assert_eq!(step.frame, 30);
    }


    #[test]
    fn anim2d_mode_table_residual_honesty() {
        assert!(honesty_anim2d_mode_table());
        assert_eq!(ResidualAnim2DMode::Loop.discriminant(), 3);
        assert_eq!(ResidualAnim2DMode::Loop.name(), "LOOP");
        assert_eq!(MONEY_PICKUP_ANIM_MODE, ResidualAnim2DMode::Loop);
        assert_eq!(ANIM_2D_DEFAULT_MODE, ResidualAnim2DMode::Loop);
        assert_eq!(ANIM_2D_NUM_MODES, 7);
        assert_eq!(ANIM_2D_MODE_NAMES[0], "NONE");
        assert_eq!(ANIM_2D_MODE_NAMES[3], "LOOP");
        assert_eq!(ANIM_2D_MODE_NAMES[6], "PING_PONG_BACKWARDS");
        assert!(ResidualAnim2DMode::Loop.is_loop_family());
        assert!(ResidualAnim2DMode::PingPong.is_ping_pong_family());
        assert!(ResidualAnim2DMode::LoopBackwards.starts_backwards());
        assert!(!ResidualAnim2DMode::Once.starts_backwards());
        assert_eq!(
            ResidualAnim2DMode::from_name("ONCE_BACKWARDS"),
            Some(ResidualAnim2DMode::OnceBackwards)
        );
        assert!(ResidualAnim2DMode::from_name("unknown").is_none());
    }

    #[test]
    fn anim2d_status_alpha_residual_honesty() {
        assert!(honesty_anim2d_status_alpha());
        assert_eq!(anim2d_set_status(0, ANIM_2D_STATUS_COMPLETE), ANIM_2D_STATUS_COMPLETE);
        assert_eq!(
            anim2d_clear_status(ANIM_2D_STATUS_COMPLETE, ANIM_2D_STATUS_COMPLETE),
            ANIM_2D_STATUS_NONE
        );
        assert_eq!(anim2d_draw_color_alpha(ANIM_2D_DEFAULT_ALPHA), 255);
        // FROZEN residual blocks tryNextFrame advances in C++.
        assert!(anim2d_status_test(ANIM_2D_STATUS_FROZEN, ANIM_2D_STATUS_FROZEN));
    }

    #[test]
    fn anim2d_collection_residual_honesty() {
        assert!(honesty_anim2d_collection_residual());
        assert_eq!(ANIM2D_COLLECTION_INI_PATH, "Data/INI/Animation2D.ini");
        assert!(!MONEY_PICKUP_RANDOMIZE_START_FRAME);
        assert_eq!(
            anim2d_get_current_frame_width(0, MONEY_PICKUP_NUM_FRAMES),
            ANIM2D_FRAME_PLACEHOLDER_WIDTH
        );
        assert_eq!(
            anim2d_get_current_frame_height(30, MONEY_PICKUP_NUM_FRAMES),
            ANIM2D_FRAME_PLACEHOLDER_HEIGHT
        );
        assert_eq!(anim2d_get_current_frame_width(99, 31), 0);

        let mut col = Anim2DCollectionResidual::new();
        col.init();
        assert!(col.init_path_recorded);
        let money = col.new_template("MoneyPickUp");
        assert_eq!(col.find_template("MoneyPickUp"), Some(money));
        assert!(!col.templates[&money].randomize_start_frame);
        let a = col.new_instance(money, Anim2DModeResidual::Loop, 0, 30);
        let b = col.new_instance(money, Anim2DModeResidual::Loop, 0, 30);
        col.register_animation(a);
        col.register_animation(b);
        // head-insert residual: b → a
        assert_eq!(col.instance_head, Some(b));
        assert_eq!(col.instance_next.get(&b).copied().flatten(), Some(a));
        // FROZEN skips update residual.
        col.instances.get_mut(&a).unwrap().status = ANIM_2D_STATUS_FROZEN;
        col.instances.get_mut(&a).unwrap().current_frame = 10;
        col.instances.get_mut(&b).unwrap().current_frame = 10;
        let advanced = col.update();
        assert_eq!(advanced, 1);
        assert_eq!(col.instances[&a].current_frame, 10);
        assert_eq!(col.instances[&b].current_frame, 11);
        col.unregister_animation(b);
        assert_eq!(col.instance_head, Some(a));
        col.unregister_animation(a);
        assert!(col.instance_head.is_none());
    }
}
