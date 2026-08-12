use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandResult {
    Success,
    InvalidTarget,
    OutOfRange,
    InsufficientResources,
    InvalidCommand,
    UnitBusy,
    TargetDestroyed,
    RequiresLineOfSight,
    InvalidLocation,
    CannotAttackTarget,
    CannotMoveToLocation,
    BuildingBlocked,
}

/// Command evaluation mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandEvaluateType {
    DoCommand,
    DoHint,
    EvaluateOnly,
}

/// A complete game command with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameCommand {
    pub command_type: CommandType,
    pub player_id: u32,
    pub command_id: u32,
    pub timestamp: SystemTime,
    pub selected_units: Vec<ObjectId>,
    pub modifier_keys: ModifierKeys,
}

/// Mouse/keyboard modifier keys
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ModifierKeys {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}
