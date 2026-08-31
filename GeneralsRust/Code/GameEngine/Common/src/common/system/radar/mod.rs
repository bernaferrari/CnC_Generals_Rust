////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: radar.rs //////////////////////////////////////////////////////////////
// Radar system functionality
// Port from C++ Radar.cpp and Radar.h (Colin Day, January 2002)
///////////////////////////////////////////////////////////////////////////////

use crate::common::game_common::ObjectShroudStatus;
use crate::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};

mod draw_events;
mod map_source;
mod objects;
mod snapshot;
mod terrain;
mod try_event;
mod window;

#[cfg(test)]
mod tests;

pub use map_source::{RadarMapSource, register_radar_map_source};
pub use objects::{
    RadarDataSink, RadarObjectInsert, RadarObjectProvider, register_radar_data_sink,
    register_radar_object_provider, resolve_radar_object_color,
};
pub use snapshot::{ensure_the_radar_snapshot_block, register_the_radar_snapshot_block};
pub use terrain::{
    RadarBridgeSample, RadarTerrainPaintSource, register_radar_terrain_paint_source,
};
pub use window::{RadarWindowGeom, RadarWindowSource, register_radar_window_source};

/// Victim kind flags used by `tryUnderAttackEvent` / `tryInfiltrationEvent`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RadarVictimInfo {
    pub is_infantry: bool,
    pub is_vehicle: bool,
    pub is_harvester: bool,
    pub is_structure: bool,
    pub is_mp_count_for_victory: bool,
    pub is_local_player: bool,
    pub is_ally: bool,
    pub player_index: i32,
}

/// Client/W3D render EVA, UI, and audio for radar events.
pub trait RadarEventFeedback: Send + Sync {
    fn trigger_radar_attack_glow(&self);
    fn show_radar_message(&self, message_key: &str);
    fn play_radar_audio(&self, event_name: &str, player_index: i32);
    fn set_eva_should_play(&self, eva_name: &str);
}

static RADAR_FEEDBACK: OnceLock<Arc<dyn RadarEventFeedback>> = OnceLock::new();

pub fn register_radar_event_feedback(hook: Arc<dyn RadarEventFeedback>) -> bool {
    RADAR_FEEDBACK.set(hook).is_ok()
}

fn radar_feedback() -> Option<&'static dyn RadarEventFeedback> {
    RADAR_FEEDBACK.get().map(|hook| hook.as_ref())
}

/// Radar cell dimensions (matches C++ RADAR_CELL_WIDTH/HEIGHT)
/// Must be power of 2 for WW3D texture requirements
pub const RADAR_CELL_WIDTH: u32 = 128;
pub const RADAR_CELL_HEIGHT: u32 = 128;

/// Maximum radar events (matches C++ MAX_RADAR_EVENTS)
pub const MAX_RADAR_EVENTS: usize = 64;

/// W3D object overlay texture refresh cadence (matches `OVERLAY_REFRESH_RATE`).
pub const W3D_RADAR_OVERLAY_REFRESH_RATE: u32 = 6;

/// Radar queue terrain refresh delay (matches C++ RADAR_QUEUE_TERRAIN_REFRESH_DELAY)
/// 3 seconds worth of logic frames
pub const RADAR_QUEUE_TERRAIN_REFRESH_DELAY: u32 = 90; // 30 FPS * 3 seconds

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RadarTerrainSample {
    pub(crate) height: f32,
    pub(crate) is_water: bool,
}

/// Radar event types (matches C++ RadarEventType)
/// Determines colors for radar events for consistent visual scheme
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RadarEventType {
    Invalid = 0,
    Construction,
    Upgrade,
    UnderAttack,
    Information,
    BeaconPulse,
    Infiltration, // Defection, hijacking, hacking, carbomb, etc.
    BattlePlan,
    StealthDiscovered,  // We discovered a stealth unit
    StealthNeutralized, // Our stealth unit has been revealed
    Fake,               // Internal event, doesn't notify player (for spacebar jump)
}

