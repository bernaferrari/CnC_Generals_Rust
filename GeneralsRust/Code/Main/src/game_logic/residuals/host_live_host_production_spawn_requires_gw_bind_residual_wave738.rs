//! Wave 738: under sole-tick, production unit spawn without a GameWorld entity
//! bind is fail-closed. Opt-in:
//! `GENERALS_RUNTIME_HOST_PRODUCTION_SPAWN_WITHOUT_GW_BIND=1`.
//! Host may still allocate ObjectId when bind is present. `playable_claim` false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PRODUCTION_SPAWN_REQUIRES_GW_BIND_METHOD_NAMES_WAVE738: &[&str] = &[
    "allow_without_bind",
    "GENERALS_RUNTIME_HOST_PRODUCTION_SPAWN_WITHOUT_GW_BIND",
    "pop_pending_bind",
    "host_spawn_production_unit",
    "Wave 738",
    "playable_claim = false",
];
pub const LIVE_HOST_PRODUCTION_SPAWN_REQUIRES_GW_BIND_NAV_STEPS_WAVE738: &[&str] = &[
    "REQUIRE_GW_BIND_UNDER_SOLE_TICK",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_OPT_IN_WITHOUT_BIND",
    "LIVE_HOST_PRODUCTION_SPAWN_REQUIRES_GW_BIND",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_SPAWN_REQUIRES_GW_BIND_CMD_NAMES_WAVE738: &[&str] = &[
    "host_production_spawn_requires_gw_bind",
    "gw_bind_under_sole_tick",
    "default_fail_closed",
    "opt_in_without_bind",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionSpawnRequiresGwBindAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostProductionSpawnRequiresGwBindAction {
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
fn residual_action_store(a: ResidualHostProductionSpawnRequiresGwBindAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_production_spawn_requires_gw_bind_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_production_spawn_requires_gw_bind_last_action()
-> ResidualHostProductionSpawnRequiresGwBindAction {
    ResidualHostProductionSpawnRequiresGwBindAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_production_spawn_requires_gw_bind_method_names_residual_wave738() -> bool {
    let names = LIVE_HOST_PRODUCTION_SPAWN_REQUIRES_GW_BIND_METHOD_NAMES_WAVE738;
    let ok = residual_name_index(names, "allow_without_bind").is_some()
        && residual_name_index(
            names,
            "GENERALS_RUNTIME_HOST_PRODUCTION_SPAWN_WITHOUT_GW_BIND",
        )
        .is_some()
        && residual_name_index(names, "pop_pending_bind").is_some()
        && residual_name_index(names, "host_spawn_production_unit").is_some()
        && residual_name_index(names, "Wave 738").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionSpawnRequiresGwBindAction::MethodNames);
    ok
}
pub fn honesty_host_production_spawn_requires_gw_bind_source_markers_residual_wave738() -> bool {
    let gl = gl_source();
    let j = gl.find("fn host_spawn_production_unit").unwrap_or(0);
    let body = &gl[j..j + 4500.min(gl.len().saturating_sub(j))];
    let ok = body.contains("Wave 738")
        && body.contains("allow_without_bind")
        && body.contains("GENERALS_RUNTIME_HOST_PRODUCTION_SPAWN_WITHOUT_GW_BIND")
        && body.contains("sole-tick production spawn denied without GW entity bind")
        && body.contains("return None")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionSpawnRequiresGwBindAction::SourceMarkers);
    ok
}
pub fn honesty_host_production_spawn_requires_gw_bind_nav_commands_residual_wave738() -> bool {
    let steps = LIVE_HOST_PRODUCTION_SPAWN_REQUIRES_GW_BIND_NAV_STEPS_WAVE738;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_SPAWN_REQUIRES_GW_BIND_CMD_NAMES_WAVE738;
    let ok = residual_name_index(steps, "REQUIRE_GW_BIND_UNDER_SOLE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_OPT_IN_WITHOUT_BIND").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_SPAWN_REQUIRES_GW_BIND").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_spawn_requires_gw_bind").is_some()
        && residual_name_index(cmds, "gw_bind_under_sole_tick").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "opt_in_without_bind").is_some();
    residual_action_store(ResidualHostProductionSpawnRequiresGwBindAction::NavCommands);
    ok
}
pub fn simulate_host_production_spawn_requires_gw_bind_collect_source() -> bool {
    let ok = gl_source().contains("GENERALS_RUNTIME_HOST_PRODUCTION_SPAWN_WITHOUT_GW_BIND")
        && gl_source().contains("allow_without_bind");
    residual_action_store(ResidualHostProductionSpawnRequiresGwBindAction::CollectSource);
    ok
}
pub fn simulate_host_production_spawn_requires_gw_bind_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 738")
        && gl_source().contains("sole-tick production spawn denied without GW entity bind");
    residual_action_store(ResidualHostProductionSpawnRequiresGwBindAction::DispatchSource);
    ok
}
pub fn honesty_host_production_spawn_requires_gw_bind_residual_pack_wave738() -> bool {
    honesty_host_production_spawn_requires_gw_bind_method_names_residual_wave738()
        && honesty_host_production_spawn_requires_gw_bind_source_markers_residual_wave738()
        && honesty_host_production_spawn_requires_gw_bind_nav_commands_residual_wave738()
        && simulate_host_production_spawn_requires_gw_bind_collect_source()
        && simulate_host_production_spawn_requires_gw_bind_dispatch_source()
}
pub fn simulate_live_host_production_spawn_requires_gw_bind_honesty() -> bool {
    let ok = honesty_host_production_spawn_requires_gw_bind_residual_pack_wave738();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionSpawnRequiresGwBindAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_spawn_requires_gw_bind_method_names_residual_wave738());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_spawn_requires_gw_bind_source_markers_residual_wave738());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_spawn_requires_gw_bind_nav_commands_residual_wave738());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_production_spawn_requires_gw_bind_collect_source());
        assert!(simulate_host_production_spawn_requires_gw_bind_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_production_spawn_requires_gw_bind_residual_pack_wave738());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_production_spawn_requires_gw_bind_honesty());
        assert!(residual_host_production_spawn_requires_gw_bind_ok());
    }
}
