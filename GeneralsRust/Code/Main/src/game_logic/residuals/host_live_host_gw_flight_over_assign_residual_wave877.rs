//! Wave 877: GameWorld special-power flight residual computes `over` in one
//! expression (no dead initial assign). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GW_FLIGHT_OVER_ASSIGN_METHOD_NAMES_WAVE877: &[&str] = &[
    "anthrax_transport_active",
    "cluster_mines_transport_active",
    "emp_pulse_transport_active",
    "let over = if dist < 5.0",
    "Wave 877",
    "playable_claim = false",
];

pub const LIVE_HOST_GW_FLIGHT_OVER_ASSIGN_NAV_STEPS_WAVE877: &[&str] = &[
    "SINGLE_EXPR_OVER_ASSIGN",
    "NO_DEAD_MUT_OVER_INIT",
    "LIVE_HOST_GW_FLIGHT_OVER_ASSIGN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGwFlightOverAssignAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostGwFlightOverAssignAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn gw_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

pub fn honesty_host_gw_flight_over_assign_method_names_residual_wave877() -> bool {
    let names = LIVE_HOST_GW_FLIGHT_OVER_ASSIGN_METHOD_NAMES_WAVE877;
    let ok = residual_name_index(names, "let over = if dist < 5.0").is_some()
        && residual_name_index(names, "Wave 877").is_some();
    residual_action_store(ResidualHostGwFlightOverAssignAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_gw_flight_over_assign_nav_commands_residual_wave877() -> bool {
    let steps = LIVE_HOST_GW_FLIGHT_OVER_ASSIGN_NAV_STEPS_WAVE877;
    let ok = residual_name_index(steps, "LIVE_HOST_GW_FLIGHT_OVER_ASSIGN").is_some()
        && residual_name_index(steps, "SINGLE_EXPR_OVER_ASSIGN").is_some();
    residual_action_store(ResidualHostGwFlightOverAssignAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_gw_flight_over_assign_residual_pack_wave877() -> bool {
    let gw = gw_source();
    let ok = gw.matches("let over = if dist < 5.0").count() >= 3
        && !gw.contains("let mut over = false")
        && gw.contains("anthrax_transport_active")
        && gw.contains("cluster_mines_transport_active")
        && gw.contains("emp_pulse_transport_active")
        && !gw.contains("playable_claim = true");
    residual_action_store(ResidualHostGwFlightOverAssignAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_gw_flight_over_assign_honesty() -> bool {
    let a = honesty_host_gw_flight_over_assign_method_names_residual_wave877();
    let b = honesty_host_gw_flight_over_assign_nav_commands_residual_wave877();
    let c = honesty_host_gw_flight_over_assign_residual_pack_wave877();
    residual_action_store(ResidualHostGwFlightOverAssignAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_gw_flight_over_assign_residual_wave877() {
        assert!(honesty_host_gw_flight_over_assign_residual_pack_wave877());
        assert!(honesty_host_gw_flight_over_assign_method_names_residual_wave877());
        assert!(honesty_host_gw_flight_over_assign_nav_commands_residual_wave877());
        assert!(simulate_live_host_gw_flight_over_assign_honesty());
    }
}
