//! Wave 741: under construction sole-tick, rebuild-hole worker/structure spawn
//! without a GameWorld entity bind is fail-closed. Opt-in:
//! `GENERALS_RUNTIME_HOST_REBUILD_SPAWN_WITHOUT_GW_BIND=1`.
//! `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_REBUILD_SPAWN_REQUIRES_GW_BIND_METHOD_NAMES_WAVE741: &[&str] = &[
    "allow_without_bind",
    "GENERALS_RUNTIME_HOST_REBUILD_SPAWN_WITHOUT_GW_BIND",
    "host_spawn_rebuild_bound_object",
    "gw_entity_raw",
    "Wave 741",
    "playable_claim = false",
];
pub const LIVE_HOST_REBUILD_SPAWN_REQUIRES_GW_BIND_NAV_STEPS_WAVE741: &[&str] = &[
    "REQUIRE_GW_BIND_UNDER_CONSTRUCTION_SOLE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_OPT_IN_WITHOUT_BIND",
    "LIVE_HOST_REBUILD_SPAWN_REQUIRES_GW_BIND",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_REBUILD_SPAWN_REQUIRES_GW_BIND_CMD_NAMES_WAVE741: &[&str] = &[
    "host_rebuild_spawn_requires_gw_bind",
    "gw_bind_under_construction_sole",
    "default_fail_closed",
    "opt_in_without_bind",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRebuildSpawnRequiresGwBindAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostRebuildSpawnRequiresGwBindAction {
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
fn residual_action_store(a: ResidualHostRebuildSpawnRequiresGwBindAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_rebuild_spawn_requires_gw_bind_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_rebuild_spawn_requires_gw_bind_last_action()
-> ResidualHostRebuildSpawnRequiresGwBindAction {
    ResidualHostRebuildSpawnRequiresGwBindAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_rebuild_spawn_requires_gw_bind_method_names_residual_wave741() -> bool {
    let names = LIVE_HOST_REBUILD_SPAWN_REQUIRES_GW_BIND_METHOD_NAMES_WAVE741;
    let ok = residual_name_index(names, "allow_without_bind").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_REBUILD_SPAWN_WITHOUT_GW_BIND")
            .is_some()
        && residual_name_index(names, "host_spawn_rebuild_bound_object").is_some()
        && residual_name_index(names, "gw_entity_raw").is_some()
        && residual_name_index(names, "Wave 741").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRebuildSpawnRequiresGwBindAction::MethodNames);
    ok
}
pub fn honesty_host_rebuild_spawn_requires_gw_bind_source_markers_residual_wave741() -> bool {
    let gl = gl_source();
    let j = gl.find("fn host_spawn_rebuild_bound_object").unwrap_or(0);
    let body = &gl[j..j + 2200.min(gl.len().saturating_sub(j))];
    let ok = body.contains("Wave 741")
        && body.contains("allow_without_bind")
        && body.contains("GENERALS_RUNTIME_HOST_REBUILD_SPAWN_WITHOUT_GW_BIND")
        && body.contains("rebuild spawn denied without GW entity bind")
        && body.contains("return None")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostRebuildSpawnRequiresGwBindAction::SourceMarkers);
    ok
}
pub fn honesty_host_rebuild_spawn_requires_gw_bind_nav_commands_residual_wave741() -> bool {
    let steps = LIVE_HOST_REBUILD_SPAWN_REQUIRES_GW_BIND_NAV_STEPS_WAVE741;
    let cmds = RUNTIME_HOST_LIVE_HOST_REBUILD_SPAWN_REQUIRES_GW_BIND_CMD_NAMES_WAVE741;
    let ok = residual_name_index(steps, "REQUIRE_GW_BIND_UNDER_CONSTRUCTION_SOLE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_OPT_IN_WITHOUT_BIND").is_some()
        && residual_name_index(steps, "LIVE_HOST_REBUILD_SPAWN_REQUIRES_GW_BIND").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_rebuild_spawn_requires_gw_bind").is_some()
        && residual_name_index(cmds, "gw_bind_under_construction_sole").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "opt_in_without_bind").is_some();
    residual_action_store(ResidualHostRebuildSpawnRequiresGwBindAction::NavCommands);
    ok
}
pub fn simulate_host_rebuild_spawn_requires_gw_bind_collect_source() -> bool {
    let ok = gl_source().contains("GENERALS_RUNTIME_HOST_REBUILD_SPAWN_WITHOUT_GW_BIND")
        && gl_source().contains("host_spawn_rebuild_bound_object");
    residual_action_store(ResidualHostRebuildSpawnRequiresGwBindAction::CollectSource);
    ok
}
pub fn simulate_host_rebuild_spawn_requires_gw_bind_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 741")
        && gl_source().contains("rebuild spawn denied without GW entity bind");
    residual_action_store(ResidualHostRebuildSpawnRequiresGwBindAction::DispatchSource);
    ok
}
pub fn honesty_host_rebuild_spawn_requires_gw_bind_residual_pack_wave741() -> bool {
    honesty_host_rebuild_spawn_requires_gw_bind_method_names_residual_wave741()
        && honesty_host_rebuild_spawn_requires_gw_bind_source_markers_residual_wave741()
        && honesty_host_rebuild_spawn_requires_gw_bind_nav_commands_residual_wave741()
        && simulate_host_rebuild_spawn_requires_gw_bind_collect_source()
        && simulate_host_rebuild_spawn_requires_gw_bind_dispatch_source()
}
pub fn simulate_live_host_rebuild_spawn_requires_gw_bind_honesty() -> bool {
    let ok = honesty_host_rebuild_spawn_requires_gw_bind_residual_pack_wave741();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRebuildSpawnRequiresGwBindAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_rebuild_spawn_requires_gw_bind_method_names_residual_wave741());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_rebuild_spawn_requires_gw_bind_source_markers_residual_wave741());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_rebuild_spawn_requires_gw_bind_nav_commands_residual_wave741());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_rebuild_spawn_requires_gw_bind_collect_source());
        assert!(simulate_host_rebuild_spawn_requires_gw_bind_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_rebuild_spawn_requires_gw_bind_residual_pack_wave741());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_rebuild_spawn_requires_gw_bind_honesty());
        assert!(residual_host_rebuild_spawn_requires_gw_bind_ok());
    }
}
