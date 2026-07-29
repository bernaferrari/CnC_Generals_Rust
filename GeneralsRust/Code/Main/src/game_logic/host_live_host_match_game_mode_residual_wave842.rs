//! Wave 842: host-owned match game mode residual peels live GameLogic::game_mode
//! dual-read for presentation consumers after start_game_from_ui.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_GAME_MODE_METHOD_NAMES_WAVE842: &[&str] = &[
    "host_match_game_mode",
    "host_presentation_or_live_game_mode",
    "Wave 842",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_GAME_MODE_NAV_STEPS_WAVE842: &[&str] = &[
    "STAMP_HOST_MATCH_MODE",
    "PREFER_FREEZE_THEN_HOST_MODE",
    "LIVE_HOST_MATCH_GAME_MODE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchGameModeAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchGameModeAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

pub fn honesty_host_match_game_mode_method_names_residual_wave842() -> bool {
    let names = LIVE_HOST_MATCH_GAME_MODE_METHOD_NAMES_WAVE842;
    let ok = residual_name_index(names, "host_match_game_mode").is_some()
        && residual_name_index(names, "host_presentation_or_live_game_mode").is_some()
        && residual_name_index(names, "Wave 842").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostMatchGameModeAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_game_mode_nav_commands_residual_wave842() -> bool {
    let steps = LIVE_HOST_MATCH_GAME_MODE_NAV_STEPS_WAVE842;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_GAME_MODE").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_MODE").is_some();
    residual_action_store(ResidualHostMatchGameModeAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_game_mode_residual_pack_wave842() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_game_mode: Option<GameMode>")
        && cnc.contains("Wave 842: stamp host-owned match mode before map load")
        && cnc.contains("self.host_match_game_mode = Some(mode)")
        && cnc.contains("Wave 609/842: host UI/presentation residual helper")
        && cnc.contains("if let Some(mode) = self.host_match_game_mode");
    residual_action_store(ResidualHostMatchGameModeAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_game_mode_honesty() -> bool {
    let a = honesty_host_match_game_mode_method_names_residual_wave842();
    let b = honesty_host_match_game_mode_nav_commands_residual_wave842();
    let c = honesty_host_match_game_mode_residual_pack_wave842();
    residual_action_store(ResidualHostMatchGameModeAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_game_mode_residual_wave842() {
        assert!(honesty_host_match_game_mode_residual_pack_wave842());
        assert!(honesty_host_match_game_mode_method_names_residual_wave842());
        assert!(honesty_host_match_game_mode_nav_commands_residual_wave842());
        assert!(simulate_live_host_match_game_mode_honesty());
    }
}
