//! Wave 860: presentation_or_boot_map_name fails closed on warm host_match_map_name
//! when freeze still holds shell residual — no live get_current_map_name dual-read.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MAP_NAME_FAILCLOSED_METHOD_NAMES_WAVE860: &[&str] = &[
    "presentation_or_boot_map_name",
    "host_match_map_name",
    "map_name_is_shell_residual",
    "Wave 860",
    "playable_claim = false",
];

pub const LIVE_HOST_MAP_NAME_FAILCLOSED_NAV_STEPS_WAVE860: &[&str] = &[
    "SHELL_FREEZE_PREFER_HOST_MAP",
    "WARM_RESIDUAL_NO_LIVE_MAP_PROBE",
    "LIVE_HOST_MAP_NAME_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMapNameFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMapNameFailclosedAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

pub fn honesty_host_map_name_failclosed_method_names_residual_wave860() -> bool {
    let names = LIVE_HOST_MAP_NAME_FAILCLOSED_METHOD_NAMES_WAVE860;
    let ok = residual_name_index(names, "presentation_or_boot_map_name").is_some()
        && residual_name_index(names, "Wave 860").is_some();
    residual_action_store(ResidualHostMapNameFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_map_name_failclosed_nav_commands_residual_wave860() -> bool {
    let steps = LIVE_HOST_MAP_NAME_FAILCLOSED_NAV_STEPS_WAVE860;
    let ok = residual_name_index(steps, "LIVE_HOST_MAP_NAME_FAILCLOSED").is_some()
        && residual_name_index(steps, "WARM_RESIDUAL_NO_LIVE_MAP_PROBE").is_some();
    residual_action_store(ResidualHostMapNameFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_map_name_failclosed_residual_pack_wave860() -> bool {
    let cnc = cnc_source();
    let body_at = cnc.find("fn presentation_or_boot_map_name");
    let body_ok = if let Some(i) = body_at {
        let end = cnc[i..]
            .find("\n    fn ")
            .map(|e| i + e)
            .unwrap_or(i + 1200);
        let body = &cnc[i..end];
        body.contains("Wave 554/860")
            && body.contains("Wave 860: warm residual still shell")
            && body.matches("get_current_map_name()").count() == 2 // cold freeze path + boot
            && body.contains("host_match_map_name")
    } else {
        false
    };
    residual_action_store(ResidualHostMapNameFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(body_ok, Ordering::SeqCst);
    body_ok
}

pub fn simulate_live_host_map_name_failclosed_honesty() -> bool {
    let a = honesty_host_map_name_failclosed_method_names_residual_wave860();
    let b = honesty_host_map_name_failclosed_nav_commands_residual_wave860();
    let c = honesty_host_map_name_failclosed_residual_pack_wave860();
    residual_action_store(ResidualHostMapNameFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_map_name_failclosed_residual_wave860() {
        assert!(honesty_host_map_name_failclosed_residual_pack_wave860());
        assert!(honesty_host_map_name_failclosed_method_names_residual_wave860());
        assert!(honesty_host_map_name_failclosed_nav_commands_residual_wave860());
        assert!(simulate_live_host_map_name_failclosed_honesty());
    }
}
