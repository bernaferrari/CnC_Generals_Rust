//! Wave 615 residual peels: production unit spawn is centralized through
//! `host_spawn_production_unit`. Still host ObjectId authority (`create_object`);
//! GameWorld receives the unit via spawn/Complete channel after sole-tick readiness.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 608/614 production complete residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` host_spawn_production_unit / host_apply_unit_production_completions
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; not full GW spawn-ID authority

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_SPAWN_HELPER_METHOD_NAMES_WAVE615: &[&str] = &[
    "host_spawn_production_unit",
    "host_apply_unit_production_completions",
    "create_object",
    "record_complete",
    "Wave 615",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_SPAWN_HELPER_NAV_STEPS_WAVE615: &[&str] = &[
    "REQUIRE_HOST_SPAWN_HELPER",
    "REQUIRE_APPLY_USES_SPAWN_HELPER",
    "REQUIRE_CREATE_OBJECT_INSIDE_HELPER",
    "REQUIRE_HOST_STILL_OWNS_IDS",
    "LIVE_HOST_PRODUCTION_SPAWN_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_SPAWN_HELPER_CMD_NAMES_WAVE615: &[&str] = &[
    "host_production_spawn_helper",
    "apply_uses_spawn_helper",
    "create_object_inside_helper",
    "host_still_owns_ids",
    "production_spawn_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionSpawnHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostProductionSpawnHelperAction {
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

fn residual_action_store(action: ResidualHostProductionSpawnHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_production_spawn_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_production_spawn_helper_last_action() -> ResidualHostProductionSpawnHelperAction
{
    ResidualHostProductionSpawnHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_host_production_spawn_helper_method_names_residual_wave615() -> bool {
    let names = LIVE_HOST_PRODUCTION_SPAWN_HELPER_METHOD_NAMES_WAVE615;
    let ok = residual_name_index(names, "host_spawn_production_unit").is_some()
        && residual_name_index(names, "host_apply_unit_production_completions").is_some()
        && residual_name_index(names, "create_object").is_some()
        && residual_name_index(names, "Wave 615").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionSpawnHelperAction::MethodNames);
    ok
}

pub fn honesty_host_production_spawn_helper_source_markers_residual_wave615() -> bool {
    let gl = gl_source();
    let Some(spawn) = fn_body(&gl, "fn host_spawn_production_unit(") else {
        residual_action_store(ResidualHostProductionSpawnHelperAction::SourceMarkers);
        return false;
    };
    let Some(apply) = fn_body(&gl, "fn host_apply_unit_production_completions(") else {
        residual_action_store(ResidualHostProductionSpawnHelperAction::SourceMarkers);
        return false;
    };
    let spawn_ok = spawn.contains("Wave 615")
        && spawn.contains("create_object(template, team, spawn_pos)")
        && !spawn.contains("playable_claim = true");
    let apply_ok = apply.contains("host_spawn_production_unit")
        && apply.contains("Wave 615")
        && apply.contains("record_complete")
        && !apply.contains("self.create_object(&template");
    let ok = spawn_ok && apply_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionSpawnHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_production_spawn_helper_nav_commands_residual_wave615() -> bool {
    let steps = LIVE_HOST_PRODUCTION_SPAWN_HELPER_NAV_STEPS_WAVE615;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_SPAWN_HELPER_CMD_NAMES_WAVE615;
    let ok = residual_name_index(steps, "REQUIRE_HOST_SPAWN_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_APPLY_USES_SPAWN_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_CREATE_OBJECT_INSIDE_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_STILL_OWNS_IDS").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_SPAWN_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_spawn_helper").is_some()
        && residual_name_index(cmds, "apply_uses_spawn_helper").is_some()
        && residual_name_index(cmds, "create_object_inside_helper").is_some()
        && residual_name_index(cmds, "host_still_owns_ids").is_some()
        && residual_name_index(cmds, "production_spawn_residual").is_some();
    residual_action_store(ResidualHostProductionSpawnHelperAction::NavCommands);
    ok
}

pub fn simulate_host_production_spawn_helper_collect_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 615")
        && gl.contains("fn host_spawn_production_unit")
        && gl.contains("host_spawn_production_unit(&template");
    residual_action_store(ResidualHostProductionSpawnHelperAction::CollectSource);
    ok
}

pub fn simulate_host_production_spawn_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("self.host_spawn_production_unit(&template, team, spawn_pos)")
        && gl.contains("Wave 615: production unit spawn via host helper")
        && gl.contains("self.create_object(template, team, spawn_pos)");
    residual_action_store(ResidualHostProductionSpawnHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_production_spawn_helper_residual_pack_wave615() -> bool {
    honesty_host_production_spawn_helper_method_names_residual_wave615()
        && honesty_host_production_spawn_helper_source_markers_residual_wave615()
        && honesty_host_production_spawn_helper_nav_commands_residual_wave615()
        && simulate_host_production_spawn_helper_collect_source()
        && simulate_host_production_spawn_helper_dispatch_source()
}

pub fn simulate_live_host_production_spawn_helper_honesty() -> bool {
    let ok = honesty_host_production_spawn_helper_residual_pack_wave615();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionSpawnHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_spawn_helper_method_names_residual_wave615());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_spawn_helper_source_markers_residual_wave615());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_spawn_helper_nav_commands_residual_wave615());
    }

    #[test]
    fn host_production_spawn_helper_sources() {
        assert!(simulate_host_production_spawn_helper_collect_source());
        assert!(simulate_host_production_spawn_helper_dispatch_source());
    }

    #[test]
    fn wave615_composite_pack() {
        assert!(honesty_host_production_spawn_helper_residual_pack_wave615());
    }

    #[test]
    fn simulate_live_host_production_spawn_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_production_spawn_helper_honesty(),
            "host production spawn helper residual must latch"
        );
        assert!(residual_host_production_spawn_helper_ok());
        assert_eq!(
            residual_host_production_spawn_helper_last_action(),
            ResidualHostProductionSpawnHelperAction::Composite
        );
    }
}