impl RadarEventType {
    /// Get color pair for this event type (matches C++ radarColorLookupTable)
    pub fn get_colors(&self) -> (RGBAColorInt, RGBAColorInt) {
        match self {
            RadarEventType::Construction => (
                RGBAColorInt {
                    r: 128,
                    g: 128,
                    b: 255,
                    a: 255,
                },
                RGBAColorInt {
                    r: 128,
                    g: 255,
                    b: 255,
                    a: 255,
                },
            ),
            RadarEventType::Upgrade => (
                RGBAColorInt {
                    r: 128,
                    g: 0,
                    b: 64,
                    a: 255,
                },
                RGBAColorInt {
                    r: 255,
                    g: 185,
                    b: 220,
                    a: 255,
                },
            ),
            RadarEventType::UnderAttack => (
                RGBAColorInt {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
                RGBAColorInt {
                    r: 255,
                    g: 128,
                    b: 128,
                    a: 255,
                },
            ),
            RadarEventType::Information => (
                RGBAColorInt {
                    r: 255,
                    g: 255,
                    b: 0,
                    a: 255,
                },
                RGBAColorInt {
                    r: 255,
                    g: 255,
                    b: 128,
                    a: 255,
                },
            ),
            RadarEventType::BeaconPulse => (
                RGBAColorInt {
                    r: 255,
                    g: 255,
                    b: 0,
                    a: 255,
                },
                RGBAColorInt {
                    r: 255,
                    g: 255,
                    b: 128,
                    a: 255,
                },
            ),
            RadarEventType::Infiltration => (
                RGBAColorInt {
                    r: 0,
                    g: 255,
                    b: 255,
                    a: 255,
                },
                RGBAColorInt {
                    r: 128,
                    g: 255,
                    b: 255,
                    a: 255,
                },
            ),
            RadarEventType::BattlePlan => (
                RGBAColorInt {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
                RGBAColorInt {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
            ),
            RadarEventType::StealthDiscovered => (
                RGBAColorInt {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                },
                RGBAColorInt {
                    r: 0,
                    g: 128,
                    b: 0,
                    a: 255,
                },
            ),
            RadarEventType::StealthNeutralized => (
                RGBAColorInt {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                },
                RGBAColorInt {
                    r: 0,
                    g: 128,
                    b: 0,
                    a: 255,
                },
            ),
            RadarEventType::Fake => (
                RGBAColorInt {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                },
                RGBAColorInt {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                },
            ),
            RadarEventType::Invalid => (
                RGBAColorInt {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
                RGBAColorInt {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                },
            ),
        }
    }
}

/// RGBA color integer (matches C++ RGBAColorInt)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RGBAColorInt {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RGBAColorInt {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_u32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    pub fn from_u32(color: u32) -> Self {
        Self {
            a: ((color >> 24) & 0xFF) as u8,
            r: ((color >> 16) & 0xFF) as u8,
            g: ((color >> 8) & 0xFF) as u8,
            b: (color & 0xFF) as u8,
        }
    }
}

/// Radar priority types (matches C++ RadarPriorityType)
/// Determines drawing order and visibility on radar
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum RadarPriorityType {
    Invalid = 0,   // Not set, won't show on radar
    NotOnRadar,    // Specifically forbidden from radar
    Structure,     // Structure drawing priority
    Unit,          // Unit drawing priority
    LocalUnitOnly, // Unit priority, only if controlled by local player
}

impl RadarPriorityType {
    /// Check if this priority is visible on radar
    pub fn is_visible(&self) -> bool {
        !matches!(
            self,
            RadarPriorityType::Invalid | RadarPriorityType::NotOnRadar
        )
    }
}

/// Cell shroud status (matches C++ CellShroudStatus from GameCommon.h)
/// Determines visibility state for fog of war
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CellShroudStatus {
    Clear = 0, // Fully visible
    Fogged,    // Previously seen, now in fog of war
    Shrouded,  // Never seen, completely dark
}

impl CellShroudStatus {
    /// Check if cell is visible (not shrouded)
    pub fn is_visible(&self) -> bool {
        matches!(self, CellShroudStatus::Clear | CellShroudStatus::Fogged)
    }

    /// Check if cell has been explored (fogged or clear)
    pub fn is_explored(&self) -> bool {
        !matches!(self, CellShroudStatus::Shrouded)
    }
}

/// 2D integer coordinates (matches C++ ICoord2D)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl ICoord2D {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Check whether a radar coordinate lies inside the fixed C++ radar cell grid.
///
/// Matches `legalRadarPoint` in `W3DRadar.cpp`.
pub fn legal_radar_point(px: i32, py: i32) -> bool {
    px >= 0 && py >= 0 && px < RADAR_CELL_WIDTH as i32 && py < RADAR_CELL_HEIGHT as i32
}

/// Convert a radar-cell coordinate to a drawn radar pixel coordinate.
///
/// This intentionally preserves the C++ W3D radar Y inversion:
/// `((RADAR_CELL_HEIGHT - 1 - radar.y) * radarHeight / RADAR_CELL_HEIGHT) + upperLeftY`.
pub fn radar_to_pixel(
    radar: &ICoord2D,
    radar_upper_left_x: i32,
    radar_upper_left_y: i32,
    radar_width: i32,
    radar_height: i32,
) -> ICoord2D {
    ICoord2D {
        x: (radar.x * radar_width / RADAR_CELL_WIDTH as i32) + radar_upper_left_x,
        y: (((RADAR_CELL_HEIGHT as i32 - 1 - radar.y) * radar_height) / RADAR_CELL_HEIGHT as i32)
            + radar_upper_left_y,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarHeroReticleRect {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarViewBoxLine {
    pub start: ICoord2D,
    pub end: ICoord2D,
    pub start_color: RGBAColorInt,
    pub end_color: RGBAColorInt,
}

/// Compute the screen rectangle used to draw the radar without distorting map aspect ratio.
///
/// Matches `W3DRadar::draw`/`findDrawPositions`: the returned points are upper-left and
/// lower-right corners inside the requested radar window.
pub fn radar_draw_positions(
    start_x: i32,
    start_y: i32,
    width: i32,
    height: i32,
    extent: Region3D,
) -> (ICoord2D, ICoord2D) {
    if width <= 0 || height <= 0 || extent.width() <= 0.0 || extent.height() <= 0.0 {
        return (
            ICoord2D::new(start_x, start_y),
            ICoord2D::new(start_x + width.max(0), start_y + height.max(0)),
        );
    }

    let ratio_width = extent.width() / width as f32;
    let ratio_height = extent.height() / height as f32;
    let mut ul = ICoord2D::new(0, 0);
    let mut lr = ICoord2D::new(0, 0);

    if ratio_width >= ratio_height {
        let radar_x = extent.width() / ratio_width;
        let radar_y = extent.height() / ratio_width;
        ul.x = 0;
        ul.y = ((height as f32 - radar_y) / 2.0) as i32;
        lr.x = radar_x as i32;
        lr.y = height - ul.y;
    } else {
        let radar_x = extent.width() / ratio_height;
        let radar_y = extent.height() / ratio_height;
        ul.x = ((width as f32 - radar_x) / 2.0) as i32;
        ul.y = 0;
        lr.x = width - ul.x;
        lr.y = radar_y as i32;
    }

    ul.x += start_x;
    ul.y += start_y;
    lr.x += start_x;
    lr.y += start_y;
    (ul, lr)
}

/// Shade an RGB color by terrain height using the exact W3D radar interpolation constants.
///
/// Matches `W3DRadar::interpolateColorForHeight`: heights above `mid_z` move toward a
/// near-white target, heights below `mid_z` move toward a dark target, and degenerate
/// flat-map ranges are nudged before interpolation.
pub fn interpolate_color_for_height(
    mut color: [f32; 3],
    height: f32,
    mut hi_z: f32,
    mid_z: f32,
    mut lo_z: f32,
) -> [f32; 3] {
    const HOW_BRIGHT: f32 = 0.95;
    const HOW_DARK: f32 = 0.60;

    if hi_z == mid_z {
        hi_z = mid_z + 0.1;
    }
    if mid_z == lo_z {
        lo_z = mid_z - 0.1;
    }
    if hi_z == lo_z {
        hi_z = lo_z + 0.2;
    }

    let (t, target) = if height >= mid_z {
        (
            (height - mid_z) / (hi_z - mid_z),
            [
                color[0] + (1.0 - color[0]) * HOW_BRIGHT,
                color[1] + (1.0 - color[1]) * HOW_BRIGHT,
                color[2] + (1.0 - color[2]) * HOW_BRIGHT,
            ],
        )
    } else {
        (
            (mid_z - height) / (mid_z - lo_z),
            [
                color[0] + (0.0 - color[0]) * HOW_DARK,
                color[1] + (0.0 - color[1]) * HOW_DARK,
                color[2] + (0.0 - color[2]) * HOW_DARK,
            ],
        )
    };

    for channel in 0..3 {
        color[channel] = (color[channel] + (target[channel] - color[channel]) * t).clamp(0.0, 1.0);
    }
    color
}

/// W3D radar event marker sizing/spin variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarEventMarkerKind {
    Beacon,
    Generic,
}

/// Screen-space geometry and faded colors for one W3D radar event marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarEventMarker {
    pub points: [ICoord2D; 3],
    pub color1: RGBAColorInt,
    pub color2: RGBAColorInt,
    pub size: i32,
}

/// Build the C++ W3D rotating triangular radar event marker.
///
/// Matches `W3DRadar::drawSingleBeaconEvent` and `drawSingleGenericEvent` for
/// marker size, spin direction, radar-to-pixel conversion, and fade alpha.
pub fn radar_event_marker(
    event: &RadarEvent,
    current_frame: u32,
    pixel_x: i32,
    pixel_y: i32,
    width: i32,
    height: i32,
    kind: RadarEventMarkerKind,
) -> RadarEventMarker {
    const SHRINK_FRAMES: f32 = 30.0 * 1.5;
    const THIRD_TURN: f32 = std::f32::consts::TAU / 3.0;

    let frame_diff = current_frame.saturating_sub(event.create_frame) as f32;
    let max_event_size = match kind {
        RadarEventMarkerKind::Beacon => width as f32 / 10.0,
        RadarEventMarkerKind::Generic => width as f32 / 2.0,
    };
    let size = (max_event_size * (1.0 - frame_diff / SHRINK_FRAMES))
        .trunc()
        .max(6.0) as i32;
    let add_angle = match kind {
        RadarEventMarkerKind::Beacon => -std::f32::consts::TAU * (frame_diff / SHRINK_FRAMES),
        RadarEventMarkerKind::Generic => std::f32::consts::TAU * (frame_diff / SHRINK_FRAMES),
    };

    let points = [0.0, THIRD_TURN, -THIRD_TURN].map(|base_angle| {
        let angle = base_angle - add_angle;
        let radar_point = ICoord2D::new(
            (angle.cos() * size as f32 + event.radar_loc.x as f32).trunc() as i32,
            (angle.sin() * size as f32 + event.radar_loc.y as f32).trunc() as i32,
        );
        radar_to_pixel(&radar_point, pixel_x, pixel_y, width, height)
    });

    RadarEventMarker {
        points,
        color1: fade_event_color(
            event.color1,
            current_frame,
            event.fade_frame,
            event.die_frame,
        ),
        color2: fade_event_color(
            event.color2,
            current_frame,
            event.fade_frame,
            event.die_frame,
        ),
        size,
    }
}

fn fade_event_color(
    mut color: RGBAColorInt,
    current_frame: u32,
    fade_frame: u32,
    die_frame: u32,
) -> RGBAColorInt {
    if current_frame > fade_frame && die_frame > fade_frame {
        let fade_span = (die_frame - fade_frame) as f32;
        let fade_progress = (current_frame - fade_frame) as f32 / fade_span;
        let alpha = (color.a as f32 * (1.0 - fade_progress)).clamp(0.0, 255.0);
        color.a = alpha.trunc() as u8;
    }
    color
}

/// 3D coordinates (matches C++ Coord3D)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// 3D region (matches C++ Region3D)
#[derive(Debug, Clone, Copy)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

impl Region3D {
    pub fn width(&self) -> f32 {
        self.hi.x - self.lo.x
    }

    pub fn height(&self) -> f32 {
        self.hi.y - self.lo.y
    }
}

/// Radar event data (matches C++ Radar::RadarEvent)
#[derive(Debug, Clone)]
pub struct RadarEvent {
    pub event_type: RadarEventType,
    pub active: bool,
    pub create_frame: u32,
    pub die_frame: u32,
    pub fade_frame: u32,
    pub color1: RGBAColorInt,
    pub color2: RGBAColorInt,
    pub world_loc: Coord3D,
    pub radar_loc: ICoord2D,
    pub sound_played: bool,
}

impl Default for RadarEvent {
    fn default() -> Self {
        Self {
            event_type: RadarEventType::Invalid,
            active: false,
            create_frame: 0,
            die_frame: 0,
            fade_frame: 0,
            color1: RGBAColorInt::new(0, 0, 0, 0),
            color2: RGBAColorInt::new(0, 0, 0, 0),
            world_loc: Coord3D::new(0.0, 0.0, 0.0),
            radar_loc: ICoord2D::new(0, 0),
            sound_played: false,
        }
    }
}

/// Radar scan area (temporary vision reveal from special power)
#[derive(Debug, Clone)]
pub struct RadarScan {
    pub world_location: Coord3D,
    pub radius: f32,
    pub expires_frame: u32,
    pub player_id: u32,
}

impl RadarScan {
    pub fn new(
        location: Coord3D,
        radius: f32,
        duration_frames: u32,
        player_id: u32,
        current_frame: u32,
    ) -> Self {
        Self {
            world_location: location,
            radius,
            expires_frame: current_frame + duration_frames,
            player_id,
        }
    }

    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn contains_position(&self, position: &Coord3D) -> bool {
        let dx = position.x - self.world_location.x;
        let dy = position.y - self.world_location.y;
        let dist_sq = dx * dx + dy * dy;
        dist_sq <= self.radius * self.radius
    }
}

/// Radar jamming source (prevents radar detection in an area)
#[derive(Debug, Clone)]
pub struct JammingSource {
    pub object_id: u32,
    pub world_location: Coord3D,
    pub jamming_radius: f32,
    pub player_id: u32,
    pub is_active: bool,
}

impl JammingSource {
    pub fn new(object_id: u32, location: Coord3D, radius: f32, player_id: u32) -> Self {
        Self {
            object_id,
            world_location: location,
            jamming_radius: radius,
            player_id,
            is_active: true,
        }
    }

    pub fn is_position_jammed(&self, position: &Coord3D) -> bool {
        if !self.is_active {
            return false;
        }
        let dx = position.x - self.world_location.x;
        let dy = position.y - self.world_location.y;
        let dist_sq = dx * dx + dy * dy;
        dist_sq <= self.jamming_radius * self.jamming_radius
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    pub fn update_position(&mut self, location: Coord3D) {
        self.world_location = location;
    }
}

/// Radar object information (matches C++ RadarObject)
#[derive(Debug, Clone)]
pub struct RadarObject {
    pub object_id: u32,
    pub color: u32,
    pub world_pos: Coord3D,
    pub radar_pos: ICoord2D,
    pub priority: RadarPriorityType,
    pub is_local: bool,
    pub is_stealth: bool,
    pub is_detected: bool,
    pub is_disguised: bool,
    pub is_enemy: bool,
    pub is_jammed: bool,
    pub stealth_revealed: bool,   // For stealth detection radar
    pub radar_range: f32,         // Radar detection range (0 = no radar capability)
    pub can_detect_stealth: bool, // Can this radar detect stealth units
    pub is_radar_provider: bool,  // This object provides radar coverage
    pub is_powered: bool,         // Is this radar powered on (for power-dependent radars)
    pub is_disabled: bool,        // Is this radar disabled (by EMP, power loss, etc.)
    pub is_hero: bool,            // Draw HeroReticle in W3D radar icon layer
    pub drawable_hidden: bool,    // C++ Drawable::m_hidden
    pub hidden_by_stealth: bool,  // C++ Drawable::m_hiddenByStealth
    /// C++ `Object::getShroudedStatus`. `Invalid` falls back to cell Clear.
    pub object_shroud: ObjectShroudStatus,
}

impl RadarObject {
    pub fn new(object_id: u32) -> Self {
        Self {
            object_id,
            color: 0xFFFFFFFF,
            world_pos: Coord3D::new(0.0, 0.0, 0.0),
            radar_pos: ICoord2D::new(0, 0),
            priority: RadarPriorityType::Invalid,
            is_local: false,
            is_stealth: false,
            is_detected: false,
            is_disguised: false,
            is_enemy: false,
            is_jammed: false,
            stealth_revealed: false,
            radar_range: 0.0,
            can_detect_stealth: false,
            is_radar_provider: false,
            is_powered: true,
            is_disabled: false,
            is_hero: false,
            drawable_hidden: false,
            hidden_by_stealth: false,
            object_shroud: ObjectShroudStatus::Invalid,
        }
    }

    /// Matches C++ `RadarObject::isTemporarilyHidden` (Radar.cpp:118-125).
    /// Hidden when `STEALTHLOOK_INVISIBLE` (enemy + stealthed + undetected
    /// + undisguised) **or** `Drawable::isDrawableEffectivelyHidden`
    /// (`m_hidden || m_hiddenByStealth`). Own stealth (`VISIBLE_FRIENDLY`)
    /// and DETECTED / `DISGUISED_ENEMY` still blip unless the drawable is
    /// script-hidden / hijacker-hidden.
    pub fn is_temporarily_hidden(&self) -> bool {
        if self.drawable_hidden || self.hidden_by_stealth {
            return true;
        }
        self.is_stealth
            && self.is_enemy
            && !self.is_detected
            && !self.is_disguised
            && !self.stealth_revealed
    }

    /// Check if this radar provider is operational
    /// Radar must be powered and not disabled to provide coverage
    pub fn is_radar_operational(&self) -> bool {
        self.is_radar_provider && self.is_powered && !self.is_disabled
    }

    /// Disable this radar (e.g., from EMP effect)
    pub fn disable_radar(&mut self) {
        self.is_disabled = true;
    }

    /// Enable this radar (e.g., EMP effect expired)
    pub fn enable_radar(&mut self) {
        self.is_disabled = false;
    }

    /// Set power state for this radar
    pub fn set_powered(&mut self, powered: bool) {
        self.is_powered = powered;
    }
}

fn radar_object_blip_cells(radar_point: ICoord2D) -> [ICoord2D; 4] {
    [
        radar_point,
        ICoord2D::new(radar_point.x, radar_point.y + 1),
        ICoord2D::new(radar_point.x + 1, radar_point.y + 1),
        ICoord2D::new(radar_point.x + 1, radar_point.y),
    ]
}

fn argb_to_rgba_bytes(color: u32) -> [u8; 4] {
    [
        ((color >> 16) & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        (color & 0xFF) as u8,
        ((color >> 24) & 0xFF) as u8,
    ]
}

fn radar_stealth_blip_alpha(current_frame: u32) -> u8 {
    const FRAMES_FOR_TRANSITION: u32 = 30;
    const MIN_ALPHA: f32 = 32.0;

    let alpha_scale =
        (current_frame % FRAMES_FOR_TRANSITION) as f32 / (FRAMES_FOR_TRANSITION as f32 / 2.0);
    let alpha = if alpha_scale > 0.0 {
        ((alpha_scale - 1.0) * (255.0 - MIN_ALPHA)) + MIN_ALPHA
    } else {
        (alpha_scale * (255.0 - MIN_ALPHA)) + MIN_ALPHA
    };

    alpha.clamp(0.0, 255.0).trunc() as u8
}

pub fn should_refresh_w3d_object_overlay(current_frame: u32) -> bool {
    current_frame % W3D_RADAR_OVERLAY_REFRESH_RATE == 0
}

fn radar_view_box_top_color() -> RGBAColorInt {
    RGBAColorInt::new(225, 225, 0, 255)
}

fn radar_view_box_bottom_color() -> RGBAColorInt {
    RGBAColorInt::new(158, 158, 0, 255)
}

fn clip_line_to_rect(
    mut start: ICoord2D,
    mut end: ICoord2D,
    clip_x: i32,
    clip_y: i32,
    clip_width: i32,
    clip_height: i32,
) -> Option<(ICoord2D, ICoord2D)> {
    const LEFT: u8 = 1;
    const RIGHT: u8 = 2;
    const BOTTOM: u8 = 4;
    const TOP: u8 = 8;

    if clip_width <= 0 || clip_height <= 0 {
        return None;
    }

    let x_min = clip_x;
    let y_min = clip_y;
    let x_max = clip_x + clip_width;
    let y_max = clip_y + clip_height;

    let compute_code = |point: ICoord2D| -> u8 {
        let mut code = 0;
        if point.x < x_min {
            code |= LEFT;
        } else if point.x > x_max {
            code |= RIGHT;
        }
        if point.y < y_min {
            code |= TOP;
        } else if point.y > y_max {
            code |= BOTTOM;
        }
        code
    };

    let mut start_code = compute_code(start);
    let mut end_code = compute_code(end);

    loop {
        if start_code | end_code == 0 {
            return Some((start, end));
        }
        if start_code & end_code != 0 {
            return None;
        }

        let out_code = if start_code != 0 {
            start_code
        } else {
            end_code
        };
        let dx = (end.x - start.x) as f32;
        let dy = (end.y - start.y) as f32;
        let mut x = 0.0;
        let mut y = 0.0;

        if out_code & TOP != 0 {
            if dy.abs() <= f32::EPSILON {
                return None;
            }
            x = start.x as f32 + dx * (y_min - start.y) as f32 / dy;
            y = y_min as f32;
        } else if out_code & BOTTOM != 0 {
            if dy.abs() <= f32::EPSILON {
                return None;
            }
            x = start.x as f32 + dx * (y_max - start.y) as f32 / dy;
            y = y_max as f32;
        } else if out_code & RIGHT != 0 {
            if dx.abs() <= f32::EPSILON {
                return None;
            }
            y = start.y as f32 + dy * (x_max - start.x) as f32 / dx;
            x = x_max as f32;
        } else if out_code & LEFT != 0 {
            if dx.abs() <= f32::EPSILON {
                return None;
            }
            y = start.y as f32 + dy * (x_min - start.x) as f32 / dx;
            x = x_min as f32;
        }

        let clipped = ICoord2D::new(x.trunc() as i32, y.trunc() as i32);
        if out_code == start_code {
            start = clipped;
            start_code = compute_code(start);
        } else {
            end = clipped;
            end_code = compute_code(end);
        }
    }
}

/// Radar system manager (matches C++ Radar class)
pub struct RadarSystem {
    /// Map extents for coordinate conversion
    map_extent: Region3D,

    /// Sampling intervals for world to radar conversion
    x_sample: f32,
    y_sample: f32,

    /// Average terrain height
    terrain_average_z: f32,

    /// Average water height
    water_average_z: f32,

    /// Radar objects sorted by priority (regular list)
    object_list: Vec<RadarObject>,

    /// Radar objects for local player only
    local_object_list: Vec<RadarObject>,

    /// Radar events array
    events: [RadarEvent; MAX_RADAR_EVENTS],

    /// Next free event index (circular buffer)
    next_free_event: usize,

    /// Last event index (for spacebar jump)
    last_event: Option<usize>,

    /// Is radar hidden
    radar_hidden: bool,

    /// Force radar on regardless of player state
    radar_force_on: bool,

    /// Frame to refresh terrain
    queue_terrain_refresh_frame: Option<u32>,

    /// Current frame counter
    current_frame: u32,

    /// Terrain texture data (RGBA8)
    terrain_texture: Vec<u8>,

    /// Optional per-cell terrain samples for terrain-texture generation.
    terrain_samples: Vec<RadarTerrainSample>,

    /// Is terrain texture dirty
    terrain_dirty: bool,

    /// Shroud status grid (matches C++ shroud system)
    /// Indexed as [y * RADAR_CELL_WIDTH + x]
    shroud_grid: Vec<CellShroudStatus>,

    /// Has shroud been cleared (for scenarios without fog of war)
    shroud_cleared: bool,

    /// GPS satellite active (reveals all units/buildings for a duration)
    /// Frame when GPS expires (0 = not active)
    gps_active_until_frame: u32,

    /// Radar scan active (reveals area around location for a duration)
    /// List of active radar scans
    radar_scans: Vec<RadarScan>,

    /// Radar jamming sources (objects creating jamming fields)
    jamming_sources: Vec<JammingSource>,

    /// C++ `m_radarWindow` — bound `ControlBar.wnd:LeftHUD` rectangle.
    radar_window: Option<window::RadarWindowGeom>,
    /// C++ `ThePlayerList->getLocalPlayer()->isPlayerActive()`.
    local_player_active: bool,
    /// C++ `Player::hasRadar()` for the local (or observed) player.
    local_has_radar: bool,
}

impl RadarSystem {
    fn terrain_height_for_cell(&self, x: i32, y: i32) -> f32 {
        if self.terrain_samples.len() == (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize {
            let ux = x.clamp(0, (RADAR_CELL_WIDTH - 1) as i32) as u32;
            let uy = y.clamp(0, (RADAR_CELL_HEIGHT - 1) as i32) as u32;
            let idx = (uy * RADAR_CELL_WIDTH + ux) as usize;
            return self.terrain_samples[idx].height;
        }
        self.terrain_average_z
    }

    fn world_to_radar_unclamped(&self, world: &Coord3D) -> Option<ICoord2D> {
        if self.x_sample <= f32::EPSILON || self.y_sample <= f32::EPSILON {
            return None;
        }

        Some(ICoord2D {
            x: ((world.x - self.map_extent.lo.x) / self.x_sample) as i32,
            y: ((world.y - self.map_extent.lo.y) / self.y_sample) as i32,
        })
    }

    /// Create new radar system
    pub fn new() -> Self {
        let grid_size = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        Self {
            map_extent: Region3D {
                lo: Coord3D::new(0.0, 0.0, 0.0),
                hi: Coord3D::new(0.0, 0.0, 0.0),
            },
            x_sample: 0.0,
            y_sample: 0.0,
            terrain_average_z: 0.0,
            water_average_z: 0.0,
            object_list: Vec::new(),
            local_object_list: Vec::new(),
            events: std::array::from_fn(|_| RadarEvent::default()),
            next_free_event: 0,
            last_event: None,
            radar_hidden: false,
            radar_force_on: false,
            queue_terrain_refresh_frame: None,
            current_frame: 0,
            terrain_texture: vec![0; (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT * 4) as usize],
            terrain_samples: Vec::new(),
            terrain_dirty: true,
            shroud_grid: vec![CellShroudStatus::Shrouded; grid_size],
            shroud_cleared: false,
            gps_active_until_frame: 0,
            radar_scans: Vec::new(),
            jamming_sources: Vec::new(),
            radar_window: None,
            local_player_active: true,
            local_has_radar: false,
        }
    }

    /// Reset radar data (matches C++ Radar::reset)
    pub fn reset(&mut self) {
        self.object_list.clear();
        self.local_object_list.clear();
        self.clear_all_events();
        self.radar_force_on = false;
        self.terrain_dirty = true;
    }

    /// Clear all radar events (matches C++ Radar::clearAllEvents)
    pub fn clear_all_events(&mut self) {
        self.next_free_event = 0;
        self.last_event = None;

        for event in &mut self.events {
            *event = RadarEvent::default();
        }
    }

    /// Update radar per frame (matches C++ Radar::update)
    pub fn update(&mut self, current_frame: u32) {
        snapshot::ensure_the_radar_snapshot_block();
        self.current_frame = current_frame;
        if self.radar_window.is_none() {
            self.bind_left_hud_window();
        }
        if !self.has_map_extent() {
            let _ = self.try_new_map_from_source();
        }

        // Update events - check if any should die
        for event in &mut self.events {
            if event.active && event.create_frame > 0 && current_frame > event.die_frame {
                event.active = false;
            }
        }

        // Check for queued terrain refresh
        if let Some(refresh_frame) = self.queue_terrain_refresh_frame {
            if current_frame.saturating_sub(refresh_frame) > RADAR_QUEUE_TERRAIN_REFRESH_DELAY {
                self.refresh_terrain();
            }
        }

        // Update GPS satellite (deactivate if expired)
        if self.is_gps_active() && current_frame >= self.gps_active_until_frame {
            self.deactivate_gps_satellite();
        }

        // Update radar scans (remove expired ones)
        self.update_radar_scans();

        // Update jamming status for all objects
        self.update_jamming_status();

        // Update stealth detection
        self.update_stealth_detection();
        self.sync_objects_from_provider();
    }

    /// Current logic frame used for radar animation and event expiry.
    pub fn current_frame(&self) -> u32 {
        self.current_frame
    }

    /// Current map extent used for radar/world coordinate conversion.
    pub fn map_extent(&self) -> Region3D {
        self.map_extent
    }

    pub fn terrain_average_z(&self) -> f32 {
        self.terrain_average_z
    }

    /// Initialize radar for new map (matches C++ Radar::newMap)
    pub fn new_map(
        &mut self,
        map_min: Coord3D,
        map_max: Coord3D,
        terrain_heights: &[(f32, f32, bool)],
    ) {
        // C++ Radar::newMap: NAMEKEY LeftHUD then reset + sample.
        self.bind_left_hud_window();
        self.reset();

        self.map_extent = Region3D {
            lo: map_min,
            hi: map_max,
        };

        // Calculate sampling intervals
        self.x_sample = self.map_extent.width() / RADAR_CELL_WIDTH as f32;
        self.y_sample = self.map_extent.height() / RADAR_CELL_HEIGHT as f32;

        // C++ averages every other radar cell (`y+=2`, `x+=2`).
        let mut terrain_sum = 0.0;
        let mut water_sum = 0.0;
        let mut terrain_count = 0;
        let mut water_count = 0;
        let expected_samples = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        let full_grid = terrain_heights.len() == expected_samples;

        if full_grid {
            for y in (0..RADAR_CELL_HEIGHT).step_by(2) {
                for x in (0..RADAR_CELL_WIDTH).step_by(2) {
                    let (_, z, is_water) = terrain_heights[(y * RADAR_CELL_WIDTH + x) as usize];
                    if is_water {
                        water_sum += z;
                        water_count += 1;
                    } else {
                        terrain_sum += z;
                        terrain_count += 1;
                    }
                }
            }
        } else {
            for &(_x, z, is_water) in terrain_heights {
                if is_water {
                    water_sum += z;
                    water_count += 1;
                } else {
                    terrain_sum += z;
                    terrain_count += 1;
                }
            }
        }

        self.terrain_average_z = if terrain_count > 0 {
            terrain_sum / terrain_count as f32
        } else {
            0.0
        };

        self.water_average_z = if water_count > 0 {
            water_sum / water_count as f32
        } else {
            0.0
        };

        self.terrain_samples.clear();
        if full_grid {
            self.terrain_samples.reserve(expected_samples);
            for &(_x, z, is_water) in terrain_heights {
                self.terrain_samples.push(RadarTerrainSample {
                    height: z,
                    is_water,
                });
            }
        }

        self.terrain_dirty = true;
        self.refresh_terrain();
        snapshot::ensure_the_radar_snapshot_block();
    }

    /// Add object to radar (matches C++ Radar::addObject)
    pub fn add_object(&mut self, mut radar_obj: RadarObject) {
        // Check if priority is visible
        if !radar_obj.priority.is_visible() {
            return;
        }

        // Convert world position to radar position
        if let Some(radar_pos) = self.world_to_radar(&radar_obj.world_pos) {
            radar_obj.radar_pos = radar_pos;
        }

        // Choose which list to add to
        let list = if radar_obj.is_local {
            &mut self.local_object_list
        } else {
            &mut self.object_list
        };

        // C++ Radar.cpp:457-507 inserts at the *head* of the matching priority
        // section (`currPriority >= newPriority`), so newer same-priority
        // objects paint first and later list entries overwrite them.
        let insert_pos = list
            .iter()
            .position(|obj| obj.priority >= radar_obj.priority)
            .unwrap_or(list.len());

        list.insert(insert_pos, radar_obj.clone());
    }

    /// Remove object from radar (matches C++ Radar::removeObject)
    pub fn remove_object(&mut self, object_id: u32) -> bool {
        if let Some(index) = self
            .local_object_list
            .iter()
            .position(|obj| obj.object_id == object_id)
        {
            self.local_object_list.remove(index);
            return true;
        }

        if let Some(index) = self
            .object_list
            .iter()
            .position(|obj| obj.object_id == object_id)
        {
            self.object_list.remove(index);
            return true;
        }

        false
    }

    /// Translate radar coordinates to world coordinates (matches C++ Radar::radarToWorld)
    pub fn radar_to_world(&self, radar: &ICoord2D) -> Option<Coord3D> {
        let x = radar.x.clamp(0, (RADAR_CELL_WIDTH - 1) as i32);
        let y = radar.y.clamp(0, (RADAR_CELL_HEIGHT - 1) as i32);

        Some(Coord3D {
            x: self.map_extent.lo.x + x as f32 * self.x_sample,
            y: self.map_extent.lo.y + y as f32 * self.y_sample,
            z: self.terrain_height_for_cell(x, y),
        })
    }

    /// Translate world coordinates to radar coordinates (matches C++ Radar::worldToRadar)
    pub fn world_to_radar(&self, world: &Coord3D) -> Option<ICoord2D> {
        if self.x_sample <= f32::EPSILON || self.y_sample <= f32::EPSILON {
            return None;
        }

        let mut x = ((world.x - self.map_extent.lo.x) / self.x_sample) as i32;
        let mut y = ((world.y - self.map_extent.lo.y) / self.y_sample) as i32;

        // Clamp to radar bounds
        x = x.clamp(0, (RADAR_CELL_WIDTH - 1) as i32);
        y = y.clamp(0, (RADAR_CELL_HEIGHT - 1) as i32);

        Some(ICoord2D { x, y })
    }

    /// Create radar event (matches C++ Radar::createEvent)
    pub fn create_event(
        &mut self,
        world_loc: &Coord3D,
        event_type: RadarEventType,
        seconds_to_live: f32,
    ) {
        let (color1, color2) = event_type.get_colors();
        self.internal_create_event(world_loc, event_type, seconds_to_live, color1, color2);
    }

    /// Create radar event with player colors (matches C++ Radar::createPlayerEvent)
    pub fn create_player_event(
        &mut self,
        player_color: u32,
        world_loc: &Coord3D,
        event_type: RadarEventType,
        seconds_to_live: f32,
    ) {
        let color1 = RGBAColorInt::from_u32(player_color);

        // Create darker version for color2
        let dark_scale = 0.75;
        let color2 = RGBAColorInt {
            r: (color1.r as f32 * (1.0 - dark_scale)) as u8,
            g: (color1.g as f32 * (1.0 - dark_scale)) as u8,
            b: (color1.b as f32 * (1.0 - dark_scale)) as u8,
            a: color1.a,
        };

        self.internal_create_event(world_loc, event_type, seconds_to_live, color1, color2);
    }

    /// Internal method to create radar event (matches C++ Radar::internalCreateEvent)
    fn internal_create_event(
        &mut self,
        world_loc: &Coord3D,
        event_type: RadarEventType,
        seconds_to_live: f32,
        color1: RGBAColorInt,
        color2: RGBAColorInt,
    ) {
        const FADE_BEFORE_DIE_SECONDS: f32 = 0.5;
        const FRAMES_PER_SECOND: u32 = 30;

        let radar_loc = self
            .world_to_radar(world_loc)
            .unwrap_or(ICoord2D::new(0, 0));

        let event = &mut self.events[self.next_free_event];
        event.event_type = event_type;
        event.active = true;
        event.create_frame = self.current_frame;
        event.die_frame = self.current_frame + (FRAMES_PER_SECOND as f32 * seconds_to_live) as u32;
        event.fade_frame =
            event.die_frame - (FRAMES_PER_SECOND as f32 * FADE_BEFORE_DIE_SECONDS) as u32;
        event.color1 = color1;
        event.color2 = color2;
        event.world_loc = *world_loc;
        event.radar_loc = radar_loc;
        event.sound_played = false;

        // Record last event (except beacon pulses)
        if event_type != RadarEventType::BeaconPulse {
            self.last_event = Some(self.next_free_event);
        }

        // Advance circular buffer
        self.next_free_event = (self.next_free_event + 1) % MAX_RADAR_EVENTS;
    }

    /// C++ `Radar::tryEvent` — type + 10s map-wide (250² check is a no-op).
    pub fn try_event(&mut self, event_type: RadarEventType, world_loc: &Coord3D) -> bool {
        if matches!(event_type, RadarEventType::Invalid) {
            return false;
        }
        if try_event::should_suppress_event(&self.events, event_type, world_loc, self.current_frame)
        {
            return false;
        }
        self.create_event(world_loc, event_type, 4.0);
        true
    }


    /// Object-aware under-attack event: ping + ControlBar glow + UI/audio/EVA.
    pub fn try_under_attack_event_for(
        &mut self,
        world_loc: &Coord3D,
        victim: Option<&RadarVictimInfo>,
    ) -> bool {
        if !self.try_event(RadarEventType::UnderAttack, world_loc) {
            return false;
        }

        if let Some(fb) = radar_feedback() {
            fb.trigger_radar_attack_glow();
            // C++ Radar.cpp:1168 — audio always attributes to the LOCAL player.
            // Every live call site gates victim == local (Object.cpp:1853), so
            // the victim index carried here IS the local index.
            let player_index = victim.map(|v| v.player_index).unwrap_or(-1);
            match victim {
                Some(v) if v.is_infantry || v.is_vehicle => {
                    if v.is_harvester {
                        fb.show_radar_message("RADAR:HarvesterUnderAttack");
                        fb.play_radar_audio("RadarHarvesterUnderAttackSound", player_index);
                    } else {
                        fb.show_radar_message("RADAR:UnitUnderAttack");
                        fb.play_radar_audio("RadarStructureUnderAttackSound", player_index);
                    }
                }
                Some(v) if v.is_structure && v.is_mp_count_for_victory => {
                    if v.is_local_player {
                        fb.set_eva_should_play("EVA_BaseUnderAttack");
                    } else if v.is_ally {
                        fb.set_eva_should_play("EVA_AllyUnderAttack");
                    }
                    fb.show_radar_message("RADAR:StructureUnderAttack");
                    fb.play_radar_audio("RadarStructureUnderAttackSound", player_index);
                }
                _ => {
                    fb.show_radar_message("RADAR:UnderAttack");
                    fb.play_radar_audio("RadarStructureUnderAttackSound", player_index);
                }
            }
        }
        true
    }

    /// Try to create infiltration event (matches C++ Radar::tryInfiltrationEvent).
    ///
    /// C++ always has an `Object*` and returns immediately unless the victim
    /// is the local player. A location-only call has no victim identity, so
    /// it must not fail-open and warn the local player of AI-vs-AI hijacks.
    pub fn try_infiltration_event(&mut self, world_loc: &Coord3D) {
        let _ = world_loc;
    }

    /// Object-aware infiltration: only the local victim gets ping + UI + audio.
    pub fn try_infiltration_event_for(
        &mut self,
        world_loc: &Coord3D,
        victim: Option<&RadarVictimInfo>,
    ) {
        let Some(v) = victim else {
            return;
        };
        if !v.is_local_player {
            return;
        }
        self.create_event(world_loc, RadarEventType::Infiltration, 4.0);
        if let Some(fb) = radar_feedback() {
            fb.show_radar_message("RADAR:Infiltration");
            fb.play_radar_audio("RadarInfiltrationSound", v.player_index);
        }
    }

    /// Get last event location (matches C++ Radar::getLastEventLoc)
    pub fn get_last_event_loc(&self) -> Option<Coord3D> {
        self.last_event.map(|idx| self.events[idx].world_loc)
    }

    /// Refresh terrain texture (matches C++ Radar::refreshTerrain / W3DRadar::buildTerrainTexture).
    /// Re-samples the registered map source so water/height/bridge changes repaint.
    pub fn refresh_terrain(&mut self) {
        let _ = self.resample_terrain_from_source();
        self.build_terrain_texture_cpp();
    }

    /// Queue terrain refresh (matches C++ Radar::queueTerrainRefresh)
    pub fn queue_terrain_refresh(&mut self) {
        self.queue_terrain_refresh_frame = Some(self.current_frame);
    }

    /// Get terrain texture data
    pub fn get_terrain_texture(&self) -> &[u8] {
        &self.terrain_texture
    }

    /// Clear the cached W3D terrain texture surface.
    ///
    /// C++ `W3DRadar::reset` clears the terrain surface without releasing the texture
    /// resource. Keep this separate from `reset` because the base `Radar` reset does not
    /// own device surfaces.
    pub fn clear_terrain_texture_rgba(&mut self) {
        self.terrain_texture.fill(0);
    }

    /// Check if terrain texture needs refresh
    pub fn is_terrain_dirty(&self) -> bool {
        self.terrain_dirty
    }

    /// Hide/show radar
    pub fn hide(&mut self, hidden: bool) {
        self.radar_hidden = hidden;
    }

    /// Check if radar is hidden
    pub fn is_radar_hidden(&self) -> bool {
        self.radar_hidden
    }

    /// Force radar on/off
    pub fn force_on(&mut self, force: bool) {
        self.radar_force_on = force;
    }

    /// Check if radar is forced on
    pub fn is_radar_forced(&self) -> bool {
        self.radar_force_on
    }
    /// C++ `Player::isPlayerActive` — observers / defeated locals see LOCAL_UNIT_ONLY.
    pub fn set_local_player_active(&mut self, active: bool) {
        self.local_player_active = active;
    }

    #[must_use]
    pub fn local_player_active(&self) -> bool {
        self.local_player_active
    }

    /// C++ `Player::hasRadar()` stamped from the live host each radar update.
    pub fn set_local_has_radar(&mut self, has_radar: bool) {
        self.local_has_radar = has_radar;
    }

    #[must_use]
    pub fn local_has_radar(&self) -> bool {
        self.local_has_radar
    }

    /// C++ `W3DRadar::draw` / LeftHUD: forced, or not hidden and local has radar.
    #[must_use]
    pub fn is_radar_shown(&self) -> bool {
        self.radar_force_on || (!self.radar_hidden && self.local_has_radar)
    }

    /// Stamp C++ object shroud onto overlay blips (`getShroudedStatus`).
    pub fn apply_object_shrouds<F>(&mut self, mut lookup: F)
    where
        F: FnMut(u32) -> Option<ObjectShroudStatus>,
    {
        for obj in self
            .object_list
            .iter_mut()
            .chain(self.local_object_list.iter_mut())
        {
            if let Some(status) = lookup(obj.object_id) {
                obj.object_shroud = status;
            }
        }
    }

    /// C++ `Radar::xfer` hidden/force-on + event ring.
    pub fn snapshot_persist_state(
        &self,
    ) -> (
        bool,
        bool,
        [RadarEvent; MAX_RADAR_EVENTS],
        usize,
        Option<usize>,
    ) {
        (
            self.radar_hidden,
            self.radar_force_on,
            self.events.clone(),
            self.next_free_event,
            self.last_event,
        )
    }

    pub fn restore_persist_state(
        &mut self,
        hidden: bool,
        forced: bool,
        events: [RadarEvent; MAX_RADAR_EVENTS],
        next_free: usize,
        last: Option<usize>,
    ) {
        self.radar_hidden = hidden;
        self.radar_force_on = forced;
        self.events = events;
        self.next_free_event = next_free.min(MAX_RADAR_EVENTS.saturating_sub(1));
        self.last_event = last.filter(|idx| *idx < MAX_RADAR_EVENTS);
    }

    /// Get drawable active events (C++ `drawEvents` skips `RADAR_EVENT_FAKE`).
    pub fn get_active_events(&self) -> Vec<&RadarEvent> {
        self.drawable_events()
    }

    /// Get all radar objects
    pub fn get_all_objects(&self) -> impl Iterator<Item = &RadarObject> {
        self.local_object_list.iter().chain(self.object_list.iter())
    }

    /// Get radar objects by priority
    pub fn get_objects_by_priority(&self, priority: RadarPriorityType) -> Vec<&RadarObject> {
        self.get_all_objects()
            .filter(|obj| obj.priority == priority)
            .collect()
    }

    /// Find object under radar pixel (matches C++ Radar::objectUnderRadarPixel)
    pub fn object_under_radar_pixel(&self, radar_pos: &ICoord2D) -> Option<u32> {
        // Search local objects first
        for obj in &self.local_object_list {
            if (obj.radar_pos.x - radar_pos.x).abs() <= 1
                && (obj.radar_pos.y - radar_pos.y).abs() <= 1
            {
                return Some(obj.object_id);
            }
        }

        // Search regular objects
        for obj in &self.object_list {
            if (obj.radar_pos.x - radar_pos.x).abs() <= 1
                && (obj.radar_pos.y - radar_pos.y).abs() <= 1
            {
                return Some(obj.object_id);
            }
        }

        None
    }

    /// Calculate distance between two world positions (2D)
    /// Helper for range calculations
    fn distance_2d_squared(pos1: &Coord3D, pos2: &Coord3D) -> f32 {
        let dx = pos1.x - pos2.x;
        let dy = pos1.y - pos2.y;
        dx * dx + dy * dy
    }

    /// Check if a position is within radar range of any radar source
    /// Used for determining if units can be seen on radar
    pub fn is_position_in_radar_range(&self, position: &Coord3D, _player_id: u32) -> bool {
        // Check all radar objects belonging to the player
        for obj in self.get_all_objects() {
            if obj.radar_range > 0.0 {
                let range_sq = obj.radar_range * obj.radar_range;
                let dist_sq = Self::distance_2d_squared(&obj.world_pos, position);

                if dist_sq <= range_sq {
                    return true;
                }
            }
        }
        false
    }

    /// Update stealth detection for all objects
    /// Checks which stealth units are revealed by stealth-detection radars
    pub fn update_stealth_detection(&mut self) {
        // First pass: collect all stealth detection radars and their positions
        let mut detection_radars: Vec<(Coord3D, f32)> = Vec::new();

        for obj in self.get_all_objects() {
            if obj.can_detect_stealth && obj.radar_range > 0.0 {
                detection_radars.push((obj.world_pos, obj.radar_range));
            }
        }

        // Second pass: check all stealth units against detection radars
        for list in [&mut self.local_object_list, &mut self.object_list] {
            for obj in list.iter_mut() {
                if obj.is_stealth {
                    // Check if within range of any stealth detection radar
                    let mut revealed = false;

                    for (radar_pos, radar_range) in &detection_radars {
                        let dist_sq = Self::distance_2d_squared(&obj.world_pos, radar_pos);
                        let range_sq = radar_range * radar_range;

                        if dist_sq <= range_sq {
                            revealed = true;

                            // Create stealth discovered event if newly revealed
                            if !obj.stealth_revealed {
                                // Event will be created by caller with proper context
                                // Store that we want to create event
                            }
                            break;
                        }
                    }

                    obj.stealth_revealed = revealed;
                }
            }
        }
    }

    /// Try to create stealth discovered event
    /// Matches C++ pattern for stealth events (referenced in Radar.h line 50)
    pub fn try_stealth_discovered_event(&mut self, world_loc: &Coord3D) -> bool {
        self.try_event(RadarEventType::StealthDiscovered, world_loc)
    }

    /// Try to create stealth neutralized event (our stealth was revealed)
    /// Matches C++ pattern for stealth events (referenced in Radar.h line 51)
    pub fn try_stealth_neutralized_event(&mut self, world_loc: &Coord3D) -> bool {
        self.try_event(RadarEventType::StealthNeutralized, world_loc)
    }

    /// Re-examine an object and update its radar data
    /// Matches C++ Radar::examineObject (line 171)
    /// Called when object properties change (team, stealth, etc.)
    pub fn examine_object(&mut self, object_id: u32, updated_obj: RadarObject) {
        // Remove old entry if exists
        self.remove_object(object_id);

        // Re-add with updated data
        self.add_object(updated_obj);
    }

    /// Get visible radar objects (filtering out hidden ones)
    /// Returns only objects that should be displayed on radar
    pub fn get_visible_objects(&self) -> Vec<&RadarObject> {
        self.get_all_objects()
            .filter(|obj| !obj.is_temporarily_hidden())
            .collect()
    }

    /// Get all radar-providing objects (objects with radar capability)
    pub fn get_radar_providers(&self) -> Vec<&RadarObject> {
        self.get_all_objects()
            .filter(|obj| obj.radar_range > 0.0)
            .collect()
    }

    /// Get all stealth detection radars
    pub fn get_stealth_detectors(&self) -> Vec<&RadarObject> {
        self.get_all_objects()
            .filter(|obj| obj.can_detect_stealth && obj.radar_range > 0.0)
            .collect()
    }

    /// Clear entire shroud (for scenarios without fog of war)
    /// Matches C++ Radar::clearShroud (virtual method in Radar.h line 194)
    pub fn clear_shroud(&mut self) {
        for cell in &mut self.shroud_grid {
            *cell = CellShroudStatus::Clear;
        }
        self.shroud_cleared = true;
        self.terrain_dirty = true;
    }

    /// Set shroud level at specific radar cell
    /// Matches C++ Radar::setShroudLevel (virtual method in Radar.h line 197)
    pub fn set_shroud_level(&mut self, x: i32, y: i32, status: CellShroudStatus) {
        // Bounds check
        if x < 0 || x >= RADAR_CELL_WIDTH as i32 || y < 0 || y >= RADAR_CELL_HEIGHT as i32 {
            return;
        }

        let index = (y as usize * RADAR_CELL_WIDTH as usize) + x as usize;
        if index < self.shroud_grid.len() {
            self.shroud_grid[index] = status;
            self.terrain_dirty = true;
        }
    }

    /// C++ `W3DRadar::setShroudLevel` — partition shroud-cell indices, not
    /// radar pixels. Converts `cell * shroudCellWidth/Height` to world, then
    /// `worldToRadar`, and paints the radar-pixel rectangle.
    pub fn set_shroud_level_from_partition_cell(
        &mut self,
        shroud_x: i32,
        shroud_y: i32,
        status: CellShroudStatus,
        cell_width: f32,
        cell_height: f32,
    ) {
        if cell_width <= f32::EPSILON || cell_height <= f32::EPSILON {
            return;
        }
        if !self.has_map_extent() {
            self.set_shroud_level(shroud_x, shroud_y, status);
            return;
        }
        let map_min_x = shroud_x as f32 * cell_width;
        let map_min_y = shroud_y as f32 * cell_height;
        let map_max_x = (shroud_x + 1) as f32 * cell_width;
        let map_max_y = (shroud_y + 1) as f32 * cell_height;
        let Some(radar_min) = self.world_to_radar(&Coord3D::new(map_min_x, map_min_y, 0.0)) else {
            return;
        };
        let Some(radar_max) = self.world_to_radar(&Coord3D::new(map_max_x, map_max_y, 0.0)) else {
            return;
        };
        let x0 = radar_min.x.min(radar_max.x);
        let x1 = radar_min.x.max(radar_max.x);
        let y0 = radar_min.y.min(radar_max.y);
        let y1 = radar_min.y.max(radar_max.y);
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set_shroud_level(x, y, status);
            }
        }
    }

    /// Get shroud level at specific radar cell
    pub fn get_shroud_level(&self, x: i32, y: i32) -> CellShroudStatus {
        // Bounds check
        if x < 0 || x >= RADAR_CELL_WIDTH as i32 || y < 0 || y >= RADAR_CELL_HEIGHT as i32 {
            return CellShroudStatus::Shrouded;
        }

        let index = (y as usize * RADAR_CELL_WIDTH as usize) + x as usize;
        if index < self.shroud_grid.len() {
            self.shroud_grid[index]
        } else {
            CellShroudStatus::Shrouded
        }
    }

    /// Get shroud level at world position
    pub fn get_shroud_level_at_world(&self, world: &Coord3D) -> CellShroudStatus {
        if let Some(radar_pos) = self.world_to_radar(world) {
            self.get_shroud_level(radar_pos.x, radar_pos.y)
        } else {
            CellShroudStatus::Shrouded
        }
    }

    /// Set shroud level for circular area (for vision radius)
    /// Used when units provide vision to reveal fog of war
    pub fn set_shroud_circle(&mut self, center: &Coord3D, radius: f32, status: CellShroudStatus) {
        if let Some(center_radar) = self.world_to_radar(center) {
            // Convert radius to radar cells
            let radar_radius = (radius / self.x_sample.max(self.y_sample)) as i32;

            // Update cells in circular area
            for dy in -radar_radius..=radar_radius {
                for dx in -radar_radius..=radar_radius {
                    // Check if within circle
                    if dx * dx + dy * dy <= radar_radius * radar_radius {
                        let x = center_radar.x + dx;
                        let y = center_radar.y + dy;
                        self.set_shroud_level(x, y, status);
                    }
                }
            }
        }
    }

    /// Update shroud based on all radar-providing objects
    /// Called each frame to update fog of war
    pub fn update_shroud_from_radar(&mut self) {
        if self.shroud_cleared || self.is_gps_active() {
            return; // Shroud disabled or GPS active
        }

        // Mark all clear cells as fogged (fog of war)
        for cell in &mut self.shroud_grid {
            if *cell == CellShroudStatus::Clear {
                *cell = CellShroudStatus::Fogged;
            }
        }

        // First pass: collect all operational radar providers
        // Only operational radars (powered, not disabled) provide vision
        let mut radar_providers: Vec<(Coord3D, f32)> = Vec::new();
        for obj in self.get_all_objects() {
            if obj.is_radar_operational() && obj.radar_range > 0.0 {
                radar_providers.push((obj.world_pos, obj.radar_range));
            }
        }

        // Second pass: clear fog around all operational radar providers
        for (world_pos, radar_range) in radar_providers {
            self.set_shroud_circle(&world_pos, radar_range, CellShroudStatus::Clear);
        }

        // Third pass: apply active radar scans
        let scan_areas: Vec<(Coord3D, f32)> = self
            .radar_scans
            .iter()
            .map(|scan| (scan.world_location, scan.radius))
            .collect();

        for (location, radius) in scan_areas {
            self.set_shroud_circle(&location, radius, CellShroudStatus::Clear);
        }

        self.terrain_dirty = true;
    }

    /// Check if shroud is cleared
    pub fn is_shroud_cleared(&self) -> bool {
        self.shroud_cleared
    }

    /// Get shroud grid for rendering
    pub fn get_shroud_grid(&self) -> &[CellShroudStatus] {
        &self.shroud_grid
    }

    /// Build the W3D radar object overlay as RGBA pixels.
    ///
    /// Matches the core `W3DRadar::renderObjectList` raster shape: each visible
    /// object draws four legal radar-cell pixels in a 2x2 block before the texture
    /// is scaled into the HUD radar rectangle.
    pub fn build_object_overlay_texture_rgba(&self) -> Vec<u8> {
        self.build_object_overlay_texture_rgba_at_frame(self.current_frame)
    }

    pub fn build_object_overlay_texture_rgba_at_frame(&self, current_frame: u32) -> Vec<u8> {
        let expected_len = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        let mut texture = vec![0; expected_len * 4];

        // C++ W3DRadar draws the regular list first, then the local list so
        // local blips overwrite enemy blips on a shared cell.
        for obj in self.object_list.iter().chain(self.local_object_list.iter()) {
            if !self.should_render_object_overlay_blip(obj) {
                continue;
            }

            let Some(radar_point) = self.world_to_radar(&obj.world_pos) else {
                continue;
            };
            let mut color = argb_to_rgba_bytes(obj.color);
            if obj.is_stealth {
                color[3] = radar_stealth_blip_alpha(current_frame);
            }

            for point in radar_object_blip_cells(radar_point) {
                if legal_radar_point(point.x, point.y) {
                    let idx = ((point.y as u32 * RADAR_CELL_WIDTH + point.x as u32) * 4) as usize;
                    texture[idx..idx + 4].copy_from_slice(&color);
                }
            }
        }

        texture
    }

    pub fn build_hero_reticle_rects(
        &self,
        pixel_x: i32,
        pixel_y: i32,
        width: i32,
        height: i32,
        icon_width: i32,
        icon_height: i32,
    ) -> Vec<RadarHeroReticleRect> {
        let hero_object_ids = self.build_hero_reticle_object_ids();
        self.build_hero_reticle_rects_for_objects(
            &hero_object_ids,
            pixel_x,
            pixel_y,
            width,
            height,
            icon_width,
            icon_height,
        )
    }

    pub fn build_hero_reticle_object_ids(&self) -> Vec<u32> {
        self.local_object_list
            .iter()
            .filter(|obj| obj.is_hero && !obj.is_temporarily_hidden())
            .map(|obj| obj.object_id)
            .collect()
    }

    pub fn build_hero_reticle_rects_for_objects(
        &self,
        object_ids: &[u32],
        pixel_x: i32,
        pixel_y: i32,
        width: i32,
        height: i32,
        icon_width: i32,
        icon_height: i32,
    ) -> Vec<RadarHeroReticleRect> {
        object_ids
            .iter()
            .filter_map(|object_id| {
                self.local_object_list
                    .iter()
                    .find(|obj| obj.object_id == *object_id)
            })
            .filter_map(|obj| {
                let radar_point = self.world_to_radar(&obj.world_pos)?;
                let offset_screen = radar_to_pixel(&radar_point, pixel_x, pixel_y, width, height);
                let x1 = offset_screen.x - (icon_width / 2) + 1;
                let y1 = offset_screen.y - (icon_height / 2);
                Some(RadarHeroReticleRect {
                    x1,
                    y1,
                    x2: x1 + icon_width,
                    y2: y1 + icon_height,
                })
            })
            .collect()
    }

    pub fn build_view_box_lines(
        &self,
        origin_world: Coord3D,
        corner_world: [Coord3D; 4],
        pixel_x: i32,
        pixel_y: i32,
        width: i32,
        height: i32,
        clip_x: i32,
        clip_y: i32,
        clip_width: i32,
        clip_height: i32,
    ) -> Vec<RadarViewBoxLine> {
        let Some(ul_radar) = self.world_to_radar_unclamped(&origin_world) else {
            return Vec::new();
        };

        let mut corners = [ICoord2D::new(0, 0); 4];
        for (index, world) in corner_world.iter().enumerate() {
            let Some(radar) = self.world_to_radar_unclamped(world) else {
                return Vec::new();
            };
            corners[index] = radar;
        }

        let mut view_box_offsets = [ICoord2D::new(0, 0); 4];
        for index in 1..4 {
            view_box_offsets[index] = ICoord2D::new(
                corners[index].x - corners[index - 1].x,
                corners[index].y - corners[index - 1].y,
            );
        }

        let top_color = radar_view_box_top_color();
        let bottom_color = radar_view_box_bottom_color();
        let mut lines = Vec::with_capacity(4);
        let mut start = radar_to_pixel(&ul_radar, pixel_x, pixel_y, width, height);
        let mut radar = ICoord2D::new(
            ul_radar.x + view_box_offsets[1].x,
            ul_radar.y + view_box_offsets[1].y,
        );
        let mut end = radar_to_pixel(&radar, pixel_x, pixel_y, width, height);

        let mut push_line =
            |start: ICoord2D, end: ICoord2D, start_color: RGBAColorInt, end_color: RGBAColorInt| {
                if let Some((start, end)) =
                    clip_line_to_rect(start, end, clip_x, clip_y, clip_width, clip_height)
                {
                    lines.push(RadarViewBoxLine {
                        start,
                        end,
                        start_color,
                        end_color,
                    });
                }
            };

        push_line(start, end, top_color, top_color);

        start = end;
        radar.x += view_box_offsets[2].x;
        radar.y += view_box_offsets[2].y;
        end = radar_to_pixel(&radar, pixel_x, pixel_y, width, height);
        push_line(start, end, top_color, bottom_color);

        start = end;
        radar.x += view_box_offsets[3].x;
        radar.y += view_box_offsets[3].y;
        end = radar_to_pixel(&radar, pixel_x, pixel_y, width, height);
        push_line(start, end, bottom_color, bottom_color);

        start = end;
        end = radar_to_pixel(&ul_radar, pixel_x, pixel_y, width, height);
        push_line(start, end, bottom_color, top_color);

        lines
    }

    fn should_render_object_overlay_blip(&self, obj: &RadarObject) -> bool {
        if obj.is_temporarily_hidden() || !obj.priority.is_visible() {
            return false;
        }
        // C++ W3DRadar.cpp:647-650 — LOCAL_UNIT_ONLY skip only while local is active.
        if obj.priority == RadarPriorityType::LocalUnitOnly
            && !obj.is_local
            && self.local_player_active
        {
            return false;
        }
        // C++ `calcStealthedStatusForPlayer` treats observers as allies, so
        // undetected enemy stealth is VISIBLE_FRIENDLY — not hidden.
        if self.local_player_active
            && obj.is_stealth
            && obj.is_enemy
            && !obj.is_detected
            && !obj.is_disguised
            && !obj.stealth_revealed
        {
            return false;
        }

        self.object_shroud_allows_overlay_blip(obj)
    }

    /// C++ `getShroudedStatus(playerIndex) > OBJECTSHROUD_PARTIAL_CLEAR` skip.
    fn object_shroud_allows_overlay_blip(&self, obj: &RadarObject) -> bool {
        match obj.object_shroud {
            ObjectShroudStatus::Invalid | ObjectShroudStatus::InvalidButPreviousValid => {
                self.get_shroud_level_at_world(&obj.world_pos) == CellShroudStatus::Clear
            }
            status => (status as u32) <= (ObjectShroudStatus::PartialClear as u32),
        }
    }

    /// Build the W3D radar shroud overlay as black RGBA pixels.
    ///
    /// Matches `W3DRadar::setShroudLevel`: shrouded cells are opaque black,
    /// fogged cells are half-alpha black, and clear cells are transparent.
    pub fn build_shroud_texture_rgba(&self) -> Vec<u8> {
        let expected_len = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        let mut texture = vec![0; expected_len * 4];

        for idx in 0..expected_len {
            let alpha = match self
                .shroud_grid
                .get(idx)
                .copied()
                .unwrap_or(CellShroudStatus::Shrouded)
            {
                CellShroudStatus::Clear => 0,
                CellShroudStatus::Fogged => 127,
                CellShroudStatus::Shrouded => 255,
            };
            texture[idx * 4 + 3] = alpha;
        }

        texture
    }

    // ===== GPS Satellite Special Power =====

    /// Activate GPS satellite (reveals entire map for duration)
    /// Typically activated via special power (e.g., USA Superweapon General)
    /// Duration is typically 30 seconds (900 frames at 30 FPS)
    pub fn activate_gps_satellite(&mut self, duration_frames: u32) {
        self.gps_active_until_frame = self.current_frame + duration_frames;

        // Clear all shroud when GPS activates
        for cell in &mut self.shroud_grid {
            *cell = CellShroudStatus::Clear;
        }
        self.terrain_dirty = true;
    }

    /// Check if GPS satellite is currently active
    pub fn is_gps_active(&self) -> bool {
        self.current_frame < self.gps_active_until_frame
    }

    /// Deactivate GPS satellite (called when duration expires)
    pub fn deactivate_gps_satellite(&mut self) {
        self.gps_active_until_frame = 0;
        // Shroud will be restored based on actual radar coverage
        self.update_shroud_from_radar();
    }

    // ===== Radar Scan Special Power =====

    /// Activate radar scan at a location (reveals area temporarily)
    /// Typically activated via special power (reveals fog of war in radius)
    /// Standard radius: ~300 units, duration: ~10 seconds (300 frames)
    pub fn activate_radar_scan(
        &mut self,
        location: Coord3D,
        radius: f32,
        duration_frames: u32,
        player_id: u32,
    ) {
        let scan = RadarScan::new(
            location,
            radius,
            duration_frames,
            player_id,
            self.current_frame,
        );
        self.radar_scans.push(scan);

        // Immediately reveal the area
        self.set_shroud_circle(&location, radius, CellShroudStatus::Clear);
    }

    /// Update active radar scans (remove expired ones)
    fn update_radar_scans(&mut self) {
        self.radar_scans
            .retain(|scan| !scan.is_expired(self.current_frame));
    }

    /// Check if a position is revealed by any active radar scan
    pub fn is_position_in_radar_scan(&self, position: &Coord3D, player_id: u32) -> bool {
        self.radar_scans
            .iter()
            .any(|scan| scan.player_id == player_id && scan.contains_position(position))
    }

    /// Get all active radar scans for a player
    pub fn get_active_radar_scans(&self, player_id: u32) -> Vec<&RadarScan> {
        self.radar_scans
            .iter()
            .filter(|scan| scan.player_id == player_id)
            .collect()
    }

    // ===== Radar Jamming =====

    /// Add a radar jamming source (e.g., GLA Radar Van)
    /// Jamming prevents enemy radar detection in the jamming radius
    pub fn add_jamming_source(
        &mut self,
        object_id: u32,
        location: Coord3D,
        radius: f32,
        player_id: u32,
    ) {
        let jammer = JammingSource::new(object_id, location, radius, player_id);
        self.jamming_sources.push(jammer);
    }

    /// Remove a jamming source (when object destroyed or disabled)
    pub fn remove_jamming_source(&mut self, object_id: u32) {
        self.jamming_sources
            .retain(|jammer| jammer.object_id != object_id);
    }

    /// Update jamming source position (for mobile jammers)
    pub fn update_jamming_source_position(&mut self, object_id: u32, location: Coord3D) {
        if let Some(jammer) = self
            .jamming_sources
            .iter_mut()
            .find(|j| j.object_id == object_id)
        {
            jammer.update_position(location);
        }
    }

    /// Enable/disable a jamming source
    pub fn set_jamming_source_active(&mut self, object_id: u32, active: bool) {
        if let Some(jammer) = self
            .jamming_sources
            .iter_mut()
            .find(|j| j.object_id == object_id)
        {
            jammer.set_active(active);
        }
    }

    /// Check if a position is jammed by enemy jammers
    /// Position is jammed if it's in range of an enemy jamming source
    pub fn is_position_jammed(&self, position: &Coord3D, friendly_player_id: u32) -> bool {
        self.jamming_sources.iter().any(|jammer| {
            jammer.player_id != friendly_player_id && jammer.is_position_jammed(position)
        })
    }

    /// Update jamming status for all radar objects
    /// Called each frame to update which objects are jammed
    pub fn update_jamming_status(&mut self) {
        // Collect jamming positions first to avoid borrow issues
        let jamming_checks: Vec<(Coord3D, u32)> = self
            .object_list
            .iter()
            .chain(self.local_object_list.iter())
            .map(|obj| (obj.world_pos, obj.object_id))
            .collect();

        let jamming_results: Vec<bool> = jamming_checks
            .iter()
            .map(|(pos, id)| self.is_position_jammed(pos, *id))
            .collect();

        // Update regular objects
        let mut result_idx = 0;
        for obj in &mut self.object_list {
            obj.is_jammed = jamming_results[result_idx];
            result_idx += 1;
        }

        // Update local objects
        for obj in &mut self.local_object_list {
            obj.is_jammed = jamming_results[result_idx];
            result_idx += 1;
        }
    }

    // ===== Power State Management =====

    /// Set power state for a radar provider object
    /// Called when building loses/regains power
    pub fn set_radar_powered(&mut self, object_id: u32, powered: bool) {
        // Update in both lists
        for obj in self
            .object_list
            .iter_mut()
            .chain(self.local_object_list.iter_mut())
        {
            if obj.object_id == object_id && obj.is_radar_provider {
                obj.set_powered(powered);
            }
        }

        // Update shroud based on new power state
        self.update_shroud_from_radar();
    }

    /// Disable a radar (e.g., from EMP effect)
    pub fn disable_radar_object(&mut self, object_id: u32) {
        for obj in self
            .object_list
            .iter_mut()
            .chain(self.local_object_list.iter_mut())
        {
            if obj.object_id == object_id && obj.is_radar_provider {
                obj.disable_radar();
            }
        }
        self.update_shroud_from_radar();
    }

    /// Enable a radar (e.g., EMP effect expired)
    pub fn enable_radar_object(&mut self, object_id: u32) {
        for obj in self
            .object_list
            .iter_mut()
            .chain(self.local_object_list.iter_mut())
        {
            if obj.object_id == object_id && obj.is_radar_provider {
                obj.enable_radar();
            }
        }
        self.update_shroud_from_radar();
    }

    /// Get all operational radar providers (powered and not disabled)
    pub fn get_operational_radars(&self) -> Vec<&RadarObject> {
        self.get_all_objects()
            .filter(|obj| obj.is_radar_operational())
            .collect()
    }

    /// Check if player has any operational radar
    pub fn has_operational_radar(&self, _player_id: u32) -> bool {
        self.get_all_objects().any(|obj| obj.is_radar_operational())
    }
}

// ------------------------------------------------------------------------------------------------
// Snapshotable implementation for RadarObject
// C++ Reference: Radar.cpp lines 130-167
// ------------------------------------------------------------------------------------------------

impl Snapshotable for RadarObject {
    /// CRC - matches C++ RadarObject::crc() (Radar.cpp line 130)
    /// C++ implementation is empty.
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    /// Save/Load transfer - matches C++ RadarObject::xfer() (Radar.cpp lines 140-172)
    ///
    /// Version Info:
    /// 1: Initial version
    ///
    /// Fields xfer'd (Radar.cpp lines 149-170):
    ///   1. ObjectID (via xferObjectID)
    ///   2. color (via xferColor)
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version: XferVersion = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("RadarObject::xfer version error: {}", e))?;

        // C++ lines 149-167: xfer object ID
        xfer.xfer_object_id(&mut self.object_id)
            .map_err(|e| format!("RadarObject::xfer objectID error: {}", e))?;

        // C++ line 170: xfer color
        let mut color = self.color as i32;
        xfer.xfer_color(&mut color)
            .map_err(|e| format!("RadarObject::xfer color error: {}", e))?;
        self.color = color as u32;

        Ok(())
    }

    /// Load post process - matches C++ RadarObject::loadPostProcess() (Radar.cpp line 177)
    /// C++ implementation is empty.
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// ------------------------------------------------------------------------------------------------
// Snapshotable implementation for RadarSystem
// C++ Reference: Radar.cpp lines 1352-1510
// ------------------------------------------------------------------------------------------------

impl Snapshotable for RadarSystem {
    /// CRC - matches C++ Radar::crc() (Radar.cpp line 1352)
    /// C++ implementation is empty.
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    /// Save/Load transfer - matches C++ Radar::xfer() (Radar.cpp lines 1455-1510)
    ///
    /// Version Info:
    /// 1: Initial version
    ///
    /// Fields xfer'd (Radar.cpp lines 1455-1509):
    ///   1. radarHidden (Bool)
    ///   2. radarForceOn (Bool)
    ///   3. localObjectList (via xferRadarObjectList helper)
    ///   4. objectList (via xferRadarObjectList helper)
    ///   5. events array (count verified as MAX_RADAR_EVENTS, then per-event fields)
    ///   6. nextFreeRadarEvent (Int)
    ///   7. lastRadarEvent (Int)
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version: XferVersion = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("RadarSystem::xfer version error: {}", e))?;

        // C++ line 1464: radar hidden
        xfer.xfer_bool(&mut self.radar_hidden)
            .map_err(|e| format!("RadarSystem::xfer radarHidden error: {}", e))?;

        // C++ line 1467: radar force on
        xfer.xfer_bool(&mut self.radar_force_on)
            .map_err(|e| format!("RadarSystem::xfer radarForceOn error: {}", e))?;

        // C++ lines 1470-1473: xfer local and regular object lists
        xfer_radar_object_list(xfer, &mut self.local_object_list)
            .map_err(|e| format!("RadarSystem::xfer localObjectList error: {}", e))?;
        xfer_radar_object_list(xfer, &mut self.object_list)
            .map_err(|e| format!("RadarSystem::xfer objectList error: {}", e))?;

        // C++ lines 1476-1502: xfer radar events
        let mut event_count_verify = MAX_RADAR_EVENTS as u16;
        xfer.xfer_unsigned_short(&mut event_count_verify)
            .map_err(|e| format!("RadarSystem::xfer eventCount error: {}", e))?;

        for event in &mut self.events {
            let mut event_type = event.event_type as u8;
            xfer.xfer_unsigned_byte(&mut event_type)
                .map_err(|e| format!("RadarSystem::xfer eventType error: {}", e))?;
            event.event_type = match event_type {
                0 => RadarEventType::Invalid,
                1 => RadarEventType::Construction,
                2 => RadarEventType::Upgrade,
                3 => RadarEventType::UnderAttack,
                4 => RadarEventType::Information,
                5 => RadarEventType::BeaconPulse,
                6 => RadarEventType::Infiltration,
                7 => RadarEventType::BattlePlan,
                8 => RadarEventType::StealthDiscovered,
                9 => RadarEventType::StealthNeutralized,
                10 => RadarEventType::Fake,
                _ => RadarEventType::Invalid,
            };

            xfer.xfer_bool(&mut event.active)
                .map_err(|e| format!("RadarSystem::xfer eventActive error: {}", e))?;
            xfer.xfer_unsigned_int(&mut event.create_frame)
                .map_err(|e| format!("RadarSystem::xfer eventCreateFrame error: {}", e))?;
            xfer.xfer_unsigned_int(&mut event.die_frame)
                .map_err(|e| format!("RadarSystem::xfer eventDieFrame error: {}", e))?;
            xfer.xfer_unsigned_int(&mut event.fade_frame)
                .map_err(|e| format!("RadarSystem::xfer eventFadeFrame error: {}", e))?;

            // C++ line 1496: xferRGBAColorInt
            let mut c1 = event.color1.to_u32();
            xfer.xfer_unsigned_int(&mut c1)
                .map_err(|e| format!("RadarSystem::xfer eventColor1 error: {}", e))?;
            event.color1 = RGBAColorInt::from_u32(c1);

            let mut c2 = event.color2.to_u32();
            xfer.xfer_unsigned_int(&mut c2)
                .map_err(|e| format!("RadarSystem::xfer eventColor2 error: {}", e))?;
            event.color2 = RGBAColorInt::from_u32(c2);

            // C++ line 1498: xferCoord3D
            xfer.xfer_real(&mut event.world_loc.x)
                .map_err(|e| format!("RadarSystem::xfer eventWorldLoc.x error: {}", e))?;
            xfer.xfer_real(&mut event.world_loc.y)
                .map_err(|e| format!("RadarSystem::xfer eventWorldLoc.y error: {}", e))?;
            xfer.xfer_real(&mut event.world_loc.z)
                .map_err(|e| format!("RadarSystem::xfer eventWorldLoc.z error: {}", e))?;

            // C++ line 1499: xferICoord2D
            xfer.xfer_int(&mut event.radar_loc.x)
                .map_err(|e| format!("RadarSystem::xfer eventRadarLoc.x error: {}", e))?;
            xfer.xfer_int(&mut event.radar_loc.y)
                .map_err(|e| format!("RadarSystem::xfer eventRadarLoc.y error: {}", e))?;

            xfer.xfer_bool(&mut event.sound_played)
                .map_err(|e| format!("RadarSystem::xfer eventSoundPlayed error: {}", e))?;
        }

        // C++ line 1505: nextFreeRadarEvent
        let mut next_free = self.next_free_event as i32;
        xfer.xfer_int(&mut next_free)
            .map_err(|e| format!("RadarSystem::xfer nextFreeRadarEvent error: {}", e))?;
        self.next_free_event = next_free as usize;

        // C++ line 1508: lastRadarEvent
        let mut last_event = self.last_event.map(|i| i as i32).unwrap_or(-1);
        xfer.xfer_int(&mut last_event)
            .map_err(|e| format!("RadarSystem::xfer lastRadarEvent error: {}", e))?;
        self.last_event = if last_event >= 0 {
            Some(last_event as usize)
        } else {
            None
        };

        Ok(())
    }

    /// Load post process - matches C++ Radar::loadPostProcess() (Radar.cpp lines 1515-1524)
    /// C++ refreshes terrain after loading. We mark terrain dirty for deferred refresh.
    fn load_post_process(&mut self) -> Result<(), String> {
        self.terrain_dirty = true;
        Ok(())
    }
}

/// Helper: xfer a radar object list (matches C++ xferRadarObjectList)
/// C++ Reference: Radar.cpp lines 1362-1448
fn xfer_radar_object_list(
    xfer: &mut dyn Xfer,
    object_list: &mut Vec<RadarObject>,
) -> Result<(), String> {
    const CURRENT_VERSION: XferVersion = 1;
    let mut version: XferVersion = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)
        .map_err(|e| format!("xferRadarObjectList version error: {}", e))?;

