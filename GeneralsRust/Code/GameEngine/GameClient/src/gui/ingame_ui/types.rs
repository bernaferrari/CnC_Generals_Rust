// Types, constants, and InGameUI struct.
// Split from `gui/ingame_ui.rs` dump. Included by `ingame_ui/mod.rs`.

/// Wave 273: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

const CMD_CONTEXTMODE_COMMAND: u32 = 0x0000_0200;

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
#[derive(Default)]
pub enum MouseCursor {
    /// C++: INVALID_MOUSE_CURSOR = -1
    Invalid = -1,
    /// C++: NONE = 0
    None = 0,
    /// C++: FIRST_CURSOR = 1, NORMAL = FIRST_CURSOR = 1
    FirstCursor = 1,
    /// C++: ARROW = 2
    #[default]
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
/// C++ W3DInGameUI::drawMoveHints draws while `elapsed <= 40`.
const MOVE_HINT_LIFETIME_FRAMES: u32 = 41;

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
#[derive(Default)]
pub enum RadiusCursorType {
    #[default]
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
/// Wave 964: presentation-owned selection residual for host path (no OBJECT_REGISTRY).
#[derive(Debug, Clone)]
pub struct PresentationSelectedUnitResidual {
    pub object_id: u32,
    pub template_name: String,
    pub position: [f32; 3],
    pub health_pct: f32,
    /// Debug/Display names of KindOf flags from presentation freeze.
    pub kind_names: Vec<String>,
    /// Wave 1040: legality residual for dual selection HUD (C++ status bits).
    pub destroyed: bool,
    pub sold: bool,
    pub unselectable: bool,
    pub masked: bool,
    pub effectively_stealthed: bool,
    pub team_name: String,
}

/// Wave 966: presentation unit catalog residual for host select-similar / matching.
#[derive(Debug, Clone)]
pub struct PresentationUnitCatalogEntry {
    pub object_id: u32,
    pub template_name: String,
    /// Team debug name (USA/China/GLA/Neutral) from presentation freeze.
    pub team_name: String,
    pub selectable: bool,
    pub position: [f32; 3],
    /// Wave 1024: yaw residual for dual-world drawable pose peel.
    pub orientation: f32,
    /// Wave 1026: disabled residual for dual-world command availability.
    pub disabled: bool,
    /// Wave 1028: under-construction residual for dual-world ControlBar state.
    pub under_construction: bool,
    /// Wave 1028: construction percent residual [0,1].
    pub construction_percent: f32,
    /// Wave 1030: garrison capacity residual for dual-world structure inventory.
    pub max_garrison: u16,
    /// Wave 1030: occupant count residual for dual-world structure inventory.
    pub occupant_count: u16,
    /// Wave 1031: OCL timer residual seconds for dual-world ControlBar OclTimer context.
    pub ocl_timer_seconds: u32,
    /// Wave 1033: sold residual for dual-world ControlBar clear (C++ OBJECT_STATUS_SOLD).
    pub sold: bool,
    /// Wave 1034: unselectable residual for dual-world selection (C++ OBJECT_STATUS_UNSELECTABLE).
    pub unselectable: bool,
    /// Wave 1035: destroyed residual for dual-world selection skip.
    pub destroyed: bool,
    /// Wave 1035: masked residual for dual-world selection (C++ OBJECT_STATUS_MASKED).
    pub masked: bool,
    /// Wave 1036: effectively stealthed residual (stealthed && !detected) for dual selection.
    pub effectively_stealthed: bool,
    /// Wave 1041: disguised residual (bomb truck etc.) for dual portrait/template.
    pub disguised: bool,
    /// Wave 1041: apparent template while disguised (non-allied viewers).
    pub disguise_as_template: Option<String>,
    /// Wave 1041: apparent team while disguised (non-allied viewers).
    pub disguise_as_team: Option<String>,
    /// Wave 968: KindOf Debug names from presentation freeze.
    pub kind_names: Vec<String>,
    /// Wave 971: special power ready residual for host SP targeting.
    pub special_power_ready: bool,
    /// Wave 979: airborne residual for host plane-camera lock.
    pub airborne_target: bool,
    /// Wave 981: FOW residual for host command-hint shroud projection.
    /// Maps presentation ObjectVisibility → ObjectShroudStatus discriminant.
    pub shroud_status: ObjectShroudStatus,
    /// Wave 982: producer/slaver residual for IgnoredInGui mouseover remap.
    pub slaver_object_id: Option<u32>,
    /// Wave 1011: health residual for dual-world portrait peel.
    pub health_current: f32,
    /// Wave 1011: max health residual for dual-world portrait peel.
    pub health_maximum: f32,
    /// Wave 1012: veterancy chevron residual for dual-world portrait.
    pub veterancy_overlay: Option<String>,
    /// Wave 1013: head production progress residual (0..1).
    pub production_progress: Option<f32>,
    /// Wave 1013: head production template residual.
    pub production_template: Option<String>,
    /// Wave 1013: production paused residual.
    pub production_paused: bool,
    /// Wave 1015: effective command-set name residual for dual-world ControlBar.
    pub command_set_name: String,
    /// Wave 1055: host control-group residual (0..9, -1 = none) for dual group numerals.
    pub hotkey_group: i8,
}

pub struct InGameUI {
    /// Selection box state
    selection_box: SelectionBox,

    /// Selection state
    selection_state: SelectionState,

    /// Wave 964: presentation selection residual (host empty dual-world path).
    presentation_selected: Vec<PresentationSelectedUnitResidual>,

    /// Wave 966: full presentation unit catalog residual (host select-similar).
    presentation_unit_catalog: Vec<PresentationUnitCatalogEntry>,

    /// Wave 968: local player team residual for host ownership queries.
    presentation_local_team_name: String,

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

impl Default for SelectionBox {
    fn default() -> Self {
        Self::new()
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

