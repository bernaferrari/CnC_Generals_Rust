//! # In-Game UI System
//!
//! Comprehensive in-game user interface system ported from C++ InGameUI.cpp
//! Handles all in-game UI elements including selection, minimap, resource display,
//! and building placement preview.
//!
//! Original C++ file: GameClient/InGameUI.cpp
//! Original Author: Michael S. Booth, March 2001

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use glam::{Vec2, Vec3};
use thiserror::Error;
use wgpu::TextureView;

use super::ui_renderer::{UIRect, UIRenderer, UIRendererError};
use super::window_video_manager::with_window_video_manager;
use crate::display::view::{with_tactical_view, with_tactical_view_ref, IPoint2, Point3};
use crate::game_text::GameText;
use crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text;
use crate::helpers::TheInGameUI;
use crate::input::keyboard::KeyboardState;
use crate::input::mouse::{with_mouse, ButtonState, MouseButton, MouseState};
use crate::message_stream::game_message::{
    Coord3D as MsgCoord3D, GameMessageType, ICoord2D as MsgICoord2D,
};
use crate::message_stream::message_stream::append_message_to_stream;
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::global_data;
use game_engine::common::ini::get_anim2d_collection;
use game_engine::common::ini::ini_language::{get_global_language_read, FontDesc};
use game_engine::common::thing::get_thing_factory;
use gamelogic::action_manager::ActionManager;
use gamelogic::commands::selection::{get_selection_manager, SelectionType};
use gamelogic::common::CommandSourceType;
use gamelogic::common::{
    Coord3D, ICoord2D, IRegion2D, KindOf, ObjectID, ObjectShroudStatus, MAX_PLAYER_COUNT,
};
use gamelogic::helpers::{TheGameLogic, TheThingFactory};
use gamelogic::object::production::construction::FoundationValidator;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::object::special_power_template::get_special_power_store;
use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;
use gamelogic::object::Object;
use gamelogic::system::shroud_manager::{get_shroud_manager, ShroudState};

/// Re-export of the INI settings type from the Common crate's INI parser.
/// C++: InGameUI fieldParseTable settings (InGameUI.cpp:752-856, ini_in_game_ui.rs)
pub use game_engine::common::ini::ini_in_game_ui::{
    Coord2D as IniCoord2D, ICoord2D as IniICoord2D, InGameUISettings as InGameUIIniSettings,
    RGBAColorInt,
};

/// In-game UI errors
#[derive(Error, Debug)]
pub enum InGameUIError {
    #[error("Renderer error: {0}")]
    RendererError(#[from] UIRendererError),
    #[error("Invalid selection: {0}")]
    InvalidSelection(String),
    #[error("Invalid object ID: {0}")]
    InvalidObjectID(u32),
    #[error("System error: {0}")]
    SystemError(String),
}

type Result<T> = std::result::Result<T, InGameUIError>;

/// Placement opacity for building preview (C++ InGameUI.cpp:77)
const PLACEMENT_OPACITY: f32 = 0.45;

/// Illegal build color - red (C++ InGameUI.cpp:78)
const ILLEGAL_BUILD_COLOR: [f32; 3] = [1.0, 0.0, 0.0];

/// Legal build color - green
const LEGAL_BUILD_COLOR: [f32; 3] = [0.0, 1.0, 0.0];

/// Maximum selection count
const MAX_SELECTION_COUNT: usize = 200;

/// Double-click time threshold (milliseconds)
const DOUBLE_CLICK_TIME_MS: u64 = 500;

/// Minimum drag distance for selection box (pixels)
const MIN_DRAG_DISTANCE: f32 = 5.0;

/// Minimum drag distance for line build placement (pixels)
const PLACEMENT_DRAG_DISTANCE: f32 = 5.0;

/// Default floating text timeout in logic frames (C++: LOGICFRAMES_PER_SECOND / 3 = 10)
const DEFAULT_FLOATING_TEXT_TIMEOUT: u32 = 10;

/// Maximum number of floating text entries
const MAX_FLOATING_TEXT: usize = 30;

/// C++: InGameUI::UIMessage (InGameUI.h:615-621)
/// Stores a single HUD message text entry. Newer messages are at lower indices.
#[derive(Debug, Clone)]
pub struct MessageText {
    /// The full text to display
    pub text: String,
    /// Packed ARGB color for this message instance (stays with it across shifts)
    pub color: u32,
    /// Logic frame when this message was created
    pub creation_frame: u32,
}

/// C++: MAX_UI_MESSAGES = 6 (InGameUI.h:622)
const MAX_UI_MESSAGES: usize = 6;

/// C++: InGameUI::MilitarySubtitleData (InGameUI.h:624-637)
/// Stores state for the military-style caption overlay.
#[derive(Debug, Clone)]
pub struct MilitarySubtitle {
    /// The complete subtitle text (each line separated by "\n")
    pub text: String,
    /// Current character index for typewriter effect
    pub index: usize,
    /// Screen position for drawing
    pub position: (f32, f32),
    /// Lifetime end frame (absolute logic frame)
    pub lifetime_frame: u32,
    /// Whether the typewriter block is drawn (true) or blank (false)
    pub block_drawn: bool,
    /// Frame at which the current block state started
    pub block_begin_frame: u32,
    /// Position where the upper-left of the block should begin
    pub block_pos: (f32, f32),
    /// If current frame >= this, increment typewriter position
    pub increment_on_frame: u32,
    /// ARGB color for subtitle text
    pub color: u32,
}

/// C++: InGameUI::FloatingTextData (InGameUI.h)
#[derive(Debug, Clone)]
pub struct FloatingTextData {
    pub text: String,
    pub position: Coord3D,
    pub color: (u8, u8, u8),
    pub creation_frame: u32,
    pub timeout: u32,
    pub move_up_speed: f32,
}

/// C++: NamedTimerInfo (InGameUI.h:217-228)
#[derive(Debug, Clone)]
pub struct NamedTimerData {
    pub name: String,
    pub text: String,
    pub is_countdown: bool,
}

/// Mouse cursor types. C++: Mouse::MouseCursor (Mouse.h:121-190)
/// Ordering and discriminant values must match C++ exactly for save/load parity.
/// Note: C++ has `NORMAL = FIRST_CURSOR` (both value 1) which Rust cannot represent
/// as two variants with the same discriminant, so we use `FirstCursor` to represent both.
/// Conditional variants (#ifdef ALLOW_DEMORALIZE / ALLOW_SURRENDER) are excluded since
/// they are not defined in retail Zero Hour builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum MouseCursor {
    /// C++: INVALID_MOUSE_CURSOR = -1
    Invalid = -1,
    /// C++: NONE = 0
    None = 0,
    /// C++: FIRST_CURSOR = 1, NORMAL = FIRST_CURSOR = 1
    FirstCursor = 1,
    /// C++: ARROW = 2
    Arrow = 2,
    /// C++: SCROLL = 3
    Scroll = 3,
    /// C++: CROSS = 4
    Cross = 4,
    /// C++: MOVETO = 5
    MoveTo = 5,
    /// C++: ATTACKMOVETO = 6
    AttackMoveTo = 6,
    /// C++: ATTACK_OBJECT = 7
    AttackObject = 7,
    /// C++: FORCE_ATTACK_OBJECT = 8
    ForceAttackObject = 8,
    /// C++: FORCE_ATTACK_GROUND = 9
    ForceAttackGround = 9,
    /// C++: BUILD_PLACEMENT = 10
    BuildPlacement = 10,
    /// C++: INVALID_BUILD_PLACEMENT = 11
    InvalidBuildPlacement = 11,
    /// C++: GENERIC_INVALID = 12
    GenericInvalid = 12,
    /// C++: SELECTING = 13
    Selecting = 13,
    /// C++: ENTER_FRIENDLY = 14
    EnterFriendly = 14,
    /// C++: ENTER_AGGRESSIVELY = 15
    EnterAggressively = 15,
    /// C++: SET_RALLY_POINT = 16
    SetRallyPoint = 16,
    /// C++: GET_REPAIRED = 17
    GetRepaired = 17,
    /// C++: GET_HEALED = 18
    GetHealed = 18,
    /// C++: DO_REPAIR = 19
    DoRepair = 19,
    /// C++: RESUME_CONSTRUCTION = 20
    ResumeConstruction = 20,
    /// C++: CAPTUREBUILDING = 21
    CaptureBuilding = 21,
    /// C++: SNIPE_VEHICLE = 22
    SnipeVehicle = 22,
    /// C++: LASER_GUIDED_MISSILES = 23
    LaserGuidedMissiles = 23,
    /// C++: TANKHUNTER_TNT_ATTACK = 24
    TankHunterTntAttack = 24,
    /// C++: STAB_ATTACK = 25
    StabAttack = 25,
    /// C++: PLACE_REMOTE_CHARGE = 26
    PlaceRemoteCharge = 26,
    /// C++: PLACE_TIMED_CHARGE = 27
    PlaceTimedCharge = 27,
    /// C++: DEFECTOR = 28
    Defector = 28,
    /// C++: DOCK = 29
    Dock = 29,
    /// C++: FIRE_FLAME = 30
    FireFlame = 30,
    /// C++: FIRE_BOMB = 31
    FireBomb = 31,
    /// C++: PLACE_BEACON = 32
    PlaceBeacon = 32,
    /// C++: DISGUISE_AS_VEHICLE = 33
    DisguiseAsVehicle = 33,
    /// C++: WAYPOINT = 34
    Waypoint = 34,
    /// C++: OUTRANGE = 35
    OutOfRange = 35,
    /// C++: STAB_ATTACK_INVALID = 36
    StabAttackInvalid = 36,
    /// C++: PLACE_CHARGE_INVALID = 37
    PlaceChargeInvalid = 37,
    /// C++: HACK = 38
    Hack = 38,
    /// C++: PARTICLE_UPLINK_CANNON = 39
    ParticleUplinkCannon = 39,
    /// C++: NUM_MOUSE_CURSORS = 40 (sentinel, keep last)
    NumMouseCursors = 40,
}

impl Default for MouseCursor {
    fn default() -> Self {
        Self::Arrow
    }
}

impl MouseCursor {
    /// Total number of cursor types (excluding Invalid and NumMouseCursors sentinels).
    pub const COUNT: i32 = 40;

    /// Convert discriminant to enum, returning None for out-of-range values.
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            -1 => Some(Self::Invalid),
            0 => Some(Self::None),
            1 => Some(Self::FirstCursor),
            2 => Some(Self::Arrow),
            3 => Some(Self::Scroll),
            4 => Some(Self::Cross),
            5 => Some(Self::MoveTo),
            6 => Some(Self::AttackMoveTo),
            7 => Some(Self::AttackObject),
            8 => Some(Self::ForceAttackObject),
            9 => Some(Self::ForceAttackGround),
            10 => Some(Self::BuildPlacement),
            11 => Some(Self::InvalidBuildPlacement),
            12 => Some(Self::GenericInvalid),
            13 => Some(Self::Selecting),
            14 => Some(Self::EnterFriendly),
            15 => Some(Self::EnterAggressively),
            16 => Some(Self::SetRallyPoint),
            17 => Some(Self::GetRepaired),
            18 => Some(Self::GetHealed),
            19 => Some(Self::DoRepair),
            20 => Some(Self::ResumeConstruction),
            21 => Some(Self::CaptureBuilding),
            22 => Some(Self::SnipeVehicle),
            23 => Some(Self::LaserGuidedMissiles),
            24 => Some(Self::TankHunterTntAttack),
            25 => Some(Self::StabAttack),
            26 => Some(Self::PlaceRemoteCharge),
            27 => Some(Self::PlaceTimedCharge),
            28 => Some(Self::Defector),
            29 => Some(Self::Dock),
            30 => Some(Self::FireFlame),
            31 => Some(Self::FireBomb),
            32 => Some(Self::PlaceBeacon),
            33 => Some(Self::DisguiseAsVehicle),
            34 => Some(Self::Waypoint),
            35 => Some(Self::OutOfRange),
            36 => Some(Self::StabAttackInvalid),
            37 => Some(Self::PlaceChargeInvalid),
            38 => Some(Self::Hack),
            39 => Some(Self::ParticleUplinkCannon),
            40 => Some(Self::NumMouseCursors),
            _ => None,
        }
    }
}

/// Mouse interaction mode. C++: InGameUI::MouseMode (InGameUI.h:599-605)
/// Tracks what kind of mouse interaction is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum MouseMode {
    /// C++: MOUSEMODE_DEFAULT = 0 — normal gameplay cursor
    #[default]
    Default = 0,
    /// C++: MOUSEMODE_BUILD_PLACE = 1 — placing a building/structure
    BuildPlace = 1,
    /// C++: MOUSEMODE_GUI_COMMAND = 2 — executing a UI command button action
    GuiCommand = 2,
}

impl MouseMode {
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => Self::BuildPlace,
            2 => Self::GuiCommand,
            _ => Self::Default,
        }
    }
}

/// Hint types for visual command feedback. C++: InGameUI::HintType (InGameUI.h:588-596)
/// MOVE_HINT = 0, ATTACK_HINT = 1, DEBUG_HINT = 2 (debug only), NUM_HINT_TYPES = 2 or 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum HintType {
    /// C++: MOVE_HINT = 0
    Move = 0,
    /// C++: ATTACK_HINT = 1
    Attack = 1,
    /// C++: FORCE_ATTACK (Rust extension for force attack hints)
    ForceAttack = 2,
    /// C++: GARRISON_HINT (Rust extension for garrison hints)
    Garrison = 3,
    /// C++: COMMAND_HINT (Rust extension for command hints)
    Command = 4,
    /// C++: Area selection hint
    AreaSelect = 5,
}

/// Hint data for visual command feedback. C++: MoveHintStruct (InGameUI.h:608-613)
/// Stores a world-space command indicator that fades over time.
#[derive(Debug, Clone)]
pub struct HintData {
    /// The type of hint being displayed
    pub hint_type: HintType,
    /// World-space start position (e.g., unit position for move commands)
    pub start: Coord3D,
    /// World-space end position (e.g., destination for move commands)
    pub end: Coord3D,
    /// Logic frame when this hint was created. C++: m_moveHint[].frame
    pub creation_frame: u32,
    /// Source object ID that issued this command. C++: m_moveHint[].sourceID
    pub source_id: u32,
    /// How many logic frames this hint should be displayed (30 FPS standard).
    /// C++: hints are drawn while frame != 0, expired by setting frame = 0.
    pub lifetime_frames: u32,
}

/// Maximum number of simultaneous move hints. C++: MAX_MOVE_HINTS = 256
const MAX_MOVE_HINTS: usize = 256;

/// C++: InGameUI m_idleWorkers[MAX_PLAYER_COUNT] — per-player idle worker tracking
#[derive(Debug, Clone)]
pub struct IdleWorkerData {
    pub object_id: ObjectID,
    pub player_index: u8,
}

/// Radius cursor types. C++: RadiusCursorType enum (InGameUI.h:45-84)
/// Tracks the kind of radius decal overlay shown when targeting special powers or attacks.
/// Ordering and values must match C++ for parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum RadiusCursorType {
    None = 0,
    AttackDamageArea = 1,
    AttackScatterArea = 2,
    AttackContinueArea = 3,
    GuardArea = 4,
    EmergencyRepair = 5,
    FriendlySpecialPower = 6,
    OffensiveSpecialPower = 7,
    SuperweaponScatterArea = 8,
    ParticleCannon = 9,
    A10Strike = 10,
    CarpetBomb = 11,
    DaisyCutter = 12,
    Paradrop = 13,
    SpySatellite = 14,
    SpectreGunship = 15,
    HelixNapalmBomb = 16,
    NuclearMissile = 17,
    EmpPulse = 18,
    ArtilleryBarrage = 19,
    NapalmStrike = 20,
    ClusterMines = 21,
    ScudStorm = 22,
    AnthraxBomb = 23,
    Ambush = 24,
    Radar = 25,
    SpyDrone = 26,
    Frenzy = 27,
    ClearMines = 28,
    Ambulance = 29,
    /// Sentinel — must be last. C++: RADIUSCURSOR_COUNT
    Count = 30,
}

impl RadiusCursorType {
    /// Total number of radius cursor types. C++: RADIUSCURSOR_COUNT
    pub const COUNT: u32 = 30;
}

impl Default for RadiusCursorType {
    fn default() -> Self {
        Self::None
    }
}

/// Radius cursor state. C++: m_curRadiusCursor + m_curRcType (InGameUI.h:799-801)
/// State-only tracking — actual decal rendering is handled by the rendering subsystem.
#[derive(Debug, Clone)]
pub struct RadiusCursorState {
    pub cursor_type: RadiusCursorType,
    pub active: bool,
    pub position: Coord3D,
    pub radius: f32,
}

impl RadiusCursorState {
    pub fn new() -> Self {
        Self {
            cursor_type: RadiusCursorType::None,
            active: false,
            position: Coord3D::new(0.0, 0.0, 0.0),
            radius: 0.0,
        }
    }
}

impl Default for RadiusCursorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Superweapon timer tracking data. C++: SuperweaponInfo (InGameUI.h:148-193)
/// Simplified from C++ — state-only tracking; rendering handled by the UI subsystem.
#[derive(Debug, Clone)]
pub struct SuperweaponTimerData {
    pub player_index: u8,
    pub object_id: ObjectID,
    pub power_name: String,
    pub ready_frame: u32,
    pub countdown_text: String,
    pub ready: bool,
    pub hidden_by_script: bool,
    pub hidden_by_science: bool,
}

/// Selection box representation
#[derive(Debug, Clone, Copy)]
pub struct SelectionBox {
    /// Starting position (screen coordinates)
    pub start: Vec2,
    /// Current position (screen coordinates)
    pub current: Vec2,
    /// Whether the selection box is active
    pub active: bool,
}

impl SelectionBox {
    pub fn new() -> Self {
        Self {
            start: Vec2::ZERO,
            current: Vec2::ZERO,
            active: false,
        }
    }

    pub fn start_at(&mut self, pos: Vec2) {
        self.start = pos;
        self.current = pos;
        self.active = true;
    }

    pub fn update(&mut self, pos: Vec2) {
        self.current = pos;
    }

    pub fn finish(&mut self) {
        self.active = false;
    }

    pub fn get_rect(&self) -> UIRect {
        let min_x = self.start.x.min(self.current.x);
        let min_y = self.start.y.min(self.current.y);
        let max_x = self.start.x.max(self.current.x);
        let max_y = self.start.y.max(self.current.y);

        UIRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    pub fn is_significant(&self) -> bool {
        let dx = self.current.x - self.start.x;
        let dy = self.current.y - self.start.y;
        (dx * dx + dy * dy).sqrt() > MIN_DRAG_DISTANCE
    }
}

/// Drawable object reference (simplified for now)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawableID(pub u32);

/// Object selection state
#[derive(Debug)]
pub struct SelectionState {
    /// Currently selected objects
    selected: Vec<DrawableID>,
    /// Maximum allowed selection count
    max_selection: usize,
    /// Last click time for double-click detection
    last_click_time: Option<Instant>,
    /// Last click position
    last_click_pos: Option<Vec2>,
    /// Selection groups (0-9)
    selection_groups: [Vec<DrawableID>; 10],
}

impl SelectionState {
    pub fn new(max_selection: usize) -> Self {
        Self {
            selected: Vec::new(),
            max_selection,
            last_click_time: None,
            last_click_pos: None,
            selection_groups: Default::default(),
        }
    }

    pub fn select(&mut self, drawable_id: DrawableID, add_to_selection: bool) {
        if !add_to_selection {
            self.selected.clear();
        }

        if !self.selected.contains(&drawable_id) && self.selected.len() < self.max_selection {
            self.selected.push(drawable_id);
        }
    }

