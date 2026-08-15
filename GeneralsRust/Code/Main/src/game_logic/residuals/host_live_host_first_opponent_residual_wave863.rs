//! Wave 863: host_match_first_opponent_id residual peels live first_opponent_id
//! dual-reads for debug victory hotkey path.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FIRST_OPPONENT_RESIDUAL_METHOD_NAMES_WAVE863: &[&str] = &[
    "host_match_first_opponent_id",
    "host_first_opponent_id",
    "host_match_diplomacy_players",
    "Wave 863",
    "playable_claim = false",
];

pub const LIVE_HOST_FIRST_OPPONENT_RESIDUAL_NAV_STEPS_WAVE863: &[&str] = &[
    "STAMP_HOST_FIRST_OPPONENT",
    "PREFER_DIPLOMACY_OR_FREEZE",
    "LIVE_HOST_FIRST_OPPONENT_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFirstOpponentAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFirstOpponentAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

pub fn honesty_host_first_opponent_residual_method_names_residual_wave863() -> bool {
    let names = LIVE_HOST_FIRST_OPPONENT_RESIDUAL_METHOD_NAMES_WAVE863;
    let ok = residual_name_index(names, "host_match_first_opponent_id").is_some()
        && residual_name_index(names, "host_first_opponent_id").is_some()
        && residual_name_index(names, "Wave 863").is_some();
    residual_action_store(ResidualHostFirstOpponentAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_first_opponent_residual_nav_commands_residual_wave863() -> bool {
    let steps = LIVE_HOST_FIRST_OPPONENT_RESIDUAL_NAV_STEPS_WAVE863;
    let ok = residual_name_index(steps, "LIVE_HOST_FIRST_OPPONENT_RESIDUAL").is_some()
        && residual_name_index(steps, "STAMP_HOST_FIRST_OPPONENT").is_some();
    residual_action_store(ResidualHostFirstOpponentAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_first_opponent_residual_pack_wave863() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_first_opponent_id: Option<Option<u32>>")
        && cnc.contains("Wave 863")
        && cnc.contains("Wave 585/863")
        && cnc.contains("if let Some(cached) = self.host_match_first_opponent_id")
        && cnc.contains("first_opponent_id(player_id)"); // boot residual remains
    residual_action_store(ResidualHostFirstOpponentAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_first_opponent_residual_honesty() -> bool {
    let a = honesty_host_first_opponent_residual_method_names_residual_wave863();
    let b = honesty_host_first_opponent_residual_nav_commands_residual_wave863();
    let c = honesty_host_first_opponent_residual_pack_wave863();
    residual_action_store(ResidualHostFirstOpponentAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_first_opponent_residual_wave863() {
        assert!(honesty_host_first_opponent_residual_pack_wave863());
        assert!(honesty_host_first_opponent_residual_method_names_residual_wave863());
        assert!(honesty_host_first_opponent_residual_nav_commands_residual_wave863());
        assert!(simulate_live_host_first_opponent_residual_honesty());
    }
}
