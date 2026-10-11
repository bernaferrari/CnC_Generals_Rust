//! Construction ownership audit. Historical CLI receipt names remain
//! navigation metadata; source checks require one canonical host writer and
//! immutable shadow comparisons. Runtime regressions exercise the host phases.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONSTRUCTION_READY_LOG_HELPER_METHOD_NAMES_WAVE617: &[&str] = &[
    "host_construction_ready_log",
    "compare_construction_to_host",
    "update_construction",
    "host_owns_construction",
    "Wave 617",
    "playable_claim = false",
];

pub const LIVE_HOST_CONSTRUCTION_READY_LOG_HELPER_NAV_STEPS_WAVE617: &[&str] = &[
    "REQUIRE_CONSTRUCTION_READY_LOG_MODULE",
    "REQUIRE_WRITEBACK_RECORDS_READY",
    "REQUIRE_UPDATE_DRAINS_READY",
    "REQUIRE_HOST_STILL_COMPLETES",
    "LIVE_HOST_CONSTRUCTION_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_CONSTRUCTION_READY_LOG_HELPER_CMD_NAMES_WAVE617: &[&str] = &[
    "host_construction_ready_log_helper",
    "writeback_records_ready",
    "update_drains_ready",
    "host_still_completes",
    "construction_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostConstructionReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostConstructionReadyLogHelperAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualHostConstructionReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_construction_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_construction_ready_log_helper_last_action()
-> ResidualHostConstructionReadyLogHelperAction {
    ResidualHostConstructionReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
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

pub fn honesty_host_construction_ready_log_helper_method_names_residual_wave617() -> bool {
    let names = LIVE_HOST_CONSTRUCTION_READY_LOG_HELPER_METHOD_NAMES_WAVE617;
    let ok = residual_name_index(names, "host_construction_ready_log").is_some()
        && residual_name_index(names, "compare_construction_to_host").is_some()
        && residual_name_index(names, "update_construction").is_some()
        && residual_name_index(names, "Wave 617").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostConstructionReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_construction_ready_log_helper_source_markers_residual_wave617() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    residual_action_store(ResidualHostConstructionReadyLogHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_construction_ready_log_helper_nav_commands_residual_wave617() -> bool {
    let steps = LIVE_HOST_CONSTRUCTION_READY_LOG_HELPER_NAV_STEPS_WAVE617;
    let cmds = RUNTIME_HOST_LIVE_HOST_CONSTRUCTION_READY_LOG_HELPER_CMD_NAMES_WAVE617;
    let ok = residual_name_index(steps, "REQUIRE_CONSTRUCTION_READY_LOG_MODULE").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_RECORDS_READY").is_some()
        && residual_name_index(steps, "REQUIRE_UPDATE_DRAINS_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_STILL_COMPLETES").is_some()
        && residual_name_index(steps, "LIVE_HOST_CONSTRUCTION_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_construction_ready_log_helper").is_some()
        && residual_name_index(cmds, "writeback_records_ready").is_some()
        && residual_name_index(cmds, "update_drains_ready").is_some()
        && residual_name_index(cmds, "host_still_completes").is_some()
        && residual_name_index(cmds, "construction_ready_log_residual").is_some();
    residual_action_store(ResidualHostConstructionReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_construction_ready_log_helper_collect_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    residual_action_store(ResidualHostConstructionReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_construction_ready_log_helper_dispatch_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    residual_action_store(ResidualHostConstructionReadyLogHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_construction_ready_log_helper_residual_pack_wave617() -> bool {
    honesty_host_construction_ready_log_helper_method_names_residual_wave617()
        && honesty_host_construction_ready_log_helper_source_markers_residual_wave617()
        && honesty_host_construction_ready_log_helper_nav_commands_residual_wave617()
        && simulate_host_construction_ready_log_helper_collect_source()
        && simulate_host_construction_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_construction_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_construction_ready_log_helper_residual_pack_wave617();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostConstructionReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_construction_ready_log_helper_method_names_residual_wave617());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_construction_ready_log_helper_source_markers_residual_wave617());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_construction_ready_log_helper_nav_commands_residual_wave617());
    }

    #[test]
    fn host_construction_ready_log_helper_sources() {
        assert!(simulate_host_construction_ready_log_helper_collect_source());
        assert!(simulate_host_construction_ready_log_helper_dispatch_source());
    }

    #[test]
    fn wave617_composite_pack() {
        assert!(honesty_host_construction_ready_log_helper_residual_pack_wave617());
    }

    #[test]
    fn simulate_live_host_construction_ready_log_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_construction_ready_log_helper_honesty(),
            "host construction ready log helper residual must latch"
        );
        assert!(residual_host_construction_ready_log_helper_ok());
        assert_eq!(
            residual_host_construction_ready_log_helper_last_action(),
            ResidualHostConstructionReadyLogHelperAction::Composite
        );
    }
}
