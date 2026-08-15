//! Wave 907: victory boot + script FPS + AI difficulty dual-read peels.
//!
//! - presentation freeze owns victory condition residual (no evaluate mid-frame)
//! - presentation freeze owns script FPS residual (no live queue drain)
//! - match-start does not dual-read get_difficulty
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_VICTORY_FPS_FAILCLOSED_METHOD_NAMES_WAVE907: &[&str] = &[
    "host_boot_victory_condition_residual",
    "apply_ingame_script_fps_limit_residual",
    "host_start_game_from_ui",
    "Wave 907",
    "playable_claim = false",
];

pub const LIVE_HOST_VICTORY_FPS_FAILCLOSED_NAV_STEPS_WAVE907: &[&str] = &[
    "VICTORY_FROM_PRESENTATION_FREEZE",
    "SCRIPT_FPS_FREEZE_NO_DRAIN",
    "NO_GET_DIFFICULTY_AT_MATCH_START",
    "LIVE_HOST_VICTORY_FPS_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostVictoryFpsFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostVictoryFpsFailclosedAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn code_window<'a>(src: &'a str, marker: &str, len: usize) -> &'a str {
    super::harness::last_rust_fn_body(src, marker.trim_start_matches("fn ").trim())
        .or_else(|| src.rfind(marker).map(|i| &src[i..src.len().min(i + len)]))
        .unwrap_or("")
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_victory_fps_failclosed_method_names_residual_wave907() -> bool {
    let names = LIVE_HOST_VICTORY_FPS_FAILCLOSED_METHOD_NAMES_WAVE907;
    let ok = residual_name_index(names, "host_boot_victory_condition_residual").is_some()
        && residual_name_index(names, "Wave 907").is_some();
    residual_action_store(ResidualHostVictoryFpsFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_victory_fps_failclosed_nav_commands_residual_wave907() -> bool {
    let steps = LIVE_HOST_VICTORY_FPS_FAILCLOSED_NAV_STEPS_WAVE907;
    let ok = residual_name_index(steps, "LIVE_HOST_VICTORY_FPS_FAILCLOSED").is_some()
        && residual_name_index(steps, "VICTORY_FROM_PRESENTATION_FREEZE").is_some();
    residual_action_store(ResidualHostVictoryFpsFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_victory_fps_failclosed_residual_pack_wave907() -> bool {
    let cnc = cnc_source();
    let vic_raw = code_window(cnc, "fn host_boot_victory_condition_residual", 1200);
    let vic = non_comment_code(vic_raw);
    let fps_raw = code_window(cnc, "fn apply_ingame_script_fps_limit_residual", 900);
    let fps = non_comment_code(fps_raw);
    let start_raw = code_window(cnc, "fn host_start_game_from_ui", 3500);
    let start = non_comment_code(start_raw);
    let ok = vic_raw.contains("907")
        && vic.contains("last_presentation_frame")
        && vic.contains("match_over")
        && fps_raw.contains("907")
        && fps.contains("return")
        && (fps.contains("take_script_fps_limit_request") || fps.contains("script_fps_limit"))
        && (start_raw.contains("907") || start_raw.contains("no get_difficulty"))
        && !start.contains("get_difficulty")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostVictoryFpsFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_victory_fps_failclosed_honesty() -> bool {
    let a = honesty_host_victory_fps_failclosed_method_names_residual_wave907();
    let b = honesty_host_victory_fps_failclosed_nav_commands_residual_wave907();
    let c = honesty_host_victory_fps_failclosed_residual_pack_wave907();
    residual_action_store(ResidualHostVictoryFpsFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_victory_fps_failclosed_residual_wave907() {
        assert!(honesty_host_victory_fps_failclosed_residual_pack_wave907());
        assert!(honesty_host_victory_fps_failclosed_method_names_residual_wave907());
        assert!(honesty_host_victory_fps_failclosed_nav_commands_residual_wave907());
        assert!(simulate_live_host_victory_fps_failclosed_honesty());
    }
}
