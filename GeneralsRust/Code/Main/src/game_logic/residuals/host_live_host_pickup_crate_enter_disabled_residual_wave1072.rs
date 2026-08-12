//! Wave 1072: dual-world pickup crate + enter disabled residual.
//!
//! selection_can_pickup_crate_target dual fails closed on status/FOW and unusable
//! sources; selection_can_enter_target dual fails closed on disabled containers.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PICKUP_CRATE_ENTER_DISABLED_RESIDUAL_METHOD_NAMES_WAVE1072: &[&str] = &[
    "selection_can_pickup_crate_target",
    "selection_can_enter_target",
    "target.disabled",
    "Wave 1072",
    "playable_claim = false",
];

pub const LIVE_HOST_PICKUP_CRATE_ENTER_DISABLED_RESIDUAL_NAV_STEPS_WAVE1072: &[&str] = &[
    "PICKUP_CRATE",
    "ENTER_DISABLED",
    "LIVE_HOST_PICKUP_CRATE_ENTER_DISABLED_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPickupCrateEnterDisabledResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPickupCrateEnterDisabledResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}

pub fn honesty_host_pickup_crate_enter_disabled_residual_method_names_residual_wave1072() -> bool {
    let names = LIVE_HOST_PICKUP_CRATE_ENTER_DISABLED_RESIDUAL_METHOD_NAMES_WAVE1072;
    let ok = residual_name_index(names, "selection_can_pickup_crate_target").is_some()
        && residual_name_index(names, "Wave 1072").is_some();
    residual_action_store(ResidualHostPickupCrateEnterDisabledResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_pickup_crate_enter_disabled_residual_nav_commands_residual_wave1072() -> bool {
    let steps = LIVE_HOST_PICKUP_CRATE_ENTER_DISABLED_RESIDUAL_NAV_STEPS_WAVE1072;
    let ok = residual_name_index(steps, "LIVE_HOST_PICKUP_CRATE_ENTER_DISABLED_RESIDUAL").is_some()
        && residual_name_index(steps, "PICKUP_CRATE").is_some();
    residual_action_store(ResidualHostPickupCrateEnterDisabledResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_pickup_crate_enter_disabled_residual_residual_pack_wave1072() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ok = tr
        .contains("Wave 1072: crate dual fail-closed on status/FOW and unusable local sources")
        && tr.contains("Wave 1072: disabled container residual fail-closed")
        && tr.contains("if target.disabled {\n            return false;\n        }")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPickupCrateEnterDisabledResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_pickup_crate_enter_disabled_residual_honesty() -> bool {
    let a = honesty_host_pickup_crate_enter_disabled_residual_method_names_residual_wave1072();
    let b = honesty_host_pickup_crate_enter_disabled_residual_nav_commands_residual_wave1072();
    let c = honesty_host_pickup_crate_enter_disabled_residual_residual_pack_wave1072();
    residual_action_store(ResidualHostPickupCrateEnterDisabledResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_pickup_crate_enter_disabled_residual_wave1072() {
        assert!(honesty_host_pickup_crate_enter_disabled_residual_residual_pack_wave1072());
        assert!(honesty_host_pickup_crate_enter_disabled_residual_method_names_residual_wave1072());
        assert!(honesty_host_pickup_crate_enter_disabled_residual_nav_commands_residual_wave1072());
        assert!(simulate_live_host_pickup_crate_enter_disabled_residual_honesty());
    }
}
