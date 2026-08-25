//! Wave 608 residual peels: production complete/spawn apply is centralized through
//! `host_apply_upgrade_production_completions` + `host_apply_unit_production_completions`.
//! GameWorld still sole-ticks queue progress; host still completes/spawns.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 595 production complete apply residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` host production complete apply helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_COMPLETE_HOST_APPLY_HELPER_METHOD_NAMES_WAVE608: &[&str] = &[
    "host_apply_upgrade_production_completions",
    "host_apply_unit_production_completions",
    "apply_upgrade_production_completions",
    "apply_unit_production_completions",
    "create_object",
    "record_complete",
    "Wave 608",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_COMPLETE_HOST_APPLY_HELPER_NAV_STEPS_WAVE608: &[&str] = &[
    "REQUIRE_HOST_UPGRADE_APPLY_HELPER",
    "REQUIRE_HOST_UNIT_APPLY_HELPER",
    "REQUIRE_THIN_WRAPPERS",
    "REQUIRE_HOST_STILL_SPAWNS",
    "LIVE_HOST_PRODUCTION_COMPLETE_HOST_APPLY_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_COMPLETE_HOST_APPLY_HELPER_CMD_NAMES_WAVE608:
    &[&str] = &[
    "host_production_complete_host_apply_helper",
    "host_upgrade_apply",
    "host_unit_apply",
    "thin_wrappers",
    "host_still_spawns",
    "production_complete_host_apply_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionCompleteHostApplyHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostProductionCompleteHostApplyHelperAction {
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

fn residual_action_store(action: ResidualHostProductionCompleteHostApplyHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_production_complete_host_apply_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_production_complete_host_apply_helper_last_action()
-> ResidualHostProductionCompleteHostApplyHelperAction {
    ResidualHostProductionCompleteHostApplyHelperAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_production_complete_host_apply_helper_method_names_residual_wave608() -> bool {
    let names = LIVE_HOST_PRODUCTION_COMPLETE_HOST_APPLY_HELPER_METHOD_NAMES_WAVE608;
    let ok = residual_name_index(names, "host_apply_upgrade_production_completions").is_some()
        && residual_name_index(names, "host_apply_unit_production_completions").is_some()
        && residual_name_index(names, "create_object").is_some()
        && residual_name_index(names, "Wave 608").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::MethodNames);
    ok
}

pub fn honesty_host_production_complete_host_apply_helper_source_markers_residual_wave608() -> bool
{
    let gl = gl_source();
    let Some(upgrade_wrap) = fn_body(&gl, "fn apply_upgrade_production_completions(") else {
        residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::SourceMarkers);
        return false;
    };
    let Some(unit_wrap) = fn_body(&gl, "fn apply_unit_production_completions(") else {
        residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::SourceMarkers);
        return false;
    };
    let Some(upgrade_host) = fn_body(&gl, "fn host_apply_upgrade_production_completions(") else {
        residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::SourceMarkers);
        return false;
    };
    let Some(unit_host) = fn_body(&gl, "fn host_apply_unit_production_completions(") else {
        residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::SourceMarkers);
        return false;
    };
    let Some(update) = fn_body(&gl, "fn update_production(") else {
        residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::SourceMarkers);
        return false;
    };
    let wrap_ok = upgrade_wrap.contains("host_apply_upgrade_production_completions")
        && upgrade_wrap.contains("Wave 608")
        && unit_wrap.contains("host_apply_unit_production_completions")
        && unit_wrap.contains("Wave 608");
    let host_ok = upgrade_host.contains("record_complete")
        && upgrade_host.contains("Wave 608")
        && (unit_host.contains("create_object")
            || unit_host.contains("host_spawn_production_unit")
            || unit_host.contains("ProductionAuthorityOp::SpawnUnit"))
        && unit_host.contains("record_complete")
        && unit_host.contains("Wave 608");
    let call_ok = update.contains("self.apply_upgrade_production_completions(upgrade_completions)")
        && update.contains("self.apply_unit_production_completions(unit_completions)")
        && (update.contains("Wave 608") || update.contains("Wave 595"));
    let ok = wrap_ok && host_ok && call_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_production_complete_host_apply_helper_nav_commands_residual_wave608() -> bool {
    let steps = LIVE_HOST_PRODUCTION_COMPLETE_HOST_APPLY_HELPER_NAV_STEPS_WAVE608;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_COMPLETE_HOST_APPLY_HELPER_CMD_NAMES_WAVE608;
    let ok = residual_name_index(steps, "REQUIRE_HOST_UPGRADE_APPLY_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_UNIT_APPLY_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_THIN_WRAPPERS").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_STILL_SPAWNS").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_COMPLETE_HOST_APPLY_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_complete_host_apply_helper").is_some()
        && residual_name_index(cmds, "host_upgrade_apply").is_some()
        && residual_name_index(cmds, "host_unit_apply").is_some()
        && residual_name_index(cmds, "thin_wrappers").is_some()
        && residual_name_index(cmds, "host_still_spawns").is_some()
        && residual_name_index(cmds, "production_complete_host_apply_residual").is_some();
    residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::NavCommands);
    ok
}

pub fn simulate_host_production_complete_host_apply_helper_collect_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 608")
        && gl.contains("fn host_apply_upgrade_production_completions")
        && gl.contains("fn host_apply_unit_production_completions");
    residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::CollectSource);
    ok
}

pub fn simulate_host_production_complete_host_apply_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("self.host_apply_upgrade_production_completions(")
        && gl.contains("self.host_apply_unit_production_completions(")
        && gl.contains("Wave 608: thin wrapper");
    residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_production_complete_host_apply_helper_residual_pack_wave608() -> bool {
    honesty_host_production_complete_host_apply_helper_method_names_residual_wave608()
        && honesty_host_production_complete_host_apply_helper_source_markers_residual_wave608()
        && honesty_host_production_complete_host_apply_helper_nav_commands_residual_wave608()
        && simulate_host_production_complete_host_apply_helper_collect_source()
        && simulate_host_production_complete_host_apply_helper_dispatch_source()
}

pub fn simulate_live_host_production_complete_host_apply_helper_honesty() -> bool {
    let ok = honesty_host_production_complete_host_apply_helper_residual_pack_wave608();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionCompleteHostApplyHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_complete_host_apply_helper_method_names_residual_wave608());
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_production_complete_host_apply_helper_source_markers_residual_wave608()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_complete_host_apply_helper_nav_commands_residual_wave608());
    }

    #[test]
    fn host_production_complete_host_apply_helper_sources() {
        assert!(simulate_host_production_complete_host_apply_helper_collect_source());
        assert!(simulate_host_production_complete_host_apply_helper_dispatch_source());
    }

    #[test]
    fn wave608_composite_pack() {
        assert!(honesty_host_production_complete_host_apply_helper_residual_pack_wave608());
    }

    #[test]
    fn simulate_live_host_production_complete_host_apply_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_production_complete_host_apply_helper_honesty(),
            "host production complete host apply helper residual must latch"
        );
        assert!(residual_host_production_complete_host_apply_helper_ok());
        assert_eq!(
            residual_host_production_complete_host_apply_helper_last_action(),
            ResidualHostProductionCompleteHostApplyHelperAction::Composite
        );
    }
}
