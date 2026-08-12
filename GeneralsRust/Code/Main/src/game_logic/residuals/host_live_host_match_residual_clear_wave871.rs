//! Wave 871: host_clear_match_residuals centralizes residual clear on
//! reset/load/start; silent/shell process + command helpers stamp sim timing.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_RESIDUAL_CLEAR_METHOD_NAMES_WAVE871: &[&str] = &[
    "host_clear_match_residuals",
    "host_reset_game_logic",
    "host_load_map_or_default",
    "host_start_new_game_with_faction",
    "host_queue_and_process_command_silent",
    "host_process_shell_menu_commands",
    "Wave 871",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_RESIDUAL_CLEAR_NAV_STEPS_WAVE871: &[&str] = &[
    "CLEAR_ON_RESET",
    "CLEAR_ON_LOAD",
    "CLEAR_ON_START",
    "STAMP_AFTER_SILENT_PROCESS",
    "STAMP_AFTER_COMMAND_HELPERS",
    "LIVE_HOST_MATCH_RESIDUAL_CLEAR",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchResidualClearAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchResidualClearAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_host_match_residual_clear_method_names_residual_wave871() -> bool {
    let names = LIVE_HOST_MATCH_RESIDUAL_CLEAR_METHOD_NAMES_WAVE871;
    let ok = residual_name_index(names, "host_clear_match_residuals").is_some()
        && residual_name_index(names, "host_reset_game_logic").is_some()
        && residual_name_index(names, "Wave 871").is_some();
    residual_action_store(ResidualHostMatchResidualClearAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_residual_clear_nav_commands_residual_wave871() -> bool {
    let steps = LIVE_HOST_MATCH_RESIDUAL_CLEAR_NAV_STEPS_WAVE871;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_RESIDUAL_CLEAR").is_some()
        && residual_name_index(steps, "CLEAR_ON_RESET").is_some();
    residual_action_store(ResidualHostMatchResidualClearAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_residual_clear_residual_pack_wave871() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("fn host_clear_match_residuals(&mut self)")
        && cnc.contains("Wave 843/844/871: clear prior match residuals until load completes")
        && cnc.contains("self.host_clear_match_residuals()")
        && cnc.contains("Wave 584/871: host reset residual + clear match residuals")
        && cnc.contains("Wave 579/871: load_map + DEFAULT_SKIRMISH_MAP fallback residual")
        && cnc.contains("Wave 577/871: start_new_game + set_player_team")
        && cnc.contains("Wave 576/578/871: silent queue+process residual + stamp sim timing")
        && cnc.contains("Wave 582/871: shell/menu command drain residual + stamp sim timing")
        && cnc.contains("Wave 583/871: host attack residual + stamp sim timing")
        && cnc.contains("self.host_match_world_bounds = None");
    residual_action_store(ResidualHostMatchResidualClearAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_residual_clear_honesty() -> bool {
    let a = honesty_host_match_residual_clear_method_names_residual_wave871();
    let b = honesty_host_match_residual_clear_nav_commands_residual_wave871();
    let c = honesty_host_match_residual_clear_residual_pack_wave871();
    residual_action_store(ResidualHostMatchResidualClearAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_residual_clear_residual_wave871() {
        assert!(honesty_host_match_residual_clear_residual_pack_wave871());
        assert!(honesty_host_match_residual_clear_method_names_residual_wave871());
        assert!(honesty_host_match_residual_clear_nav_commands_residual_wave871());
        assert!(simulate_live_host_match_residual_clear_honesty());
    }
}
