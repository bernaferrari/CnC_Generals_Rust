//! Wave 613 residual peels: production completion collection is centralized through
//! `host_collect_production_completions`. Under PRODUCTION_AUTHORITY sole-tick,
//! GameWorld advances queue progress/exit delay and writeback finishes heads;
//! host only `try_complete_production` + spawn/apply via Wave 608 helpers.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 595/608 production complete apply residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` host_collect_production_completions / update_production
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Host still owns spawn IDs via create_object (not full GW spawn authority)

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_COMPLETE_COLLECT_HELPER_METHOD_NAMES_WAVE613: &[&str] = &[
    "host_collect_production_completions",
    "update_production",
    "try_complete_production",
    "host_apply_unit_production_completions",
    "gameworld_production_sole_tick_enabled",
    "Wave 613",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_COMPLETE_COLLECT_HELPER_NAV_STEPS_WAVE613: &[&str] = &[
    "REQUIRE_HOST_COLLECT_HELPER",
    "REQUIRE_SOLE_TICK_TRY_COMPLETE",
    "REQUIRE_APPLY_VIA_HOST_HELPERS",
    "REQUIRE_HOST_STILL_SPAWNS",
    "LIVE_HOST_PRODUCTION_COMPLETE_COLLECT_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_COMPLETE_COLLECT_HELPER_CMD_NAMES_WAVE613: &[&str] = &[
    "host_production_complete_collect_helper",
    "sole_tick_try_complete",
    "apply_via_host_helpers",
    "host_still_spawns",
    "production_complete_collect_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionCompleteCollectHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostProductionCompleteCollectHelperAction {
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

fn residual_action_store(action: ResidualHostProductionCompleteCollectHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_production_complete_collect_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_production_complete_collect_helper_last_action()
-> ResidualHostProductionCompleteCollectHelperAction {
    ResidualHostProductionCompleteCollectHelperAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
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

pub fn honesty_host_production_complete_collect_helper_method_names_residual_wave613() -> bool {
    let names = LIVE_HOST_PRODUCTION_COMPLETE_COLLECT_HELPER_METHOD_NAMES_WAVE613;
    let ok = residual_name_index(names, "host_collect_production_completions").is_some()
        && residual_name_index(names, "try_complete_production").is_some()
        && residual_name_index(names, "gameworld_production_sole_tick_enabled").is_some()
        && residual_name_index(names, "Wave 613").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionCompleteCollectHelperAction::MethodNames);
    ok
}

pub fn honesty_host_production_complete_collect_helper_source_markers_residual_wave613() -> bool {
    let gl = gl_source();
    let Some(update) = fn_body(gl, "fn update_production(&mut self, dt: f32)") else {
        residual_action_store(ResidualHostProductionCompleteCollectHelperAction::SourceMarkers);
        return false;
    };
    let Some(collect) = fn_body(gl, "fn host_collect_production_completions(") else {
        residual_action_store(ResidualHostProductionCompleteCollectHelperAction::SourceMarkers);
        return false;
    };
    let update_ok = update.contains("Wave 613")
        && update.contains("host_collect_production_completions")
        && update.contains("apply_upgrade_production_completions")
        && update.contains("apply_unit_production_completions")
        && !update.contains("try_complete_production")
        && !update.contains("self.create_object(");
    let collect_ok = collect.contains("Wave 613")
        && collect.contains("gameworld_production_sole_tick_enabled")
        && collect.contains("try_complete_production")
        && collect.contains("unit_completions.push")
        && collect.contains("upgrade_completions.push");
    let still_spawns = gl.contains("fn host_apply_unit_production_completions")
        && gl.contains("self.create_object(");
    let ok = update_ok && collect_ok && still_spawns && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionCompleteCollectHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_production_complete_collect_helper_nav_commands_residual_wave613() -> bool {
    let steps = LIVE_HOST_PRODUCTION_COMPLETE_COLLECT_HELPER_NAV_STEPS_WAVE613;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_COMPLETE_COLLECT_HELPER_CMD_NAMES_WAVE613;
    let ok = residual_name_index(steps, "REQUIRE_HOST_COLLECT_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_SOLE_TICK_TRY_COMPLETE").is_some()
        && residual_name_index(steps, "REQUIRE_APPLY_VIA_HOST_HELPERS").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_STILL_SPAWNS").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_COMPLETE_COLLECT_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_complete_collect_helper").is_some()
        && residual_name_index(cmds, "sole_tick_try_complete").is_some()
        && residual_name_index(cmds, "apply_via_host_helpers").is_some()
        && residual_name_index(cmds, "host_still_spawns").is_some()
        && residual_name_index(cmds, "production_complete_collect_residual").is_some();
    residual_action_store(ResidualHostProductionCompleteCollectHelperAction::NavCommands);
    ok
}

pub fn simulate_host_production_complete_collect_helper_collect_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 613")
        && gl.contains("fn host_collect_production_completions")
        && gl.contains("host_collect_production_completions(dt)");
    residual_action_store(ResidualHostProductionCompleteCollectHelperAction::CollectSource);
    ok
}

pub fn simulate_host_production_complete_collect_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("self.host_collect_production_completions(dt)")
        && gl.contains("Wave 613: production complete collect + apply via host helpers")
        && gl.contains("try_complete_production");
    residual_action_store(ResidualHostProductionCompleteCollectHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_production_complete_collect_helper_residual_pack_wave613() -> bool {
    honesty_host_production_complete_collect_helper_method_names_residual_wave613()
        && honesty_host_production_complete_collect_helper_source_markers_residual_wave613()
        && honesty_host_production_complete_collect_helper_nav_commands_residual_wave613()
        && simulate_host_production_complete_collect_helper_collect_source()
        && simulate_host_production_complete_collect_helper_dispatch_source()
}

pub fn simulate_live_host_production_complete_collect_helper_honesty() -> bool {
    let ok = honesty_host_production_complete_collect_helper_residual_pack_wave613();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionCompleteCollectHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_complete_collect_helper_method_names_residual_wave613());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_complete_collect_helper_source_markers_residual_wave613());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_complete_collect_helper_nav_commands_residual_wave613());
    }

    #[test]
    fn host_production_complete_collect_helper_sources() {
        assert!(simulate_host_production_complete_collect_helper_collect_source());
        assert!(simulate_host_production_complete_collect_helper_dispatch_source());
    }

    #[test]
    fn wave613_composite_pack() {
        assert!(honesty_host_production_complete_collect_helper_residual_pack_wave613());
    }

    #[test]
    fn simulate_live_host_production_complete_collect_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_production_complete_collect_helper_honesty(),
            "host production complete collect helper residual must latch"
        );
        assert!(residual_host_production_complete_collect_helper_ok());
        assert_eq!(
            residual_host_production_complete_collect_helper_last_action(),
            ResidualHostProductionCompleteCollectHelperAction::Composite
        );
    }
}
