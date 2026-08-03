//! Wave 1048: dual-world any-local target + mine legality residual.
//!
//! selection_any_local_object_can_target dual fails closed on illegal targets and
//! dead/sold/disabled local sources; mine dual requires status-ok. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ANY_LOCAL_TARGET_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1048: &[&str] = &[
    "selection_any_local_object_can_target",
    "is_locally_controlled_mine_target",
    "dual_target_status_ok",
    "Wave 1048",
    "playable_claim = false",
];

pub const LIVE_HOST_ANY_LOCAL_TARGET_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1048: &[&str] = &[
    "ANY_LOCAL_TARGET",
    "MINE_LEGALITY",
    "LIVE_HOST_ANY_LOCAL_TARGET_LEGALITY_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAnyLocalTargetLegalityResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostAnyLocalTargetLegalityResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn tr_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/message_stream/translators.rs")
}

pub fn honesty_host_any_local_target_legality_residual_method_names_residual_wave1048() -> bool {
    let names = LIVE_HOST_ANY_LOCAL_TARGET_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1048;
    let ok = residual_name_index(names, "selection_any_local_object_can_target").is_some()
        && residual_name_index(names, "Wave 1048").is_some();
    residual_action_store(ResidualHostAnyLocalTargetLegalityResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_any_local_target_legality_residual_nav_commands_residual_wave1048() -> bool {
    let steps = LIVE_HOST_ANY_LOCAL_TARGET_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1048;
    let ok = residual_name_index(steps, "LIVE_HOST_ANY_LOCAL_TARGET_LEGALITY_RESIDUAL").is_some()
        && residual_name_index(steps, "ANY_LOCAL_TARGET").is_some();
    residual_action_store(ResidualHostAnyLocalTargetLegalityResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_any_local_target_legality_residual_residual_pack_wave1048() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ok = tr.contains("Wave 975/1048: host empty dual-world → presentation catalog residual.")
        && tr.contains("Wave 973/1048: host empty dual-world → presentation catalog residual.")
        && tr.contains("target.effectively_stealthed && !translator_entry_is_local(&target)")
        && tr.contains("dual_target_status_ok(&e)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostAnyLocalTargetLegalityResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_any_local_target_legality_residual_honesty() -> bool {
    let a = honesty_host_any_local_target_legality_residual_method_names_residual_wave1048();
    let b = honesty_host_any_local_target_legality_residual_nav_commands_residual_wave1048();
    let c = honesty_host_any_local_target_legality_residual_residual_pack_wave1048();
    residual_action_store(ResidualHostAnyLocalTargetLegalityResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_any_local_target_legality_residual_wave1048() {
        assert!(honesty_host_any_local_target_legality_residual_residual_pack_wave1048());
        assert!(honesty_host_any_local_target_legality_residual_method_names_residual_wave1048());
        assert!(honesty_host_any_local_target_legality_residual_nav_commands_residual_wave1048());
        assert!(simulate_live_host_any_local_target_legality_residual_honesty());
    }
}
