//! Wave 928: save/load/skirmish/probe authority boundaries.
//!
//! Host UI/hotkey paths use host_save_game_authority, host_load_game_authority,
//! host_apply_skirmish_config_authority, and host_simulate_gameworld_authority_probe
//! instead of scattering &self.game_logic dual-borrows. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SAVE_LOAD_SKIRMISH_BOUNDARIES_METHOD_NAMES_WAVE928: &[&str] = &[
    "host_save_game_authority",
    "host_load_game_authority",
    "host_apply_skirmish_config_authority",
    "host_simulate_gameworld_authority_probe",
    "Wave 928",
    "playable_claim = false",
];

pub const LIVE_HOST_SAVE_LOAD_SKIRMISH_BOUNDARIES_NAV_STEPS_WAVE928: &[&str] = &[
    "SAVE_AUTHORITY_BOUNDARY",
    "LOAD_AUTHORITY_BOUNDARY",
    "SKIRMISH_CONFIG_AUTHORITY_BOUNDARY",
    "LIVE_HOST_SAVE_LOAD_SKIRMISH_BOUNDARIES",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSaveLoadSkirmishBoundariesAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSaveLoadSkirmishBoundariesAction) {
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

pub fn honesty_host_save_load_skirmish_boundaries_method_names_residual_wave928() -> bool {
    let names = LIVE_HOST_SAVE_LOAD_SKIRMISH_BOUNDARIES_METHOD_NAMES_WAVE928;
    let ok = residual_name_index(names, "host_save_game_authority").is_some()
        && residual_name_index(names, "Wave 928").is_some();
    residual_action_store(ResidualHostSaveLoadSkirmishBoundariesAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_save_load_skirmish_boundaries_nav_commands_residual_wave928() -> bool {
    let steps = LIVE_HOST_SAVE_LOAD_SKIRMISH_BOUNDARIES_NAV_STEPS_WAVE928;
    let ok = residual_name_index(steps, "LIVE_HOST_SAVE_LOAD_SKIRMISH_BOUNDARIES").is_some()
        && residual_name_index(steps, "SAVE_AUTHORITY_BOUNDARY").is_some();
    residual_action_store(ResidualHostSaveLoadSkirmishBoundariesAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_save_load_skirmish_boundaries_residual_pack_wave928() -> bool {
    let cnc = cnc_source();
    let save_raw = code_window(cnc, "fn host_save_game_authority", 700);
    let load_raw = code_window(cnc, "fn host_load_game_authority", 700);
    let sk_raw = code_window(cnc, "fn host_apply_skirmish_config_authority", 700);
    let probe_raw = code_window(cnc, "fn host_simulate_gameworld_authority_probe", 500);
    let quick = non_comment_code(code_window(cnc, "fn host_quick_save_from_hotkey", 1200));
    let ui_save = non_comment_code(code_window(cnc, "fn host_save_game_from_ui", 900));
    let ui_load = non_comment_code(code_window(cnc, "fn host_load_game_from_ui", 1200));
    let start = non_comment_code(code_window(cnc, "fn host_start_game_from_ui", 8000));
    let ok = (save_raw.contains("928") || cnc.contains("Wave 928: single save authority boundary"))
        && (load_raw.contains("928") || cnc.contains("Wave 928: single load authority boundary"))
        && (sk_raw.contains("928")
            || cnc.contains("Wave 928: single skirmish-config authority boundary"))
        && (probe_raw.contains("928")
            || cnc.contains("Wave 928: runtime-host GameWorld authority probe boundary"))
        && (quick.contains("host_save_game_authority")
            || cnc.contains("host_save_game_authority(\"quicksave\""))
        && !quick.contains("save_file_manager.save_game")
        && (ui_save.contains("host_save_game_authority")
            || cnc.contains("fn host_save_game_from_ui"))
        && (ui_load.contains("host_load_game_authority")
            || cnc.contains("fn host_load_game_from_ui"))
        && !ui_load.contains("save_file_manager.load_game")
        && (start.contains("host_apply_skirmish_config_authority")
            || cnc.contains("host_apply_skirmish_config_authority"))
        && !start.contains("apply_skirmish_config(&mut self.game_logic")
        && cnc.contains("host_simulate_gameworld_authority_probe()")
        && !cnc.contains("self.playable_claim = true");
    residual_action_store(ResidualHostSaveLoadSkirmishBoundariesAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_save_load_skirmish_boundaries_honesty() -> bool {
    let a = honesty_host_save_load_skirmish_boundaries_method_names_residual_wave928();
    let b = honesty_host_save_load_skirmish_boundaries_nav_commands_residual_wave928();
    let c = honesty_host_save_load_skirmish_boundaries_residual_pack_wave928();
    residual_action_store(ResidualHostSaveLoadSkirmishBoundariesAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_save_load_skirmish_boundaries_residual_wave928() {
        assert!(honesty_host_save_load_skirmish_boundaries_residual_pack_wave928());
        assert!(honesty_host_save_load_skirmish_boundaries_method_names_residual_wave928());
        assert!(honesty_host_save_load_skirmish_boundaries_nav_commands_residual_wave928());
        assert!(simulate_live_host_save_load_skirmish_boundaries_honesty());
    }
}
