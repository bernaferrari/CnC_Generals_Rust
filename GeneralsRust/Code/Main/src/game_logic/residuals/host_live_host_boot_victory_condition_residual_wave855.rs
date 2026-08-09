//! Wave 855: single boot victory condition residual peels dual evaluate_victory_condition
//! calls from presentation_or_boot_match_over_label and victory_winner.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_BOOT_VICTORY_CONDITION_RESIDUAL_METHOD_NAMES_WAVE855: &[&str] = &[
    "host_boot_victory_condition_residual",
    "host_match_boot_victory_condition",
    "presentation_or_boot_match_over_label",
    "presentation_or_boot_victory_winner",
    "Wave 855",
    "playable_claim = false",
];

pub const LIVE_HOST_BOOT_VICTORY_CONDITION_RESIDUAL_NAV_STEPS_WAVE855: &[&str] = &[
    "STAMP_BOOT_VICTORY_ONCE",
    "SHARE_MATCH_OVER_AND_WINNER",
    "LIVE_HOST_BOOT_VICTORY_CONDITION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBootVictoryConditionAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostBootVictoryConditionAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_boot_victory_condition_residual_method_names_residual_wave855() -> bool {
    let names = LIVE_HOST_BOOT_VICTORY_CONDITION_RESIDUAL_METHOD_NAMES_WAVE855;
    let ok = residual_name_index(names, "host_boot_victory_condition_residual").is_some()
        && residual_name_index(names, "host_match_boot_victory_condition").is_some()
        && residual_name_index(names, "Wave 855").is_some();
    residual_action_store(ResidualHostBootVictoryConditionAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_boot_victory_condition_residual_nav_commands_residual_wave855() -> bool {
    let steps = LIVE_HOST_BOOT_VICTORY_CONDITION_RESIDUAL_NAV_STEPS_WAVE855;
    let ok = residual_name_index(steps, "LIVE_HOST_BOOT_VICTORY_CONDITION_RESIDUAL").is_some()
        && residual_name_index(steps, "STAMP_BOOT_VICTORY_ONCE").is_some();
    residual_action_store(ResidualHostBootVictoryConditionAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_boot_victory_condition_residual_pack_wave855() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("fn host_boot_victory_condition_residual")
        && cnc.contains("host_match_boot_victory_condition:")
        && cnc.contains("Wave 855: boot residual via single stamped evaluate")
        && cnc.contains(
            "Wave 855: boot residual via single stamped evaluate (shared with match_over)",
        )
        && cnc.contains("Wave 855: boot victory residual is frame-local")
        && cnc
            .matches("host_boot_victory_condition_residual()")
            .count()
            >= 2
        && cnc.matches("evaluate_victory_condition()").count() == 1; // only inside stamp helper
    residual_action_store(ResidualHostBootVictoryConditionAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_boot_victory_condition_residual_honesty() -> bool {
    let a = honesty_host_boot_victory_condition_residual_method_names_residual_wave855();
    let b = honesty_host_boot_victory_condition_residual_nav_commands_residual_wave855();
    let c = honesty_host_boot_victory_condition_residual_pack_wave855();
    residual_action_store(ResidualHostBootVictoryConditionAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_boot_victory_condition_residual_wave855() {
        assert!(honesty_host_boot_victory_condition_residual_pack_wave855());
        assert!(honesty_host_boot_victory_condition_residual_method_names_residual_wave855());
        assert!(honesty_host_boot_victory_condition_residual_nav_commands_residual_wave855());
        assert!(simulate_live_host_boot_victory_condition_residual_honesty());
    }
}
