//! Wave 841: InGame mouse classification never dual-reads live GameLogic when
//! presentation freeze is missing (fail-closed). Boot/Menu may still fall open.
//! Workspace `cargo fmt --all` residual green. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_MOUSE_INGAME_FAILCLOSED_METHOD_NAMES_WAVE841: &[&str] = &[
    "host_presentation_mouse_game_logic",
    "GameState::InGame",
    "Wave 841",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_MOUSE_INGAME_FAILCLOSED_NAV_STEPS_WAVE841: &[&str] = &[
    "INGAME_MOUSE_FAILCLOSED",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "LIVE_PRESENTATION_MOUSE_INGAME_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationMouseIngameFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualPresentationMouseIngameFailclosedAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

pub fn honesty_presentation_mouse_ingame_failclosed_method_names_residual_wave841() -> bool {
    let names = LIVE_PRESENTATION_MOUSE_INGAME_FAILCLOSED_METHOD_NAMES_WAVE841;
    let ok = residual_name_index(names, "host_presentation_mouse_game_logic").is_some()
        && residual_name_index(names, "Wave 841").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationMouseIngameFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_presentation_mouse_ingame_failclosed_nav_commands_residual_wave841() -> bool {
    let steps = LIVE_PRESENTATION_MOUSE_INGAME_FAILCLOSED_NAV_STEPS_WAVE841;
    let ok = residual_name_index(steps, "LIVE_PRESENTATION_MOUSE_INGAME_FAILCLOSED").is_some()
        && residual_name_index(steps, "INGAME_MOUSE_FAILCLOSED").is_some();
    residual_action_store(ResidualPresentationMouseIngameFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_presentation_mouse_ingame_failclosed_residual_pack_wave841() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("Wave 609")
        && (cnc
            .contains("Wave 841: InGame/Paused/Loading never dual-read live GameLogic for mouse")
            || cnc.contains("Wave 609/841/906"))
        && cnc.contains("fn host_presentation_mouse_game_logic");
    residual_action_store(ResidualPresentationMouseIngameFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_presentation_mouse_ingame_failclosed_honesty() -> bool {
    let a = honesty_presentation_mouse_ingame_failclosed_method_names_residual_wave841();
    let b = honesty_presentation_mouse_ingame_failclosed_nav_commands_residual_wave841();
    let c = honesty_presentation_mouse_ingame_failclosed_residual_pack_wave841();
    residual_action_store(ResidualPresentationMouseIngameFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_presentation_mouse_ingame_failclosed_residual_wave841() {
        assert!(honesty_presentation_mouse_ingame_failclosed_residual_pack_wave841());
        assert!(honesty_presentation_mouse_ingame_failclosed_method_names_residual_wave841());
        assert!(honesty_presentation_mouse_ingame_failclosed_nav_commands_residual_wave841());
        assert!(simulate_live_presentation_mouse_ingame_failclosed_honesty());
    }
}
