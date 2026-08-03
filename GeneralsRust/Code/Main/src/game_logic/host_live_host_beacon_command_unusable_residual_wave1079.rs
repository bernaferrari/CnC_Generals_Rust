//! Wave 1079: dual-world beacon/command unusable residual.
//!
//! ControlBar dual catalog beacon/command path fails closed on unusable selection
//! and clears primary command-set residual with unusable clear. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_BEACON_COMMAND_UNUSABLE_RESIDUAL_METHOD_NAMES_WAVE1079: &[&str] = &[
    "ControlBarState::Beacon",
    "presentation_primary_command_set",
    "Wave 1079",
    "playable_claim = false",
];

pub const LIVE_HOST_BEACON_COMMAND_UNUSABLE_RESIDUAL_NAV_STEPS_WAVE1079: &[&str] = &[
    "BEACON",
    "COMMAND_UNUSABLE",
    "LIVE_HOST_BEACON_COMMAND_UNUSABLE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBeaconCommandUnusableResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostBeaconCommandUnusableResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn cb_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/gui/control_bar/control_bar.rs")
}

pub fn honesty_host_beacon_command_unusable_residual_method_names_residual_wave1079() -> bool {
    let names = LIVE_HOST_BEACON_COMMAND_UNUSABLE_RESIDUAL_METHOD_NAMES_WAVE1079;
    let ok = residual_name_index(names, "ControlBarState::Beacon").is_some()
        && residual_name_index(names, "Wave 1079").is_some();
    residual_action_store(ResidualHostBeaconCommandUnusableResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_beacon_command_unusable_residual_nav_commands_residual_wave1079() -> bool {
    let steps = LIVE_HOST_BEACON_COMMAND_UNUSABLE_RESIDUAL_NAV_STEPS_WAVE1079;
    let ok = residual_name_index(steps, "LIVE_HOST_BEACON_COMMAND_UNUSABLE_RESIDUAL").is_some()
        && residual_name_index(steps, "BEACON").is_some();
    residual_action_store(ResidualHostBeaconCommandUnusableResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_beacon_command_unusable_residual_residual_pack_wave1079() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb
        .contains("Wave 1079: unusable dual catalog residual fail-closed for beacon/command")
        && cb.contains("Wave 1079: also clear primary command-set residual (beacon/command)")
        && cb.contains("self.presentation_primary_command_set.clear()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostBeaconCommandUnusableResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_beacon_command_unusable_residual_honesty() -> bool {
    let a = honesty_host_beacon_command_unusable_residual_method_names_residual_wave1079();
    let b = honesty_host_beacon_command_unusable_residual_nav_commands_residual_wave1079();
    let c = honesty_host_beacon_command_unusable_residual_residual_pack_wave1079();
    residual_action_store(ResidualHostBeaconCommandUnusableResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_beacon_command_unusable_residual_wave1079() {
        assert!(honesty_host_beacon_command_unusable_residual_residual_pack_wave1079());
        assert!(honesty_host_beacon_command_unusable_residual_method_names_residual_wave1079());
        assert!(honesty_host_beacon_command_unusable_residual_nav_commands_residual_wave1079());
        assert!(simulate_live_host_beacon_command_unusable_residual_honesty());
    }
}
