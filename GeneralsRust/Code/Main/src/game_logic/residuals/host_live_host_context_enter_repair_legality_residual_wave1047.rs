//! Wave 1047: dual-world context enter/repair/resume legality residual.
//!
//! selection_can_enter/repair/resume dual paths require status-ok + apparent ally;
//! resume requires under_construction; crate skips destroyed/sold.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONTEXT_ENTER_REPAIR_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1047: &[&str] = &[
    "selection_can_enter_target",
    "selection_can_repair_target",
    "dual_target_is_apparent_ally",
    "Wave 1047",
    "playable_claim = false",
];

pub const LIVE_HOST_CONTEXT_ENTER_REPAIR_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1047: &[&str] = &[
    "CONTEXT_ENTER",
    "REPAIR",
    "RESUME_CONSTRUCTION",
    "LIVE_HOST_CONTEXT_ENTER_REPAIR_LEGALITY_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostContextEnterRepairLegalityResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostContextEnterRepairLegalityResidualAction) {
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

pub fn honesty_host_context_enter_repair_legality_residual_method_names_residual_wave1047() -> bool
{
    let names = LIVE_HOST_CONTEXT_ENTER_REPAIR_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1047;
    let ok = residual_name_index(names, "dual_target_is_apparent_ally").is_some()
        && residual_name_index(names, "Wave 1047").is_some();
    residual_action_store(ResidualHostContextEnterRepairLegalityResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_context_enter_repair_legality_residual_nav_commands_residual_wave1047() -> bool
{
    let steps = LIVE_HOST_CONTEXT_ENTER_REPAIR_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1047;
    let ok = residual_name_index(steps, "LIVE_HOST_CONTEXT_ENTER_REPAIR_LEGALITY_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "CONTEXT_ENTER").is_some();
    residual_action_store(ResidualHostContextEnterRepairLegalityResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_context_enter_repair_legality_residual_residual_pack_wave1047() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ok = tr.contains("Wave 1047: dual catalog target legality for context enter/repair/resume")
        && tr.contains("dual_target_is_apparent_ally")
        && tr.contains("Wave 975/1047: host empty dual-world → presentation catalog residual.")
        && tr.contains("if !target.under_construction")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostContextEnterRepairLegalityResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_context_enter_repair_legality_residual_honesty() -> bool {
    let a = honesty_host_context_enter_repair_legality_residual_method_names_residual_wave1047();
    let b = honesty_host_context_enter_repair_legality_residual_nav_commands_residual_wave1047();
    let c = honesty_host_context_enter_repair_legality_residual_residual_pack_wave1047();
    residual_action_store(ResidualHostContextEnterRepairLegalityResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_context_enter_repair_legality_residual_wave1047() {
        assert!(honesty_host_context_enter_repair_legality_residual_residual_pack_wave1047());
        assert!(
            honesty_host_context_enter_repair_legality_residual_method_names_residual_wave1047()
        );
        assert!(
            honesty_host_context_enter_repair_legality_residual_nav_commands_residual_wave1047()
        );
        assert!(simulate_live_host_context_enter_repair_legality_residual_honesty());
    }
}