    pub fn deselect(&mut self, drawable_id: DrawableID) {
        self.selected.retain(|&id| id != drawable_id);
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    pub fn is_selected(&self, drawable_id: DrawableID) -> bool {
        self.selected.contains(&drawable_id)
    }

    pub fn get_selected(&self) -> &[DrawableID] {
        &self.selected
    }

    pub fn count(&self) -> usize {
        self.selected.len()
    }

    pub fn set_group(&mut self, group: usize, selection: Vec<DrawableID>) {
        if group < 10 {
            self.selection_groups[group] = selection;
        }
    }

    pub fn get_group(&self, group: usize) -> Option<&[DrawableID]> {
        if group < 10 {
            Some(&self.selection_groups[group])
        } else {
            None
        }
    }

    pub fn detect_double_click(&mut self, pos: Vec2) -> bool {
        let now = Instant::now();
        let is_double = if let (Some(last_time), Some(last_pos)) =
            (self.last_click_time, self.last_click_pos)
        {
            let time_ok = now.duration_since(last_time).as_millis() < DOUBLE_CLICK_TIME_MS as u128;
            let dist = (pos - last_pos).length();
            time_ok && dist < 10.0
        } else {
            false
        };

        self.last_click_time = Some(now);
        self.last_click_pos = Some(pos);

        is_double
    }
}

/// Building placement preview state
#[derive(Debug, Clone)]
pub struct PlacementPreview {
    /// Building template name
    pub template_name: String,
    /// World position
    pub position: Vec3,
    /// Rotation angle (radians)
    pub rotation: f32,
    /// Whether placement is legal at current position
    pub is_legal: bool,
    /// Building footprint size
    pub footprint: Vec2,
    /// Preview mesh/texture
    pub preview_texture: Option<String>,
}

impl PlacementPreview {
    pub fn new(template_name: String, footprint: Vec2) -> Self {
        Self {
            template_name,
            position: Vec3::ZERO,
            rotation: 0.0,
            is_legal: false,
            footprint,
            preview_texture: None,
        }
    }

    pub fn update_position(&mut self, position: Vec3, is_legal: bool) {
        self.position = position;
        self.is_legal = is_legal;
    }

    pub fn rotate(&mut self, delta: f32) {
        self.rotation = (self.rotation + delta) % (2.0 * std::f32::consts::PI);
    }

    pub fn get_color(&self) -> [f32; 4] {
        if self.is_legal {
            [
                LEGAL_BUILD_COLOR[0],
                LEGAL_BUILD_COLOR[1],
                LEGAL_BUILD_COLOR[2],
                PLACEMENT_OPACITY,
            ]
        } else {
            [
                ILLEGAL_BUILD_COLOR[0],
                ILLEGAL_BUILD_COLOR[1],
                ILLEGAL_BUILD_COLOR[2],
                PLACEMENT_OPACITY,
            ]
        }
    }
}

/// A single minimap ping animation.
#[derive(Debug, Clone)]
pub struct MinimapPing {
    pub world_pos: Vec2,
    pub color: [f32; 4],
    pub creation_frame: u32,
    pub lifetime_frames: u32,
}

/// Minimap state and rendering
#[derive(Debug)]
pub struct Minimap {
    /// Position on screen (bottom-left corner)
    pub position: Vec2,
    /// Size in pixels
    pub size: Vec2,
    /// World bounds represented by minimap
    pub world_bounds: (Vec2, Vec2), // (min, max)
    /// Current camera position in world
    pub camera_position: Vec3,
    /// Camera viewport size
    pub camera_viewport: Vec2,
    /// Minimap texture
    pub texture: Option<Arc<TextureView>>,
    /// Whether minimap is visible
    pub visible: bool,
    /// Unit icons on minimap
    pub unit_icons: HashMap<DrawableID, MinimapIcon>,
}

#[derive(Debug, Clone)]
pub struct MinimapIcon {
    pub position: Vec2,
    pub color: [f32; 4],
    pub size: f32,
}

impl Minimap {
    pub fn new(position: Vec2, size: Vec2) -> Self {
        Self {
            position,
            size,
            world_bounds: (Vec2::ZERO, Vec2::new(1000.0, 1000.0)),
            camera_position: Vec3::ZERO,
            camera_viewport: Vec2::new(800.0, 600.0),
            texture: None,
            visible: true,
            unit_icons: HashMap::new(),
        }
    }

    pub fn world_to_minimap(&self, world_pos: Vec2) -> Vec2 {
        let (min, max) = self.world_bounds;
        let normalized = (world_pos - min) / (max - min);
        self.position + normalized * self.size
    }

    pub fn minimap_to_world(&self, minimap_pos: Vec2) -> Vec2 {
        let (min, max) = self.world_bounds;
        let normalized = (minimap_pos - self.position) / self.size;
        min + normalized * (max - min)
    }

    pub fn contains_point(&self, screen_pos: Vec2) -> bool {
        let rect = UIRect::new(self.position.x, self.position.y, self.size.x, self.size.y);
        rect.contains(screen_pos.x, screen_pos.y)
    }

    pub fn update_icon(&mut self, id: DrawableID, world_pos: Vec2, color: [f32; 4]) {
        let minimap_pos = self.world_to_minimap(world_pos);
        self.unit_icons.insert(
            id,
            MinimapIcon {
                position: minimap_pos,
                color,
                size: 2.0,
            },
        );
    }

    pub fn remove_icon(&mut self, id: DrawableID) {
        self.unit_icons.remove(&id);
    }
}

/// Resource display HUD
#[derive(Debug, Clone)]
pub struct ResourceDisplay {
    /// Money/credits
    pub credits: i32,
    /// Power available
    pub power_available: i32,
    /// Power used
    pub power_used: i32,
    /// Display position
    pub position: Vec2,
    /// Whether to show detailed info
    pub show_details: bool,
}

impl ResourceDisplay {
    pub fn new(position: Vec2) -> Self {
        Self {
            credits: 0,
            power_available: 0,
            power_used: 0,
            position,
            show_details: true,
        }
    }

    pub fn update(&mut self, credits: i32, power_available: i32, power_used: i32) {
        self.credits = credits;
        self.power_available = power_available;
        self.power_used = power_used;
    }

    pub fn get_power_percentage(&self) -> f32 {
        if self.power_available > 0 {
            (self.power_used as f32 / self.power_available as f32).min(1.0)
        } else {
            0.0
        }
    }

    pub fn is_power_deficit(&self) -> bool {
        self.power_used > self.power_available
    }
}

/// C++: WorldAnimationOptions (InGameUI.h:269-272)
/// Bit-flag options for world animations. Ordering and values match C++ for parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldAnimationOptions(u32);

impl WorldAnimationOptions {
    pub const NONE: Self = Self(0x00000000);
    /// C++: WORLD_ANIM_FADE_ON_EXPIRE = 0x00000001
    pub const FADE_ON_EXPIRE: Self = Self(0x00000001);
    /// C++: WORLD_ANIM_PLAY_ONCE_AND_DESTROY = 0x00000002
    pub const PLAY_ONCE_AND_DESTROY: Self = Self(0x00000002);

    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

/// C++: WorldAnimationData (InGameUI.h:275-289)
/// Tracks state for a world-space 2D animation.
#[derive(Clone)]
pub struct WorldAnimationData {
    /// C++: m_anim — the live Anim2D instance
    anim: Arc<parking_lot::Mutex<crate::system::Anim2D>>,
    /// C++: m_worldPos
    world_pos: Coord3D,
    /// C++: m_expireFrame
    expire_frame: u32,
    /// C++: m_options
    options: WorldAnimationOptions,
    /// C++: m_zRisePerSecond
    z_rise_per_second: f32,
}

/// Main in-game UI manager
pub struct InGameUI {
    /// Selection box state
    selection_box: SelectionBox,

    /// Selection state
    selection_state: SelectionState,

    /// Current placement preview (if any)
    placement_preview: Option<PlacementPreview>,

    /// Minimap
    minimap: Minimap,

    /// Resource display
    resource_display: ResourceDisplay,

    /// UI renderer
    renderer: Arc<RwLock<UIRenderer>>,

    /// Screen dimensions
    screen_size: Vec2,

    /// Whether UI is enabled
    enabled: bool,

    /// Current player id (local player)
    player_id: u32,

    /// Accumulated UI time (seconds)
    ui_time: f32,

    /// Last update time
    last_update: Instant,

    pub floating_texts: Vec<FloatingTextData>,
    pub idle_workers: Vec<IdleWorkerData>,
    pub current_frame: u32,

    radius_cursor: RadiusCursorState,
    superweapon_timers: Vec<SuperweaponTimerData>,

    /// C++: m_mouseMode (InGameUI.h:770)
    mouse_mode: MouseMode,
    /// C++: TheMouse->m_currentCursor, stored here for parity. C++: m_mouseModeCursor (InGameUI.h:771)
    current_cursor: MouseCursor,
    /// C++: m_mouseModeCursor (InGameUI.h:771) — cursor to restore after GUI command completes
    mouse_mode_cursor: MouseCursor,
    /// C++: m_isScrolling (InGameUI.h:768)
    is_scrolling: bool,
    /// C++: m_isSelecting (InGameUI.h:769)
    is_selecting: bool,
    /// C++: m_scrollAmt (InGameUI.h:773)
    scroll_amount_x: f32,
    scroll_amount_y: f32,
    /// C++: m_mousedOverDrawableID (InGameUI.h:772)
    moused_over_drawable_id: u32,
    /// C++: m_moveHint[MAX_MOVE_HINTS] + m_nextMoveHint (InGameUI.h:694-695)
    hints: Vec<HintData>,
    next_hint_index: usize,

    named_timers: Vec<NamedTimerData>,
    named_timer_last_flash_frame: i32,
    named_timer_used_flash_color: bool,
    show_named_timers: bool,

    gui_command: Option<String>,
    quit_menu_visible: bool,

    window_layouts: HashMap<String, bool>,

    // ── Message display (C++: m_messageColor1/2, m_messagePosition, etc.) ──
    message_color1: u32,
    message_color2: u32,
    message_position: (i32, i32),
    message_font_name: String,
    message_point_size: i32,
    message_bold: bool,
    message_delay_ms: i32,
    messages_enabled: bool,
    messages: Vec<MessageText>,

    // ── Military subtitle (C++: m_militaryCaption*, m_militarySubtitle) ──
    military_caption_color: (u8, u8, u8, u8),
    military_caption_position: (i32, i32),
    military_caption_title_font: String,
    military_caption_title_point_size: i32,
    military_caption_title_bold: bool,
    military_caption_font: String,
    military_caption_point_size: i32,
    military_caption_bold: bool,
    military_caption_randomize_typing: bool,
    military_caption_speed: i32,
    current_military_subtitle: Option<MilitarySubtitle>,
    tooltips_disabled_until: u32,

    // ── Floating text INI values (C++: m_floatingTextTimeOut, etc.) ──
    floating_text_timeout_frames: u32,
    floating_text_move_up_speed: f32,
    floating_text_vanish_rate: f32,

    // ── Superweapon countdown (C++: m_superweaponPosition, etc.) ──
    superweapon_countdown_position: (f32, f32),
    superweapon_flash_duration: f32,
    superweapon_flash_color: u32,
    superweapon_normal_font: String,
    superweapon_normal_point_size: i32,
    superweapon_normal_bold: bool,
    superweapon_ready_font: String,
    superweapon_ready_point_size: i32,
    superweapon_ready_bold: bool,
    superweapon_last_flash_frame: u32,
    superweapon_used_flash_color: bool,

    // ── Popup messages (C++: m_popupMessageColor) ──
    popup_message_color: u32,

    // ── Drawable caption (C++: m_drawableCaption*) ──
    drawable_caption_font: String,
    drawable_caption_point_size: i32,
    drawable_caption_bold: bool,
    drawable_caption_color: u32,

    // ── Scroll anchors (C++: m_drawRMBScrollAnchor, m_moveRMBScrollAnchor) ──
    draw_rmb_scroll_anchor: bool,
    move_rmb_scroll_anchor: bool,

    // ── Combat modes (C++ InGameUI.h:812-816) ──────────────────────────
    /// C++: m_waypointMode (InGameUI.h:812) — are we in waypoint plotting mode?
    waypoint_mode: bool,
    /// C++: m_forceAttackMode (InGameUI.h:813) — are we in force attack mode? (CTRL key)
    force_attack_mode: bool,
    /// C++: m_forceMoveToMode (InGameUI.h:814) — are we in force move mode?
    force_move_to_mode: bool,
    /// C++: m_attackMoveToMode (InGameUI.h:815) — are we in attack move mode?
    attack_move_to_mode: bool,
    /// C++: m_preferSelection (InGameUI.h:816) — shift key has been depressed
    prefer_selection_mode: bool,

    // ── Camera control state (C++ InGameUI.h:818-822) ──────────────────
    /// C++: m_cameraRotatingLeft (InGameUI.h:818) — KP4
    camera_rotating_left: bool,
    /// C++: m_cameraRotatingRight (InGameUI.h:819) — KP6
    camera_rotating_right: bool,
    /// C++: m_cameraZoomingIn (InGameUI.h:820) — KP8
    camera_zooming_in: bool,
    /// C++: m_cameraZoomingOut (InGameUI.h:822) — KP2
    camera_zooming_out: bool,
    /// C++: m_cameraTrackingDrawable (InGameUI.h:821)
    camera_tracking_drawable: bool,

    // ── Selection tracking (C++ InGameUI.h:707) ────────────────────────
    /// C++: m_frameSelectionChanged (InGameUI.h:707) — Frame when the selection last changed
    frame_selection_changed: u32,

    /// C++: m_duringDoubleClickAttackMoveGuardHintTimer (InGameUI.h)
    /// When > 0, command hints are suppressed. Decremented each logic frame.
    double_click_attack_move_guard_timer: u32,

    // ── Movie playback (C++: InGameUI.h:688,713-718) ──
    /// C++: m_currentlyPlayingMovie (InGameUI.h:688)
    currently_playing_movie: Option<String>,
    /// C++: m_cameoVideoBuffer/m_cameoVideoStream (InGameUI.h:717-718)
    cameo_movie_playing: Option<String>,

    // ── World animations (C++: m_worldAnimationList, InGameUI.h:830) ──
    world_animations: Vec<WorldAnimationData>,

    // ── Superweapon script visibility (C++: m_superweaponHiddenByScript, InGameUI.h:680) ──
    /// C++: m_superweaponHiddenByScript — when true, superweapon timers are hidden globally
    superweapon_hidden_by_script: bool,

    // ── Minimap ping animations ──
    /// Active minimap pings, each with a world position and expiration frame.
    minimap_pings: Vec<MinimapPing>,

    /// C++: TheRecorder->getMode() == RECORDERMODETYPE_PLAYBACK.
    recorder_playback_active: bool,
    /// C++: TheLookAtTranslator->hasMouseMovedRecently().
    look_at_mouse_moved_recently: bool,
}

impl InGameUI {
    pub fn new(renderer: Arc<RwLock<UIRenderer>>, screen_width: f32, screen_height: f32) -> Self {
        let minimap_size = 200.0;
        let minimap_margin = 10.0;

        Self {
            selection_box: SelectionBox::new(),
            selection_state: SelectionState::new(MAX_SELECTION_COUNT),
            placement_preview: None,
            minimap: Minimap::new(
                Vec2::new(
                    screen_width - minimap_size - minimap_margin,
                    screen_height - minimap_size - minimap_margin,
                ),
                Vec2::new(minimap_size, minimap_size),
            ),
            resource_display: ResourceDisplay::new(Vec2::new(10.0, 10.0)),
            renderer,
            screen_size: Vec2::new(screen_width, screen_height),
            enabled: true,
            player_id: 0,
            ui_time: 0.0,
            last_update: Instant::now(),
            floating_texts: Vec::new(),
            idle_workers: Vec::new(),
            current_frame: 0,
            radius_cursor: RadiusCursorState::new(),
            superweapon_timers: Vec::new(),
            mouse_mode: MouseMode::Default,
            current_cursor: MouseCursor::Arrow,
            mouse_mode_cursor: MouseCursor::Arrow,
            is_scrolling: false,
            is_selecting: false,
            scroll_amount_x: 0.0,
            scroll_amount_y: 0.0,
            moused_over_drawable_id: 0,
            hints: Vec::new(),
            next_hint_index: 0,
            named_timers: Vec::new(),
            named_timer_last_flash_frame: 0,
            named_timer_used_flash_color: false,
            show_named_timers: true,
            gui_command: None,
            quit_menu_visible: false,
            window_layouts: HashMap::new(),

            // Message display defaults (C++ constructor: InGameUI.cpp:899-906)
            message_color1: 0xFFFFFFFF, // GameMakeColor(255,255,255,255)
            message_color2: 0xFFB4B4B4, // GameMakeColor(180,180,180,255)
            message_position: (10, 10),
            message_font_name: "Arial".to_string(),
            message_point_size: 10,
            message_bold: false,
            message_delay_ms: 5000,
            messages_enabled: true, // C++: m_messagesOn = TRUE
            messages: Vec::new(),

            // Military caption defaults (C++ constructor: InGameUI.cpp:908-924)
            military_caption_color: (200, 200, 30, 255),
            military_caption_position: (10, 380),
            military_caption_title_font: "Courier".to_string(),
            military_caption_title_point_size: 12,
            military_caption_title_bold: true,
            military_caption_font: "Courier".to_string(),
            military_caption_point_size: 12,
            military_caption_bold: false,
            military_caption_randomize_typing: false,
            military_caption_speed: 1,
            current_military_subtitle: None,
            tooltips_disabled_until: 0,

            // Floating text INI defaults (C++ constructor: InGameUI.cpp:1013-1015)
            floating_text_timeout_frames: DEFAULT_FLOATING_TEXT_TIMEOUT,
            floating_text_move_up_speed: 1.0,
            floating_text_vanish_rate: 0.1,

            // Superweapon countdown defaults (C++ constructor: InGameUI.cpp:980-992)
            superweapon_countdown_position: (0.7, 0.7),
            superweapon_flash_duration: 1.0,
            superweapon_flash_color: 0xFFFFFFFF,
            superweapon_normal_font: "Arial".to_string(),
            superweapon_normal_point_size: 10,
            superweapon_normal_bold: false,
            superweapon_ready_font: "Arial".to_string(),
            superweapon_ready_point_size: 10,
            superweapon_ready_bold: false,
            superweapon_last_flash_frame: 0,
            superweapon_used_flash_color: true,

            // Popup message defaults (C++ constructor: InGameUI.cpp:925)
            popup_message_color: 0xFFFFFFFF,

            // Drawable caption defaults (C++ constructor: InGameUI.cpp:1017-1020)
            drawable_caption_font: "Arial".to_string(),
            drawable_caption_point_size: 10,
            drawable_caption_bold: false,
            drawable_caption_color: 0xFFFFFFFF,

            // Scroll anchors (C++ constructor: InGameUI.cpp:1022-1023)
            draw_rmb_scroll_anchor: false,
            move_rmb_scroll_anchor: false,

            // Combat modes (C++ constructor: InGameUI.cpp)
            waypoint_mode: false,
            force_attack_mode: false,
            force_move_to_mode: false,
            attack_move_to_mode: false,
            prefer_selection_mode: false,

            // Camera control (C++ constructor: InGameUI.cpp)
            camera_rotating_left: false,
            camera_rotating_right: false,
            camera_zooming_in: false,
            camera_zooming_out: false,
            camera_tracking_drawable: false,

            // Selection tracking
            frame_selection_changed: 0,
            double_click_attack_move_guard_timer: 0,

            // Movie playback (C++ constructor: InGameUI.cpp)
            currently_playing_movie: None,
            cameo_movie_playing: None,

            // World animations
            world_animations: Vec::new(),

            // C++: m_superweaponHiddenByScript = FALSE (InGameUI.cpp:993)
            superweapon_hidden_by_script: false,

            // Minimap pings
            minimap_pings: Vec::new(),

            recorder_playback_active: false,
            look_at_mouse_moved_recently: true,
        }
    }

