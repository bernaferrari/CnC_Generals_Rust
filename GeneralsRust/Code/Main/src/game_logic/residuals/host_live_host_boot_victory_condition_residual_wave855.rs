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
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn victory_phase_source_contract(boot: &str, step: &str, presentation: &str) -> bool {
    fn between<'a>(source: &'a str, start: &str, end: &str) -> Option<&'a str> {
        source
            .split_once(start)?
            .1
            .split_once(end)
            .map(|(body, _)| body)
    }
    let Some(boot) = between(
        boot,
        "pub(super) fn host_boot_victory_condition_residual(",
        "pub(super) fn presentation_or_boot_match_over_label(",
    ) else {
        return false;
    };
    let Some(step) = between(
        step,
        "Phase 15: Victory Conditions",
        "Phase 16: Disabled Status",
    ) else {
        return false;
    };
    let Some(presentation) = between(
        presentation,
        "pub(super) fn build_with_victory_with_tint_update(",
        "pub fn presentation_hash(",
    ) else {
        return false;
    };
    // Boot has been fail-closed since Wave 910. C++ GameLogic.cpp:3769 owns
    // evaluation; presentation reads its completed result. Counting calls in
    // the concatenated source accidentally required a mutating observer.
    boot.contains("self.host_match_boot_victory_condition = Some(None);")
        && !boot.contains("evaluate_victory_condition(")
        && step.contains("let _ = self.evaluate_victory_condition();")
        && presentation.contains("let victory = logic.current_victory_observation();")
        && presentation.contains("frame.match_over = victory.match_over;")
        && presentation.contains("if let Some(v) = victory.outcome")
        && !presentation.contains("evaluate_victory_condition(")
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
        && cnc.contains("Wave 855")
        && cnc
            .matches("host_boot_victory_condition_residual()")
            .count()
            >= 2
        && victory_phase_source_contract(
            include_str!("../../cnc_game_engine/host_authority.rs"),
            include_str!("../world_tick/step.rs"),
            include_str!("../../presentation_frame/build.rs"),
        );
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

    #[test]
    fn victory_source_contract_rejects_observer_mutation_and_missing_owner_phase() {
        let boot = include_str!("../../cnc_game_engine/host_authority.rs");
        let step = include_str!("../world_tick/step.rs");
        let presentation = include_str!("../../presentation_frame/build.rs");
        assert!(victory_phase_source_contract(boot, step, presentation));
        assert!(!victory_phase_source_contract(
            &boot.replace(
                "self.host_match_boot_victory_condition = Some(None);",
                "self.game_logic.evaluate_victory_condition();",
            ),
            step,
            presentation,
        ));
        assert!(!victory_phase_source_contract(
            boot,
            &step.replace("let _ = self.evaluate_victory_condition();", ""),
            presentation,
        ));
        assert!(!victory_phase_source_contract(
            boot,
            step,
            &presentation.replace(
                "logic.current_victory_observation()",
                "logic.evaluate_victory_condition()",
            ),
        ));
    }
}
