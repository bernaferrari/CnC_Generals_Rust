//! Errors shared by the simulation and the hosts driving it.

/// Result of an operation on the simulation.
pub type GameLogicResult<T> = Result<T, GameLogicError>;

/// Main error type for game logic operations
#[derive(Debug, thiserror::Error)]
pub enum GameLogicError {
    /// AI system errors
    #[error("AI error: {0}")]
    Ai(#[from] AiError),
    /// Weapon system errors
    #[error("Weapon error: {0}")]
    Weapon(#[from] WeaponError),

    /// Invalid object reference
    #[error("Invalid object ID: {0}")]
    InvalidObject(u32),

    /// System not initialized
    #[error("System not initialized: {0}")]
    SystemNotInitialized(String),

    /// Exact synchronous Team creation failed; retry reports the retained cause.
    #[error("{0}")]
    TeamCreationFailed(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Threading error
    #[error("Threading error: {0}")]
    Threading(String),

    /// Module failure
    #[error("Module error: {0}")]
    ModuleError(String),

    /// I/O operation failure
    #[error("I/O error: {0}")]
    IO(String),
}

impl From<generals_map::DecodeError> for GameLogicError {
    fn from(error: generals_map::DecodeError) -> Self {
        Self::Configuration(error.to_string())
    }
}

/// Failure while resolving or dispatching a simulation AI command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    InvalidObject,
    EmptyGroup,
    InvalidCommand,
    PathfindingFailed,
    NoPathfinder,
    InvalidTarget,
    LockFailed,
    NotInitialized,
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::InvalidObject => write!(f, "Invalid object"),
            AiError::EmptyGroup => write!(f, "Empty group"),
            AiError::InvalidCommand => write!(f, "Invalid command"),
            AiError::PathfindingFailed => write!(f, "Pathfinding failed"),
            AiError::NoPathfinder => write!(f, "No pathfinder available"),
            AiError::InvalidTarget => write!(f, "Invalid target"),
            AiError::LockFailed => write!(f, "Lock failed"),
            AiError::NotInitialized => write!(f, "AI integration not initialized"),
        }
    }
}

impl std::error::Error for AiError {}

impl From<String> for AiError {
    fn from(_: String) -> Self {
        AiError::PathfindingFailed
    }
}

/// Weapon firing errors
#[derive(Debug, Clone, PartialEq)]
pub enum WeaponError {
    NoAmmo,
    NotReady { time_remaining: f32 },
    OutOfRange { distance: f32, max_range: f32 },
    TargetObstructed,
    TargetNotVisible,
    InvalidTarget,
    NoTemplate,
    SystemError(String),
}

impl std::fmt::Display for WeaponError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeaponError::NoAmmo => write!(f, "Weapon has no ammunition"),
            WeaponError::NotReady { time_remaining } => {
                write!(
                    f,
                    "Weapon not ready to fire ({:.2}s remaining)",
                    time_remaining
                )
            }
            WeaponError::OutOfRange {
                distance,
                max_range,
            } => {
                write!(
                    f,
                    "Target out of range ({:.1} > {:.1})",
                    distance, max_range
                )
            }
            WeaponError::TargetObstructed => write!(f, "Line of sight to target obstructed"),
            WeaponError::TargetNotVisible => write!(f, "Target is outside vision range"),
            WeaponError::InvalidTarget => write!(f, "Invalid or dead target"),
            WeaponError::NoTemplate => write!(f, "Weapon template not available"),
            WeaponError::SystemError(msg) => write!(f, "System error: {}", msg),
        }
    }
}

impl std::error::Error for WeaponError {}