    /// Handle mouse input for selection box
    pub fn handle_mouse_input(
        &mut self,
        mouse: &MouseState,
        keyboard: &KeyboardState,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let mouse_pos = Vec2::new(mouse.position().0, mouse.position().1);
        let left_button = mouse.button_state(MouseButton::Left);
        let right_button = mouse.button_state(MouseButton::Right);
        let add_to_selection = keyboard.is_ctrl_pressed() || keyboard.is_shift_pressed();

        // Check if clicking on minimap
        if self.minimap.contains_point(mouse_pos) {
            if left_button.just_pressed() {
                // Click on minimap - move camera
                let world_pos = self.minimap.minimap_to_world(mouse_pos);
                log::debug!("Minimap click at world position: {:?}", world_pos);
                with_tactical_view(|view| {
                    view.look_at(&Point3::new(world_pos.x, world_pos.y, 0.0));
                });
            }
            return Ok(());
        }

        if self.handle_pending_special_power(mouse_pos, left_button, right_button)? {
            return Ok(());
        }

        // Handle selection box
        match left_button {
            ButtonState::JustPressed => {
                // Start selection box
                self.selection_box.start_at(mouse_pos);

                // Check for double-click
                if self.selection_state.detect_double_click(mouse_pos) {
                    log::debug!("Double-click detected at {:?}", mouse_pos);
                    if let Some(clicked_id) = self.pick_object_at_screen(mouse_pos) {
                        self.select_similar_units(clicked_id, add_to_selection)?;
                    }
                }
            }
            ButtonState::Pressed => {
                // Update selection box
                if self.selection_box.active {
                    self.selection_box.update(mouse_pos);
                }
            }
            ButtonState::JustReleased => {
                // Finish selection box
                if self.selection_box.active {
                    if self.selection_box.is_significant() {
                        // Perform box selection
                        let rect = self.selection_box.get_rect();
                        log::debug!("Selection box: {:?}", rect);
                        let selection_type = if add_to_selection {
                            SelectionType::Add
                        } else {
                            SelectionType::Replace
                        };
                        self.perform_box_selection(rect, selection_type)?;
                    } else {
                        // Single click selection
                        let selection_type = if keyboard.is_ctrl_pressed() {
                            SelectionType::Toggle
                        } else if keyboard.is_shift_pressed() {
                            SelectionType::Add
                        } else {
                            SelectionType::Replace
                        };
                        self.perform_click_selection(mouse_pos, selection_type)?;
                    }
                    self.selection_box.finish();
                }
            }
            _ => {}
        }

        // Handle building placement
        if self.placement_preview.is_some() {
            if let Some(world_pos) = self.screen_to_world(mouse_pos) {
                if let Some(preview) = self.placement_preview.as_mut() {
                    preview.position = Vec3::new(world_pos.x, world_pos.y, world_pos.z);
                    let validator = FoundationValidator::new_strict();
                    preview.is_legal = validator
                        .validate_placement(
                            &world_pos,
                            &preview.template_name,
                            preview.rotation,
                            self.player_id as ObjectID,
                        )
                        .is_ok();
                    TheInGameUI::set_placement_angle(preview.rotation);
                }
            }

            if TheInGameUI::is_placement_anchored() {
                if let Some(preview) = self.placement_preview.as_ref() {
                    if let Some(template) = TheThingFactory::find_template(&preview.template_name) {
                        if template.is_kind_of(KindOf::Barrier) {
                            if let Some((start, _)) = TheInGameUI::get_placement_points() {
                                let current =
                                    MsgICoord2D::new(mouse_pos.x as i32, mouse_pos.y as i32);
                                let dx = (current.x - start.x) as f32;
                                let dy = (current.y - start.y) as f32;
                                if (dx * dx + dy * dy).sqrt() >= PLACEMENT_DRAG_DISTANCE {
                                    TheInGameUI::set_placement_end(Some(current));
                                }
                            }
                        }
                    }
                }
            }

            if mouse.button_state(MouseButton::Left).just_pressed() {
                let (is_legal, template_name, rotation) = match self.placement_preview.as_ref() {
                    Some(preview) => (
                        preview.is_legal,
                        preview.template_name.clone(),
                        preview.rotation,
                    ),
                    None => (false, String::new(), 0.0),
                };

                if is_legal {
                    let template = match TheThingFactory::find_template(&template_name) {
                        Some(template) => template,
                        None => return Ok(()),
                    };
                    let build_id = template.get_id();
                    let is_line_build = template.is_kind_of(KindOf::Barrier);

                    if is_line_build {
                        let start = MsgICoord2D::new(mouse_pos.x as i32, mouse_pos.y as i32);
                        if !TheInGameUI::is_placement_anchored() {
                            TheInGameUI::set_placement_start(Some(start));
                            return Ok(());
                        }
                        TheInGameUI::set_placement_end(Some(start.clone()));
                        if let Some((start, end)) = TheInGameUI::get_placement_points() {
                            let dx = (end.x - start.x) as f32;
                            let dy = (end.y - start.y) as f32;
                            if (dx * dx + dy * dy).sqrt() < PLACEMENT_DRAG_DISTANCE {
                                return Ok(());
                            }
                            let Some(start_world) =
                                self.screen_to_world(Vec2::new(start.x as f32, start.y as f32))
                            else {
                                return Ok(());
                            };
                            let Some(end_world) =
                                self.screen_to_world(Vec2::new(end.x as f32, end.y as f32))
                            else {
                                return Ok(());
                            };
                            let _ = append_message_to_stream(GameMessageType::DozerConstructLine(
                                build_id,
                                MsgCoord3D::new(start_world.x, start_world.y, start_world.z),
                                MsgCoord3D::new(end_world.x, end_world.y, end_world.z),
                                rotation,
                            ));
                        }
                    } else if let Some(world_pos) = self.screen_to_world(mouse_pos) {
                        let _ = append_message_to_stream(GameMessageType::DozerConstruct(
                            build_id,
                            MsgCoord3D::new(world_pos.x, world_pos.y, world_pos.z),
                            rotation,
                        ));
                    }

                    TheInGameUI::place_build_available(None, None);
                    TheInGameUI::set_placement_start(None);
                    self.placement_preview = None;
                }
            }
        }

        Ok(())
    }

    fn handle_pending_special_power(
        &mut self,
        mouse_pos: Vec2,
        left_button: ButtonState,
        right_button: ButtonState,
    ) -> Result<bool> {
        let Some(pending) = TheInGameUI::get_pending_special_power() else {
            return Ok(false);
        };

        if right_button.just_pressed() {
            TheInGameUI::clear_pending_special_power();
            return Ok(true);
        }

        if !left_button.just_pressed() {
            return Ok(true);
        }

        let options = SpecialPowerCommandOption::from_bits_truncate(pending.options);
        let mut issued = false;

        if options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        ) {
            if let Some(target_id) = self.pick_object_at_screen(mouse_pos) {
                if self.is_valid_special_power_target(
                    pending.source_object_id,
                    pending.power_id,
                    target_id,
                    pending.options,
                ) {
                    let _ = append_message_to_stream(GameMessageType::DoSpecialPowerAtObject(
                        pending.power_id,
                        target_id,
                        pending.options,
                        pending.source_object_id,
                    ));
                    issued = true;
                }
            }
        }

        if !issued
            && options.intersects(
                SpecialPowerCommandOption::NEED_TARGET_POS
                    | SpecialPowerCommandOption::ATTACK_OBJECTS_POSITION,
            )
        {
            if let Some(world_pos) = self.screen_to_world(mouse_pos) {
                let _ = append_message_to_stream(GameMessageType::DoSpecialPowerAtLocation(
                    pending.power_id,
                    MsgCoord3D::new(world_pos.x, world_pos.y, world_pos.z),
                    0.0,
                    0,
                    pending.options,
                    pending.source_object_id,
                ));
                issued = true;
            }
        }

        if issued {
            let reselection_required = get_special_power_store()
                .and_then(|store| {
                    store
                        .find_special_power_template_by_id(pending.power_id)
                        .map(|template| template.is_shortcut_power())
                })
                .unwrap_or(false)
                && self.source_has_overridable_special_power_destination(pending.source_object_id);
            if reselection_required {
                let _ = append_message_to_stream(GameMessageType::CreateSelectedGroupNoSound(
                    true,
                    vec![pending.source_object_id],
                ));
            }
            TheInGameUI::clear_pending_special_power();
        } else if options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER
                | SpecialPowerCommandOption::NEED_TARGET_POS
                | SpecialPowerCommandOption::ATTACK_OBJECTS_POSITION,
        ) {
            let _ = append_message_to_stream(GameMessageType::DoInvalidHint);
        }

        Ok(true)
    }

    fn source_has_overridable_special_power_destination(&self, source_object_id: ObjectID) -> bool {
        if source_object_id == 0 {
            return false;
        }
        let Some(source_obj) = OBJECT_REGISTRY.get_object(source_object_id) else {
            return false;
        };
        let Ok(source_guard) = source_obj.read() else {
            return false;
        };
        if source_guard.is_effectively_dead() {
            return false;
        }

        for behavior_arc in source_guard.get_behavior_modules() {
            let Ok(mut behavior_lock) = behavior_arc.lock() else {
                continue;
            };
            let Some(update) = behavior_lock.get_special_power_update_interface() else {
                continue;
            };
            if update.does_special_power_have_overridable_destination_active()
                || update.does_special_power_have_overridable_destination()
            {
                return true;
            }
        }

        false
    }

    fn is_valid_special_power_target(
        &self,
        source_object_id: ObjectID,
        power_id: u32,
        target_id: ObjectID,
        options_bits: u32,
    ) -> bool {
        let options = SpecialPowerCommandOption::from_bits_truncate(options_bits);
        let needs_object = options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        );
        if !needs_object {
            return true;
        }

        let target = OBJECT_REGISTRY.get_object(target_id);
        let Some(target) = target else {
            return false;
        };
        let Ok(target_guard) = target.read() else {
            return false;
        };
        if target_guard.is_effectively_dead() {
            return false;
        }

        let Some(source_obj) = OBJECT_REGISTRY.get_object(source_object_id) else {
            return false;
        };
        let Ok(source_guard) = source_obj.read() else {
            return false;
        };
        if source_guard.is_effectively_dead() {
            return false;
        }

        let Some(store) = get_special_power_store() else {
            return false;
        };
        let Some(template) = store.find_special_power_template_by_id(power_id) else {
            return false;
        };

        ActionManager::can_do_special_power_at_object(
            &source_guard,
            &target_guard,
            CommandSourceType::FromPlayer,
            template,
            options_bits,
            false,
        )
    }

