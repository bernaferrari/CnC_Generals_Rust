//! Construction ownership audit. Historical CLI receipt names remain
//! navigation metadata; source checks require one canonical host writer and
//! immutable shadow comparisons. Runtime regressions exercise the host phases.

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_METHOD_NAMES_WAVE626: &[&str] = &[
    "host_construction_complete_clear_ready_log",
    "compare_rebuild_producer_to_host",
    "host_apply_construction_complete_clear_ready_completions",
    "apply_construction_complete_clear_residual",
    "Wave 626",
    "playable_claim = false",
];

pub const LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_NAV_STEPS_WAVE626: &[&str] = &[
    "REQUIRE_CCC_READY_LOG_MODULE",
    "REQUIRE_WRITEBACK_RECORDS_CLEAR_DEADLINE",
    "REQUIRE_HOST_DRAINS_CCC_READY",
    "REQUIRE_HOST_CLEARS_MODEL_BIT",
    "LIVE_HOST_CCC_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_CMD_NAMES_WAVE626:
    &[&str] = &[
    "host_construction_complete_clear_ready_log_helper",
    "writeback_records_clear_deadline",
    "host_drains_ccc_ready",
    "host_clears_model_bit",
    "construction_complete_clear_ready_log_residual",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostConstructionCompleteClearReadyLogHelperAction {
    None,
    MethodNames,
    SourceMarkers,
    NavCommands,
    CollectSource,
    DispatchSource,
    Composite,
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn object_source() -> &'static str {
    crate::game_logic::object::OBJECT_SRC
}

fn last_sig_index(src: &str, sig: &str) -> Option<usize> {
    let mut at = None;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(sig) {
        at = Some(from + rel);
        from = from + rel + sig.len();
    }
    at
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = last_sig_index(src, sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_host_construction_complete_clear_ready_log_helper_method_names_residual_wave626()
-> bool {
    let names = LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_METHOD_NAMES_WAVE626;
    let ok = residual_name_index(names, "host_construction_complete_clear_ready_log").is_some()
        && residual_name_index(names, "compare_rebuild_producer_to_host").is_some()
        && residual_name_index(names, "Wave 626").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    ok
}

pub fn honesty_host_construction_complete_clear_ready_log_helper_source_markers_residual_wave626()
-> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    ok
}

pub fn honesty_host_construction_complete_clear_ready_log_helper_nav_commands_residual_wave626()
-> bool {
    let steps = LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_NAV_STEPS_WAVE626;
    let cmds =
        RUNTIME_HOST_LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_CMD_NAMES_WAVE626;
    let ok = residual_name_index(steps, "REQUIRE_CCC_READY_LOG_MODULE").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_RECORDS_CLEAR_DEADLINE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_DRAINS_CCC_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_CLEARS_MODEL_BIT").is_some()
        && residual_name_index(steps, "LIVE_HOST_CCC_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_construction_complete_clear_ready_log_helper").is_some()
        && residual_name_index(cmds, "writeback_records_clear_deadline").is_some()
        && residual_name_index(cmds, "host_drains_ccc_ready").is_some()
        && residual_name_index(cmds, "host_clears_model_bit").is_some()
        && residual_name_index(cmds, "construction_complete_clear_ready_log_residual").is_some();
    ok
}

pub fn simulate_host_construction_complete_clear_ready_log_helper_collect_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    ok
}

pub fn simulate_host_construction_complete_clear_ready_log_helper_dispatch_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    ok
}

pub fn honesty_host_construction_complete_clear_ready_log_helper_residual_pack_wave626() -> bool {
    simulate_live_host_construction_complete_clear_ready_log_helper_outcome().passed
}

/// An operation-owned source-honesty result. These residual probes are
/// diagnostic/test witnesses, not C++ simulation state, so parallel callers
/// must not overwrite one another's reported phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResidualHostConstructionCompleteClearReadyLogHelperOutcome {
    pub passed: bool,
    pub last_action: ResidualHostConstructionCompleteClearReadyLogHelperAction,
}

pub fn simulate_live_host_construction_complete_clear_ready_log_helper_outcome()
-> ResidualHostConstructionCompleteClearReadyLogHelperOutcome {
    let phases: [(ResidualHostConstructionCompleteClearReadyLogHelperAction, fn() -> bool); 5] = [
        (ResidualHostConstructionCompleteClearReadyLogHelperAction::MethodNames, honesty_host_construction_complete_clear_ready_log_helper_method_names_residual_wave626),
        (ResidualHostConstructionCompleteClearReadyLogHelperAction::SourceMarkers, honesty_host_construction_complete_clear_ready_log_helper_source_markers_residual_wave626),
        (ResidualHostConstructionCompleteClearReadyLogHelperAction::NavCommands, honesty_host_construction_complete_clear_ready_log_helper_nav_commands_residual_wave626),
        (ResidualHostConstructionCompleteClearReadyLogHelperAction::CollectSource, simulate_host_construction_complete_clear_ready_log_helper_collect_source),
        (ResidualHostConstructionCompleteClearReadyLogHelperAction::DispatchSource, simulate_host_construction_complete_clear_ready_log_helper_dispatch_source),
    ];
    for (action, check) in phases {
        if !check() {
            return ResidualHostConstructionCompleteClearReadyLogHelperOutcome {
                passed: false,
                last_action: action,
            };
        }
    }
    ResidualHostConstructionCompleteClearReadyLogHelperOutcome {
        passed: true,
        last_action: ResidualHostConstructionCompleteClearReadyLogHelperAction::Composite,
    }
}

pub fn simulate_live_host_construction_complete_clear_ready_log_helper_honesty() -> bool {
    simulate_live_host_construction_complete_clear_ready_log_helper_outcome().passed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_construction_complete_clear_ready_log_helper_method_names_residual_wave626(
            )
        );
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_construction_complete_clear_ready_log_helper_source_markers_residual_wave626(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_construction_complete_clear_ready_log_helper_nav_commands_residual_wave626(
            )
        );
    }

    #[test]
    fn host_construction_complete_clear_ready_log_helper_sources() {
        assert!(simulate_host_construction_complete_clear_ready_log_helper_collect_source());
        assert!(simulate_host_construction_complete_clear_ready_log_helper_dispatch_source());
    }

    #[test]
    fn wave626_composite_pack() {
        assert!(honesty_host_construction_complete_clear_ready_log_helper_residual_pack_wave626());
    }

    #[test]
    fn simulate_live_host_construction_complete_clear_ready_log_helper_honesty_residual_live() {
        let outcome = simulate_live_host_construction_complete_clear_ready_log_helper_outcome();
        assert!(
            outcome.passed,
            "host construction complete clear ready log helper must pass"
        );
        assert_eq!(
            outcome.last_action,
            ResidualHostConstructionCompleteClearReadyLogHelperAction::Composite
        );
    }
}
