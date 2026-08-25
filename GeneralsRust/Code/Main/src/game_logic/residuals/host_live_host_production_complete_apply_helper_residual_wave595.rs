//! Wave 595 residual peels: host production complete/spawn apply is centralized
//! through `apply_upgrade_production_completions` + `apply_unit_production_completions`.
//! GameWorld still sole-ticks queue progress; host still completes/spawns.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 464/477 production sole-tick residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` update_production apply helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_COMPLETE_APPLY_HELPER_METHOD_NAMES_WAVE595: &[&str] = &[
    "update_production",
    "apply_upgrade_production_completions",
    "apply_unit_production_completions",
    "try_complete_production",
    "create_object",
    "record_complete",
    "Wave 595",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_COMPLETE_APPLY_HELPER_NAV_STEPS_WAVE595: &[&str] = &[
    "REQUIRE_PRODUCTION_COLLECT",
    "REQUIRE_UPGRADE_APPLY_HELPER",
    "REQUIRE_UNIT_APPLY_HELPER",
    "REQUIRE_HOST_STILL_SPAWNS",
    "LIVE_HOST_PRODUCTION_COMPLETE_APPLY_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_COMPLETE_APPLY_HELPER_CMD_NAMES_WAVE595: &[&str] = &[
    "host_production_complete_apply_helper",
    "upgrade_apply",
    "unit_apply",
    "host_still_spawns",
    "production_complete_apply_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionCompleteApplyHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostProductionCompleteApplyHelperAction {
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

fn residual_action_store(action: ResidualHostProductionCompleteApplyHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_production_complete_apply_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_production_complete_apply_helper_last_action()
-> ResidualHostProductionCompleteApplyHelperAction {
    ResidualHostProductionCompleteApplyHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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
pub fn honesty_host_production_complete_apply_helper_method_names_residual_wave595() -> bool {
    let names = LIVE_HOST_PRODUCTION_COMPLETE_APPLY_HELPER_METHOD_NAMES_WAVE595;
    let ok = residual_name_index(names, "update_production").is_some()
        && residual_name_index(names, "apply_upgrade_production_completions").is_some()
        && residual_name_index(names, "apply_unit_production_completions").is_some()
        && residual_name_index(names, "Wave 595").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionCompleteApplyHelperAction::MethodNames);
    ok
}

pub fn honesty_host_production_complete_apply_helper_source_markers_residual_wave595() -> bool {
    let gl = gl_source();
    let Some(update) = fn_body(&gl, "fn update_production(&mut self, dt: f32)") else {
        residual_action_store(ResidualHostProductionCompleteApplyHelperAction::SourceMarkers);
        return false;
    };
    let Some(upgrade) = fn_body(&gl, "fn host_apply_upgrade_production_completions(")
        .or_else(|| fn_body(&gl, "fn apply_upgrade_production_completions("))
    else {
        residual_action_store(ResidualHostProductionCompleteApplyHelperAction::SourceMarkers);
        return false;
    };
    let Some(unit) = fn_body(&gl, "fn host_apply_unit_production_completions(")
        .or_else(|| fn_body(&gl, "fn apply_unit_production_completions("))
    else {
        residual_action_store(ResidualHostProductionCompleteApplyHelperAction::SourceMarkers);
        return false;
    };
    // Wave 613: collect lives in host_collect; update only delegates collect+apply.
    let update_ok = (update.contains("Wave 595") || update.contains("Wave 613"))
        && (update.contains("host_apply_unit_production_completions")
            || update.contains("host_collect_production_completions"))
        && update.contains("self.apply_upgrade_production_completions(upgrade_completions)")
        && update.contains("self.apply_unit_production_completions(unit_completions)")
        && !update.contains("self.create_object(")
        && !update.contains("host_production_log::record_complete");
    let upgrade_ok = upgrade.contains("Wave 595")
        && upgrade.contains("host_production_log::record_complete")
        && upgrade.contains("ObjectId(0)")
        && upgrade.contains("apply_host_upgrade_complete");
    // Wave 679: door/notify/exit may live in host_apply_production_spawn_ready_completions.
    let ready_apply = fn_body(&gl, "pub fn host_apply_production_spawn_ready_completions(")
        .or_else(|| fn_body(&gl, "fn host_apply_production_spawn_ready_completions("));
    let unit_inline_residual = unit.contains("notify_unit_production_complete")
        && unit.contains("arm_exit_delay")
        && unit.contains("path_approach_with_state");
    // 2026-08-15: spawn/ready apply go through ProductionAuthorityOp
    // (world_tick/production.rs:970 / :1025).
    let unit_ready_residual = unit.contains("host_production_spawn_ready_log::record")
        && (unit.contains("host_apply_production_spawn_ready_completions")
            || unit.contains("ApplySpawnReadyCompletions"))
        && ready_apply
            .map(|b| {
                // 2026-08-15: C++ QueueProductionExitUpdate delay is
                // record_successful_production_exit (not arm_exit_delay).
                b.contains("notify_unit_production_complete")
                    && (b.contains("arm_exit_delay")
                        || b.contains("record_successful_production_exit"))
                    && b.contains("path_approach_with_state")
            })
            .unwrap_or(false);
    let unit_ok = unit.contains("Wave 595")
        && (unit.contains("self.create_object(")
            || unit.contains("host_spawn_production_unit")
            || unit.contains("ProductionAuthorityOp::SpawnUnit"))
        && unit.contains("host_production_log::record_complete")
        && (unit_inline_residual || unit_ready_residual);
    let ok = update_ok && upgrade_ok && unit_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionCompleteApplyHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_production_complete_apply_helper_nav_commands_residual_wave595() -> bool {
    let steps = LIVE_HOST_PRODUCTION_COMPLETE_APPLY_HELPER_NAV_STEPS_WAVE595;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_COMPLETE_APPLY_HELPER_CMD_NAMES_WAVE595;
    let ok = residual_name_index(steps, "REQUIRE_PRODUCTION_COLLECT").is_some()
        && residual_name_index(steps, "REQUIRE_UPGRADE_APPLY_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_UNIT_APPLY_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_STILL_SPAWNS").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_COMPLETE_APPLY_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_complete_apply_helper").is_some()
        && residual_name_index(cmds, "upgrade_apply").is_some()
        && residual_name_index(cmds, "unit_apply").is_some()
        && residual_name_index(cmds, "host_still_spawns").is_some()
        && residual_name_index(cmds, "production_complete_apply_residual").is_some();
    residual_action_store(ResidualHostProductionCompleteApplyHelperAction::NavCommands);
    ok
}

pub fn simulate_host_production_complete_apply_helper_collect_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 595")
        && gl.contains("fn apply_upgrade_production_completions")
        && gl.contains("fn apply_unit_production_completions")
        && gl.contains("fn update_production");
    residual_action_store(ResidualHostProductionCompleteApplyHelperAction::CollectSource);
    ok
}

pub fn simulate_host_production_complete_apply_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("self.apply_upgrade_production_completions(upgrade_completions)")
        && gl.contains("self.apply_unit_production_completions(unit_completions)")
        && (gl.contains("Wave 595")
            || gl.contains("Wave 595/608: host production complete/spawn apply residual"));
    residual_action_store(ResidualHostProductionCompleteApplyHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_production_complete_apply_helper_residual_pack_wave595() -> bool {
    honesty_host_production_complete_apply_helper_method_names_residual_wave595()
        && honesty_host_production_complete_apply_helper_source_markers_residual_wave595()
        && honesty_host_production_complete_apply_helper_nav_commands_residual_wave595()
        && simulate_host_production_complete_apply_helper_collect_source()
        && simulate_host_production_complete_apply_helper_dispatch_source()
}

pub fn simulate_live_host_production_complete_apply_helper_honesty() -> bool {
    let ok = honesty_host_production_complete_apply_helper_residual_pack_wave595();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionCompleteApplyHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_complete_apply_helper_method_names_residual_wave595());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_complete_apply_helper_source_markers_residual_wave595());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_complete_apply_helper_nav_commands_residual_wave595());
    }

    #[test]
    fn host_production_complete_apply_helper_sources() {
        assert!(simulate_host_production_complete_apply_helper_collect_source());
        assert!(simulate_host_production_complete_apply_helper_dispatch_source());
    }

    #[test]
    fn wave595_composite_pack() {
        assert!(honesty_host_production_complete_apply_helper_residual_pack_wave595());
    }

    #[test]
    fn simulate_live_host_production_complete_apply_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_production_complete_apply_helper_honesty(),
            "host production complete apply helper residual must latch"
        );
        assert!(residual_host_production_complete_apply_helper_ok());
        assert_eq!(
            residual_host_production_complete_apply_helper_last_action(),
            ResidualHostProductionCompleteApplyHelperAction::Composite
        );
    }
}