    /// Perform box selection
    fn perform_box_selection(&mut self, rect: UIRect, selection_type: SelectionType) -> Result<()> {
        let start = Vec2::new(rect.x, rect.y);
        let end = Vec2::new(rect.x + rect.width, rect.y + rect.height);
        let Some(world_start) = self.screen_to_world(start) else {
            return Ok(());
        };
        let Some(world_end) = self.screen_to_world(end) else {
            return Ok(());
        };

        let min_x = world_start.x.min(world_end.x).floor() as i32;
        let max_x = world_start.x.max(world_end.x).ceil() as i32;
        let min_y = world_start.y.min(world_end.y).floor() as i32;
        let max_y = world_start.y.max(world_end.y).ceil() as i32;

        let region = IRegion2D::new(ICoord2D::new(min_x, min_y), ICoord2D::new(max_x, max_y));

        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_in_region(region, selection_type, None);
            }
        }
        self.sync_selection_state();
        Ok(())
    }

    /// Perform single click selection
    fn perform_click_selection(&mut self, pos: Vec2, selection_type: SelectionType) -> Result<()> {
        if let Some(object_id) = self.pick_object_at_screen(pos) {
            let selection_manager = get_selection_manager();
            let mut manager = match selection_manager.write() {
                Ok(manager) => manager,
                Err(_) => {
                    self.sync_selection_state();
                    return Ok(());
                }
            };
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(vec![object_id], selection_type);
            }
        } else if matches!(selection_type, SelectionType::Replace) {
            let selection_manager = get_selection_manager();
            let mut manager = match selection_manager.write() {
                Ok(manager) => manager,
                Err(_) => {
                    self.sync_selection_state();
                    return Ok(());
                }
            };
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.clear_selection();
            }
        }
        self.sync_selection_state();
        Ok(())
    }

    fn screen_to_world(&self, screen_pos: Vec2) -> Option<Coord3D> {
        let screen_pt = IPoint2::new(screen_pos.x as i32, screen_pos.y as i32);
        with_tactical_view_ref(|view| {
            view.screen_to_world(&screen_pt)
                .ok()
                .map(|pt| Coord3D::new(pt.x, pt.y, pt.z))
        })
    }

    fn world_to_screen(&self, world: &Coord3D) -> Option<Vec2> {
        let point = Point3::new(world.x, world.y, world.z);
        with_tactical_view_ref(|view| {
            view.world_to_screen(&point)
                .map(|pt| Vec2::new(pt.x as f32, pt.y as f32))
        })
    }

    fn pick_object_at_screen(&self, screen_pos: Vec2) -> Option<ObjectID> {
        const PICK_RADIUS_WORLD: f32 = 12.0;
        let Some(world) = self.screen_to_world(screen_pos) else {
            return None;
        };

        let mut best: Option<(ObjectID, f32)> = None;
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_selectable() {
                continue;
            }
            let pos = guard.get_position();
            let dx = pos.x - world.x;
            let dy = pos.y - world.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= PICK_RADIUS_WORLD * PICK_RADIUS_WORLD {
                if best
                    .map(|(_, best_dist)| dist_sq < best_dist)
                    .unwrap_or(true)
                {
                    best = Some((guard.get_id(), dist_sq));
                }
            }
        }
        best.map(|(id, _)| id)
    }

    fn select_similar_units(
        &mut self,
        template_object_id: ObjectID,
        add_to_selection: bool,
    ) -> Result<()> {
        let Some(reference) = OBJECT_REGISTRY.get_object(template_object_id) else {
            return Ok(());
        };
        let Ok(reference_guard) = reference.read() else {
            return Ok(());
        };
        let template_name = reference_guard.get_template_name().to_string();
        let owner_id = reference_guard
            .get_controlling_player_id()
            .map(|id| id as i32);

        let mut matching: Vec<ObjectID> = Vec::new();
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_selectable() {
                continue;
            }
            if guard.get_template_name() != template_name {
                continue;
            }
            if let Some(owner) = owner_id {
                if guard.get_controlling_player_id().map(|id| id as i32) != Some(owner) {
                    continue;
                }
            }
            matching.push(guard.get_id());
        }

        if matching.is_empty() {
            return Ok(());
        }

        let selection_type = if add_to_selection {
            SelectionType::Add
        } else {
            SelectionType::Replace
        };
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(matching, selection_type);
            }
        }
        self.sync_selection_state();
        Ok(())
    }

    fn sync_selection_state(&mut self) {
        let selection_manager = get_selection_manager();
        let selected_objects = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|selection| selection.get_selected_objects())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        self.selection_state.selected = selected_objects.into_iter().map(DrawableID).collect();
    }

    fn find_selected_builder(&self) -> Option<ObjectID> {
        let selection_manager = get_selection_manager();
        let selected_ids = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|selection| selection.get_selected_objects())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        for object_id in &selected_ids {
            if let Some(object_arc) = TheGameLogic::find_object_by_id(*object_id) {
                if let Ok(object_guard) = object_arc.read() {
                    if object_guard.is_kind_of(KindOf::Dozer) {
                        return Some(*object_id);
                    }
                }
            }
        }

        selected_ids.first().copied()
    }

    /// Start building placement preview
    pub fn start_building_placement(&mut self, template_name: String, footprint: Vec2) {
        self.placement_preview = Some(PlacementPreview::new(template_name, footprint));
        let builder_id = self.find_selected_builder();
        TheInGameUI::place_build_available(
            self.placement_preview
                .as_ref()
                .map(|preview| preview.template_name.clone()),
            builder_id,
        );
    }

    /// Cancel building placement
    pub fn cancel_building_placement(&mut self) {
        self.placement_preview = None;
        TheInGameUI::place_build_available(None, None);
        TheInGameUI::set_placement_start(None);
    }

    /// Update resources display
    pub fn update_resources(&mut self, credits: i32, power_available: i32, power_used: i32) {
        self.resource_display
            .update(credits, power_available, power_used);
    }

    /// Update minimap world bounds
    pub fn set_minimap_world_bounds(&mut self, min: Vec2, max: Vec2) {
        self.minimap.world_bounds = (min, max);
    }

    /// Update minimap camera position
    pub fn update_camera(&mut self, position: Vec3, viewport: Vec2) {
        self.minimap.camera_position = position;
        self.minimap.camera_viewport = viewport;
    }

    /// Add or update unit icon on minimap
    pub fn update_minimap_unit(&mut self, id: u32, world_pos: Vec2, color: [f32; 4]) {
        self.minimap.update_icon(DrawableID(id), world_pos, color);
    }

    /// Remove unit from minimap
    pub fn remove_minimap_unit(&mut self, id: u32) {
        self.minimap.remove_icon(DrawableID(id));
    }

    /// Select object
    pub fn select_object(&mut self, id: u32, add_to_selection: bool) {
        let selection_type = if add_to_selection {
            SelectionType::Add
        } else {
            SelectionType::Replace
        };
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(vec![id as ObjectID], selection_type);
            }
        }
        self.sync_selection_state();
    }

    /// Deselect object
    pub fn deselect_object(&mut self, id: u32) {
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(vec![id as ObjectID], SelectionType::Remove);
            }
        }
        self.sync_selection_state();
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.clear_selection();
            }
        }
        self.sync_selection_state();
    }

    /// Get current selection
    pub fn get_selection(&self) -> Vec<u32> {
        let selection_manager = get_selection_manager();
        if let Ok(manager) = selection_manager.read() {
            if let Some(selection) = manager.get_player_selection_ref(self.player_id as i32) {
                return selection
                    .get_selected_objects()
                    .into_iter()
                    .map(|id| id as u32)
                    .collect();
            }
        }
        self.selection_state
            .get_selected()
            .iter()
            .map(|id| id.0)
            .collect()
    }

    /// Set selection group
    pub fn set_selection_group(&mut self, group: usize) {
        if group < 10 {
            let selection_manager = get_selection_manager();
            if let Ok(mut manager) = selection_manager.write() {
                if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                    selection.create_control_group(group);
                }
            }
            self.sync_selection_state();
        }
    }

    /// Recall selection group
    pub fn recall_selection_group(&mut self, group: usize) {
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_control_group(group, false);
            }
        }
        self.sync_selection_state();
    }

    /// Set local player id for selection routing.
    pub fn set_player_id(&mut self, player_id: u32) {
        self.player_id = player_id;
        self.sync_selection_state();
    }

    pub fn add_floating_text(&mut self, text: String, position: Coord3D, color: (u8, u8, u8)) {
        if self.floating_texts.len() >= MAX_FLOATING_TEXT {
            self.floating_texts.remove(0);
        }
        self.floating_texts.push(FloatingTextData {
            text,
            position,
            color,
            creation_frame: self.current_frame,
            timeout: DEFAULT_FLOATING_TEXT_TIMEOUT,
            move_up_speed: 1.0,
        });
    }

    pub fn clear_floating_texts(&mut self) {
        self.floating_texts.clear();
    }

    pub fn update_floating_texts(&mut self) {
        self.floating_texts
            .retain(|ft| self.current_frame - ft.creation_frame < ft.timeout);
        for ft in &mut self.floating_texts {
            ft.position.z += ft.move_up_speed;
        }
    }

    pub fn add_idle_worker(&mut self, object_id: ObjectID, player_index: u8) {
        if !self.idle_workers.iter().any(|w| w.object_id == object_id) {
            self.idle_workers.push(IdleWorkerData {
                object_id,
                player_index,
            });
        }
    }

    pub fn remove_idle_worker(&mut self, object_id: ObjectID, _player_index: u8) {
        self.idle_workers.retain(|w| w.object_id != object_id);
    }

    pub fn find_idle_worker(&self, object_id: ObjectID) -> bool {
        self.idle_workers.iter().any(|w| w.object_id == object_id)
    }

    pub fn get_idle_worker_count(&self, player_index: u8) -> usize {
        self.idle_workers
            .iter()
            .filter(|w| w.player_index == player_index)
            .count()
    }

    pub fn select_next_idle_worker(&self, player_index: u8) -> Option<ObjectID> {
        self.idle_workers
            .iter()
            .find(|w| w.player_index == player_index)
            .map(|w| w.object_id)
    }

    pub fn reset_idle_workers(&mut self) {
        self.idle_workers.clear();
    }

    pub fn set_radius_cursor(
        &mut self,
        cursor_type: RadiusCursorType,
        position: Coord3D,
        radius: f32,
    ) {
        if cursor_type == self.radius_cursor.cursor_type && self.radius_cursor.active {
            return;
        }
        if cursor_type == RadiusCursorType::None {
            self.clear_radius_cursor();
            return;
        }
        if radius <= 0.0 {
            return;
        }
        self.radius_cursor.cursor_type = cursor_type;
        self.radius_cursor.active = true;
        self.radius_cursor.position = position;
        self.radius_cursor.radius = radius;
    }

    pub fn clear_radius_cursor(&mut self) {
        self.radius_cursor.cursor_type = RadiusCursorType::None;
        self.radius_cursor.active = false;
        self.radius_cursor.radius = 0.0;
    }

    pub fn is_radius_cursor_active(&self) -> bool {
        self.radius_cursor.active
    }

    pub fn get_radius_cursor_type(&self) -> RadiusCursorType {
        self.radius_cursor.cursor_type
    }

    pub fn update_radius_cursor(&mut self, mouse_pos: Coord3D) {
        if !self.radius_cursor.active {
            return;
        }
        self.radius_cursor.position = mouse_pos;
    }

    pub fn add_superweapon_timer(
        &mut self,
        player_index: u8,
        object_id: ObjectID,
        power_name: String,
        ready_frame: u32,
    ) {
        let existing = self.superweapon_timers.iter().any(|t| {
            t.player_index == player_index && t.power_name == power_name && t.object_id == object_id
        });
        if existing {
            return;
        }
        self.superweapon_timers.push(SuperweaponTimerData {
            player_index,
            object_id,
            power_name,
            ready_frame,
            countdown_text: String::new(),
            ready: false,
            hidden_by_script: false,
            hidden_by_science: false,
        });
    }

    pub fn remove_superweapon_timer(
        &mut self,
        player_index: u8,
        object_id: ObjectID,
        power_name: &str,
    ) -> bool {
        let before = self.superweapon_timers.len();
        self.superweapon_timers.retain(|t| {
            !(t.player_index == player_index
                && t.object_id == object_id
                && t.power_name == power_name)
        });
        self.superweapon_timers.len() < before
    }

    pub fn update_superweapon_timers(&mut self, current_frame: u32) {
        const LOGICFRAMES_PER_SECOND: u32 = 30;
        for timer in &mut self.superweapon_timers {
            if timer.hidden_by_script || timer.hidden_by_science {
                continue;
            }
            if current_frame >= timer.ready_frame && timer.ready_frame > 0 {
                if !timer.ready {
                    timer.ready = true;
                }
                timer.countdown_text = "READY".to_string();
            } else if timer.ready_frame > 0 {
                let remaining = timer.ready_frame.saturating_sub(current_frame);
                let total_seconds = remaining / LOGICFRAMES_PER_SECOND;
                let minutes = total_seconds / 60;
                let seconds = total_seconds % 60;
                timer.countdown_text = format!("{}:{:02}", minutes, seconds);
                timer.ready = false;
            }
        }
    }

    pub fn get_superweapon_timers(&self) -> &[SuperweaponTimerData] {
        &self.superweapon_timers
    }

    // ── Mouse cursor system ──────────────────────────────────────────────
    // C++: InGameUI::setMouseCursor() (InGameUI.cpp:516-525)

    pub fn set_mouse_cursor(&mut self, cursor: MouseCursor) {
        self.current_cursor = cursor;
        if self.mouse_mode == MouseMode::GuiCommand
            && cursor != MouseCursor::Arrow
            && cursor != MouseCursor::Scroll
        {
            self.mouse_mode_cursor = cursor;
        }
    }

    pub fn get_mouse_cursor(&self) -> MouseCursor {
        self.current_cursor
    }

    pub fn set_mouse_mode(&mut self, mode: MouseMode) {
        self.mouse_mode = mode;
        if mode != MouseMode::GuiCommand {
            self.mouse_mode_cursor = MouseCursor::Arrow;
        }
    }

    pub fn get_mouse_mode(&self) -> MouseMode {
        self.mouse_mode
    }

    pub fn get_mouse_mode_cursor(&self) -> MouseCursor {
        self.mouse_mode_cursor
    }

    // ── Scroll / select state ────────────────────────────────────────────
    // C++: InGameUI::setScrolling() (InGameUI.cpp:2787)
    // C++: InGameUI::setSelecting() (InGameUI.cpp:2824)

    pub fn set_scrolling(&mut self, scrolling: bool) {
        if self.is_scrolling == scrolling {
            return;
        }
        if scrolling {
            self.set_mouse_cursor(MouseCursor::Scroll);
        } else {
            self.set_mouse_cursor(MouseCursor::Arrow);
        }
        self.is_scrolling = scrolling;
        if !scrolling {
            self.scroll_amount_x = 0.0;
            self.scroll_amount_y = 0.0;
        }
    }

    pub fn is_scrolling(&self) -> bool {
        self.is_scrolling
    }

    pub fn set_selecting(&mut self, selecting: bool) {
        if self.is_selecting == selecting {
            return;
        }
        self.is_selecting = selecting;
    }

    pub fn is_selecting(&self) -> bool {
        self.is_selecting
    }

    pub fn set_scroll_amount(&mut self, x: f32, y: f32) {
        self.scroll_amount_x = x;
        self.scroll_amount_y = y;
    }

    pub fn get_scroll_amount(&self) -> (f32, f32) {
        (self.scroll_amount_x, self.scroll_amount_y)
    }

    pub fn set_moused_over_drawable_id(&mut self, id: u32) {
        self.moused_over_drawable_id = id;
    }

    pub fn get_moused_over_drawable_id(&self) -> u32 {
        self.moused_over_drawable_id
    }

    pub fn set_recorder_playback_active(&mut self, active: bool) {
        self.recorder_playback_active = active;
    }

    pub fn set_look_at_mouse_moved_recently(&mut self, moved_recently: bool) {
        self.look_at_mouse_moved_recently = moved_recently;
    }

    // ── Hint system ──────────────────────────────────────────────────────
    // C++: InGameUI::createMoveHint() (InGameUI.cpp:2141)
    // C++: InGameUI::createAttackHint() (InGameUI.cpp:2176)
    // C++: InGameUI::expireHint() (InGameUI.cpp:3812)

    pub fn create_move_hint(&mut self, start: Coord3D, end: Coord3D, source_id: u32) {
        self.expire_hint_for_source(HintType::Move, source_id);

        if self.hints.len() >= MAX_MOVE_HINTS {
            self.expire_oldest_hint(HintType::Move);
        }

        self.hints.push(HintData {
            hint_type: HintType::Move,
            start,
            end,
            creation_frame: self.current_frame,
            source_id,
            lifetime_frames: 60,
        });
    }

    pub fn create_attack_hint(&mut self, start: Coord3D, end: Coord3D, source_id: u32) {
        self.expire_hint_for_source(HintType::Attack, source_id);

        if self.hints.len() >= MAX_MOVE_HINTS {
            self.expire_oldest_hint(HintType::Attack);
        }

        self.hints.push(HintData {
            hint_type: HintType::Attack,
            start,
            end,
            creation_frame: self.current_frame,
            source_id,
            lifetime_frames: 60,
        });
    }

    pub fn create_force_attack_hint(&mut self, start: Coord3D, end: Coord3D, source_id: u32) {
        if self.hints.len() >= MAX_MOVE_HINTS {
            self.expire_oldest_hint(HintType::ForceAttack);
        }

        self.hints.push(HintData {
            hint_type: HintType::ForceAttack,
            start,
            end,
            creation_frame: self.current_frame,
            source_id,
            lifetime_frames: 60,
        });
    }

    pub fn create_garrison_hint(&mut self, start: Coord3D, end: Coord3D, source_id: u32) {
        if self.hints.len() >= MAX_MOVE_HINTS {
            self.expire_oldest_hint(HintType::Garrison);
        }

        self.hints.push(HintData {
            hint_type: HintType::Garrison,
            start,
            end,
            creation_frame: self.current_frame,
            source_id,
            lifetime_frames: 60,
        });
    }

    pub fn begin_area_select_hint(&mut self) {
        self.hints.push(HintData {
            hint_type: HintType::AreaSelect,
            start: Coord3D::new(0.0, 0.0, 0.0),
            end: Coord3D::new(0.0, 0.0, 0.0),
            creation_frame: self.current_frame,
            source_id: 0,
            lifetime_frames: 300,
        });
    }

    pub fn end_area_select_hint(&mut self) {
        if let Some(pos) = self
            .hints
            .iter()
            .rposition(|h| h.hint_type == HintType::AreaSelect)
        {
            self.hints.remove(pos);
        }
    }

    /// C++: InGameUI::expireHint() (InGameUI.cpp:3812) — expire a specific hint by type and index.
    pub fn expire_hint(&mut self, hint_type: HintType, hint_index: usize) {
        if hint_index >= self.hints.len() {
            return;
        }
        if self.hints[hint_index].hint_type == hint_type {
            self.hints.remove(hint_index);
        }
    }

    pub fn expire_hints(&mut self) {
        self.hints
            .retain(|h| self.current_frame < h.creation_frame + h.lifetime_frames);
    }

    pub fn clear_hints(&mut self) {
        self.hints.clear();
    }

    pub fn get_hints(&self) -> &[HintData] {
        &self.hints
    }

    // ── Named timer system ─────────────────────────────────────────────
    // C++: InGameUI::addNamedTimer() (InGameUI.cpp)
    // C++: InGameUI::removeNamedTimer() (InGameUI.cpp)
    // C++: InGameUI::showNamedTimerDisplay() (InGameUI.cpp)

    pub fn add_named_timer(&mut self, name: &str, text: String, is_countdown: bool) {
        self.named_timers.retain(|t| t.name != name);
        self.named_timers.push(NamedTimerData {
            name: name.to_string(),
            text,
            is_countdown,
        });
    }

    pub fn remove_named_timer(&mut self, name: &str) {
        self.named_timers.retain(|t| t.name != name);
    }

    pub fn show_named_timer_display(&mut self, show: bool) {
        self.show_named_timers = show;
    }

    pub fn get_named_timers(&self) -> &[NamedTimerData] {
        &self.named_timers
    }

    // ── GUI command system ─────────────────────────────────────────────
    // C++: InGameUI::setGUICommand() (InGameUI.cpp:2865)
    // C++: InGameUI::getGUICommand() (InGameUI.cpp:2923)

    pub fn set_gui_command(&mut self, command: Option<String>) {
        self.gui_command = command;
    }

    pub fn get_gui_command(&self) -> Option<&String> {
        self.gui_command.as_ref()
    }

    // ── Quit menu ──────────────────────────────────────────────────────
    // C++: InGameUI::setQuitMenuVisible() (InGameUI.h:460)
    // C++: InGameUI::isQuitMenuVisible() (InGameUI.h:461)

    pub fn set_quit_menu_visible(&mut self, visible: bool) {
        self.quit_menu_visible = visible;
    }

    pub fn is_quit_menu_visible(&self) -> bool {
        self.quit_menu_visible
    }

    // ── Window layout registration ─────────────────────────────────────
    // C++: InGameUI::registerWindowLayout() (InGameUI.cpp)
    // C++: InGameUI::unregisterWindowLayout() (InGameUI.cpp)

    pub fn register_window_layout(&mut self, name: &str) {
        self.window_layouts.insert(name.to_string(), true);
    }

    pub fn unregister_window_layout(&mut self, name: &str) {
        self.window_layouts.remove(name);
    }

    pub fn is_window_layout_registered(&self, name: &str) -> bool {
        self.window_layouts.get(name).copied().unwrap_or(false)
    }

    // ── Region builder helper ──────────────────────────────────────────

    pub fn build_region(&self, x: f32, y: f32, width: f32, height: f32) -> IRegion2D {
        IRegion2D::new(
            ICoord2D::new(x as i32, y as i32),
            ICoord2D::new((x + width) as i32, (y + height) as i32),
        )
    }

    fn expire_hint_for_source(&mut self, hint_type: HintType, source_id: u32) {
        if source_id == 0 {
            return;
        }
        self.hints
            .retain(|h| !(h.hint_type == hint_type && h.source_id == source_id));
    }

    fn expire_oldest_hint(&mut self, hint_type: HintType) {
        if let Some(pos) = self.hints.iter().position(|h| h.hint_type == hint_type) {
            self.hints.remove(pos);
        }
    }

    // ── Draw pipeline: main render entry ─────────────────────────────
    // C++: InGameUI::draw() (pure virtual in C++, implemented in subclasses)
    // C++ calls drawSelectionAnims(), drawPlacementCursor(), drawRadarBall(), etc.

    /// Main draw pipeline. Renders selection indicators, placement cursor,
    /// team/waypoint overlay lines, and minimap ping animations.
    /// C++: InGameUI::draw() — pure virtual, subclass calls sub-draw methods.
    pub fn draw(&mut self) -> std::result::Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        let mut renderer = self
            .renderer
            .write()
            .map_err(|_| "Failed to lock renderer".to_string())?;

        self.draw_selection_anims(&mut renderer)?;

        if let Some(ref preview) = self.placement_preview {
            let pos = Coord3D::new(preview.position.x, preview.position.y, preview.position.z);
            self.draw_placement_cursor(&mut renderer, &pos)?;
        }

        self.draw_team_waypoint_lines(&mut renderer)?;
        self.draw_minimap_pings(&mut renderer)?;

        if self.selection_box.active && self.selection_box.is_significant() {
            self.render_selection_box(&mut renderer)
                .map_err(|e| e.to_string())?;
        }

        if self.minimap.visible {
            self.render_minimap(&mut renderer)
                .map_err(|e| e.to_string())?;
        }

        self.render_resources(&mut renderer)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// C++: drawSelectionAnims() — iterates selected drawables, draws
    /// health bars (green/yellow/red based on HP), veterancy pips, and
    /// selection rings on terrain.
    fn draw_selection_anims(&self, renderer: &mut UIRenderer) -> std::result::Result<(), String> {
        let selected = self.get_selection();
        for &obj_id in &selected {
            let obj = match OBJECT_REGISTRY.get_object(obj_id) {
                Some(o) => o,
                None => continue,
            };
            let guard = match obj.read() {
                Ok(g) => g,
                Err(_) => continue,
            };

            let pos = guard.get_position();
            let world = Coord3D::new(pos.x, pos.y, pos.z);
            let Some(screen) = self.world_to_screen(&world) else {
                continue;
            };

            let health_pct = guard.get_health_percentage();
            if health_pct > 0.0 {
                let bar_width = 40.0;
                let bar_height = 4.0;
                let bar_x = screen.x - bar_width / 2.0;
                let bar_y = screen.y - 30.0;

                renderer
                    .draw_rect_with_scissor(
                        UIRect::new(bar_x, bar_y, bar_width, bar_height),
                        [0.2, 0.2, 0.2, 0.7],
                        None,
                    )
                    .map_err(|e| e.to_string())?;

                let fill_color = if health_pct > 0.66 {
                    [0.0, 1.0, 0.0, 0.9]
                } else if health_pct > 0.33 {
                    [1.0, 1.0, 0.0, 0.9]
                } else {
                    [1.0, 0.0, 0.0, 0.9]
                };

                renderer
                    .draw_rect_with_scissor(
                        UIRect::new(bar_x, bar_y, bar_width * health_pct, bar_height),
                        fill_color,
                        None,
                    )
                    .map_err(|e| e.to_string())?;
            }

            let rank = guard.get_veterancy_level();
            let rank_val = rank as i32;
            if rank_val > 0 {
                let pip_size = 4.0;
                let pip_x = screen.x + 20.0;
                let pip_y = screen.y - 25.0;
                for i in 0..rank_val.min(3) {
                    renderer
                        .draw_rect_with_scissor(
                            UIRect::new(
                                pip_x,
                                pip_y - (i as f32) * (pip_size + 1.0),
                                pip_size,
                                pip_size,
                            ),
                            [1.0, 0.85, 0.0, 1.0],
                            None,
                        )
                        .map_err(|e| e.to_string())?;
                }
            }

            let ring_radius = 20.0f32;
            let segments = 24u32;
            for i in 0..segments {
                let angle1 = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
                let angle2 = ((i + 1) as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
                let x1 = screen.x + ring_radius * angle1.cos();
                let y1 = screen.y + ring_radius * angle1.sin();
                let x2 = screen.x + ring_radius * angle2.cos();
                let y2 = screen.y + ring_radius * angle2.sin();
                renderer.draw_line(
                    Vec2::new(x1, y1),
                    Vec2::new(x2, y2),
                    1.5,
                    [0.0, 1.0, 0.0, 0.6],
                    0.0,
                );
            }
        }
        Ok(())
    }

    /// C++: drawPlacementCursor() — renders ghost building at the given
    /// position using the placement template's drawable. Tints green if
    /// placeable, red if blocked.
    fn draw_placement_cursor(
        &self,
        renderer: &mut UIRenderer,
        pos: &Coord3D,
    ) -> std::result::Result<(), String> {
        let Some(ref preview) = self.placement_preview else {
            return Ok(());
        };

        let Some(screen_pos) = self.world_to_screen(pos) else {
            return Ok(());
        };

        let size = preview.footprint * 50.0;
        let rect = UIRect::new(
            screen_pos.x - size.x / 2.0,
            screen_pos.y - size.y / 2.0,
            size.x,
            size.y,
        );

        let tint_color = if preview.is_legal {
            [
                LEGAL_BUILD_COLOR[0],
                LEGAL_BUILD_COLOR[1],
                LEGAL_BUILD_COLOR[2],
                PLACEMENT_OPACITY,
            ]
        } else {
            [
                ILLEGAL_BUILD_COLOR[0],
                ILLEGAL_BUILD_COLOR[1],
                ILLEGAL_BUILD_COLOR[2],
                PLACEMENT_OPACITY,
            ]
        };

        let border_color = if preview.is_legal {
            [0.0, 1.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };

        renderer
            .draw_rect_outline_with_scissor(rect, 2.0, border_color, None)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Draw team/waypoint overlay lines connecting waypoints for selected units.
    /// C++: waypoint lines are drawn in the tactical view overlay.
    fn draw_team_waypoint_lines(
        &self,
        renderer: &mut UIRenderer,
    ) -> std::result::Result<(), String> {
        for hint in &self.hints {
            if hint.hint_type != HintType::Move && hint.hint_type != HintType::Command {
                continue;
            }
            if self.current_frame >= hint.creation_frame + hint.lifetime_frames {
                continue;
            }

            let start_screen = self.world_to_screen(&hint.start);
            let end_screen = self.world_to_screen(&hint.end);
            if let (Some(s), Some(e)) = (start_screen, end_screen) {
                let fade = 1.0
                    - (self.current_frame - hint.creation_frame) as f32
                        / hint.lifetime_frames as f32;
                let alpha = fade.clamp(0.0, 1.0);
                let line_color = match hint.hint_type {
                    HintType::Move => [0.0, 1.0, 0.0, alpha * 0.6],
                    HintType::Command => [1.0, 1.0, 0.0, alpha * 0.6],
                    _ => [1.0, 1.0, 1.0, alpha * 0.6],
                };
                renderer.draw_line(s, e, 1.5, line_color, 0.0);
            }
        }
        Ok(())
    }

    /// Draw minimap ping animations — expanding circles at ping locations.
    fn draw_minimap_pings(&self, renderer: &mut UIRenderer) -> std::result::Result<(), String> {
        for ping in &self.minimap_pings {
            let elapsed = self.current_frame.saturating_sub(ping.creation_frame);
            if elapsed >= ping.lifetime_frames {
                continue;
            }

            let progress = elapsed as f32 / ping.lifetime_frames as f32;
            let alpha = 1.0 - progress;
            let minimap_pos = self.minimap.world_to_minimap(ping.world_pos);
            let max_radius = 15.0f32;
            let radius = max_radius * progress;

            let segments = 16u32;
            for i in 0..segments {
                let a1 = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
                let a2 = ((i + 1) as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
                let x1 = minimap_pos.x + radius * a1.cos();
                let y1 = minimap_pos.y + radius * a1.sin();
                let x2 = minimap_pos.x + radius * a2.cos();
                let y2 = minimap_pos.y + radius * a2.sin();
                let color = [
                    ping.color[0],
                    ping.color[1],
                    ping.color[2],
                    alpha * ping.color[3],
                ];
                renderer.draw_line(
                    Vec2::new(x1, y1),
                    Vec2::new(x2, y2),
                    1.5,
                    [0.0, 1.0, 0.0, 0.6],
                    0.0,
                );
            }
        }
        Ok(())
    }

    /// Add a minimap ping at the given world position.
    pub fn add_minimap_ping(&mut self, world_pos: Vec2, color: [f32; 4], lifetime_frames: u32) {
        self.minimap_pings.push(MinimapPing {
            world_pos,
            color,
            creation_frame: self.current_frame,
            lifetime_frames,
        });
    }

    /// Expire old minimap pings.
    pub fn expire_minimap_pings(&mut self) {
        self.minimap_pings
            .retain(|p| self.current_frame < p.creation_frame + p.lifetime_frames);
    }

    // ── Control group methods ────────────────────────────────────────
    // C++: InGameUI has 10 control groups (0-9), mapped to Ctrl+0 through Ctrl+9

    /// Add a single object to a control group. C++: binds object to group number.
    pub fn add_to_control_group(&mut self, group: i32, obj_id: ObjectID) {
        if !(0..=9).contains(&group) {
            return;
        }
        let group_idx = group as usize;
        let selection_manager = get_selection_manager();
        let Ok(mut manager) = selection_manager.write() else {
            return;
        };
        if let Some(state) = manager.get_player_selection(self.player_id as i32) {
            let group_ids = state.get_control_group_objects(group_idx).to_vec();
            if !group_ids.contains(&obj_id) {
                let mut updated = group_ids;
                updated.push(obj_id);
                state.set_control_group_objects(group_idx, updated);
            }
        }
    }

    /// Get all objects in a control group. Returns empty vec if group is empty.
    pub fn get_control_group(&self, group: i32) -> Vec<ObjectID> {
        if !(0..=9).contains(&group) {
            return Vec::new();
        }
        let selection_manager = get_selection_manager();
        if let Ok(manager) = selection_manager.read() {
            if let Some(state) = manager.get_player_selection_ref(self.player_id as i32) {
                return state.get_control_group_objects(group as usize).to_vec();
            }
        }
        Vec::new()
    }

    /// Select all objects in a control group. Replaces current selection.
    pub fn select_control_group(&mut self, group: i32) {
        if !(0..=9).contains(&group) {
            return;
        }
        let group_idx = group as usize;
        let group_ids = {
            let selection_manager = get_selection_manager();
            let Ok(manager) = selection_manager.read() else {
                return;
            };
            let Some(state) = manager.get_player_selection_ref(self.player_id as i32) else {
                return;
            };
            let ids = state.get_control_group_objects(group_idx);
            if ids.is_empty() {
                return;
            }
            ids.to_vec()
        };

        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(state) = manager.get_player_selection(self.player_id as i32) {
                state.select_objects(group_ids, SelectionType::Replace);
            }
        }
        self.frame_selection_changed = self.current_frame;
        self.sync_selection_state();
    }

    // ── Build placement template methods ─────────────────────────────
    // C++: placeBuildAvailable(), getPendingPlaceType()

    /// Set the current build placement template. C++: placeBuildAvailable()
    /// Passing None clears the placement state.
    pub fn set_placement_template(&mut self, template: Option<String>) {
        match template {
            Some(name) => {
                let footprint = TheThingFactory::find_template(&name)
                    .map(|t| {
                        let info = t.get_template_geometry_info();
                        let half_w = (info.bounds.max.x - info.bounds.min.x) / 2.0;
                        let half_h = (info.bounds.max.y - info.bounds.min.y) / 2.0;
                        Vec2::new(half_w.abs().max(1.0), half_h.abs().max(1.0))
                    })
                    .unwrap_or(Vec2::new(1.0, 1.0));
                self.start_building_placement(name, footprint);
            }
            None => {
                self.cancel_building_placement();
            }
        }
    }

    /// Get the current placement template name. C++: getPendingPlaceType()
    pub fn get_placement_template(&self) -> Option<&str> {
        self.placement_preview
            .as_ref()
            .map(|p| p.template_name.as_str())
    }

    /// Check if placement is legal at the given position.
    /// C++: BuildAssistant::isLocationLegalToBuild() with quick checks.
    /// Validates terrain passability, object overlap, and proximity.
    pub fn can_place_at(&self, pos: &Coord3D) -> bool {
        let Some(ref preview) = self.placement_preview else {
            return false;
        };

        let validator = FoundationValidator::new_strict();
        validator
            .validate_placement(
                pos,
                &preview.template_name,
                preview.rotation,
                self.player_id as ObjectID,
            )
            .is_ok()
    }

    /// Set/get superweapon display hidden by script.
    /// C++: setSuperweaponDisplayEnabledByScript() / getSuperweaponDisplayEnabledByScript()
    pub fn set_superweapon_hidden_by_script(&mut self, hidden: bool) {
        self.superweapon_hidden_by_script = hidden;
    }

    pub fn is_superweapon_hidden_by_script(&self) -> bool {
        self.superweapon_hidden_by_script
    }

    /// Render the in-game UI
    pub fn render(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let mut renderer = self
            .renderer
            .write()
            .map_err(|_| InGameUIError::SystemError("Failed to lock renderer".into()))?;

        // Render selection box
        if self.selection_box.active && self.selection_box.is_significant() {
            self.render_selection_box(&mut renderer)?;
        }

        // Render minimap
        if self.minimap.visible {
            self.render_minimap(&mut renderer)?;
        }

        // Render resource display
        self.render_resources(&mut renderer)?;

        // Render placement preview
        if let Some(ref preview) = self.placement_preview {
            self.render_placement_preview(&mut renderer, preview)?;
        }

        Ok(())
    }

    /// Render selection box
    fn render_selection_box(&self, renderer: &mut UIRenderer) -> Result<()> {
        let rect = self.selection_box.get_rect();

        // Draw box outline
        renderer.draw_rect_outline_with_scissor(
            rect,
            2.0,
            [0.0, 1.0, 0.0, 0.8], // Green with alpha
            None,
        )?;

        // Draw semi-transparent fill
        renderer.draw_rect_with_scissor(
            rect,
            [0.0, 1.0, 0.0, 0.2], // Green with low alpha
            None,
        )?;

        Ok(())
    }

    /// Render minimap
    fn render_minimap(&self, renderer: &mut UIRenderer) -> Result<()> {
        let minimap_rect = UIRect::new(
            self.minimap.position.x,
            self.minimap.position.y,
            self.minimap.size.x,
            self.minimap.size.y,
        );

        // Draw minimap background
        renderer.draw_rect_with_scissor(minimap_rect, [0.1, 0.1, 0.1, 0.8], None)?;

        // Draw border
        renderer.draw_rect_outline_with_scissor(minimap_rect, 2.0, [0.5, 0.5, 0.5, 1.0], None)?;

        // Draw camera viewport indicator
        let cam_pos_2d = Vec2::new(
            self.minimap.camera_position.x,
            self.minimap.camera_position.z,
        );
        let cam_minimap = self.minimap.world_to_minimap(cam_pos_2d);
        let viewport_size = self.minimap.camera_viewport
            * (self.minimap.size / (self.minimap.world_bounds.1 - self.minimap.world_bounds.0));

        let viewport_rect = UIRect::new(
            cam_minimap.x - viewport_size.x / 2.0,
            cam_minimap.y - viewport_size.y / 2.0,
            viewport_size.x,
            viewport_size.y,
        );

        renderer.draw_rect_outline_with_scissor(viewport_rect, 1.0, [1.0, 1.0, 1.0, 0.8], None)?;

        // Draw unit icons
        for (_, icon) in &self.minimap.unit_icons {
            renderer.draw_rect_with_scissor(
                UIRect::new(
                    icon.position.x - icon.size / 2.0,
                    icon.position.y - icon.size / 2.0,
                    icon.size,
                    icon.size,
                ),
                icon.color,
                None,
            )?;
        }

        Ok(())
    }

    /// Render resource display
    fn render_resources(&self, renderer: &mut UIRenderer) -> Result<()> {
        let pos = self.resource_display.position;

        // Background panel
        renderer.draw_rect_with_scissor(
            UIRect::new(pos.x, pos.y, 250.0, 80.0),
            [0.0, 0.0, 0.0, 0.7],
            None,
        )?;

        // Credits text
        let credits_text = format!("${}", self.resource_display.credits);
        renderer.draw_text_simple(
            &credits_text,
            Vec2::new(pos.x + 10.0, pos.y + 10.0),
            16.0,
            [1.0, 1.0, 0.0, 1.0], // Yellow
        )?;

        // Power text
        let power_color = if self.resource_display.is_power_deficit() {
            [1.0, 0.0, 0.0, 1.0] // Red if deficit
        } else {
            [0.0, 1.0, 0.0, 1.0] // Green if OK
        };

        let power_text = format!(
            "Power: {}/{}",
            self.resource_display.power_used, self.resource_display.power_available
        );
        renderer.draw_text_simple(
            &power_text,
            Vec2::new(pos.x + 10.0, pos.y + 35.0),
            14.0,
            power_color,
        )?;

        // Power bar
        let power_pct = self.resource_display.get_power_percentage();
        let bar_width = 200.0;
        let bar_height = 15.0;

        // Bar background
        renderer.draw_rect_with_scissor(
            UIRect::new(pos.x + 10.0, pos.y + 55.0, bar_width, bar_height),
            [0.3, 0.3, 0.3, 1.0],
            None,
        )?;

        // Bar fill
        renderer.draw_rect_with_scissor(
            UIRect::new(
                pos.x + 10.0,
                pos.y + 55.0,
                bar_width * power_pct,
                bar_height,
            ),
            power_color,
            None,
        )?;

        Ok(())
    }

    /// Render building placement preview
    fn render_placement_preview(
        &self,
        renderer: &mut UIRenderer,
        preview: &PlacementPreview,
    ) -> Result<()> {
        let world = Coord3D::new(preview.position.x, preview.position.y, preview.position.z);
        let Some(screen_pos) = self.world_to_screen(&world) else {
            return Ok(());
        };
        let size = preview.footprint * 50.0; // Scale for visibility

        let rect = UIRect::new(
            screen_pos.x - size.x / 2.0,
            screen_pos.y - size.y / 2.0,
            size.x,
            size.y,
        );

        // Draw semi-transparent preview
        renderer.draw_rect_with_scissor(rect, preview.get_color(), None)?;

        // Draw border
        let border_color = if preview.is_legal {
            [0.0, 1.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0, 1.0]
        };

        renderer.draw_rect_outline_with_scissor(rect, 2.0, border_color, None)?;

        Ok(())
    }

    /// Update UI state
    pub fn update(&mut self, delta_time: Duration) {
        self.last_update = Instant::now();
        self.ui_time += delta_time.as_secs_f32();
        if let Ok(mut renderer) = self.renderer.write() {
            renderer.set_time(self.ui_time);
        }
    }

    /// Resize UI elements
    pub fn resize(&mut self, width: f32, height: f32) {
        self.screen_size = Vec2::new(width, height);

        // Reposition minimap to bottom-right
        let minimap_margin = 10.0;
        self.minimap.position = Vec2::new(
            width - self.minimap.size.x - minimap_margin,
            height - self.minimap.size.y - minimap_margin,
        );
    }

    /// Enable/disable UI
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if UI is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    // ── Combat mode methods ────────────────────────────────────────────
    // C++: InGameUI.h:506-519

    pub fn set_force_attack_mode(&mut self, enabled: bool) {
        self.force_attack_mode = enabled;
    }

    pub fn is_in_force_attack_mode(&self) -> bool {
        self.force_attack_mode
    }

    pub fn set_force_move_to_mode(&mut self, enabled: bool) {
        self.force_move_to_mode = enabled;
    }

    pub fn is_in_force_move_to_mode(&self) -> bool {
        self.force_move_to_mode
    }

    pub fn toggle_attack_move_to_mode(&mut self) -> bool {
        self.attack_move_to_mode = !self.attack_move_to_mode;
        self.attack_move_to_mode
    }

    pub fn is_in_attack_move_to_mode(&self) -> bool {
        self.attack_move_to_mode
    }

    pub fn clear_attack_move_to_mode(&mut self) {
        self.attack_move_to_mode = false;
    }

    pub fn set_waypoint_mode(&mut self, enabled: bool) {
        self.waypoint_mode = enabled;
    }

    pub fn is_in_waypoint_mode(&self) -> bool {
        self.waypoint_mode
    }

    pub fn set_prefer_selection_mode(&mut self, enabled: bool) {
        self.prefer_selection_mode = enabled;
    }

    pub fn is_in_prefer_selection_mode(&self) -> bool {
        self.prefer_selection_mode
    }

    // ── Camera control methods ─────────────────────────────────────────
    // C++: InGameUI.h:521-531

    pub fn set_camera_rotate_left(&mut self, set: bool) {
        self.camera_rotating_left = set;
    }

    pub fn is_camera_rotating_left(&self) -> bool {
        self.camera_rotating_left
    }

    pub fn set_camera_rotate_right(&mut self, set: bool) {
        self.camera_rotating_right = set;
    }

    pub fn is_camera_rotating_right(&self) -> bool {
        self.camera_rotating_right
    }

    pub fn set_camera_zoom_in(&mut self, set: bool) {
        self.camera_zooming_in = set;
    }

    pub fn is_camera_zooming_in(&self) -> bool {
        self.camera_zooming_in
    }

    pub fn set_camera_zoom_out(&mut self, set: bool) {
        self.camera_zooming_out = set;
    }

    pub fn is_camera_zooming_out(&self) -> bool {
        self.camera_zooming_out
    }

    pub fn set_camera_tracking_drawable(&mut self, set: bool) {
        self.camera_tracking_drawable = set;
    }

    pub fn is_camera_tracking_drawable(&self) -> bool {
        self.camera_tracking_drawable
    }

    // ── Selection query methods ────────────────────────────────────────
    // C++: InGameUI.cpp:4116 (areSelectedObjectsControllable)
    // C++: InGameUI.cpp:3333 (isAnySelectedKindOf)
    // C++: InGameUI.cpp:3357 (isAllSelectedKindOf)

    pub fn are_selected_objects_controllable(&self) -> bool {
        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return false;
        };
        let Some(selection) = manager.get_player_selection_ref(self.player_id as i32) else {
            return false;
        };
        let selected = selection.get_selected_objects();
        if selected.is_empty() {
            return false;
        }
        // C++: All selected objects have the same local controller, return first one
        if let Some(&first_id) = selected.first() {
            if let Some(obj) = TheGameLogic::find_object_by_id(first_id) {
                if let Ok(guard) = obj.read() {
                    return guard.is_locally_controlled();
                }
            }
        }
        false
    }

    pub fn is_any_selected_kind_of(&self, kind_of: KindOf) -> bool {
        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return false;
        };
        let Some(selection) = manager.get_player_selection_ref(self.player_id as i32) else {
            return false;
        };
        for object_id in selection.get_selected_objects() {
            if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(guard) = obj.read() {
                    if guard.is_kind_of(kind_of) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn is_all_selected_kind_of(&self, kind_of: KindOf) -> bool {
        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return true; // vacuously true when nothing selected (matches C++ empty-loop behavior)
        };
        let Some(selection) = manager.get_player_selection_ref(self.player_id as i32) else {
            return true;
        };
        for object_id in selection.get_selected_objects() {
            if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(guard) = obj.read() {
                    if !guard.is_kind_of(kind_of) {
                        return false;
                    }
                }
            }
        }
        true
    }

    // ── Advanced selection methods ─────────────────────────────────────
    // C++: InGameUI.cpp:4900 (selectUnitsMatchingCurrentSelection)
    // C++: InGameUI.cpp:4850 (selectMatchingAcrossMap)

    pub fn select_units_matching_current_selection(&mut self) -> i32 {
        // C++: First tries selectMatchingAcrossScreen(), if 0 results tries selectMatchingAcrossMap()
        let screen_count = self.select_matching_across_screen();
        if screen_count > 0 {
            return screen_count;
        }
        self.select_matching_across_map()
    }

    pub fn select_matching_across_screen(&mut self) -> i32 {
        let screen_region = with_tactical_view_ref(|view| {
            let tl = view.screen_to_world(&IPoint2::new(0, 0)).ok()?;
            let br = view
                .screen_to_world(&IPoint2::new(
                    self.screen_size.x as i32,
                    self.screen_size.y as i32,
                ))
                .ok()?;
            Some(IRegion2D::new(
                ICoord2D::new(tl.x.min(br.x).floor() as i32, tl.y.min(br.y).floor() as i32),
                ICoord2D::new(tl.x.max(br.x).ceil() as i32, tl.y.max(br.y).ceil() as i32),
            ))
        });
        let region = match screen_region {
            Some(r) => r,
            None => return self.select_matching_across_map(),
        };
        self.select_matching_across_region(&region)
    }

    fn select_matching_across_region(&mut self, region: &IRegion2D) -> i32 {
        let selection_manager = get_selection_manager();
        let selected_ids = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|s| s.get_selected_objects())
                .unwrap_or_default()
        } else {
            return -1;
        };

        let mut templates: Vec<String> = Vec::new();
        for &object_id in &selected_ids {
            if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(guard) = obj.read() {
                    if guard.is_locally_controlled() {
                        let name = guard.get_template_name().to_string();
                        if !templates.contains(&name) {
                            templates.push(name);
                        }
                    }
                }
            }
        }

        if templates.is_empty() {
            return -1;
        }

        let mut matching: Vec<ObjectID> = Vec::new();
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_selectable() || !guard.is_locally_controlled() {
                continue;
            }
            let pos = guard.get_position();
            if pos.x < region.lo.x as f32
                || pos.x > region.hi.x as f32
                || pos.y < region.lo.y as f32
                || pos.y > region.hi.y as f32
            {
                continue;
            }
            if templates.iter().any(|t| t == guard.get_template_name()) {
                matching.push(guard.get_id());
            }
        }

        if matching.is_empty() {
            return 0;
        }

        let count = matching.len() as i32;
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(matching, SelectionType::Add);
            }
        }
        self.frame_selection_changed = self.current_frame;
        self.sync_selection_state();
        count
    }

    pub fn select_matching_across_map(&mut self) -> i32 {
        // C++: InGameUI.cpp:4671 (selectMatchingAcrossRegion with NULL region)
        // Gets templates from current selection, iterates all objects, selects matching
        let selection_manager = get_selection_manager();
        let selected_ids = if let Ok(manager) = selection_manager.read() {
            manager
                .get_player_selection_ref(self.player_id as i32)
                .map(|s| s.get_selected_objects())
                .unwrap_or_default()
        } else {
            return -1;
        };

        // Collect unique template names from locally-controlled selected objects
        let mut templates: Vec<String> = Vec::new();
        for &object_id in &selected_ids {
            if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(guard) = obj.read() {
                    if guard.is_locally_controlled() {
                        let name = guard.get_template_name().to_string();
                        if !templates.contains(&name) {
                            templates.push(name);
                        }
                    }
                }
            }
        }

        if templates.is_empty() {
            return -1;
        }

        // Select all matching objects across the map
        let mut matching: Vec<ObjectID> = Vec::new();
        for obj in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = obj.read() else {
                continue;
            };
            if !guard.is_selectable() {
                continue;
            }
            if !guard.is_locally_controlled() {
                continue;
            }
            let obj_template = guard.get_template_name();
            if templates.iter().any(|t| t == obj_template) {
                matching.push(guard.get_id());
            }
        }

        if matching.is_empty() {
            return 0;
        }

        let count = matching.len() as i32;
        let selection_manager = get_selection_manager();
        if let Ok(mut manager) = selection_manager.write() {
            if let Some(selection) = manager.get_player_selection(self.player_id as i32) {
                selection.select_objects(matching, SelectionType::Add);
            }
        }
        self.frame_selection_changed = self.current_frame;
        self.sync_selection_state();
        count
    }

    // ── Drawable lifecycle ─────────────────────────────────────────────
    // C++: InGameUI.cpp:3415 (disregardDrawable)

    pub fn disregard_drawable(&mut self, drawable_id: u32) {
        self.deselect_object(drawable_id);
    }

    // ── Selection change tracking ──────────────────────────────────────

    pub fn get_frame_selection_changed(&self) -> u32 {
        self.frame_selection_changed
    }

    // ── Movie playback ────────────────────────────────────────────────
    // C++: InGameUI.cpp:3874 (playMovie), 3901 (stopMovie),
    //       3929 (playCameoMovie), 3959 (stopCameoMovie)

    pub fn play_movie(&mut self, movie_name: &str) {
        self.stop_movie();
        self.currently_playing_movie = Some(movie_name.to_string());
    }

    pub fn stop_movie(&mut self) {
        if self.currently_playing_movie.is_some() {
            with_window_video_manager(|manager| manager.stop_all_movies());
        }
        self.currently_playing_movie = None;
    }

    pub fn is_movie_playing(&self) -> bool {
        self.currently_playing_movie.is_some()
    }

    pub fn get_currently_playing_movie(&self) -> Option<&str> {
        self.currently_playing_movie.as_deref()
    }

    pub fn play_cameo_movie(&mut self, movie_name: &str) {
        self.stop_cameo_movie();
        self.cameo_movie_playing = Some(movie_name.to_string());
    }

    pub fn stop_cameo_movie(&mut self) {
        if self.cameo_movie_playing.is_some() {
            with_window_video_manager(|manager| manager.stop_all_movies());
        }
        self.cameo_movie_playing = None;
    }

    pub fn is_cameo_movie_playing(&self) -> bool {
        self.cameo_movie_playing.is_some()
    }

    // ── World animations ──────────────────────────────────────────────
    // C++: InGameUI.cpp:5257 (addWorldAnimation), 5292 (clearWorldAnimations),
    //       5323 (updateAndDrawWorldAnimations)

    pub fn add_world_animation(
        &mut self,
        animation_name: &str,
        pos: Coord3D,
        options: WorldAnimationOptions,
        duration_seconds: f32,
        z_rise_per_second: f32,
    ) {
        if duration_seconds <= 0.0 || animation_name.is_empty() {
            return;
        }

        let Some(collection) = get_anim2d_collection() else {
            return;
        };
        let collection_guard = collection.read();
        let template = collection_guard.find_template(&AsciiString::from(animation_name));
        let Some(template) = template else {
            return;
        };
        drop(collection_guard);

        let anim = crate::system::Anim2D::new(template, None);

        let expire_frame = self.current_frame + (duration_seconds * 30.0) as u32;
        self.world_animations.push(WorldAnimationData {
            anim,
            world_pos: pos,
            expire_frame,
            options,
            z_rise_per_second,
        });
    }

    pub fn clear_world_animations(&mut self) {
        self.world_animations.clear();
    }

    pub fn update_and_draw_world_animations(&mut self) {
        const FRAMES_BEFORE_EXPIRE_TO_FADE: u32 = 30;

        let current_frame = self.current_frame;
        let paused = TheGameLogic::is_game_paused();

        let local_player_index = gamelogic::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|player| player.read().ok().map(|g| g.get_player_index() as u32));

        let mut i = 0;
        while i < self.world_animations.len() {
            let expired = if !paused {
                current_frame >= self.world_animations[i].expire_frame
                    || (self.world_animations[i]
                        .options
                        .contains(WorldAnimationOptions::PLAY_ONCE_AND_DESTROY)
                        && self.world_animations[i]
                            .anim
                            .lock()
                            .get_status()
                            .contains(crate::system::Anim2DStatus::COMPLETE))
            } else {
                current_frame >= self.world_animations[i].expire_frame
            };

            if expired {
                self.world_animations.remove(i);
                continue;
            }

            if !paused && self.world_animations[i].z_rise_per_second != 0.0 {
                self.world_animations[i].world_pos.z +=
                    self.world_animations[i].z_rise_per_second / 30.0;
            }

            let shrouded = local_player_index
                .map(|player_idx| {
                    get_shroud_manager()
                        .lock()
                        .ok()
                        .map(|shroud| {
                            shroud.get_shroud_state(player_idx, &self.world_animations[i].world_pos)
                                != ShroudState::Visible
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false);

            if shrouded {
                i += 1;
                continue;
            }

            if self.world_animations[i]
                .options
                .contains(WorldAnimationOptions::FADE_ON_EXPIRE)
            {
                let frames_till_expire = self.world_animations[i]
                    .expire_frame
                    .saturating_sub(current_frame);
                if frames_till_expire < FRAMES_BEFORE_EXPIRE_TO_FADE {
                    let alpha = frames_till_expire as f32 / FRAMES_BEFORE_EXPIRE_TO_FADE as f32;
                    self.world_animations[i].anim.lock().set_alpha(alpha);
                }
            }

            let screen = self.world_to_screen(&self.world_animations[i].world_pos);
            if let Some(screen) = screen {
                let mut anim_guard = self.world_animations[i].anim.lock();
                let width = anim_guard.get_current_frame_width() as f32;
                let height = anim_guard.get_current_frame_height() as f32;

                let zoom_scale = with_tactical_view_ref(|view| {
                    let max_zoom = view.max_zoom();
                    let zoom = view.zoom();
                    if zoom > 0.0 {
                        max_zoom / zoom
                    } else {
                        1.0
                    }
                });

                let scaled_width = (width * zoom_scale) as i32;
                let scaled_height = (height * zoom_scale) as i32;

                let draw_x = (screen.x - scaled_width as f32 / 2.0) as i32;
                let draw_y = (screen.y - scaled_height as f32 / 2.0) as i32;

                anim_guard.draw_sized(draw_x, draw_y, scaled_width, scaled_height);
            }

            i += 1;
        }
    }

    // ── Lifecycle methods ──────────────────────────────────────────────
    // C++: InGameUI.cpp:1571 (preDraw)
    // C++: InGameUI.cpp:3426 (postDraw)

    pub fn pre_draw(&mut self, frame: u32) {
        self.current_frame = frame;
        self.expire_hints();
        self.update_floating_texts();
        self.update_superweapon_timers(frame);
        self.update_military_subtitle();
        self.update_and_draw_world_animations();
    }

    pub fn post_draw(&mut self, _frame: u32) {
        // C++: postDraw renders messages, military subtitles, superweapon timers
        // Rendering is handled separately in the Rust architecture; this hook
        // exists for any post-render cleanup logic needed later.
    }

    // ── Input enable/disable with mode clearing ────────────────────────
    // C++: InGameUI.cpp:3382 (setInputEnabled)

    pub fn set_input_enabled_and_clear_modes(&mut self, enabled: bool) {
        if !enabled {
            self.set_selecting(false);
        }

        if !enabled {
            // C++: Clear all special modes when input is disabled (cinematic safety)
            self.force_attack_mode = false;
            self.force_move_to_mode = false;
            self.waypoint_mode = false;
            self.prefer_selection_mode = false;
            self.camera_rotating_left = false;
            self.camera_rotating_right = false;
            self.camera_zooming_in = false;
            self.camera_zooming_out = false;
        }
        self.enabled = enabled;
    }
}

impl Default for SelectionBox {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotable for InGameUI {
    fn crc(&self, _xfer: &mut dyn Xfer) -> std::result::Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> std::result::Result<(), String> {
        let current_version: XferVersion = 3;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        if version >= 2 {
            xfer.xfer_int(&mut self.named_timer_last_flash_frame)
                .map_err(|e| e.to_string())?;
            xfer.xfer_bool(&mut self.named_timer_used_flash_color)
                .map_err(|e| e.to_string())?;
            xfer.xfer_bool(&mut self.show_named_timers)
                .map_err(|e| e.to_string())?;

            let mut timer_count = self.named_timers.len() as i32;
            xfer.xfer_int(&mut timer_count).map_err(|e| e.to_string())?;

            if xfer.is_writing() {
                for timer in self.named_timers.iter() {
                    let mut name = timer.name.clone();
                    let mut text = timer.text.clone();
                    let mut is_countdown = timer.is_countdown;
                    xfer.xfer_ascii_string(&mut name)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_unicode_string(&mut text)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_bool(&mut is_countdown)
                        .map_err(|e| e.to_string())?;
                }
            } else if xfer.is_reading() {
                self.named_timers.clear();
                for _ in 0..timer_count {
                    let mut name = String::new();
                    let mut text = String::new();
                    let mut is_countdown = false;
                    xfer.xfer_ascii_string(&mut name)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_unicode_string(&mut text)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_bool(&mut is_countdown)
                        .map_err(|e| e.to_string())?;
                    self.add_named_timer(&name, text, is_countdown);
                }
            }
        }

        xfer.xfer_bool(&mut self.quit_menu_visible)
            .map_err(|e| e.to_string())?;

        // C++: xfer->xferBool(&m_superweaponHiddenByScript) (InGameUI.cpp:387)
        xfer.xfer_bool(&mut self.superweapon_hidden_by_script)
            .map_err(|e| e.to_string())?;

        // Save/restore selection list (object IDs)
        let mut selection_count = self.get_selection().len() as i32;
        xfer.xfer_int(&mut selection_count)
            .map_err(|e| e.to_string())?;

        if xfer.is_writing() {
            for obj_id in self.get_selection() {
                let mut id = obj_id as u32;
                xfer.xfer_u32(&mut id).map_err(|e| e.to_string())?;
            }
        } else if xfer.is_reading() {
            let mut ids: Vec<ObjectID> = Vec::with_capacity(selection_count.max(0) as usize);
            for _ in 0..selection_count.max(0) {
                let mut id: u32 = 0;
                xfer.xfer_u32(&mut id).map_err(|e| e.to_string())?;
                ids.push(id);
            }
            let selection_manager = get_selection_manager();
            if let Ok(mut manager) = selection_manager.write() {
                if let Some(state) = manager.get_player_selection(self.player_id as i32) {
                    state.select_objects(ids, SelectionType::Replace);
                }
            }
            self.sync_selection_state();
        }

        // Save/restore control groups (10 groups, each a list of object IDs)
        for group_idx in 0..10usize {
            let group = self.get_control_group(group_idx as i32);
            let mut count = group.len() as i32;
            xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;

            if xfer.is_writing() {
                for obj_id in group {
                    let mut id = obj_id as u32;
                    xfer.xfer_u32(&mut id).map_err(|e| e.to_string())?;
                }
            } else if xfer.is_reading() {
                let mut group_ids: Vec<ObjectID> = Vec::with_capacity(count.max(0) as usize);
                for _ in 0..count.max(0) {
                    let mut id: u32 = 0;
                    xfer.xfer_u32(&mut id).map_err(|e| e.to_string())?;
                    group_ids.push(id);
                }
                let sm = get_selection_manager();
                let Ok(mut manager) = sm.write() else {
                    continue;
                };
                if let Some(state) = manager.get_player_selection(self.player_id as i32) {
                    state.set_control_group_objects(group_idx, group_ids);
                }
            }
        }

        // Save/restore superweapon timer data
        // C++: iterates m_superweapons[playerIndex][powerName] list, saves per-entry
        if xfer.is_writing() {
            let mut sw_count = self.superweapon_timers.len() as i32;
            xfer.xfer_int(&mut sw_count).map_err(|e| e.to_string())?;
            for timer in &self.superweapon_timers {
                let mut player_index = timer.player_index as i32;
                let mut object_id = timer.object_id as u32;
                let mut ready_frame = timer.ready_frame;
                let mut hidden_by_script = timer.hidden_by_script;
                let mut hidden_by_science = timer.hidden_by_science;
                let mut ready = timer.ready;
                let mut power_name = timer.power_name.clone();

                xfer.xfer_int(&mut player_index)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut object_id).map_err(|e| e.to_string())?;
                xfer.xfer_ascii_string(&mut power_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut ready_frame).map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_script)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_science)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut ready).map_err(|e| e.to_string())?;
            }
        } else if xfer.is_reading() {
            let mut sw_count: i32 = 0;
            xfer.xfer_int(&mut sw_count).map_err(|e| e.to_string())?;
            self.superweapon_timers.clear();
            for _ in 0..sw_count.max(0) {
                let mut player_index: i32 = 0;
                let mut object_id: u32 = 0;
                let mut power_name = String::new();
                let mut ready_frame: u32 = 0;
                let mut hidden_by_script = false;
                let mut hidden_by_science = false;
                let mut ready = false;

                xfer.xfer_int(&mut player_index)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut object_id).map_err(|e| e.to_string())?;
                xfer.xfer_ascii_string(&mut power_name)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_u32(&mut ready_frame).map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_script)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut hidden_by_science)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_bool(&mut ready).map_err(|e| e.to_string())?;

                self.superweapon_timers.push(SuperweaponTimerData {
                    player_index: player_index as u8,
                    object_id,
                    power_name,
                    ready_frame,
                    countdown_text: String::new(),
                    ready,
                    hidden_by_script,
                    hidden_by_science,
                });
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> std::result::Result<(), String> {
        Ok(())
    }
}

