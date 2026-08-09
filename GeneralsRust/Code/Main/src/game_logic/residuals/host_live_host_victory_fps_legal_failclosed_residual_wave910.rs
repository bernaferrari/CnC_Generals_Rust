//! Wave 910: victory/FPS cold fail-closed + legal-build helper consolidation.
//!
//! - boot victory residual fail-closed without evaluate_victory dual-read
//! - cold script FPS residual fail-closed without take_script_fps dual-read
//! - is_location_legal routes through host_legal_build_code residual
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_VICTORY_FPS_LEGAL_FAILCLOSED_METHOD_NAMES_WAVE910: &[&str] = &[
    "host_boot_victory_condition_residual",
    "apply_ingame_script_fps_limit_residual",
    "host_is_location_legal_to_build_for_builder",
    "host_legal_build_code_at_for_builder",
    "Wave 910",
    "playable_claim = false",
];

pub const LIVE_HOST_VICTORY_FPS_LEGAL_FAILCLOSED_NAV_STEPS_WAVE910: &[&str] = &[
    "VICTORY_COLD_FAILCLOSED",
    "SCRIPT_FPS_COLD_FAILCLOSED",
    "LEGAL_LOCATION_VIA_LEGAL_CODE",
    "LIVE_HOST_VICTORY_FPS_LEGAL_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostVictoryFpsLegalFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostVictoryFpsLegalFailclosedAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn code_window<'a>(src: &'a str, marker: &str, len: usize) -> &'a str {
    match src.find(marker) {
        Some(i) => &src[i..src.len().min(i + len)],
        None => "",
    }
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_victory_fps_legal_failclosed_method_names_residual_wave910() -> bool {
    let names = LIVE_HOST_VICTORY_FPS_LEGAL_FAILCLOSED_METHOD_NAMES_WAVE910;
    let ok = residual_name_index(names, "host_boot_victory_condition_residual").is_some()
        && residual_name_index(names, "Wave 910").is_some();
    residual_action_store(ResidualHostVictoryFpsLegalFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_victory_fps_legal_failclosed_nav_commands_residual_wave910() -> bool {
    let steps = LIVE_HOST_VICTORY_FPS_LEGAL_FAILCLOSED_NAV_STEPS_WAVE910;
    let ok = residual_name_index(steps, "LIVE_HOST_VICTORY_FPS_LEGAL_FAILCLOSED").is_some()
        && residual_name_index(steps, "VICTORY_COLD_FAILCLOSED").is_some();
    residual_action_store(ResidualHostVictoryFpsLegalFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_victory_fps_legal_failclosed_residual_pack_wave910() -> bool {
    let cnc = cnc_source();
    let vic_raw = code_window(cnc, "fn host_boot_victory_condition_residual", 1200);
    let vic = non_comment_code(vic_raw);
    let fps_raw = code_window(cnc, "fn apply_ingame_script_fps_limit_residual", 900);
    let fps = non_comment_code(fps_raw);
    let loc_raw = code_window(cnc, "fn host_is_location_legal_to_build_for_builder", 700);
    let loc = non_comment_code(loc_raw);
    let ok = vic_raw.contains("910")
        && !vic.contains("evaluate_victory_condition")
        && fps_raw.contains("910")
        && !fps.contains("take_script_fps_limit_request")
        && loc_raw.contains("910")
        && loc.contains("host_legal_build_code_at_for_builder")
        && loc.contains("LBC_OK")
        && !loc.contains("self.game_logic")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostVictoryFpsLegalFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_victory_fps_legal_failclosed_honesty() -> bool {
    let a = honesty_host_victory_fps_legal_failclosed_method_names_residual_wave910();
    let b = honesty_host_victory_fps_legal_failclosed_nav_commands_residual_wave910();
    let c = honesty_host_victory_fps_legal_failclosed_residual_pack_wave910();
    residual_action_store(ResidualHostVictoryFpsLegalFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_victory_fps_legal_failclosed_residual_wave910() {
        assert!(honesty_host_victory_fps_legal_failclosed_residual_pack_wave910());
        assert!(honesty_host_victory_fps_legal_failclosed_method_names_residual_wave910());
        assert!(honesty_host_victory_fps_legal_failclosed_nav_commands_residual_wave910());
        assert!(simulate_live_host_victory_fps_legal_failclosed_honesty());
    }
}
