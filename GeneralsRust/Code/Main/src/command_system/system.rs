use super::*;

/// Main command system that handles all RTS commands
pub struct CommandSystem {
    /// Current command mode (force attack, build mode, etc.)
    pub current_mode: CommandMode,

    /// Commands waiting to be processed
    pub(super) command_queue: VecDeque<GameCommand>,

    /// Current command ID counter
    pub(super) next_command_id: u32,

    /// Mouse drag tracking
    pub(super) mouse_drag_start: Option<Vec2>,
    pub(super) mouse_down_time: Option<Instant>,

    /// Command history for undo/replay
    pub(super) command_history: Vec<GameCommand>,

    /// Player-specific command settings
    pub(super) player_settings: HashMap<u32, PlayerCommandSettings>,
}

/// Per-player command settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerCommandSettings {
    pub auto_attack: bool,
    pub smart_select: bool,
    pub formation_move: bool,
    pub waypoint_mode: bool,
}

impl Default for PlayerCommandSettings {
    fn default() -> Self {
        Self {
            auto_attack: false,
            smart_select: true,
            formation_move: true,
            waypoint_mode: false,
        }
    }
}

impl Default for CommandSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Global command system instance
pub(super) static COMMAND_SYSTEM: OnceLock<Mutex<CommandSystem>> = OnceLock::new();

/// Initialize the global command system
pub fn init_command_system() {
    let _ = COMMAND_SYSTEM.get_or_init(|| {
        log::info!("Command system initialized");
        Mutex::new(CommandSystem::new())
    });
}

/// Get the global command system instance
pub fn get_command_system() -> &'static Mutex<CommandSystem> {
    COMMAND_SYSTEM.get_or_init(|| {
        log::info!("Command system initialized");
        Mutex::new(CommandSystem::new())
    })
}