impl InGameUI {
    // ── HUD message system ──────────────────────────────────────────────
    // C++: InGameUI::message() (InGameUI.cpp:1993), addMessageText() (InGameUI.cpp:2061)

    pub fn message(&mut self, text: &str) {
        self.add_message_text(text, None);
    }

    pub fn message_color(&mut self, text: &str, color: u32) {
        self.add_message_text(text, Some(color));
    }

    fn add_message_text(&mut self, text: &str, rgb_color: Option<u32>) {
        if !self.messages_enabled {
            return;
        }

        let color1 = rgb_color.unwrap_or(self.message_color1);
        let color2 = rgb_color.unwrap_or(self.message_color2);

        let color = if self.messages.is_empty() || self.messages[0].color == color2 {
            color1
        } else {
            color2
        };

        let msg = MessageText {
            text: text.to_string(),
            color,
            creation_frame: self.current_frame,
        };

        self.messages.insert(0, msg);
        if self.messages.len() > MAX_UI_MESSAGES {
            self.messages.truncate(MAX_UI_MESSAGES);
        }
    }

    pub fn toggle_messages(&mut self) -> bool {
        self.messages_enabled = !self.messages_enabled;
        self.messages_enabled
    }

    pub fn are_messages_enabled(&self) -> bool {
        self.messages_enabled
    }