    let mut count = object_list.len() as u16;
    xfer.xfer_unsigned_short(&mut count)
        .map_err(|e| format!("xferRadarObjectList count error: {}", e))?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for obj in object_list.iter_mut() {
                Snapshotable::xfer(obj, xfer)?;
            }
        }
        XferMode::Load => {
            object_list.clear();
            for _ in 0..count {
                let mut radar_obj = RadarObject::new(0);
                Snapshotable::xfer(&mut radar_obj, xfer)?;
                object_list.push(radar_obj);
            }
        }
        _ => {
            return Err(format!(
                "xferRadarObjectList - unknown xfer mode {:?}",
                xfer.get_xfer_mode()
            ));
        }
    }

    Ok(())
}

impl Default for RadarSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Global radar system singleton
static RADAR_SYSTEM: RwLock<Option<Arc<RwLock<RadarSystem>>>> = RwLock::new(None);

/// Get global radar system
pub fn get_radar_system() -> Arc<RwLock<RadarSystem>> {
    let created = {
        let mut guard = RADAR_SYSTEM.write().unwrap();
        if guard.is_none() {
            *guard = Some(Arc::new(RwLock::new(RadarSystem::new())));
            true
        } else {
            false
        }
    };
    if created {
        snapshot::ensure_the_radar_snapshot_block();
    }
    let guard = RADAR_SYSTEM.read().unwrap();
    guard.as_ref().unwrap().clone()
}
