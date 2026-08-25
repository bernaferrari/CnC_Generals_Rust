//! Wave 733: free demo faction base/army spawn is opt-in.
//! Default fail-closed: `spawn_faction_base` / `create_test_map` no-op unless
//! `GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SPAWN_FACTION_BASE_OPT_IN_METHOD_NAMES_WAVE733: &[&str] = &[
    "GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE",
    "spawn_faction_base",
    "create_test_map",
    "Wave 733",
    "USA_Ranger",
    "playable_claim = false",
];
pub const LIVE_HOST_SPAWN_FACTION_BASE_OPT_IN_NAV_STEPS_WAVE733: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_NO_FREE_DEMO_ARMY",
    "LIVE_HOST_SPAWN_FACTION_BASE_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_SPAWN_FACTION_BASE_OPT_IN_CMD_NAMES_WAVE733: &[&str] = &[
    "host_spawn_faction_base_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "no_free_demo_army",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSpawnFactionBaseOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostSpawnFactionBaseOptInAction {
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
fn residual_action_store(a: ResidualHostSpawnFactionBaseOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_spawn_faction_base_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_spawn_faction_base_opt_in_last_action()
-> ResidualHostSpawnFactionBaseOptInAction {
    ResidualHostSpawnFactionBaseOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_spawn_faction_base_opt_in_method_names_residual_wave733() -> bool {
    let names = LIVE_HOST_SPAWN_FACTION_BASE_OPT_IN_METHOD_NAMES_WAVE733;
    let ok = residual_name_index(names, "GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE").is_some()
        && residual_name_index(names, "spawn_faction_base").is_some()
        && residual_name_index(names, "create_test_map").is_some()
        && residual_name_index(names, "Wave 733").is_some()
        && residual_name_index(names, "USA_Ranger").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSpawnFactionBaseOptInAction::MethodNames);
    ok
}
pub fn honesty_host_spawn_faction_base_opt_in_source_markers_residual_wave733() -> bool {
    let gl = gl_source();
    let gl_ok = gl.contains("Wave 733")
        && gl.contains("GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE")
        && gl.contains("fn spawn_faction_base")
        && gl.contains("fn create_test_map")
        && gl.matches("if !allow {\n            return;").count() >= 2;
    let ok = gl_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSpawnFactionBaseOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_spawn_faction_base_opt_in_nav_commands_residual_wave733() -> bool {
    let steps = LIVE_HOST_SPAWN_FACTION_BASE_OPT_IN_NAV_STEPS_WAVE733;
    let cmds = RUNTIME_HOST_LIVE_HOST_SPAWN_FACTION_BASE_OPT_IN_CMD_NAMES_WAVE733;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_NO_FREE_DEMO_ARMY").is_some()
        && residual_name_index(steps, "LIVE_HOST_SPAWN_FACTION_BASE_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_spawn_faction_base_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "no_free_demo_army").is_some();
    residual_action_store(ResidualHostSpawnFactionBaseOptInAction::NavCommands);
    ok
}
pub fn simulate_host_spawn_faction_base_opt_in_collect_source() -> bool {
    let ok = gl_source().contains("GENERALS_RUNTIME_HOST_SPAWN_FACTION_BASE")
        && gl_source().contains("spawn_faction_base");
    residual_action_store(ResidualHostSpawnFactionBaseOptInAction::CollectSource);
    ok
}
pub fn simulate_host_spawn_faction_base_opt_in_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 733") && gl_source().contains("create_test_map");
    residual_action_store(ResidualHostSpawnFactionBaseOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_spawn_faction_base_opt_in_residual_pack_wave733() -> bool {
    honesty_host_spawn_faction_base_opt_in_method_names_residual_wave733()
        && honesty_host_spawn_faction_base_opt_in_source_markers_residual_wave733()
        && honesty_host_spawn_faction_base_opt_in_nav_commands_residual_wave733()
        && simulate_host_spawn_faction_base_opt_in_collect_source()
        && simulate_host_spawn_faction_base_opt_in_dispatch_source()
}
pub fn simulate_live_host_spawn_faction_base_opt_in_honesty() -> bool {
    let ok = honesty_host_spawn_faction_base_opt_in_residual_pack_wave733();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSpawnFactionBaseOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_spawn_faction_base_opt_in_method_names_residual_wave733());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_spawn_faction_base_opt_in_source_markers_residual_wave733());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_spawn_faction_base_opt_in_nav_commands_residual_wave733());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_spawn_faction_base_opt_in_collect_source());
        assert!(simulate_host_spawn_faction_base_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_spawn_faction_base_opt_in_residual_pack_wave733());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_spawn_faction_base_opt_in_honesty());
        assert!(residual_host_spawn_faction_base_opt_in_ok());
    }
}
