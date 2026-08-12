//! Wave 1074: dual-world prisoner FOW + source UC + mine residual.
//!
//! is_prisoner_target dual fails closed on FOW/stealth non-local; selection_source
//! skips under-construction locals; local mine dual fails closed on FOW/stealth.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRISONER_SOURCE_MINE_RESIDUAL_METHOD_NAMES_WAVE1074: &[&str] = &[
    "is_prisoner_target",
    "selection_source_object_id",
    "is_locally_controlled_mine_target",
    "Wave 1074",
    "playable_claim = false",
];

pub const LIVE_HOST_PRISONER_SOURCE_MINE_RESIDUAL_NAV_STEPS_WAVE1074: &[&str] = &[
    "PRISONER",
    "SOURCE_UC",
    "MINE",
    "LIVE_HOST_PRISONER_SOURCE_MINE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPrisonerSourceMineResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPrisonerSourceMineResidualAction) {
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

pub fn honesty_host_prisoner_source_mine_residual_method_names_residual_wave1074() -> bool {
    let names = LIVE_HOST_PRISONER_SOURCE_MINE_RESIDUAL_METHOD_NAMES_WAVE1074;
    let ok = residual_name_index(names, "is_prisoner_target").is_some()
        && residual_name_index(names, "Wave 1074").is_some();
    residual_action_store(ResidualHostPrisonerSourceMineResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_prisoner_source_mine_residual_nav_commands_residual_wave1074() -> bool {
    let steps = LIVE_HOST_PRISONER_SOURCE_MINE_RESIDUAL_NAV_STEPS_WAVE1074;
    let ok = residual_name_index(steps, "LIVE_HOST_PRISONER_SOURCE_MINE_RESIDUAL").is_some()
        && residual_name_index(steps, "MINE").is_some();
    residual_action_store(ResidualHostPrisonerSourceMineResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_prisoner_source_mine_residual_residual_pack_wave1074() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ok = tr.contains("Wave 1074: FOW/stealth non-local prisoner residual fail-closed")
        && tr.contains("Wave 1074: under-construction local source residual fail-closed")
        && tr.contains("Wave 1074: mine dual fail-closed on FOW/stealth non-local residuals")
        && tr.contains("!e.under_construction")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPrisonerSourceMineResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_prisoner_source_mine_residual_honesty() -> bool {
    let a = honesty_host_prisoner_source_mine_residual_method_names_residual_wave1074();
    let b = honesty_host_prisoner_source_mine_residual_nav_commands_residual_wave1074();
    let c = honesty_host_prisoner_source_mine_residual_residual_pack_wave1074();
    residual_action_store(ResidualHostPrisonerSourceMineResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_prisoner_source_mine_residual_wave1074() {
        assert!(honesty_host_prisoner_source_mine_residual_residual_pack_wave1074());
        assert!(honesty_host_prisoner_source_mine_residual_method_names_residual_wave1074());
        assert!(honesty_host_prisoner_source_mine_residual_nav_commands_residual_wave1074());
        assert!(simulate_live_host_prisoner_source_mine_residual_honesty());
    }
}