    pub fn expire_messages(&mut self) {
        let delay_frames = (self.message_delay_ms as f32 / 33.0) as u32;
        self.messages
            .retain(|m| self.current_frame < m.creation_frame + delay_frames);
    }

    pub fn remove_message_at_index(&mut self, index: usize) {
        if index < self.messages.len() {
            self.messages.remove(index);
        }
    }

    pub fn get_messages(&self) -> &[MessageText] {
        &self.messages
    }

    pub fn get_message_color1(&self) -> u32 {
        self.message_color1
    }

    pub fn get_message_color2(&self) -> u32 {
        self.message_color2
    }

    pub fn get_message_position(&self) -> (i32, i32) {
        self.message_position
    }

    pub fn get_message_font_name(&self) -> &str {
        &self.message_font_name
    }

    pub fn get_message_point_size(&self) -> i32 {
        self.message_point_size
    }

    pub fn is_message_bold(&self) -> bool {
        self.message_bold
    }

    // ── Military subtitle system ─────────────────────────────────────────
    // C++: InGameUI::militarySubtitle() (InGameUI.cpp:4039)
    // C++: InGameUI::removeMilitarySubtitle() (InGameUI.cpp:4093)

    pub fn military_subtitle(&mut self, title: &str, duration_ms: i32) {
        update_diplomacy_briefing_text(title, false);
        let title = Self::military_caption_text(title);
        if title.is_empty() || duration_ms <= 0 {
            return;
        }

        let multiplier_x = self.screen_size.x / 800.0;
        let multiplier_y = self.screen_size.y / 600.0;

        let pos_x = self.military_caption_position.0 as f32 * multiplier_x;
        let pos_y = self.military_caption_position.1 as f32 * multiplier_y;

        let lifetime_frame = self.current_frame + (30 * duration_ms as u32) / 1000;
        self.disable_tooltips_until(lifetime_frame);

        let color = ((self.military_caption_color.3 as u32) << 24)
            | ((self.military_caption_color.0 as u32) << 16)
            | ((self.military_caption_color.1 as u32) << 8)
            | (self.military_caption_color.2 as u32);

        self.current_military_subtitle = Some(MilitarySubtitle {
            text: title,
            index: 0,
            position: (pos_x, pos_y),
            lifetime_frame,
            block_drawn: true,
            block_begin_frame: self.current_frame,
            block_pos: (pos_x, pos_y),
            increment_on_frame: self.current_frame + Self::military_caption_delay_frames(),
            color,
        });
    }

    fn military_caption_text(label: &str) -> String {
        GameText::fetch(label)
    }

    fn mouseover_tooltip_text(template_name: &str, display_name: &str) -> Option<String> {
        let mut tooltip = display_name.trim().to_string();
        if tooltip.is_empty() {
            tooltip = GameText::fetch(&format!("ThingTemplate:{template_name}"));
        }

        if tooltip.is_empty() || tooltip == GameText::fetch("OBJECT:Prop") {
            return None;
        }

        Some(tooltip)
    }

    fn mouseover_tooltip_for_template(template_name: &str) -> Option<String> {
        let display_name = get_thing_factory()
            .ok()
            .and_then(|guard| {
                guard
                    .as_ref()
                    .and_then(|factory| factory.find_template(template_name, false))
                    .map(|template| template.get_display_name().to_string())
            })
            .unwrap_or_default();
        Self::mouseover_tooltip_text(template_name, &display_name)
    }

    fn format_supply_warehouse_tooltip_feedback(
        label: &str,
        boxes_stored: i32,
        base_value_per_supply_box: i32,
    ) -> String {
        let value = boxes_stored.max(0) * base_value_per_supply_box.max(0);
        let value_text = value.to_string();
        if label.contains("%d") {
            label.replace("%d", &value_text)
        } else if label.contains("%i") {
            label.replace("%i", &value_text)
        } else if label.contains("{}") {
            label.replacen("{}", &value_text, 1)
        } else {
            format!("{label}{value_text}")
        }
    }

    fn supply_warehouse_tooltip_feedback(
        boxes_stored: i32,
        base_value_per_supply_box: i32,
    ) -> String {
        let label = GameText::fetch("TOOLTIP:SupplyWarehouse");
        Self::format_supply_warehouse_tooltip_feedback(
            &label,
            boxes_stored,
            base_value_per_supply_box,
        )
    }

