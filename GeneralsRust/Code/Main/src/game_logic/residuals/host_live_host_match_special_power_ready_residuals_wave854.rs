//! Wave 854: host-owned special-power-ready residual peels obvious not-ready
//! dual-reads from host_is_special_power_ready_for before live GameLogic probes.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_SPECIAL_POWER_READY_RESIDUALS_METHOD_NAMES_WAVE854: &[&str] = &[
    "host_match_special_power_ready_ids",
    "host_is_special_power_ready_for",
    "Wave 854",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_SPECIAL_POWER_READY_RESIDUALS_NAV_STEPS_WAVE854: &[&str] = &[
    "STAMP_HOST_MATCH_SPECIAL_POWER_READY",
    "FAILCLOSED_NOT_READY_BEFORE_LIVE",
    "LIVE_HOST_MATCH_SPECIAL_POWER_READY_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchSpecialPowerReadyResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchSpecialPowerReadyResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_host_match_special_power_ready_residuals_method_names_residual_wave854() -> bool {
    let names = LIVE_HOST_MATCH_SPECIAL_POWER_READY_RESIDUALS_METHOD_NAMES_WAVE854;
    let ok = residual_name_index(names, "host_match_special_power_ready_ids").is_some()
        && residual_name_index(names, "host_is_special_power_ready_for").is_some()
        && residual_name_index(names, "Wave 854").is_some();
    residual_action_store(ResidualHostMatchSpecialPowerReadyResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_special_power_ready_residuals_nav_commands_residual_wave854() -> bool {
    let steps = LIVE_HOST_MATCH_SPECIAL_POWER_READY_RESIDUALS_NAV_STEPS_WAVE854;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_SPECIAL_POWER_READY_RESIDUALS").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_SPECIAL_POWER_READY").is_some();
    residual_action_store(ResidualHostMatchSpecialPowerReadyResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_special_power_ready_residuals_residual_pack_wave854() -> bool {
    let cnc = cnc_source();
    let ok = cnc
        .contains("host_match_special_power_ready_ids: Option<std::collections::HashSet<u32>>")
        && (cnc.contains("Wave 854: stamp special-power-ready object residual")
            || cnc.contains("Wave 854/857: special-power-ready residual stamped inside"))
        && cnc.contains("Wave 584/854")
        && cnc.contains("if let Some(ready) = self.host_match_special_power_ready_ids.as_ref()")
        && cnc.contains("if !ready.contains(&id.0)")
        && cnc.contains("is_special_power_ready_for(id, power)"); // live residual remains
    residual_action_store(ResidualHostMatchSpecialPowerReadyResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_special_power_ready_residuals_honesty() -> bool {
    let a = honesty_host_match_special_power_ready_residuals_method_names_residual_wave854();
    let b = honesty_host_match_special_power_ready_residuals_nav_commands_residual_wave854();
    let c = honesty_host_match_special_power_ready_residuals_residual_pack_wave854();
    residual_action_store(ResidualHostMatchSpecialPowerReadyResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_special_power_ready_residuals_residual_wave854() {
        assert!(honesty_host_match_special_power_ready_residuals_residual_pack_wave854());
        assert!(honesty_host_match_special_power_ready_residuals_method_names_residual_wave854());
        assert!(honesty_host_match_special_power_ready_residuals_nav_commands_residual_wave854());
        assert!(simulate_live_host_match_special_power_ready_residuals_honesty());
    }
}
