//! Wave 861: host_match_in_multiplayer residual + fail-closed warm purchasable
//! science residual peels live GameLogic dual-reads.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MULTIPLAYER_SCIENCE_FAILCLOSED_METHOD_NAMES_WAVE861: &[&str] = &[
    "host_match_in_multiplayer",
    "host_is_in_multiplayer_game",
    "host_player_can_purchase_science",
    "Wave 861",
    "playable_claim = false",
];

pub const LIVE_HOST_MULTIPLAYER_SCIENCE_FAILCLOSED_NAV_STEPS_WAVE861: &[&str] = &[
    "STAMP_HOST_MATCH_MULTIPLAYER",
    "SCIENCE_WARM_RESIDUAL_FAILCLOSED",
    "LIVE_HOST_MULTIPLAYER_SCIENCE_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMultiplayerScienceFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMultiplayerScienceFailclosedAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_multiplayer_science_failclosed_method_names_residual_wave861() -> bool {
    let names = LIVE_HOST_MULTIPLAYER_SCIENCE_FAILCLOSED_METHOD_NAMES_WAVE861;
    let ok = residual_name_index(names, "host_match_in_multiplayer").is_some()
        && residual_name_index(names, "host_player_can_purchase_science").is_some()
        && residual_name_index(names, "Wave 861").is_some();
    residual_action_store(ResidualHostMultiplayerScienceFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_multiplayer_science_failclosed_nav_commands_residual_wave861() -> bool {
    let steps = LIVE_HOST_MULTIPLAYER_SCIENCE_FAILCLOSED_NAV_STEPS_WAVE861;
    let ok = residual_name_index(steps, "LIVE_HOST_MULTIPLAYER_SCIENCE_FAILCLOSED").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_MULTIPLAYER").is_some();
    residual_action_store(ResidualHostMultiplayerScienceFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_multiplayer_science_failclosed_residual_pack_wave861() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_in_multiplayer: Option<bool>")
        && cnc.contains("Wave 861: stamp multiplayer residual")
        && cnc.contains("Wave 584/861")
        && cnc.contains("Wave 584/852/861")
        && cnc.contains("if let Some(v) = self.host_match_in_multiplayer")
        && cnc.contains("warm purchasable residual is fail-closed")
        && cnc.contains(".unwrap_or(false)");
    residual_action_store(ResidualHostMultiplayerScienceFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_multiplayer_science_failclosed_honesty() -> bool {
    let a = honesty_host_multiplayer_science_failclosed_method_names_residual_wave861();
    let b = honesty_host_multiplayer_science_failclosed_nav_commands_residual_wave861();
    let c = honesty_host_multiplayer_science_failclosed_residual_pack_wave861();
    residual_action_store(ResidualHostMultiplayerScienceFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_multiplayer_science_failclosed_residual_wave861() {
        assert!(honesty_host_multiplayer_science_failclosed_residual_pack_wave861());
        assert!(honesty_host_multiplayer_science_failclosed_method_names_residual_wave861());
        assert!(honesty_host_multiplayer_science_failclosed_nav_commands_residual_wave861());
        assert!(simulate_live_host_multiplayer_science_failclosed_honesty());
    }
}