    fn supply_warehouse_boxes_for_object(object: &Object) -> Option<i32> {
        for behavior in object.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
                continue;
            };
            let Some(dock) = behavior.get_dock_update_interface() else {
                continue;
            };
            if let Some(boxes) = dock.supply_warehouse_boxes_stored() {
                return Some(boxes);
            }
        }
        None
    }

    fn ignored_gui_slaver_id_for_object(object: &Object) -> Option<ObjectID> {
        if !object.is_kind_of(KindOf::IgnoredInGui) {
            return None;
        }

        for behavior in object.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
                continue;
            };
            let Some(slaved) = behavior.get_slaved_update_interface() else {
                continue;
            };
            let Some(slaver_id) = slaved.slaver_id() else {
                continue;
            };
            if OBJECT_REGISTRY.get_object(slaver_id).is_some() {
                return Some(slaver_id);
            }
        }
        None
    }

    fn mouseover_drawable_id_for_object(drawable_id: u32, object: &Object) -> u32 {
        Self::ignored_gui_slaver_id_for_object(object).unwrap_or(drawable_id)
    }

    fn mouseover_tooltip_visible_for_shroud(status: ObjectShroudStatus) -> bool {
        matches!(
            status,
            ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear
        )
    }

    fn command_hint_after_shroud_projection(
        hint_type: CommandHintType,
        target_shroud: Option<ObjectShroudStatus>,
    ) -> CommandHintType {
        if matches!(
            hint_type,
            CommandHintType::AttackObject | CommandHintType::AttackObjectAfterMoving
        ) && target_shroud == Some(ObjectShroudStatus::Shrouded)
        {
            CommandHintType::MoveTo
        } else {
            hint_type
        }
    }

    fn consume_double_click_attack_move_guard_hint(timer: &mut u32) -> bool {
        if *timer == 0 {
            return false;
        }

        *timer -= 1;
        *timer > 0
    }

    fn default_command_hint_blocked_by_source(source_locally_controlled: Option<bool>) -> bool {
        source_locally_controlled == Some(false)
    }

    fn move_to_cursor_for_context(
        draw_selectable: bool,
        target_locally_controlled: bool,
        target_is_mine: bool,
        source_is_local_structure: bool,
    ) -> MouseCursor {
        if !draw_selectable && source_is_local_structure {
            MouseCursor::GenericInvalid
        } else if draw_selectable && target_locally_controlled && !target_is_mine {
            MouseCursor::Selecting
        } else {
            MouseCursor::MoveTo
        }
    }

    fn mouseover_cursor_update_allowed(
        recorder_playback_active: bool,
        look_at_mouse_moved_recently: bool,
    ) -> bool {
        !recorder_playback_active || look_at_mouse_moved_recently
    }

    fn command_hint_update_allowed(
        is_scrolling: bool,
        is_selecting: bool,
        recorder_playback_active: bool,
    ) -> bool {
        !(is_scrolling || is_selecting || recorder_playback_active)
    }

    fn selected_source_id_for_command_hint(&self) -> Option<u32> {
        let selected = self.get_selection();
        (selected.len() == 1).then(|| selected[0])
    }

    fn command_hint_source_context(object_id: u32) -> Option<(bool, bool)> {
        OBJECT_REGISTRY.get_object(object_id).and_then(|obj| {
            obj.read().ok().map(|guard| {
                (
                    guard.is_locally_controlled(),
                    guard.is_kind_of(KindOf::Structure),
                )
            })
        })
    }

    fn military_caption_delay_frames() -> u32 {
        let delay_ms = get_global_language_read()
            .map(|language| language.military_caption_delay_ms)
            .unwrap_or(750);
        Self::milliseconds_to_logic_frames(delay_ms)
    }

    fn milliseconds_to_logic_frames(milliseconds: i32) -> u32 {
        (30 * milliseconds.max(0) as u32) / 1000
    }

    pub fn remove_military_subtitle(&mut self) {
        self.current_military_subtitle = None;
        self.clear_tooltips_disabled();
    }

    pub fn get_military_subtitle(&self) -> Option<&MilitarySubtitle> {
        self.current_military_subtitle.as_ref()
    }

    pub fn expire_military_subtitle(&mut self) {
        if let Some(ref sub) = self.current_military_subtitle {
            if self.current_frame >= sub.lifetime_frame {
                self.remove_military_subtitle();
            }
        }
    }

    pub fn disable_tooltips_until(&mut self, frame_num: u32) {
        if frame_num > self.tooltips_disabled_until {
            self.tooltips_disabled_until = frame_num;
        }
    }

    pub fn clear_tooltips_disabled(&mut self) {
        self.tooltips_disabled_until = 0;
    }

    pub fn are_tooltips_disabled(&self) -> bool {
        self.current_frame < self.tooltips_disabled_until
    }

    fn update_military_subtitle(&mut self) {
        let speed_frames = self.military_caption_speed_frames();
        let point_size = self.military_caption_point_size;
        let char_width = self.caption_char_width();
        let delay_frames = Self::military_caption_delay_frames();
        Self::update_military_subtitle_state(
            &mut self.current_military_subtitle,
            self.current_frame,
            speed_frames,
            point_size,
            char_width,
            delay_frames,
        );
    }

    fn update_military_subtitle_state(
        current_subtitle: &mut Option<MilitarySubtitle>,
        current_frame: u32,
        speed_frames: u32,
        point_size: i32,
        char_width: f32,
        delay_frames: u32,
    ) {
        let Some(subtitle) = current_subtitle.as_mut() else {
            return;
        };

        if subtitle.lifetime_frame < current_frame {
            let alpha = (subtitle.color >> 24) as i32;
            let fade_amount = ((current_frame - subtitle.lifetime_frame) as f32 * 0.1) as i32;
            if alpha - fade_amount < 0 {
                *current_subtitle = None;
            } else {
                let new_alpha = (alpha - fade_amount) as u32;
                subtitle.color = (subtitle.color & 0x00FF_FFFF) | (new_alpha << 24);
            }
            return;
        }

        if subtitle.block_begin_frame + 9 < current_frame {
            subtitle.block_begin_frame = current_frame;
            subtitle.block_drawn = !subtitle.block_drawn;
        }

        if subtitle.increment_on_frame >= current_frame {
            return;
        }

        let Some(ch) = subtitle.text.chars().nth(subtitle.index) else {
            subtitle.increment_on_frame = subtitle.lifetime_frame + 1;
            return;
        };

        if ch == '\n' {
            subtitle.block_pos.0 = subtitle.position.0;
            subtitle.block_pos.1 += point_size.max(1) as f32;
            subtitle.block_drawn = true;
            subtitle.increment_on_frame = current_frame + delay_frames;
        } else {
            let printed_chars_on_line = subtitle
                .text
                .chars()
                .take(subtitle.index + 1)
                .fold(0usize, |count, c| if c == '\n' { 0 } else { count + 1 });
            subtitle.block_pos.0 =
                subtitle.position.0 + (printed_chars_on_line as f32 * char_width);
            subtitle.increment_on_frame = current_frame + speed_frames;
        }

        subtitle.index += 1;
        if subtitle.index >= subtitle.text.chars().count() {
            subtitle.increment_on_frame = subtitle.lifetime_frame + 1;
        }
    }

    fn caption_char_width(&self) -> f32 {
        self.military_caption_point_size.max(1) as f32 * 0.6
    }

    fn military_caption_speed_frames(&self) -> u32 {
        get_global_language_read()
            .map(|language| language.military_caption_speed.max(0) as u32)
            .unwrap_or_else(|| self.military_caption_speed.max(0) as u32)
    }

    // ── Popup message system ─────────────────────────────────────────────
    // C++: InGameUI::popupMessage() (InGameUI.cpp:5137)

    pub fn get_popup_message_color(&self) -> u32 {
        self.popup_message_color
    }

    // ── INI settings loading ─────────────────────────────────────────────
    // C++: InGameUI::init() loads Data\INI\InGameUI.ini via TheINIParser

    pub fn init_from_settings(&mut self, settings: &InGameUIIniSettings) {
        if settings.max_selection_size > 0 {
            self.selection_state = SelectionState::new(settings.max_selection_size as usize);
        }

        self.message_color1 = settings.message_color1;
        self.message_color2 = settings.message_color2;
        self.message_position = (settings.message_position.x, settings.message_position.y);
        self.message_font_name = settings.message_font.clone();
        self.message_point_size = settings.message_point_size;
        self.message_bold = settings.message_bold;
        self.message_delay_ms = settings.message_delay_ms;

        self.military_caption_color = (
            settings.military_caption_color.red,
            settings.military_caption_color.green,
            settings.military_caption_color.blue,
            settings.military_caption_color.alpha,
        );
        self.military_caption_position = (
            settings.military_caption_position.x,
            settings.military_caption_position.y,
        );
        self.military_caption_title_font = settings.military_caption_title_font.clone();
        self.military_caption_title_point_size = settings.military_caption_title_point_size;
        self.military_caption_title_bold = settings.military_caption_title_bold;
        self.military_caption_font = settings.military_caption_font.clone();
        self.military_caption_point_size = settings.military_caption_point_size;
        self.military_caption_bold = settings.military_caption_bold;
        self.military_caption_randomize_typing = settings.military_caption_randomize_typing;
        self.military_caption_speed = settings.military_caption_speed;

        self.superweapon_countdown_position = (
            settings.superweapon_position.x,
            settings.superweapon_position.y,
        );
        self.superweapon_flash_duration = settings.superweapon_flash_duration;
        self.superweapon_flash_color = settings.superweapon_flash_color;
        self.superweapon_normal_font = settings.superweapon_normal_font.clone();
        self.superweapon_normal_point_size = settings.superweapon_normal_point_size;
        self.superweapon_normal_bold = settings.superweapon_normal_bold;
        self.superweapon_ready_font = settings.superweapon_ready_font.clone();
        self.superweapon_ready_point_size = settings.superweapon_ready_point_size;
        self.superweapon_ready_bold = settings.superweapon_ready_bold;

        self.drawable_caption_font = settings.drawable_caption_font.clone();
        self.drawable_caption_point_size = settings.drawable_caption_point_size;
        self.drawable_caption_bold = settings.drawable_caption_bold;
        self.drawable_caption_color = settings.drawable_caption_color;

        self.draw_rmb_scroll_anchor = settings.draw_rmb_scroll_anchor;
        self.move_rmb_scroll_anchor = settings.move_rmb_scroll_anchor;

        self.apply_global_language_font_overrides();
    }

    fn apply_global_language_font_overrides(&mut self) {
        let Some(language) = get_global_language_read() else {
            return;
        };

        if let Some((name, size, bold)) = Self::language_font_override(&language.message_font) {
            self.message_font_name = name;
            self.message_point_size = size;
            self.message_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.military_caption_title_font)
        {
            self.military_caption_title_font = name;
            self.military_caption_title_point_size = size;
            self.military_caption_title_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.military_caption_font)
        {
            self.military_caption_font = name;
            self.military_caption_point_size = size;
            self.military_caption_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.superweapon_countdown_normal_font)
        {
            self.superweapon_normal_font = name;
            self.superweapon_normal_point_size = size;
            self.superweapon_normal_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.superweapon_countdown_ready_font)
        {
            self.superweapon_ready_font = name;
            self.superweapon_ready_point_size = size;
            self.superweapon_ready_bold = bold;
        }

        if let Some((name, size, bold)) =
            Self::language_font_override(&language.drawable_caption_font)
        {
            self.drawable_caption_font = name;
            self.drawable_caption_point_size = size;
            self.drawable_caption_bold = bold;
        }
    }

    fn language_font_override(font: &FontDesc) -> Option<(String, i32, bool)> {
        (!font.name.is_empty() && *font != FontDesc::default())
            .then(|| (font.name.clone(), font.size, font.bold))
    }

    // ── Command hint system ──────────────────────────────────────────────
    // C++: InGameUI::createCommandHint() (InGameUI.cpp:2500-2772)
    // C++: InGameUI::createMouseoverHint() (InGameUI.cpp:2217-2494)

    /// Invalid drawable ID sentinel. C++: INVALID_DRAWABLE_ID (Drawable.h)
    /// In Rust, 0 is used as the invalid sentinel for moused_over_drawable_id.
    const INVALID_DRAWABLE_ID: u32 = 0;

    /// Get selection count. C++: InGameUI::getSelectCount() (InGameUI.h)
    fn get_select_count(&self) -> usize {
        self.get_selection().len()
    }

    fn cursor_name_to_i32(&self, name: &str) -> i32 {
        match name {
            "ARROW" => MouseCursor::Arrow as i32,
            "SELECTING" => MouseCursor::Selecting as i32,
            "MOVETO" => MouseCursor::MoveTo as i32,
            "ATTACKMOVETO" => MouseCursor::AttackMoveTo as i32,
            "ATTACK_OBJECT" => MouseCursor::AttackObject as i32,
            "FORCE_ATTACK_OBJECT" => MouseCursor::ForceAttackObject as i32,
            "FORCE_ATTACK_GROUND" => MouseCursor::ForceAttackGround as i32,
            "BUILD_PLACEMENT" => MouseCursor::BuildPlacement as i32,
            "INVALID_BUILD_PLACEMENT" => MouseCursor::InvalidBuildPlacement as i32,
            "GENERIC_INVALID" => MouseCursor::GenericInvalid as i32,
            "SET_RALLY_POINT" => MouseCursor::SetRallyPoint as i32,
            "GET_REPAIRED" => MouseCursor::GetRepaired as i32,
            "DOCK" => MouseCursor::Dock as i32,
            "GET_HEALED" => MouseCursor::GetHealed as i32,
            "DO_REPAIR" => MouseCursor::DoRepair as i32,
            "RESUME_CONSTRUCTION" => MouseCursor::ResumeConstruction as i32,
            "ENTER_FRIENDLY" => MouseCursor::EnterFriendly as i32,
            "ENTER_AGGRESSIVELY" => MouseCursor::EnterAggressively as i32,
            "DEFECTOR" => MouseCursor::Defector as i32,
            "CAPTUREBUILDING" => MouseCursor::CaptureBuilding as i32,
            "HACK" => MouseCursor::Hack as i32,
            "OUTRANGE" => MouseCursor::OutOfRange as i32,
            "WAYPOINT" => MouseCursor::Waypoint as i32,
            _ => MouseCursor::Arrow as i32,
        }
    }

    /// Port of C++ InGameUI::createCommandHint() (InGameUI.cpp:2500-2772).
    ///
    /// Handles 25+ message types across 3 mouse modes to set the appropriate
    /// mouse cursor and radius cursor as a preview of what command would be
    /// issued if the player clicked.
    pub fn create_command_hint(&mut self, hint_type: CommandHintType) {
        // Early exit: no cursor hints while scrolling, selecting, or in playback
        if !Self::command_hint_update_allowed(
            self.is_scrolling,
            self.is_selecting,
            self.recorder_playback_active,
        ) {
            return;
        }

        // C++: setRadiusCursorNone() at the top of createCommandHint
        self.clear_radius_cursor();

        // C++: doubleClickAttackMove guard timer — suppresses hints for a few frames
        // after a double-click attack-move to prevent spurious cursor flicker.
        if Self::consume_double_click_attack_move_guard_hint(
            &mut self.double_click_attack_move_guard_timer,
        ) {
            self.set_mouse_cursor(MouseCursor::ForceAttackGround);
            self.set_radius_cursor(
                RadiusCursorType::GuardArea,
                Coord3D::new(0.0, 0.0, 0.0),
                1.0,
            );
            return;
        }

        let target_shroud = match hint_type {
            CommandHintType::AttackObject | CommandHintType::AttackObjectAfterMoving => {
                if self.moused_over_drawable_id == Self::INVALID_DRAWABLE_ID {
                    None
                } else {
                    OBJECT_REGISTRY
                        .get_object(self.moused_over_drawable_id)
                        .and_then(|obj| {
                            obj.read()
                                .ok()
                                .map(|guard| guard.get_shrouded_status(self.player_id as i32))
                        })
                }
            }
            _ => None,
        };
        let hint_type = Self::command_hint_after_shroud_projection(hint_type, target_shroud);

        // C++: underWindow — WindowManager not yet ported; no opaque window
        // can cover the game area in the current architecture, so underWindow = false.
        let _under_window = false;

        match self.mouse_mode {
            MouseMode::Default => {
                // C++: InGameUI.cpp:2585-2688
                // This section only applies when there is no specific cursor mode happening.
                // C++: if (underWindow || (srcObj && !srcObj->isLocallyControlled()))
                // underWindow = false (WindowManager not ported; no opaque window covers game area)
                let source_context = self
                    .selected_source_id_for_command_hint()
                    .and_then(Self::command_hint_source_context);
                if Self::default_command_hint_blocked_by_source(
                    source_context.map(|(locally_controlled, _)| locally_controlled),
                ) {
                    self.set_mouse_cursor(MouseCursor::Arrow);
                    return;
                }

                match hint_type {
                    CommandHintType::MoveTo => {
                        // C++: MSG_DO_MOVETO_HINT (InGameUI.cpp:2595-2608)
                        // If hovering over a selectable, locally-controlled, non-mine drawable,
                        // C++ uses SELECTING cursor instead of MoveTo.
                        let source_is_local_structure = source_context == Some((true, true));
                        if self.moused_over_drawable_id != Self::INVALID_DRAWABLE_ID {
                            if let Some(obj) =
                                OBJECT_REGISTRY.get_object(self.moused_over_drawable_id)
                            {
                                if let Ok(guard) = obj.read() {
                                    self.set_mouse_cursor(Self::move_to_cursor_for_context(
                                        guard.is_selectable(),
                                        guard.is_locally_controlled(),
                                        guard.is_kind_of(KindOf::Mine),
                                        source_is_local_structure,
                                    ));
                                } else {
                                    self.set_mouse_cursor(Self::move_to_cursor_for_context(
                                        false,
                                        false,
                                        false,
                                        source_is_local_structure,
                                    ));
                                }
                            } else {
                                self.set_mouse_cursor(Self::move_to_cursor_for_context(
                                    false,
                                    false,
                                    false,
                                    source_is_local_structure,
                                ));
                            }
                        } else {
                            self.set_mouse_cursor(Self::move_to_cursor_for_context(
                                false,
                                false,
                                false,
                                source_is_local_structure,
                            ));
                        }
                    }
                    CommandHintType::AttackMoveTo => {
                        // C++: MSG_DO_ATTACKMOVETO_HINT (InGameUI.cpp:2610-2615)
                        if self.moused_over_drawable_id != Self::INVALID_DRAWABLE_ID {
                            if let Some(obj) =
                                OBJECT_REGISTRY.get_object(self.moused_over_drawable_id)
                            {
                                if let Ok(guard) = obj.read() {
                                    if guard.is_selectable() && guard.is_locally_controlled() {
                                        self.set_mouse_cursor(MouseCursor::Selecting);
                                    } else {
                                        self.set_mouse_cursor(MouseCursor::AttackMoveTo);
                                    }
                                } else {
                                    self.set_mouse_cursor(MouseCursor::AttackMoveTo);
                                }
                            } else {
                                self.set_mouse_cursor(MouseCursor::AttackMoveTo);
                            }
                        } else {
                            self.set_mouse_cursor(MouseCursor::AttackMoveTo);
                        }
                    }
                    CommandHintType::AddWaypoint => {
                        // C++: MSG_ADD_WAYPOINT_HINT (InGameUI.cpp:2616-2618)
                        self.set_mouse_cursor(MouseCursor::Waypoint);
                    }
                    CommandHintType::AttackObject => {
                        // C++: MSG_DO_ATTACK_OBJECT_HINT (InGameUI.cpp:2619-2621)
                        self.set_mouse_cursor(MouseCursor::AttackObject);
                    }
                    CommandHintType::AttackObjectAfterMoving => {
                        // C++: MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT (InGameUI.cpp:2622-2624)
                        self.set_mouse_cursor(MouseCursor::OutOfRange);
                    }
                    CommandHintType::ForceAttackObject => {
                        // C++: MSG_DO_FORCE_ATTACK_OBJECT_HINT (InGameUI.cpp:2625-2627)
                        self.set_mouse_cursor(MouseCursor::ForceAttackObject);
                    }
                    CommandHintType::ForceAttackGround => {
                        // C++: MSG_DO_FORCE_ATTACK_GROUND_HINT (InGameUI.cpp:2628-2630)
                        self.set_mouse_cursor(MouseCursor::ForceAttackGround);
                    }
                    CommandHintType::GetRepaired => {
                        // C++: MSG_GET_REPAIRED_HINT (InGameUI.cpp:2631-2633)
                        self.set_mouse_cursor(MouseCursor::GetRepaired);
                    }
                    CommandHintType::Dock => {
                        // C++: MSG_DOCK_HINT (InGameUI.cpp:2634-2636)
                        self.set_mouse_cursor(MouseCursor::Dock);
                    }
                    CommandHintType::GetHealed => {
                        // C++: MSG_GET_HEALED_HINT (InGameUI.cpp:2637-2639)
                        self.set_mouse_cursor(MouseCursor::GetHealed);
                    }
                    CommandHintType::DoRepair => {
                        // C++: MSG_DO_REPAIR_HINT (InGameUI.cpp:2640-2642)
                        self.set_mouse_cursor(MouseCursor::DoRepair);
                    }
                    CommandHintType::ResumeConstruction => {
                        // C++: MSG_RESUME_CONSTRUCTION_HINT (InGameUI.cpp:2643-2645)
                        self.set_mouse_cursor(MouseCursor::ResumeConstruction);
                    }
                    CommandHintType::Enter => {
                        // C++: MSG_ENTER_HINT (InGameUI.cpp:2646-2648)
                        self.set_mouse_cursor(MouseCursor::EnterFriendly);
                    }
                    CommandHintType::ConvertToCarbomb
                    | CommandHintType::Hijack
                    | CommandHintType::Sabotage => {
                        // C++: MSG_CONVERT_TO_CARBOMB_HINT, MSG_HIJACK_HINT,
                        //       MSG_SABOTAGE_HINT (InGameUI.cpp:2649-2653)
                        self.set_mouse_cursor(MouseCursor::EnterAggressively);
                    }
                    CommandHintType::Defector => {
                        // C++: MSG_DEFECTOR_HINT (InGameUI.cpp:2654-2656)
                        self.set_mouse_cursor(MouseCursor::Defector);
                    }
                    CommandHintType::PickUpPrisoner => {
                        // C++: MSG_PICK_UP_PRISONER_HINT (InGameUI.cpp:2658-2661)
                        // ALLOW_SURRENDER conditional — not in retail Zero Hour
                        // Keep for parity if the build supports it
                        self.set_mouse_cursor(MouseCursor::Defector); // Closest available cursor
                    }
                    CommandHintType::CaptureBuilding => {
                        // C++: MSG_CAPTUREBUILDING_HINT (InGameUI.cpp:2662-2664)
                        self.set_mouse_cursor(MouseCursor::CaptureBuilding);
                    }
                    CommandHintType::Hack => {
                        // C++: MSG_HACK_HINT (InGameUI.cpp:2665-2667)
                        self.set_mouse_cursor(MouseCursor::Hack);
                    }
                    CommandHintType::ImpossibleAttack => {
                        // C++: MSG_IMPOSSIBLE_ATTACK_HINT (InGameUI.cpp:2668-2670)
                        self.set_mouse_cursor(MouseCursor::GenericInvalid);
                    }
                    CommandHintType::SetRallyPoint => {
                        // C++: MSG_SET_RALLY_POINT_HINT (InGameUI.cpp:2671-2676)
                        // If hovering over a selectable, locally-controlled drawable, use SELECTING
                        if self.moused_over_drawable_id != Self::INVALID_DRAWABLE_ID {
                            if let Some(obj) =
                                OBJECT_REGISTRY.get_object(self.moused_over_drawable_id)
                            {
                                if let Ok(guard) = obj.read() {
                                    if guard.is_selectable() && guard.is_locally_controlled() {
                                        self.set_mouse_cursor(MouseCursor::Selecting);
                                    } else {
                                        self.set_mouse_cursor(MouseCursor::SetRallyPoint);
                                    }
                                } else {
                                    self.set_mouse_cursor(MouseCursor::SetRallyPoint);
                                }
                            } else {
                                self.set_mouse_cursor(MouseCursor::SetRallyPoint);
                            }
                        } else {
                            self.set_mouse_cursor(MouseCursor::SetRallyPoint);
                        }
                    }
                    CommandHintType::SpecialPowerOverrideDestination => {
                        // C++: MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT (InGameUI.cpp:2677-2679)
                        self.set_mouse_cursor(MouseCursor::ParticleUplinkCannon);
                    }
                    CommandHintType::DoSalvage => {
                        // C++: MSG_DO_SALVAGE_HINT (InGameUI.cpp:2680-2682)
                        self.set_mouse_cursor(MouseCursor::MoveTo);
                    }
                    CommandHintType::Invalid => {
                        // C++: MSG_DO_INVALID_HINT (InGameUI.cpp:2683-2685)
                        self.set_mouse_cursor(MouseCursor::GenericInvalid);
                    }
                    CommandHintType::ValidGuiCommand | CommandHintType::InvalidGuiCommand => {
                        // These are handled in MOUSEMODE_GUI_COMMAND, not here.
                        // Fall through to no-op in Default mode.
                    }
                }
            }
            MouseMode::BuildPlace => {
                // C++: InGameUI.cpp:2689-2708
                // underWindow = false (WindowManager not ported)

                match hint_type {
                    CommandHintType::MoveTo
                    | CommandHintType::AttackMoveTo
                    | CommandHintType::AddWaypoint => {
                        // C++: MSG_DO_MOVETO_HINT, MSG_DO_ATTACKMOVETO_HINT, MSG_ADD_WAYPOINT
                        // C++: setMouseCursor(Mouse::BUILD_PLACEMENT) (InGameUI.cpp:2701)
                        self.set_mouse_cursor(MouseCursor::BuildPlacement);
                    }
                    CommandHintType::AttackObject | CommandHintType::AttackObjectAfterMoving => {
                        // C++: MSG_DO_ATTACK_OBJECT_HINT, MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT
                        // C++: setMouseCursor(Mouse::INVALID_BUILD_PLACEMENT) (InGameUI.cpp:2705)
                        self.set_mouse_cursor(MouseCursor::InvalidBuildPlacement);
                    }
                    _ => {
                        // Other hint types in build-place mode default to build cursor
                        self.set_mouse_cursor(MouseCursor::BuildPlacement);
                    }
                }
            }
            MouseMode::GuiCommand => {
                // C++: InGameUI.cpp:2710-2769
                // underWindow = false (WindowManager not ported)

                if let Some(pending) = TheInGameUI::get_pending_command() {
                    let cursor_name = &pending.cursor_name;
                    if !cursor_name.is_empty() {
                        if let Some(cursor) =
                            MouseCursor::from_i32(self.cursor_name_to_i32(cursor_name))
                        {
                            self.set_mouse_cursor(cursor);
                        } else {
                            self.set_mouse_cursor(self.mouse_mode_cursor);
                        }
                    } else {
                        self.set_mouse_cursor(self.mouse_mode_cursor);
                    }
                    let rc_type = &pending.radius_cursor_type;
                    if !rc_type.is_empty() && !rc_type.eq_ignore_ascii_case("NONE") {
                        TheInGameUI::set_radius_cursor_active_with_type(rc_type);
                    }
                } else {
                    self.set_mouse_cursor(self.mouse_mode_cursor);
                }
            }
        }
    }

    /// Port of C++ InGameUI::createMouseoverHint() (InGameUI.cpp:2217-2494).
    ///
    /// Handles mouse-over drawable/location hints. Updates the moused-over
    /// drawable ID and sets the cursor to SELECTING for selectable+controlled
    /// drawables, or ARROW otherwise.
    ///
    /// Simplified from C++: player-name suffixes and special object cases are
    /// deferred until the rest of the tooltip path is ported.
    pub fn create_mouseover_hint(&mut self, drawable_id: Option<u32>, is_location_hint: bool) {
        // Phase 1: Early exit guards
        // C++: if (m_isScrolling || m_isSelecting) return;
        if self.is_scrolling || self.is_selecting {
            return;
        }

        // C++: underWindow — WindowManager not yet ported; underWindow = false
        let _under_window = false;

        // Phase 2: Update moused_over_drawable_id
        // C++: InGameUI.cpp:2254-2454 — extensive tooltip/drawable logic
        let old_id = self.moused_over_drawable_id;
        if is_location_hint {
            // C++: else branch (MSG_MOUSEOVER_LOCATION_HINT) — line 2451-2454
            self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
        } else if let Some(draw_id) = drawable_id {
            with_mouse(|m| m.set_cursor_tooltip(String::new(), None, None, None));
            if draw_id == Self::INVALID_DRAWABLE_ID {
                self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
            } else {
                if let Some(obj) = OBJECT_REGISTRY.get_object(draw_id) {
                    if let Ok(guard) = obj.read() {
                        self.moused_over_drawable_id =
                            Self::mouseover_drawable_id_for_object(draw_id, &guard);

                        // C++: TheMouse->setCursorTooltip(displayName, -1, playerColor, widthMult)
                        // Deferred C++ behaviors: Disguiser detection, multiplayer player suffix,
                        // stealth-garrison player color.
                        let visible = Self::mouseover_tooltip_visible_for_shroud(
                            guard.get_shrouded_status(self.player_id as i32),
                        );
                        if visible {
                            if let Some(mut display_name) =
                                Self::mouseover_tooltip_for_template(guard.get_template_name())
                            {
                                if let Some(boxes) = Self::supply_warehouse_boxes_for_object(&guard)
                                {
                                    let base_value = global_data::read_safe()
                                        .map(|data| data.base_value_per_supply_box)
                                        .unwrap_or(100);
                                    display_name.push_str(
                                        &Self::supply_warehouse_tooltip_feedback(boxes, base_value),
                                    );
                                }
                                let indicator = guard.get_indicator_color();
                                with_mouse(|m| {
                                    m.set_cursor_tooltip(
                                        display_name,
                                        Some(-1),
                                        Some([indicator.r, indicator.g, indicator.b, indicator.a]),
                                        None,
                                    );
                                });
                            }
                        }
                    }
                } else {
                    self.moused_over_drawable_id = draw_id;
                }
            }
        } else {
            self.moused_over_drawable_id = Self::INVALID_DRAWABLE_ID;
        }

        // C++: TheMouse->resetTooltipDelay() when ID changes
        if old_id != self.moused_over_drawable_id {
            with_mouse(|m| m.reset_tooltip_delay());
        }

        // Phase 3: Cursor assignment
        // C++: InGameUI.cpp:2462-2493
        if self.mouse_mode == MouseMode::Default
            && !self.is_scrolling
            && !self.is_selecting
            && self.get_select_count() == 0
            && Self::mouseover_cursor_update_allowed(
                self.recorder_playback_active,
                self.look_at_mouse_moved_recently,
            )
        {
            if self.moused_over_drawable_id != Self::INVALID_DRAWABLE_ID {
                // C++: CanSelectDrawable(draw, FALSE) and obj->isLocallyControlled()
                let can_select = match OBJECT_REGISTRY.get_object(self.moused_over_drawable_id) {
                    Some(obj_ref) => obj_ref
                        .read()
                        .map(|g| g.is_selectable() && g.is_locally_controlled())
                        .unwrap_or(false),
                    None => false,
                };
                if can_select {
                    self.set_mouse_cursor(MouseCursor::Selecting);
                } else {
                    self.set_mouse_cursor(MouseCursor::Arrow);
                }
            } else {
                self.set_mouse_cursor(MouseCursor::Arrow);
            }
        } else if self.mouse_mode == MouseMode::GuiCommand {
            // C++: InGameUI.cpp:2490-2493
            // Restore the saved command cursor
            self.set_mouse_cursor(self.mouse_mode_cursor);
        }
    }
}

