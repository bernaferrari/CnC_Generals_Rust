/// Wave 344: skip only when BOTH the dual-world registry and GameLogic are empty.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    if !crate::object::registry::OBJECT_REGISTRY.is_empty() {
        return false;
    }
    match crate::system::game_logic::get_game_logic().try_lock() {
        Ok(logic) => logic.all_objects.is_empty() && logic.objects.is_empty(),
        // Lock held = live GameLogic call; C++ has no registry skip.
        Err(_) => false,
    }
}

const DEFAULT_WORLD_WIDTH: Real = 64.0;
const DEFAULT_WORLD_HEIGHT: Real = 64.0;

fn should_freeze_time(
    tactical_frozen: bool,
    camera_movement_finished: bool,
    script_frozen: bool,
) -> bool {
    (tactical_frozen && !camera_movement_finished) || script_frozen
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildableStatus {
    Available,
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrcMode {
    Disabled,
    Cached,
    Recalc,
}

/// C++ `XferCRC` used by `GameLogic::getCRC`. Implements both Xfer stacks so

/// C++ `GameLogic::sendObjectCreated` — create a drawable and bind both worlds.
/// Id-keyed: the object is already registered by every caller, so the template
/// is read through a scoped registry borrow and no object Arc handle is
/// retained across the drawable handoff.
pub fn send_object_created(object_id: ObjectID) {
    let Some(client) = TheGameClient::get() else {
        return;
    };
    // Scoped registry borrow: objects that already own a drawable yield None.
    // `flatten` collapses the lookup's Option with the drawable guard's Option,
    // so only the cloned template (no object Arc handle) escapes.
    let template = OBJECT_REGISTRY
        .with_object(object_id, |guard| {
            if guard.get_drawable().is_some() {
                None
            } else {
                Some(guard.get_template().clone())
            }
        })
        .flatten();
    let Some(template) = template else {
        return;
    };
    let draw_id = client.create_drawable(template.as_ref());
    bind_object_and_drawable(object_id, draw_id);
}

/// C++ `GameLogic::sendObjectCreated(this)` from inside Object::initObject.
/// The factory holds the Object write lock throughout init, so this variant
/// uses the already-borrowed identity and installs the binding directly.
pub(crate) fn send_object_created_borrowed(object: &mut Object, object_arc: &Arc<RwLock<Object>>) {
    if object.get_drawable().is_some() {
        return;
    }
    let Some(client) = TheGameClient::get() else {
        return;
    };
    let object_id = object.get_id();
    let draw_id = client.create_drawable(object.get_template().as_ref());
    let Some(drawable) = client.get_drawable_arc(draw_id) else {
        return;
    };
    if let Ok(mut draw) = drawable.write() {
        draw.friend_bind_to_object_with_id(object_id, object_arc);
    }
    object.set_drawable(Some(drawable));
}

/// C++ `GameLogic::bindObjectAndDrawable`.
pub fn bind_object_and_drawable(object_id: ObjectID, drawable_id: ObjectID) {
    let Some(client) = TheGameClient::get() else {
        return;
    };
    let Some(drawable) = client.get_drawable_arc(drawable_id) else {
        return;
    };
    let Some(object) = OBJECT_REGISTRY.get_object(object_id).or_else(|| {
        get_game_logic()
            .try_lock()
            .ok()
            .and_then(|logic| logic.find_object_by_id(object_id))
    }) else {
        return;
    };
    if let Ok(mut draw) = drawable.write() {
        draw.friend_bind_to_object(&object);
    }
    {
        let mut obj = object.write().ok();
        if let Some(obj) = obj.as_mut() {
            obj.set_drawable(Some(drawable));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Skirmish,
}

pub trait SubsystemInterface: Send + Sync {
    fn update(&mut self, _delta_time: f32) {}
}

pub const MAX_SLOTS: usize = 32;

#[derive(Debug, Default)]
pub struct PartitionManagerFactory;

#[derive(Debug, Default)]
pub struct TheObjectFactory;

impl TheObjectFactory {
    pub fn find_template(name: &str) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        crate::helpers::TheThingFactory::find_template(name)
    }

    pub fn new_object(
        template: Arc<dyn crate::common::ThingTemplate>,
        team: Option<Arc<RwLock<Team>>>,
    ) -> Result<Arc<RwLock<Object>>, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_object_with_status(template, team, crate::common::ObjectStatusMaskType::NONE)
    }

    /// C++ `ThingFactory::newObject` / `friend_createObject` — statusBits then onCreate.
    pub fn new_object_with_status(
        template: Arc<dyn crate::common::ThingTemplate>,
        team: Option<Arc<RwLock<Team>>>,
        status_bits: crate::common::ObjectStatusMaskType,
    ) -> Result<Arc<RwLock<Object>>, Box<dyn std::error::Error + Send + Sync>> {
        let object_id = {
            let mutex = get_game_logic();
            let mut logic = mutex
                .lock()
                .map_err(|_| "GameLogic mutex poisoned when allocating object id")?;
            logic.allocate_object_id()
        };

        let mut status_mask = template.get_initial_object_status();
        status_mask |= status_bits;
        let object = Object::new_with_id(template.clone(), object_id, status_mask, team)?;

        {
            let mutex = get_game_logic();
            let mut logic = mutex
                .lock()
                .map_err(|_| "GameLogic mutex poisoned when registering object")?;
            logic
                .register_object(object.clone())
                .map_err(|err| format!("Failed to register object: {:?}", err))?;
        }

        {
            let mut obj_guard = object
                .write()
                .map_err(|e| format!("object write lock poisoned: {}", e))?;
            obj_guard.init_object()?;
        }

        Ok(object)
    }
}

/// Fixed simulation frame rate (30 FPS for C&C Generals)
pub const DEFAULT_TICK_FPS: u32 = 30;
/// Fixed time step per frame in seconds
pub const FIXED_DELTA_TIME: f32 = 1.0 / 30.0;

/// C++ parity hook for `setFPMode()` from `FPUControl.h`.
///
/// The original game resets x87 control flags because DirectX could leave FP state dirty.
/// In Rust on modern targets we run with stable IEEE-754 defaults, so this is intentionally
/// a no-op placeholder and explicit call site for parity bookkeeping.
pub fn set_fp_mode() {}

// C++ GameLogic::update has no per-frame sleepy cap. MAX_SUO=256 is destroy
// bookkeeping only. Do not throttle due modules here.

/// Game mode constants (matching C++ enum values)
pub const GAME_SINGLE_PLAYER: Int = 0;
pub const GAME_LAN: Int = 1;
pub const GAME_SKIRMISH: Int = 2;
pub const GAME_REPLAY: Int = 3;
pub const GAME_SHELL: Int = 4;
pub const GAME_INTERNET: Int = 5;
pub const GAME_NONE: Int = 6;

/// Error types for GameLogic operations
#[derive(Debug, Clone)]
pub enum GameLogicError {
    /// Object with specified ID was not found
    ObjectNotFound(ObjectID),
    /// Physics system error
    PhysicsError(String),
    /// Scripting system error
    ScriptError(String),
    /// AI system error
    AIError(String),
    /// Invalid state transition or operation
    InvalidState(String),
    /// Command processing error
    CommandError(String),
    /// Vision/shroud update error
    VisionError(String),
    /// Generic error with message
    Generic(String),
}

impl std::fmt::Display for GameLogicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameLogicError::ObjectNotFound(id) => write!(f, "Object not found: {}", id),
            GameLogicError::PhysicsError(msg) => write!(f, "Physics error: {}", msg),
            GameLogicError::ScriptError(msg) => write!(f, "Script error: {}", msg),
            GameLogicError::AIError(msg) => write!(f, "AI error: {}", msg),
            GameLogicError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            GameLogicError::CommandError(msg) => write!(f, "Command error: {}", msg),
            GameLogicError::VisionError(msg) => write!(f, "Vision error: {}", msg),
            GameLogicError::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for GameLogicError {}

/// Player configuration for game setup
#[derive(Debug, Clone)]
pub struct PlayerConfig {
    pub name: String,
    pub faction: String,
    pub color: Color,
    pub is_human: bool,
    pub team_id: Int,
}

/// Main GameLogic singleton - orchestrates all game systems
///
/// ## C++ Reference: GameLogic class (GameLogic.h lines 104-390)
///
/// This structure maintains the entire game state and coordinates updates
/// across all subsystems. It mirrors the C++ GameLogic singleton.
pub struct GameLogic {
    // World dimensions
    width: Real,
    height: Real,

    // Frame tracking
    frame: UnsignedInt,
    game_time: f32,
    is_in_update: Bool,

    // Random seed for deterministic replay/sync
    random_seed: u64,

    // CRC for lockstep synchronization (C++ GameLogic.h:272)
    crc_cache: UnsignedInt,
    crc_interval: UnsignedInt,

    // Object management
    next_object_id: ObjectID,
    all_objects: Vec<ObjectID>,
    dead_objects: Vec<ObjectID>,
    objects: HashMap<ObjectID, Arc<RwLock<Object>>>,

    // Player/Team management (references only)
    // Actual player list is managed by player_list() singleton

    // Subsystems
    partition_manager: PartitionManager,
    physics_world: PhysicsWorld,

    // Event/Command queues
    event_queue: Vec<GameEvent>,
    command_queue: VecDeque<GameCommand>,
    /// Active formations for this world. One manager per GameLogic so
    /// concurrent games do not share formation membership or ids.
    formation_manager: crate::commands::FormationManager,
    /// Command execution for this world. Separate from the shared command queue.
    command_processor: crate::commands::CommandProcessor,
    radar_updates: Vec<RadarUpdate>,
    objects_changed_trigger_areas: VecDeque<ObjectID>,
    frame_objects_changed_trigger_areas: UnsignedInt,

    // Game state
    game_mode: Int,
    game_paused: Bool,
    loading_map: Bool,
    loading_save: Bool,
    is_scoring_enabled: Bool,
    show_behind_building_markers: Bool,
    draw_icon_ui: Bool,
    show_dynamic_lod: Bool,
    rank_level_limit: Int,
    buildable_status_overrides: HashMap<String, Int>,
    superweapon_restriction: UnsignedShort,

    // Update module tracking (sleepy vs normal updates)
    sleepy_updates: SleepyUpdateQueue,
    normal_updates: Vec<NormalUpdateEntry>,
    // Registration order only; heap/normal entries own each shared module once.
    module_lookup: HashMap<ObjectID, Vec<ModuleIdentity>>,
    global_weapon_bonus_set: WeaponBonusSet,

    // Control bar button overrides (C++ GameLogic.h line 266: ControlBarOverrideMap)
    control_bar_overrides: HashMap<String, Option<String>>,

    // C++ parity: m_objectTOC — compact thing-template name→id map for save/load
    object_toc: Vec<ObjectTOCEntry>,

    /// Honesty: last `update()` returned without running a C++-parity tick
    /// (empty dual-world registry AND empty `objects`). Host may still treat
    /// `Ok(())` as "frame accepted"; this flag says it was an empty no-op.
    pub last_update_was_empty_noop: bool,
    /// Count of empty-world no-op ticks. Not a C++ `m_frame` increment.
    pub empty_world_tick: UnsignedInt,
}

impl GameLogic {
    /// True when the last `update()` skipped C++ phase order (empty world).
    #[inline]
    pub fn last_update_was_empty_noop(&self) -> bool {
        self.last_update_was_empty_noop
    }

    /// How many empty-world no-op ticks have been accepted. Not `m_frame`.
    #[inline]
    pub fn empty_world_tick_count(&self) -> UnsignedInt {
        self.empty_world_tick
    }
}

#[derive(Debug, Clone)]
pub struct ObjectTOCEntry {
    pub name: String,
    pub id: UnsignedShort,
}

/// Entry for sleepy update queue (priority queue by wake frame)
#[derive(Clone)]
pub struct SleepyUpdateEntry {
    wake_frame: UnsignedInt,
    phase: SleepyUpdatePhase,
    object_id: ObjectID,
    module: UpdateModulePtr,
}

/// Entry for normal (every-frame) update queue
#[derive(Clone)]
struct NormalUpdateEntry {
    object_id: ObjectID,
    module: UpdateModulePtr,
}

impl Default for GameLogic {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            frame: 0,
            game_time: 0.0,
            is_in_update: false,
            random_seed: 0,
            crc_cache: 0,
            crc_interval: game_engine::common::crc_debug::replay_crc_interval() as UnsignedInt,
            next_object_id: 1,
            all_objects: Vec::new(),
            dead_objects: Vec::new(),
            objects: HashMap::new(),
            partition_manager: PartitionManager::new(),
            physics_world: PhysicsWorld::new(),
            event_queue: Vec::new(),
            command_queue: VecDeque::new(),
            formation_manager: crate::commands::FormationManager::new(),
            command_processor: crate::commands::CommandProcessor::new(),
            radar_updates: Vec::new(),
            objects_changed_trigger_areas: VecDeque::new(),
            frame_objects_changed_trigger_areas: 0,

            game_mode: GAME_NONE,
            game_paused: false,
            loading_map: false,
            loading_save: false,
            is_scoring_enabled: true,
            show_behind_building_markers: true,
            draw_icon_ui: true,
            show_dynamic_lod: true,
            rank_level_limit: 1000,
            buildable_status_overrides: HashMap::new(),
            superweapon_restriction: 0,
            sleepy_updates: SleepyUpdateQueue::default(),
            normal_updates: Vec::new(),
            module_lookup: HashMap::new(),
            global_weapon_bonus_set: WeaponBonusSet::new(),
            control_bar_overrides: HashMap::new(),
            object_toc: Vec::new(),
            last_update_was_empty_noop: false,
            empty_world_tick: 0,
        }
    }
}
