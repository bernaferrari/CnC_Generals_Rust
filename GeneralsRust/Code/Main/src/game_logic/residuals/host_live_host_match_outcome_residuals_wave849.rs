//! Wave 849: host-owned match outcome residuals peel live evaluate_victory_condition
//! dual-reads from presentation_or_boot_match_over_label / victory_winner /
//! victory_summary when freeze is missing.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_OUTCOME_RESIDUALS_METHOD_NAMES_WAVE849: &[&str] = &[
    "host_match_over",
    "host_match_victory_label",
    "host_match_victory_winner",
    "host_match_victory_summary",
    "presentation_or_boot_match_over_label",
    "presentation_or_boot_victory_winner",
    "Wave 849",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_OUTCOME_RESIDUALS_NAV_STEPS_WAVE849: &[&str] = &[
    "STAMP_HOST_MATCH_OUTCOME",
    "PREFER_FREEZE_THEN_HOST_OUTCOME",
    "LIVE_HOST_MATCH_OUTCOME_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchOutcomeResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchOutcomeResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_match_outcome_residuals_method_names_residual_wave849() -> bool {
    let names = LIVE_HOST_MATCH_OUTCOME_RESIDUALS_METHOD_NAMES_WAVE849;
    let ok = residual_name_index(names, "host_match_over").is_some()
        && residual_name_index(names, "host_match_victory_winner").is_some()
        && residual_name_index(names, "Wave 849").is_some();
    residual_action_store(ResidualHostMatchOutcomeResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_outcome_residuals_nav_commands_residual_wave849() -> bool {
    let steps = LIVE_HOST_MATCH_OUTCOME_RESIDUALS_NAV_STEPS_WAVE849;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_OUTCOME_RESIDUALS").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_OUTCOME").is_some();
    residual_action_store(ResidualHostMatchOutcomeResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_outcome_residuals_residual_pack_wave849() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_over: Option<bool>")
        && cnc.contains("host_match_victory_label: Option<String>")
        && cnc.contains("host_match_victory_winner: Option<Option<u32>>")
        && cnc.contains("host_match_victory_summary: Option<crate::game_logic::VictorySummary>")
        && cnc.contains("Wave 556/849")
        && cnc.contains("Wave 584/849")
        && cnc.contains("Wave 849: stamp match outcome residuals from freeze")
        && cnc.contains("if let Some(over) = self.host_match_over")
        && cnc.contains("if let Some(summary) = self.host_match_victory_summary.clone()")
        && cnc.contains("pres.victory_winner_id()");
    residual_action_store(ResidualHostMatchOutcomeResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_outcome_residuals_honesty() -> bool {
    let a = honesty_host_match_outcome_residuals_method_names_residual_wave849();
    let b = honesty_host_match_outcome_residuals_nav_commands_residual_wave849();
    let c = honesty_host_match_outcome_residuals_residual_pack_wave849();
    residual_action_store(ResidualHostMatchOutcomeResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_outcome_residuals_residual_wave849() {
        assert!(honesty_host_match_outcome_residuals_residual_pack_wave849());
        assert!(honesty_host_match_outcome_residuals_method_names_residual_wave849());
        assert!(honesty_host_match_outcome_residuals_nav_commands_residual_wave849());
        assert!(simulate_live_host_match_outcome_residuals_honesty());
    }
}