/// Command hint types. C++: GameMessage types used in InGameUI::createCommandHint() (InGameUI.cpp:2500-2772)
///
/// Maps from GameMessageType variants to the cursor assignment logic in createCommandHint.
/// Each variant corresponds to one or more C++ GameMessage::Type values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandHintType {
    /// C++: MSG_DO_MOVETO_HINT
    MoveTo,
    /// C++: MSG_DO_ATTACKMOVETO_HINT
    AttackMoveTo,
    /// C++: MSG_ADD_WAYPOINT_HINT
    AddWaypoint,
    /// C++: MSG_DO_ATTACK_OBJECT_HINT
    AttackObject,
    /// C++: MSG_DO_ATTACK_OBJECT_AFTER_MOVING_HINT
    AttackObjectAfterMoving,
    /// C++: MSG_DO_FORCE_ATTACK_OBJECT_HINT
    ForceAttackObject,
    /// C++: MSG_DO_FORCE_ATTACK_GROUND_HINT
    ForceAttackGround,
    /// C++: MSG_GET_REPAIRED_HINT
    GetRepaired,
    /// C++: MSG_DOCK_HINT
    Dock,
    /// C++: MSG_GET_HEALED_HINT
    GetHealed,
    /// C++: MSG_DO_REPAIR_HINT
    DoRepair,
    /// C++: MSG_RESUME_CONSTRUCTION_HINT
    ResumeConstruction,
    /// C++: MSG_ENTER_HINT
    Enter,
    /// C++: MSG_CONVERT_TO_CARBOMB_HINT
    ConvertToCarbomb,
    /// C++: MSG_HIJACK_HINT
    Hijack,
    /// C++: MSG_SABOTAGE_HINT
    Sabotage,
    /// C++: MSG_DEFECTOR_HINT
    Defector,
    /// C++: MSG_PICK_UP_PRISONER_HINT (ALLOW_SURRENDER conditional)
    PickUpPrisoner,
    /// C++: MSG_CAPTUREBUILDING_HINT
    CaptureBuilding,
    /// C++: MSG_HACK_HINT
    Hack,
    /// C++: MSG_IMPOSSIBLE_ATTACK_HINT
    ImpossibleAttack,
    /// C++: MSG_SET_RALLY_POINT_HINT
    SetRallyPoint,
    /// C++: MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION_HINT
    SpecialPowerOverrideDestination,
    /// C++: MSG_DO_SALVAGE_HINT
    DoSalvage,
    /// C++: MSG_DO_INVALID_HINT
    Invalid,
    /// C++: MSG_VALID_GUICOMMAND_HINT
    ValidGuiCommand,
    /// C++: MSG_INVALID_GUICOMMAND_HINT
    InvalidGuiCommand,
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::ini::ini_language::init_global_language;
    use game_engine::common::language::Language;

    #[test]
    fn test_selection_box() {
        let mut box_sel = SelectionBox::new();
        assert!(!box_sel.active);

        box_sel.start_at(Vec2::new(10.0, 10.0));
        assert!(box_sel.active);

        box_sel.update(Vec2::new(100.0, 100.0));
        assert!(box_sel.is_significant());

        let rect = box_sel.get_rect();
        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.y, 10.0);
        assert_eq!(rect.width, 90.0);
        assert_eq!(rect.height, 90.0);
    }

    #[test]
    fn test_minimap_conversion() {
        let minimap = Minimap::new(Vec2::new(600.0, 400.0), Vec2::new(200.0, 200.0));

        let world_pos = Vec2::new(500.0, 500.0);
        let minimap_pos = minimap.world_to_minimap(world_pos);

        // Should be roughly in middle of minimap
        assert!((minimap_pos.x - 700.0).abs() < 1.0);
        assert!((minimap_pos.y - 500.0).abs() < 1.0);
    }

    #[test]
    fn test_selection_state() {
        let mut state = SelectionState::new(10);

        state.select(DrawableID(1), false);
        assert_eq!(state.count(), 1);

        state.select(DrawableID(2), true);
        assert_eq!(state.count(), 2);

        state.deselect(DrawableID(1));
        assert_eq!(state.count(), 1);
        assert!(!state.is_selected(DrawableID(1)));
        assert!(state.is_selected(DrawableID(2)));
    }

    #[test]
    fn test_placement_preview() {
        let mut preview = PlacementPreview::new("GLA_SupplyStash".into(), Vec2::new(3.0, 3.0));

        preview.update_position(Vec3::new(100.0, 0.0, 100.0), true);
        assert!(preview.is_legal);

        let color = preview.get_color();
        assert_eq!(color[0], LEGAL_BUILD_COLOR[0]);
        assert_eq!(color[3], PLACEMENT_OPACITY);
    }

    #[test]
    fn test_resource_display() {
        let mut display = ResourceDisplay::new(Vec2::ZERO);

        display.update(10000, 100, 50);
        assert_eq!(display.credits, 10000);
        assert!(!display.is_power_deficit());
        assert!((display.get_power_percentage() - 0.5).abs() < 0.01);

        display.update(5000, 100, 150);
        assert!(display.is_power_deficit());
    }

    #[test]
    fn military_caption_delay_uses_global_language_default_delay_ms() {
        init_global_language();
        assert_eq!(InGameUI::military_caption_delay_frames(), 22);
    }

    #[test]
    fn military_caption_milliseconds_convert_to_logic_frames() {
        assert_eq!(InGameUI::milliseconds_to_logic_frames(750), 22);
        assert_eq!(InGameUI::milliseconds_to_logic_frames(1000), 30);
        assert_eq!(InGameUI::milliseconds_to_logic_frames(-1), 0);
    }

    #[test]
    fn military_caption_fetches_localized_text() {
        Language::clear_localized_strings();
        Language::register_localized_string("SCRIPT:Briefing", "Localized briefing text");

        assert_eq!(
            InGameUI::military_caption_text("SCRIPT:Briefing"),
            "Localized briefing text"
        );

        Language::clear_localized_strings();
    }

    #[test]
    fn mouseover_tooltip_prefers_template_display_name() {
        Language::clear_localized_strings();

        assert_eq!(
            InGameUI::mouseover_tooltip_text("UnitA", "Explicit display name"),
            Some("Explicit display name".to_string())
        );

        Language::clear_localized_strings();
    }

    #[test]
    fn mouseover_tooltip_falls_back_to_thing_template_label() {
        Language::clear_localized_strings();

        assert_eq!(
            InGameUI::mouseover_tooltip_text("UnitA", ""),
            Some("ThingTemplate:UnitA".to_string())
        );

        Language::clear_localized_strings();
    }

    #[test]
    fn supply_warehouse_tooltip_feedback_formats_placeholder_value() {
        assert_eq!(
            InGameUI::format_supply_warehouse_tooltip_feedback(" ($%d)", 12, 100),
            " ($1200)"
        );
    }

    #[test]
    fn supply_warehouse_tooltip_feedback_appends_without_placeholder() {
        assert_eq!(
            InGameUI::format_supply_warehouse_tooltip_feedback(" supplies: ", 3, 200),
            " supplies: 600"
        );
    }

    #[test]
    fn mouseover_tooltip_suppresses_props() {
        Language::clear_localized_strings();
        Language::register_localized_string("OBJECT:Prop", "Prop");

        assert_eq!(InGameUI::mouseover_tooltip_text("Tree01", "Prop"), None);

        Language::clear_localized_strings();
    }

    #[test]
    fn mouseover_tooltip_only_shows_for_visible_shroud_states() {
        assert!(InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::Clear
        ));
        assert!(InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::PartialClear
        ));
        assert!(!InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::Fogged
        ));
        assert!(!InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::Shrouded
        ));
        assert!(!InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::InvalidButPreviousValid
        ));
        assert!(!InGameUI::mouseover_tooltip_visible_for_shroud(
            ObjectShroudStatus::Invalid
        ));
    }

    #[test]
    fn mouseover_cursor_updates_match_cpp_replay_gate() {
        assert!(InGameUI::mouseover_cursor_update_allowed(false, false));
        assert!(InGameUI::mouseover_cursor_update_allowed(false, true));
        assert!(InGameUI::mouseover_cursor_update_allowed(true, true));
        assert!(!InGameUI::mouseover_cursor_update_allowed(true, false));
    }

    #[test]
    fn command_hint_updates_match_cpp_replay_gate() {
        assert!(InGameUI::command_hint_update_allowed(false, false, false));
        assert!(!InGameUI::command_hint_update_allowed(true, false, false));
        assert!(!InGameUI::command_hint_update_allowed(false, true, false));
        assert!(!InGameUI::command_hint_update_allowed(false, false, true));
    }

    #[test]
    fn command_attack_hints_only_downgrade_for_fully_shrouded_targets() {
        assert_eq!(
            InGameUI::command_hint_after_shroud_projection(
                CommandHintType::AttackObject,
                Some(ObjectShroudStatus::Shrouded)
            ),
            CommandHintType::MoveTo
        );
        assert_eq!(
            InGameUI::command_hint_after_shroud_projection(
                CommandHintType::AttackObjectAfterMoving,
                Some(ObjectShroudStatus::Shrouded)
            ),
            CommandHintType::MoveTo
        );
        assert_eq!(
            InGameUI::command_hint_after_shroud_projection(
                CommandHintType::AttackObject,
                Some(ObjectShroudStatus::Fogged)
            ),
            CommandHintType::AttackObject
        );
        assert_eq!(
            InGameUI::command_hint_after_shroud_projection(
                CommandHintType::AttackObjectAfterMoving,
                Some(ObjectShroudStatus::Fogged)
            ),
            CommandHintType::AttackObjectAfterMoving
        );
        assert_eq!(
            InGameUI::command_hint_after_shroud_projection(
                CommandHintType::ForceAttackObject,
                Some(ObjectShroudStatus::Shrouded)
            ),
            CommandHintType::ForceAttackObject
        );
    }

    #[test]
    fn double_click_attack_move_guard_hint_uses_cpp_predecrement_semantics() {
        let mut timer = 2;
        assert!(InGameUI::consume_double_click_attack_move_guard_hint(
            &mut timer
        ));
        assert_eq!(timer, 1);

        assert!(!InGameUI::consume_double_click_attack_move_guard_hint(
            &mut timer
        ));
        assert_eq!(timer, 0);

        assert!(!InGameUI::consume_double_click_attack_move_guard_hint(
            &mut timer
        ));
        assert_eq!(timer, 0);
    }

    #[test]
    fn default_command_hints_are_blocked_by_nonlocal_selected_source() {
        assert!(InGameUI::default_command_hint_blocked_by_source(Some(
            false
        )));
        assert!(!InGameUI::default_command_hint_blocked_by_source(Some(
            true
        )));
        assert!(!InGameUI::default_command_hint_blocked_by_source(None));
    }

    #[test]
    fn move_to_cursor_matches_cpp_source_and_target_context() {
        assert_eq!(
            InGameUI::move_to_cursor_for_context(false, false, false, true),
            MouseCursor::GenericInvalid
        );
        assert_eq!(
            InGameUI::move_to_cursor_for_context(true, true, false, false),
            MouseCursor::Selecting
        );
        assert_eq!(
            InGameUI::move_to_cursor_for_context(true, true, true, false),
            MouseCursor::MoveTo
        );
        assert_eq!(
            InGameUI::move_to_cursor_for_context(false, false, false, false),
            MouseCursor::MoveTo
        );
    }

    #[test]
    fn language_font_override_extracts_explicit_language_font() {
        assert_eq!(
            InGameUI::language_font_override(&FontDesc::new("Localized Caption", 14, true)),
            Some(("Localized Caption".to_string(), 14, true))
        );
    }

    #[test]
    fn language_font_override_ignores_default_language_font_descriptor() {
        assert_eq!(InGameUI::language_font_override(&FontDesc::default()), None);
    }

    #[test]
    fn military_caption_update_respects_initial_delay_then_types() {
        let mut subtitle = Some(MilitarySubtitle {
            text: "AB".to_string(),
            index: 0,
            position: (10.0, 20.0),
            lifetime_frame: 120,
            block_drawn: true,
            block_begin_frame: 0,
            block_pos: (10.0, 20.0),
            increment_on_frame: 22,
            color: 0xFFC8_C81E,
        });

        InGameUI::update_military_subtitle_state(&mut subtitle, 22, 1, 12, 7.2, 22);
        let state = subtitle.as_ref().unwrap();
        assert_eq!(state.index, 0);
        assert_eq!(state.block_pos, (10.0, 20.0));

        InGameUI::update_military_subtitle_state(&mut subtitle, 23, 1, 12, 7.2, 22);
        let state = subtitle.as_ref().unwrap();
        assert_eq!(state.index, 1);
        assert_eq!(state.increment_on_frame, 24);
        assert_eq!(state.block_pos.0, 17.2);
    }

    #[test]
    fn military_caption_update_fades_after_lifetime_before_removal() {
        let mut subtitle = Some(MilitarySubtitle {
            text: "A".to_string(),
            index: 1,
            position: (0.0, 0.0),
            lifetime_frame: 10,
            block_drawn: true,
            block_begin_frame: 0,
            block_pos: (0.0, 0.0),
            increment_on_frame: 11,
            color: 0x02C8_C81E,
        });

        InGameUI::update_military_subtitle_state(&mut subtitle, 15, 1, 12, 7.2, 22);
        assert_eq!(subtitle.as_ref().unwrap().color >> 24, 2);

        InGameUI::update_military_subtitle_state(&mut subtitle, 40, 1, 12, 7.2, 22);
        assert!(subtitle.is_none());
    }
}
