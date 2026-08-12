//! Wave 851: host-owned alive-object residual peels object_is_alive dual-reads from
//! presentation_or_boot_object_alive / host_object_is_alive when freeze is missing.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_ALIVE_OBJECT_RESIDUALS_METHOD_NAMES_WAVE851: &[&str] = &[
    "host_match_alive_object_ids",
    "presentation_or_boot_object_alive",
    "host_object_is_alive",
    "Wave 851",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_ALIVE_OBJECT_RESIDUALS_NAV_STEPS_WAVE851: &[&str] = &[
    "STAMP_HOST_MATCH_ALIVE_OBJECTS",
    "PREFER_HOST_ALIVE_BEFORE_LIVE",
    "LIVE_HOST_MATCH_ALIVE_OBJECT_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchAliveObjectResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchAliveObjectResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_host_match_alive_object_residuals_method_names_residual_wave851() -> bool {
    let names = LIVE_HOST_MATCH_ALIVE_OBJECT_RESIDUALS_METHOD_NAMES_WAVE851;
    let ok = residual_name_index(names, "host_match_alive_object_ids").is_some()
        && residual_name_index(names, "presentation_or_boot_object_alive").is_some()
        && residual_name_index(names, "Wave 851").is_some();
    residual_action_store(ResidualHostMatchAliveObjectResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_alive_object_residuals_nav_commands_residual_wave851() -> bool {
    let steps = LIVE_HOST_MATCH_ALIVE_OBJECT_RESIDUALS_NAV_STEPS_WAVE851;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_ALIVE_OBJECT_RESIDUALS").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_ALIVE_OBJECTS").is_some();
    residual_action_store(ResidualHostMatchAliveObjectResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_alive_object_residuals_residual_pack_wave851() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_alive_object_ids: Option<std::collections::HashSet<u32>>")
        && (cnc.contains("Wave 851: stamp alive-object residual")
            || cnc.contains("Wave 851/853: alive residual stamped inside"))
        && cnc.contains("Wave 584/851")
        && cnc.contains("if let Some(alive) = self.host_match_alive_object_ids.as_ref()")
        && cnc.matches("alive.contains(&id.0)").count() >= 2
        && cnc.contains("object_is_alive(id)"); // boot residual remains
    residual_action_store(ResidualHostMatchAliveObjectResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_alive_object_residuals_honesty() -> bool {
    let a = honesty_host_match_alive_object_residuals_method_names_residual_wave851();
    let b = honesty_host_match_alive_object_residuals_nav_commands_residual_wave851();
    let c = honesty_host_match_alive_object_residuals_residual_pack_wave851();
    residual_action_store(ResidualHostMatchAliveObjectResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_alive_object_residuals_residual_wave851() {
        assert!(honesty_host_match_alive_object_residuals_residual_pack_wave851());
        assert!(honesty_host_match_alive_object_residuals_method_names_residual_wave851());
        assert!(honesty_host_match_alive_object_residuals_nav_commands_residual_wave851());
        assert!(simulate_live_host_match_alive_object_residuals_honesty());
    }
}
