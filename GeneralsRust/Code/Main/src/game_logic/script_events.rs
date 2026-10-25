use super::victory_conditions::AllianceState;

/// C++ ScriptActions NAMED_*_SPECIAL_POWER_COUNTDOWN residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedSpecialPowerCountdownOp {
    /// NAMED_STOP_SPECIAL_POWER_COUNTDOWN → pauseCountdown(true)
    Stop,
    /// NAMED_START_SPECIAL_POWER_COUNTDOWN → pauseCountdown(false)
    Start,
    /// NAMED_SET_SPECIAL_POWER_COUNTDOWN → setReadyFrame(now + seconds)
    Set,
    /// NAMED_ADD_SPECIAL_POWER_COUNTDOWN → setReadyFrame(ready + seconds)
    Add,
}

/// Events emitted by gameplay systems that scripts/radar/UI can consume.
#[derive(Debug, Clone)]
pub enum ScriptEvent {
    PlayerDefeated {
        player_id: u32,
    },
    AllianceStateChanged {
        player_id: u32,
        state: AllianceState,
    },
    RevealMapForPlayer {
        player_id: u32,
    },
}
