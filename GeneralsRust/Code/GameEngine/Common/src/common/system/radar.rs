////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: radar.rs //////////////////////////////////////////////////////////////
// Radar system functionality
// Port from C++ Radar.cpp and Radar.h (Colin Day, January 2002)
///////////////////////////////////////////////////////////////////////////////

use crate::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use std::sync::{Arc, RwLock};

/// Radar cell dimensions (matches C++ RADAR_CELL_WIDTH/HEIGHT)
/// Must be power of 2 for WW3D texture requirements
pub const RADAR_CELL_WIDTH: u32 = 128;
pub const RADAR_CELL_HEIGHT: u32 = 128;

/// Maximum radar events (matches C++ MAX_RADAR_EVENTS)
pub const MAX_RADAR_EVENTS: usize = 64;

/// Radar queue terrain refresh delay (matches C++ RADAR_QUEUE_TERRAIN_REFRESH_DELAY)
/// 3 seconds worth of logic frames
pub const RADAR_QUEUE_TERRAIN_REFRESH_DELAY: u32 = 90; // 30 FPS * 3 seconds

#[derive(Debug, Clone, Copy, Default)]
struct RadarTerrainSample {
    height: f32,
    is_water: bool,
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
    pub is_jammed: bool,
    pub stealth_revealed: bool,   // For stealth detection radar
    pub radar_range: f32,         // Radar detection range (0 = no radar capability)
    pub can_detect_stealth: bool, // Can this radar detect stealth units
    pub is_radar_provider: bool,  // This object provides radar coverage
    pub is_powered: bool,         // Is this radar powered on (for power-dependent radars)
    pub is_disabled: bool,        // Is this radar disabled (by EMP, power loss, etc.)
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
            is_jammed: false,
            stealth_revealed: false,
            radar_range: 0.0,
            can_detect_stealth: false,
            is_radar_provider: false,
            is_powered: true,
            is_disabled: false,
        }
    }

    /// Check if object is temporarily hidden from radar
    /// Matches C++ RadarObject::isTemporarilyHidden (lines 118-125)
    pub fn is_temporarily_hidden(&self) -> bool {
        // Stealth units are hidden unless revealed by stealth detection
        (self.is_stealth && !self.stealth_revealed) || self.is_jammed
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
        self.current_frame = current_frame;

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
    }

    /// Current logic frame used for radar animation and event expiry.
    pub fn current_frame(&self) -> u32 {
        self.current_frame
    }

    /// Current map extent used for radar/world coordinate conversion.
    pub fn map_extent(&self) -> Region3D {
        self.map_extent
    }

    /// Initialize radar for new map (matches C++ Radar::newMap)
    pub fn new_map(
        &mut self,
        map_min: Coord3D,
        map_max: Coord3D,
        terrain_heights: &[(f32, f32, bool)],
    ) {
        self.reset();

        self.map_extent = Region3D {
            lo: map_min,
            hi: map_max,
        };

        // Calculate sampling intervals
        self.x_sample = self.map_extent.width() / RADAR_CELL_WIDTH as f32;
        self.y_sample = self.map_extent.height() / RADAR_CELL_HEIGHT as f32;

        // Calculate average terrain and water heights
        let mut terrain_sum = 0.0;
        let mut water_sum = 0.0;
        let mut terrain_count = 0;
        let mut water_count = 0;

        for &(_x, z, is_water) in terrain_heights {
            if is_water {
                water_sum += z;
                water_count += 1;
            } else {
                terrain_sum += z;
                terrain_count += 1;
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

        let expected_samples = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        self.terrain_samples.clear();
        if terrain_heights.len() == expected_samples {
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

        // Insert sorted by priority
        let insert_pos = list
            .iter()
            .position(|obj| obj.priority > radar_obj.priority)
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

    /// Try to create under attack event (matches C++ Radar::tryUnderAttackEvent)
    pub fn try_under_attack_event(&mut self, world_loc: &Coord3D) -> bool {
        const CLOSE_ENOUGH_DISTANCE_SQ: f32 = 250.0 * 250.0;
        const FRAMES_BETWEEN_EVENTS: u32 = 300; // 10 seconds at 30 FPS

        // Check if there's a recent attack event nearby
        for event in &self.events {
            if event.event_type == RadarEventType::UnderAttack && event.active {
                let dx = event.world_loc.x - world_loc.x;
                let dy = event.world_loc.y - world_loc.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= CLOSE_ENOUGH_DISTANCE_SQ {
                    if self.current_frame - event.create_frame < FRAMES_BETWEEN_EVENTS {
                        return false; // Too soon, reject
                    }
                }
            }
        }

        // Create new event
        self.create_event(world_loc, RadarEventType::UnderAttack, 4.0);
        true
    }

    /// Try to create infiltration event (matches C++ Radar::tryInfiltrationEvent)
    pub fn try_infiltration_event(&mut self, world_loc: &Coord3D) {
        self.create_event(world_loc, RadarEventType::Infiltration, 4.0);
    }

    /// Get last event location (matches C++ Radar::getLastEventLoc)
    pub fn get_last_event_loc(&self) -> Option<Coord3D> {
        self.last_event.map(|idx| self.events[idx].world_loc)
    }

    /// Refresh terrain texture (matches C++ Radar::refreshTerrain)
    pub fn refresh_terrain(&mut self) {
        self.queue_terrain_refresh_frame = None;
        self.terrain_dirty = true;

        let expected_samples = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        if self.terrain_samples.len() == expected_samples {
            let mut min_h = f32::MAX;
            let mut max_h = f32::MIN;
            for sample in &self.terrain_samples {
                min_h = min_h.min(sample.height);
                max_h = max_h.max(sample.height);
            }
            let range = (max_h - min_h).max(1.0);
            let mid_h = self.terrain_average_z;
            let idx = |x: u32, y: u32| -> usize { (y * RADAR_CELL_WIDTH + x) as usize };

            for y in 0..RADAR_CELL_HEIGHT {
                for x in 0..RADAR_CELL_WIDTH {
                    let sample = self.terrain_samples[idx(x, y)];
                    let h = sample.height;
                    let elevation = ((h - min_h) / range).clamp(0.0, 1.0);

                    let (mut r, mut g, mut b) = if sample.is_water {
                        (44.0, 86.0, 140.0)
                    } else {
                        (
                            54.0 + (182.0 - 54.0) * elevation,
                            66.0 + (166.0 - 66.0) * elevation,
                            46.0 + (120.0 - 46.0) * elevation,
                        )
                    };

                    if sample.is_water {
                        let depth = ((self.water_average_z - h) / range).clamp(0.0, 1.0);
                        b += 26.0 * depth;
                        g += 8.0 * depth;
                        r *= 1.0 - 0.10 * depth;
                    }

                    let [ir, ig, ib] = interpolate_color_for_height(
                        [r / 255.0, g / 255.0, b / 255.0],
                        h,
                        max_h,
                        mid_h,
                        min_h,
                    );
                    r = ir * 255.0;
                    g = ig * 255.0;
                    b = ib * 255.0;

                    let base = ((y * RADAR_CELL_WIDTH + x) * 4) as usize;
                    self.terrain_texture[base] = r.clamp(0.0, 255.0) as u8;
                    self.terrain_texture[base + 1] = g.clamp(0.0, 255.0) as u8;
                    self.terrain_texture[base + 2] = b.clamp(0.0, 255.0) as u8;
                    self.terrain_texture[base + 3] = 255;
                }
            }
        } else {
            for i in 0..self.terrain_texture.len() / 4 {
                let idx = i * 4;
                // Fallback to legacy terrain tint when detailed samples are unavailable.
                self.terrain_texture[idx] = 139;
                self.terrain_texture[idx + 1] = 119;
                self.terrain_texture[idx + 2] = 70;
                self.terrain_texture[idx + 3] = 255;
            }
        }

        self.terrain_dirty = false;
    }

    /// Queue terrain refresh (matches C++ Radar::queueTerrainRefresh)
    pub fn queue_terrain_refresh(&mut self) {
        self.queue_terrain_refresh_frame = Some(self.current_frame);
    }

    /// Get terrain texture data
    pub fn get_terrain_texture(&self) -> &[u8] {
        &self.terrain_texture
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

    /// Get all active events
    pub fn get_active_events(&self) -> Vec<&RadarEvent> {
        self.events.iter().filter(|e| e.active).collect()
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
        const CLOSE_ENOUGH_DISTANCE_SQ: f32 = 250.0 * 250.0;
        const FRAMES_BETWEEN_EVENTS: u32 = 300; // 10 seconds at 30 FPS

        // Check if there's a recent stealth discovered event nearby
        for event in &self.events {
            if event.event_type == RadarEventType::StealthDiscovered && event.active {
                let dx = event.world_loc.x - world_loc.x;
                let dy = event.world_loc.y - world_loc.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= CLOSE_ENOUGH_DISTANCE_SQ {
                    if self.current_frame - event.create_frame < FRAMES_BETWEEN_EVENTS {
                        return false; // Too soon, reject
                    }
                }
            }
        }

        // Create new event
        self.create_event(world_loc, RadarEventType::StealthDiscovered, 4.0);
        true
    }

    /// Try to create stealth neutralized event (our stealth was revealed)
    /// Matches C++ pattern for stealth events (referenced in Radar.h line 51)
    pub fn try_stealth_neutralized_event(&mut self, world_loc: &Coord3D) -> bool {
        const CLOSE_ENOUGH_DISTANCE_SQ: f32 = 250.0 * 250.0;
        const FRAMES_BETWEEN_EVENTS: u32 = 300; // 10 seconds at 30 FPS

        // Check if there's a recent stealth neutralized event nearby
        for event in &self.events {
            if event.event_type == RadarEventType::StealthNeutralized && event.active {
                let dx = event.world_loc.x - world_loc.x;
                let dy = event.world_loc.y - world_loc.y;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= CLOSE_ENOUGH_DISTANCE_SQ {
                    if self.current_frame - event.create_frame < FRAMES_BETWEEN_EVENTS {
                        return false; // Too soon, reject
                    }
                }
            }
        }

        // Create new event
        self.create_event(world_loc, RadarEventType::StealthNeutralized, 4.0);
        true
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
        let expected_len = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        let mut texture = vec![0; expected_len * 4];

        for obj in self.get_all_objects() {
            if !self.should_render_object_overlay_blip(obj) {
                continue;
            }

            let Some(radar_point) = self.world_to_radar(&obj.world_pos) else {
                continue;
            };
            let color = argb_to_rgba_bytes(obj.color);

            for point in radar_object_blip_cells(radar_point) {
                if legal_radar_point(point.x, point.y) {
                    let idx = ((point.y as u32 * RADAR_CELL_WIDTH + point.x as u32) * 4) as usize;
                    texture[idx..idx + 4].copy_from_slice(&color);
                }
            }
        }

        texture
    }

    fn should_render_object_overlay_blip(&self, obj: &RadarObject) -> bool {
        if obj.is_temporarily_hidden() || !obj.priority.is_visible() {
            return false;
        }
        if obj.priority == RadarPriorityType::LocalUnitOnly && !obj.is_local {
            return false;
        }

        self.get_shroud_level_at_world(&obj.world_pos) == CellShroudStatus::Clear
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
    let mut guard = RADAR_SYSTEM.write().unwrap();
    if guard.is_none() {
        *guard = Some(Arc::new(RwLock::new(RadarSystem::new())));
    }
    guard.as_ref().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radar_system_creation() {
        let radar = RadarSystem::new();
        assert!(!radar.is_radar_hidden());
        assert!(!radar.is_radar_forced());
    }

    #[test]
    fn test_radar_priority_visibility() {
        assert!(!RadarPriorityType::Invalid.is_visible());
        assert!(!RadarPriorityType::NotOnRadar.is_visible());
        assert!(RadarPriorityType::Structure.is_visible());
        assert!(RadarPriorityType::Unit.is_visible());
        assert!(RadarPriorityType::LocalUnitOnly.is_visible());
    }

    #[test]
    fn test_world_to_radar_conversion() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let world = Coord3D::new(512.0, 512.0, 0.0);
        let radar_pos = radar.world_to_radar(&world).unwrap();
        assert_eq!(radar_pos.x, 64); // Middle of 128x128 radar
        assert_eq!(radar_pos.y, 64);
    }

    #[test]
    fn test_radar_to_world_conversion() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let radar_pos = ICoord2D::new(64, 64);
        let world = radar.radar_to_world(&radar_pos).unwrap();
        assert!((world.x - 512.0).abs() < 1.0);
        assert!((world.y - 512.0).abs() < 1.0);
    }

    #[test]
    fn test_new_map_builds_initial_terrain_texture_like_w3d() {
        let mut radar = RadarSystem::new();

        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        assert!(!radar.is_terrain_dirty());
        assert!(radar
            .get_terrain_texture()
            .chunks_exact(4)
            .all(|pixel| pixel[3] == 255));
    }

    #[test]
    fn test_legal_radar_point_matches_w3d_bounds() {
        assert!(legal_radar_point(0, 0));
        assert!(legal_radar_point(
            RADAR_CELL_WIDTH as i32 - 1,
            RADAR_CELL_HEIGHT as i32 - 1
        ));
        assert!(!legal_radar_point(-1, 0));
        assert!(!legal_radar_point(0, -1));
        assert!(!legal_radar_point(RADAR_CELL_WIDTH as i32, 0));
        assert!(!legal_radar_point(0, RADAR_CELL_HEIGHT as i32));
    }

    #[test]
    fn test_radar_to_pixel_matches_w3d_inverted_y_mapping() {
        let upper_left_x = 10;
        let upper_left_y = 20;
        let width = 256;
        let height = 128;

        assert_eq!(
            radar_to_pixel(
                &ICoord2D::new(0, 0),
                upper_left_x,
                upper_left_y,
                width,
                height
            ),
            ICoord2D::new(10, 147)
        );
        assert_eq!(
            radar_to_pixel(
                &ICoord2D::new(127, 127),
                upper_left_x,
                upper_left_y,
                width,
                height
            ),
            ICoord2D::new(264, 20)
        );
        assert_eq!(
            radar_to_pixel(
                &ICoord2D::new(64, 64),
                upper_left_x,
                upper_left_y,
                width,
                height
            ),
            ICoord2D::new(138, 83)
        );
    }

    #[test]
    fn test_radar_draw_positions_letterboxes_wide_maps() {
        let extent = Region3D {
            lo: Coord3D::new(0.0, 0.0, 0.0),
            hi: Coord3D::new(2000.0, 1000.0, 0.0),
        };

        let (ul, lr) = radar_draw_positions(10, 20, 100, 100, extent);

        assert_eq!(ul, ICoord2D::new(10, 45));
        assert_eq!(lr, ICoord2D::new(110, 95));
    }

    #[test]
    fn test_radar_draw_positions_pillarboxes_tall_maps() {
        let extent = Region3D {
            lo: Coord3D::new(0.0, 0.0, 0.0),
            hi: Coord3D::new(1000.0, 2000.0, 0.0),
        };

        let (ul, lr) = radar_draw_positions(10, 20, 100, 100, extent);

        assert_eq!(ul, ICoord2D::new(35, 20));
        assert_eq!(lr, ICoord2D::new(85, 120));
    }

    #[test]
    fn test_interpolate_color_for_height_matches_w3d_lighting() {
        let base = [0.2, 0.4, 0.6];

        let lighter = interpolate_color_for_height(base, 75.0, 100.0, 50.0, 0.0);
        assert!((lighter[0] - 0.58).abs() < 0.0001);
        assert!((lighter[1] - 0.685).abs() < 0.0001);
        assert!((lighter[2] - 0.79).abs() < 0.0001);

        let darker = interpolate_color_for_height(base, 25.0, 100.0, 50.0, 0.0);
        assert!((darker[0] - 0.14).abs() < 0.0001);
        assert!((darker[1] - 0.28).abs() < 0.0001);
        assert!((darker[2] - 0.42).abs() < 0.0001);
    }

    #[test]
    fn test_interpolate_color_for_height_handles_flat_w3d_ranges() {
        let color = interpolate_color_for_height([0.5, 0.5, 0.5], 10.0, 10.0, 10.0, 10.0);
        assert!(color.iter().all(|channel| channel.is_finite()));
        assert!(color.iter().all(|channel| (0.0..=1.0).contains(channel)));
    }

    #[test]
    fn test_generic_radar_event_marker_matches_w3d_triangle_at_create_frame() {
        let event = RadarEvent {
            active: true,
            create_frame: 10,
            die_frame: 100,
            fade_frame: 90,
            color1: RGBAColorInt::new(255, 0, 0, 200),
            color2: RGBAColorInt::new(255, 255, 0, 180),
            radar_loc: ICoord2D::new(64, 64),
            ..RadarEvent::default()
        };

        let marker =
            radar_event_marker(&event, 10, 10, 20, 100, 100, RadarEventMarkerKind::Generic);

        assert_eq!(marker.size, 50);
        assert_eq!(
            marker.points,
            [
                ICoord2D::new(99, 69),
                ICoord2D::new(39, 35),
                ICoord2D::new(39, 103),
            ]
        );
        assert_eq!(marker.color1.a, 200);
        assert_eq!(marker.color2.a, 180);
    }

    #[test]
    fn test_beacon_radar_event_marker_matches_w3d_size_and_fade() {
        let event = RadarEvent {
            active: true,
            create_frame: 10,
            die_frame: 100,
            fade_frame: 90,
            color1: RGBAColorInt::new(255, 0, 0, 200),
            color2: RGBAColorInt::new(255, 255, 0, 180),
            radar_loc: ICoord2D::new(64, 64),
            ..RadarEvent::default()
        };

        let marker = radar_event_marker(&event, 95, 10, 20, 100, 100, RadarEventMarkerKind::Beacon);

        assert_eq!(marker.size, 6);
        assert_eq!(marker.color1.a, 100);
        assert_eq!(marker.color2.a, 90);
    }

    #[test]
    fn test_world_to_radar_respects_nonzero_map_origin() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(256.0, 128.0, 0.0),
            Coord3D::new(1280.0, 1152.0, 100.0),
            &[],
        );

        let world = Coord3D::new(768.0, 640.0, 0.0);
        let radar_pos = radar.world_to_radar(&world).unwrap();
        assert_eq!(radar_pos.x, 64);
        assert_eq!(radar_pos.y, 64);
    }

    #[test]
    fn test_radar_to_world_uses_sampled_cell_height_when_available() {
        let mut radar = RadarSystem::new();
        let mut terrain = Vec::with_capacity((RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize);
        for _ in 0..(RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) {
            terrain.push((0.0, 10.0, false));
        }
        let target_x = 64u32;
        let target_y = 64u32;
        let idx = (target_y * RADAR_CELL_WIDTH + target_x) as usize;
        terrain[idx] = (0.0, 77.0, false);

        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &terrain,
        );

        let world = radar
            .radar_to_world(&ICoord2D::new(target_x as i32, target_y as i32))
            .unwrap();
        assert!((world.z - 77.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_radar_event_creation() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let world_loc = Coord3D::new(100.0, 100.0, 0.0);
        radar.create_event(&world_loc, RadarEventType::UnderAttack, 4.0);

        let active_events = radar.get_active_events();
        assert_eq!(active_events.len(), 1);
        assert_eq!(active_events[0].event_type, RadarEventType::UnderAttack);
    }

    #[test]
    fn test_radar_event_expiration() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let world_loc = Coord3D::new(100.0, 100.0, 0.0);
        radar.create_event(&world_loc, RadarEventType::Information, 1.0);

        // Event should be active initially
        assert_eq!(radar.get_active_events().len(), 1);

        // Update past expiration (1 second = 30 frames)
        radar.update(35);
        assert_eq!(radar.get_active_events().len(), 0);
    }

    #[test]
    fn queued_terrain_refresh_ignores_earlier_frame_without_underflow() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );
        radar.refresh_terrain();
        assert!(!radar.is_terrain_dirty());

        radar.update(100);
        radar.queue_terrain_refresh();
        radar.update(50);

        assert!(!radar.is_terrain_dirty());
    }

    #[test]
    fn test_add_remove_object() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let mut obj = RadarObject::new(1);
        obj.color = 0xFF0000FF;
        obj.world_pos = Coord3D::new(100.0, 100.0, 0.0);
        obj.priority = RadarPriorityType::Unit;
        obj.is_local = false;

        radar.add_object(obj);
        assert_eq!(radar.object_list.len(), 1);

        radar.remove_object(1);
        assert_eq!(radar.object_list.len(), 0);
    }

    #[test]
    fn test_remove_object_searches_local_and_regular_lists() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let mut local = RadarObject::new(1);
        local.world_pos = Coord3D::new(100.0, 100.0, 0.0);
        local.priority = RadarPriorityType::Unit;
        local.is_local = true;
        radar.add_object(local);

        let mut regular = RadarObject::new(2);
        regular.world_pos = Coord3D::new(200.0, 200.0, 0.0);
        regular.priority = RadarPriorityType::Unit;
        radar.add_object(regular);

        assert!(radar.remove_object(2));
        assert_eq!(radar.local_object_list.len(), 1);
        assert_eq!(radar.object_list.len(), 0);

        assert!(radar.remove_object(1));
        assert_eq!(radar.local_object_list.len(), 0);
        assert_eq!(radar.object_list.len(), 0);
    }

    #[test]
    fn test_remove_object_survives_priority_insert_shifts() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let mut unit = RadarObject::new(1);
        unit.world_pos = Coord3D::new(100.0, 100.0, 0.0);
        unit.priority = RadarPriorityType::Unit;
        radar.add_object(unit);

        let mut structure = RadarObject::new(2);
        structure.world_pos = Coord3D::new(200.0, 200.0, 0.0);
        structure.priority = RadarPriorityType::Structure;
        radar.add_object(structure);

        assert_eq!(radar.object_list[0].object_id, 2);
        assert_eq!(radar.object_list[1].object_id, 1);

        assert!(radar.remove_object(1));
        assert_eq!(radar.object_list.len(), 1);
        assert_eq!(radar.object_list[0].object_id, 2);
    }

    #[test]
    fn test_event_colors() {
        let (color1, _color2) = RadarEventType::UnderAttack.get_colors();
        assert_eq!(color1.r, 255);
        assert_eq!(color1.g, 0);
        assert_eq!(color1.b, 0);
    }

    #[test]
    fn test_shroud_management() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        // Initially all shrouded
        assert_eq!(radar.get_shroud_level(64, 64), CellShroudStatus::Shrouded);

        // Set to clear
        radar.set_shroud_level(64, 64, CellShroudStatus::Clear);
        assert_eq!(radar.get_shroud_level(64, 64), CellShroudStatus::Clear);

        // Clear all shroud
        radar.clear_shroud();
        assert_eq!(radar.get_shroud_level(64, 64), CellShroudStatus::Clear);
        assert!(radar.is_shroud_cleared());
    }

    #[test]
    fn test_shroud_texture_matches_w3d_alpha_levels() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );
        radar.set_shroud_level(0, 0, CellShroudStatus::Clear);
        radar.set_shroud_level(1, 0, CellShroudStatus::Fogged);
        radar.set_shroud_level(2, 0, CellShroudStatus::Shrouded);

        let texture = radar.build_shroud_texture_rgba();

        assert_eq!(
            texture.len(),
            (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT * 4) as usize
        );
        assert_eq!(&texture[0..4], &[0, 0, 0, 0]);
        assert_eq!(&texture[4..8], &[0, 0, 0, 127]);
        assert_eq!(&texture[8..12], &[0, 0, 0, 255]);
    }

    #[test]
    fn test_object_overlay_texture_matches_w3d_2x2_blip_shape() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(128.0, 128.0, 0.0),
            &[],
        );
        radar.clear_shroud();
        let mut obj = RadarObject::new(1);
        obj.world_pos = Coord3D::new(10.0, 20.0, 0.0);
        obj.priority = RadarPriorityType::Unit;
        obj.color = 0xAA112233;
        radar.add_object(obj);

        let texture = radar.build_object_overlay_texture_rgba();
        let pixel = |x: u32, y: u32| -> &[u8] {
            let idx = ((y * RADAR_CELL_WIDTH + x) * 4) as usize;
            &texture[idx..idx + 4]
        };

        assert_eq!(pixel(10, 20), &[0x11, 0x22, 0x33, 0xAA]);
        assert_eq!(pixel(10, 21), &[0x11, 0x22, 0x33, 0xAA]);
        assert_eq!(pixel(11, 21), &[0x11, 0x22, 0x33, 0xAA]);
        assert_eq!(pixel(11, 20), &[0x11, 0x22, 0x33, 0xAA]);
        assert_eq!(pixel(9, 20), &[0, 0, 0, 0]);
    }

    #[test]
    fn test_object_overlay_texture_skips_fogged_and_nonlocal_local_only_blips() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(128.0, 128.0, 0.0),
            &[],
        );
        radar.clear_shroud();

        let mut fogged = RadarObject::new(1);
        fogged.world_pos = Coord3D::new(10.0, 20.0, 0.0);
        fogged.priority = RadarPriorityType::Unit;
        fogged.color = 0xFFFF0000;
        radar.add_object(fogged);
        radar.set_shroud_level(10, 20, CellShroudStatus::Fogged);

        let mut nonlocal_local_only = RadarObject::new(2);
        nonlocal_local_only.world_pos = Coord3D::new(30.0, 40.0, 0.0);
        nonlocal_local_only.priority = RadarPriorityType::LocalUnitOnly;
        nonlocal_local_only.color = 0xFF00FF00;
        nonlocal_local_only.is_local = false;
        radar.add_object(nonlocal_local_only);

        let mut local_only = RadarObject::new(3);
        local_only.world_pos = Coord3D::new(50.0, 60.0, 0.0);
        local_only.priority = RadarPriorityType::LocalUnitOnly;
        local_only.color = 0xFF0000FF;
        local_only.is_local = true;
        radar.add_object(local_only);

        let texture = radar.build_object_overlay_texture_rgba();
        let pixel = |x: u32, y: u32| -> &[u8] {
            let idx = ((y * RADAR_CELL_WIDTH + x) * 4) as usize;
            &texture[idx..idx + 4]
        };

        assert_eq!(pixel(10, 20), &[0, 0, 0, 0]);
        assert_eq!(pixel(30, 40), &[0, 0, 0, 0]);
        assert_eq!(pixel(50, 60), &[0, 0x00, 0xFF, 0xFF]);
    }

    #[test]
    fn test_shroud_circle() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let center = Coord3D::new(512.0, 512.0, 0.0);
        let radius = 100.0;

        radar.set_shroud_circle(&center, radius, CellShroudStatus::Clear);

        // Center should be clear
        let center_radar = radar.world_to_radar(&center).unwrap();
        assert_eq!(
            radar.get_shroud_level(center_radar.x, center_radar.y),
            CellShroudStatus::Clear
        );
    }

    #[test]
    fn test_stealth_detection() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        // Add stealth detection radar
        let mut detector = RadarObject::new(1);
        detector.color = 0xFFFFFFFF;
        detector.world_pos = Coord3D::new(500.0, 500.0, 0.0);
        detector.priority = RadarPriorityType::Structure;
        detector.radar_range = 200.0;
        detector.can_detect_stealth = true;
        radar.add_object(detector);

        // Add stealth unit nearby
        let mut stealth_unit = RadarObject::new(2);
        stealth_unit.color = 0xFF0000FF;
        stealth_unit.world_pos = Coord3D::new(550.0, 550.0, 0.0);
        stealth_unit.priority = RadarPriorityType::Unit;
        stealth_unit.is_stealth = true;
        radar.add_object(stealth_unit);

        // Update stealth detection
        radar.update_stealth_detection();

        // Verify stealth unit is revealed
        let revealed_count = radar
            .get_all_objects()
            .filter(|obj| obj.is_stealth && obj.stealth_revealed)
            .count();
        assert_eq!(revealed_count, 1);
    }

    #[test]
    fn test_radar_range_detection() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        // Add radar with range
        let mut radar_obj = RadarObject::new(1);
        radar_obj.color = 0xFFFFFFFF;
        radar_obj.world_pos = Coord3D::new(500.0, 500.0, 0.0);
        radar_obj.priority = RadarPriorityType::Structure;
        radar_obj.radar_range = 100.0;
        radar.add_object(radar_obj);

        // Position within range
        let in_range = Coord3D::new(550.0, 550.0, 0.0);
        assert!(radar.is_position_in_radar_range(&in_range, 0));

        // Position out of range
        let out_of_range = Coord3D::new(700.0, 700.0, 0.0);
        assert!(!radar.is_position_in_radar_range(&out_of_range, 0));
    }

    #[test]
    fn test_stealth_events() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let world_loc = Coord3D::new(100.0, 100.0, 0.0);

        // First event should succeed
        assert!(radar.try_stealth_discovered_event(&world_loc));

        // Second event at same location should fail (too soon)
        assert!(!radar.try_stealth_discovered_event(&world_loc));

        // Event far away should succeed
        let far_loc = Coord3D::new(500.0, 500.0, 0.0);
        assert!(radar.try_stealth_discovered_event(&far_loc));
    }

    #[test]
    fn test_visible_objects_filtering() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        // Add visible object
        let mut visible = RadarObject::new(1);
        visible.world_pos = Coord3D::new(100.0, 100.0, 0.0);
        visible.priority = RadarPriorityType::Unit;
        radar.add_object(visible);

        // Add hidden stealth object
        let mut hidden = RadarObject::new(2);
        hidden.world_pos = Coord3D::new(200.0, 200.0, 0.0);
        hidden.priority = RadarPriorityType::Unit;
        hidden.is_stealth = true;
        radar.add_object(hidden);

        // Should only get visible objects
        assert_eq!(radar.get_visible_objects().len(), 1);
        assert_eq!(radar.get_all_objects().count(), 2);
    }

    #[test]
    fn test_refresh_terrain_uses_sampled_height_gradient() {
        let mut radar = RadarSystem::new();
        let sample_count = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        let mut samples = Vec::with_capacity(sample_count);
        for y in 0..RADAR_CELL_HEIGHT {
            for x in 0..RADAR_CELL_WIDTH {
                let height = x as f32 / (RADAR_CELL_WIDTH - 1) as f32 * 80.0 + y as f32 * 0.01;
                samples.push((x as f32, height, false));
            }
        }

        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &samples,
        );
        radar.refresh_terrain();
        let tex = radar.get_terrain_texture();
        let left = &tex[0..4];
        let right_offset = ((RADAR_CELL_WIDTH - 1) * 4) as usize;
        let right = &tex[right_offset..right_offset + 4];

        assert_ne!(left[0], right[0]);
        assert_ne!(left[1], right[1]);
    }

    #[test]
    fn test_refresh_terrain_tints_water_cells_blue() {
        let mut radar = RadarSystem::new();
        let sample_count = (RADAR_CELL_WIDTH * RADAR_CELL_HEIGHT) as usize;
        let mut samples = Vec::with_capacity(sample_count);
        for i in 0..sample_count {
            let is_water = i == 0;
            samples.push((0.0, 10.0, is_water));
        }

        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &samples,
        );
        radar.refresh_terrain();
        let tex = radar.get_terrain_texture();
        let first = &tex[0..4];

        assert!(first[2] > first[0]);
    }

    #[test]
    fn test_examine_object() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        // Add object
        let mut obj = RadarObject::new(1);
        obj.world_pos = Coord3D::new(100.0, 100.0, 0.0);
        obj.priority = RadarPriorityType::Unit;
        radar.add_object(obj.clone());

        // Update object position
        let mut updated = obj;
        updated.world_pos = Coord3D::new(200.0, 200.0, 0.0);
        radar.examine_object(1, updated);

        // Should still have 1 object
        assert_eq!(radar.get_all_objects().count(), 1);
    }

    // ===== New comprehensive radar tests =====

    #[test]
    fn test_gps_satellite_activation() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        // GPS not active initially
        assert!(!radar.is_gps_active());

        // Activate GPS for 900 frames (30 seconds)
        radar.activate_gps_satellite(900);
        assert!(radar.is_gps_active());

        // All shroud should be clear
        assert!(radar
            .get_shroud_grid()
            .iter()
            .all(|&cell| cell == CellShroudStatus::Clear));

        // Update to expiration
        radar.update(900);
        assert!(!radar.is_gps_active());
    }

    #[test]
    fn test_radar_scan_activation() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let scan_location = Coord3D::new(500.0, 500.0, 0.0);
        let scan_radius = 300.0;
        let player_id = 1;

        // Activate radar scan
        radar.activate_radar_scan(scan_location, scan_radius, 300, player_id);

        // Check that scan is active
        let scans = radar.get_active_radar_scans(player_id);
        assert_eq!(scans.len(), 1);

        // Position inside scan should be revealed
        let inside_pos = Coord3D::new(550.0, 550.0, 0.0);
        assert!(radar.is_position_in_radar_scan(&inside_pos, player_id));

        // Position outside scan should not be revealed
        let outside_pos = Coord3D::new(900.0, 900.0, 0.0);
        assert!(!radar.is_position_in_radar_scan(&outside_pos, player_id));

        // Update past expiration
        radar.update(301);
        assert_eq!(radar.get_active_radar_scans(player_id).len(), 0);
    }

    #[test]
    fn test_radar_jamming() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        let jammer_location = Coord3D::new(500.0, 500.0, 0.0);
        let jamming_radius = 200.0;
        let jammer_player_id = 2; // Enemy player

        // Add jamming source
        radar.add_jamming_source(100, jammer_location, jamming_radius, jammer_player_id);

        // Position inside jamming radius should be jammed for player 1
        let jammed_pos = Coord3D::new(550.0, 550.0, 0.0);
        assert!(radar.is_position_jammed(&jammed_pos, 1));

        // Position outside jamming radius should not be jammed
        let clear_pos = Coord3D::new(800.0, 800.0, 0.0);
        assert!(!radar.is_position_jammed(&clear_pos, 1));

        // Disable jammer
        radar.set_jamming_source_active(100, false);
        assert!(!radar.is_position_jammed(&jammed_pos, 1));

        // Remove jammer
        radar.remove_jamming_source(100);
        assert!(radar.jamming_sources.is_empty());
    }

    #[test]
    fn test_radar_power_states() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        // Add radar provider
        let mut radar_obj = RadarObject::new(1);
        radar_obj.world_pos = Coord3D::new(500.0, 500.0, 0.0);
        radar_obj.priority = RadarPriorityType::Structure;
        radar_obj.radar_range = 200.0;
        radar_obj.is_radar_provider = true;
        radar_obj.is_powered = true;
        radar.add_object(radar_obj);

        // Radar should be operational
        assert!(radar.has_operational_radar(0));

        // Disable power
        radar.set_radar_powered(1, false);
        assert!(!radar.has_operational_radar(0));

        // Re-enable power
        radar.set_radar_powered(1, true);
        assert!(radar.has_operational_radar(0));
    }

    #[test]
    fn test_radar_emp_disable() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        // Add radar provider
        let mut radar_obj = RadarObject::new(1);
        radar_obj.world_pos = Coord3D::new(500.0, 500.0, 0.0);
        radar_obj.priority = RadarPriorityType::Structure;
        radar_obj.radar_range = 200.0;
        radar_obj.is_radar_provider = true;
        radar.add_object(radar_obj);

        // Radar operational initially
        assert!(radar.has_operational_radar(0));

        // Apply EMP
        radar.disable_radar_object(1);
        assert!(!radar.has_operational_radar(0));

        // EMP expires
        radar.enable_radar_object(1);
        assert!(radar.has_operational_radar(0));
    }

    #[test]
    fn test_radar_object_operational_check() {
        let mut obj = RadarObject::new(1);
        obj.is_radar_provider = true;
        obj.is_powered = true;
        obj.is_disabled = false;

        // Should be operational
        assert!(obj.is_radar_operational());

        // Not powered
        obj.set_powered(false);
        assert!(!obj.is_radar_operational());
        obj.set_powered(true);

        // Disabled by EMP
        obj.disable_radar();
        assert!(!obj.is_radar_operational());
        obj.enable_radar();
        assert!(obj.is_radar_operational());
    }

    #[test]
    fn test_radar_scan_expiration() {
        let scan = RadarScan::new(
            Coord3D::new(100.0, 100.0, 0.0),
            300.0,
            100, // duration
            1,   // player_id
            0,   // current_frame
        );

        assert!(!scan.is_expired(50));
        assert!(!scan.is_expired(99));
        assert!(scan.is_expired(100));
        assert!(scan.is_expired(150));
    }

    #[test]
    fn test_jamming_source_range() {
        let jammer = JammingSource::new(1, Coord3D::new(500.0, 500.0, 0.0), 200.0, 1);

        // Position inside radius
        let inside = Coord3D::new(550.0, 550.0, 0.0);
        assert!(jammer.is_position_jammed(&inside));

        // Position outside radius
        let outside = Coord3D::new(800.0, 800.0, 0.0);
        assert!(!jammer.is_position_jammed(&outside));
    }

    #[test]
    fn test_jamming_update_on_objects() {
        let mut radar = RadarSystem::new();
        radar.new_map(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1024.0, 1024.0, 100.0),
            &[],
        );

        // Add object
        let mut obj = RadarObject::new(10);
        obj.world_pos = Coord3D::new(550.0, 550.0, 0.0);
        obj.priority = RadarPriorityType::Unit;
        radar.add_object(obj);

        // Add jammer affecting object
        radar.add_jamming_source(1, Coord3D::new(500.0, 500.0, 0.0), 200.0, 2);

        // Update jamming status
        radar.update_jamming_status();

        // Object should be jammed
        let obj_jammed = radar
            .get_all_objects()
            .find(|o| o.object_id == 10)
            .map(|o| o.is_jammed)
            .unwrap_or(false);
        assert!(obj_jammed);
    }

    #[test]
    fn test_shroud_status_visibility() {
        assert!(CellShroudStatus::Clear.is_visible());
        assert!(CellShroudStatus::Fogged.is_visible());
        assert!(!CellShroudStatus::Shrouded.is_visible());

        assert!(CellShroudStatus::Clear.is_explored());
        assert!(CellShroudStatus::Fogged.is_explored());
        assert!(!CellShroudStatus::Shrouded.is_explored());
    }
}
