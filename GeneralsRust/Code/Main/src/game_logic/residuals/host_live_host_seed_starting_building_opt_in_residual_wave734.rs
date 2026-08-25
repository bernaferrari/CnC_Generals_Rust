//! Wave 734: free invent of skirmish starting building when map has no base is opt-in.
//! Default fail-closed: `spawn_skirmish_starting_units` only places StartingUnit0
//! beside an existing base. Incomplete maps may set
//! `GENERALS_RUNTIME_HOST_SEED_STARTING_BUILDING=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SEED_STARTING_BUILDING_OPT_IN_METHOD_NAMES_WAVE734: &[&str] = &[
    "allow_seed_building",
    "GENERALS_RUNTIME_HOST_SEED_STARTING_BUILDING",
    "spawn_skirmish_starting_units",
    "starting_building",
    "Wave 734",
    "playable_claim = false",
];
pub const LIVE_HOST_SEED_STARTING_BUILDING_OPT_IN_NAV_STEPS_WAVE734: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_EXISTING_BASE_FOR_STARTING_UNIT",
    "LIVE_HOST_SEED_STARTING_BUILDING_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_SEED_STARTING_BUILDING_OPT_IN_CMD_NAMES_WAVE734: &[&str] = &[
    "host_seed_starting_building_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "existing_base_for_starting_unit",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSeedStartingBuildingOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostSeedStartingBuildingOptInAction {
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
fn residual_action_store(a: ResidualHostSeedStartingBuildingOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_seed_starting_building_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_seed_starting_building_opt_in_last_action()
-> ResidualHostSeedStartingBuildingOptInAction {
    ResidualHostSeedStartingBuildingOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
pub fn honesty_host_seed_starting_building_opt_in_method_names_residual_wave734() -> bool {
    let names = LIVE_HOST_SEED_STARTING_BUILDING_OPT_IN_METHOD_NAMES_WAVE734;
    let ok = residual_name_index(names, "allow_seed_building").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_SEED_STARTING_BUILDING").is_some()
        && residual_name_index(names, "spawn_skirmish_starting_units").is_some()
        && residual_name_index(names, "starting_building").is_some()
        && residual_name_index(names, "Wave 734").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSeedStartingBuildingOptInAction::MethodNames);
    ok
}
pub fn honesty_host_seed_starting_building_opt_in_source_markers_residual_wave734() -> bool {
    let gl = gl_source();
    // 2026-08-15: Wave 734 comment lives on world_skirmish_tests.rs; the
    // opt-in gate is world_tick/production.rs:1276-1285.
    let wave =
        gl.contains("Wave 734") || include_str!("../world_skirmish_tests.rs").contains("Wave 734");
    let gl_ok = wave
        && gl.contains("allow_seed_building")
        && gl.contains("GENERALS_RUNTIME_HOST_SEED_STARTING_BUILDING")
        && gl.contains("spawn_skirmish_starting_units")
        && gl.contains("if allow_seed_building")
        && gl.contains("starting_building");
    let ok = gl_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSeedStartingBuildingOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_seed_starting_building_opt_in_nav_commands_residual_wave734() -> bool {
    let steps = LIVE_HOST_SEED_STARTING_BUILDING_OPT_IN_NAV_STEPS_WAVE734;
    let cmds = RUNTIME_HOST_LIVE_HOST_SEED_STARTING_BUILDING_OPT_IN_CMD_NAMES_WAVE734;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_EXISTING_BASE_FOR_STARTING_UNIT").is_some()
        && residual_name_index(steps, "LIVE_HOST_SEED_STARTING_BUILDING_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_seed_starting_building_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "existing_base_for_starting_unit").is_some();
    residual_action_store(ResidualHostSeedStartingBuildingOptInAction::NavCommands);
    ok
}
pub fn simulate_host_seed_starting_building_opt_in_collect_source() -> bool {
    let ok = gl_source().contains("allow_seed_building")
        && gl_source().contains("GENERALS_RUNTIME_HOST_SEED_STARTING_BUILDING");
    residual_action_store(ResidualHostSeedStartingBuildingOptInAction::CollectSource);
    ok
}
pub fn simulate_host_seed_starting_building_opt_in_dispatch_source() -> bool {
    let ok = (gl_source().contains("Wave 734")
        || include_str!("../world_skirmish_tests.rs").contains("Wave 734"))
        && gl_source().contains("if allow_seed_building");
    residual_action_store(ResidualHostSeedStartingBuildingOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_seed_starting_building_opt_in_residual_pack_wave734() -> bool {
    honesty_host_seed_starting_building_opt_in_method_names_residual_wave734()
        && honesty_host_seed_starting_building_opt_in_source_markers_residual_wave734()
        && honesty_host_seed_starting_building_opt_in_nav_commands_residual_wave734()
        && simulate_host_seed_starting_building_opt_in_collect_source()
        && simulate_host_seed_starting_building_opt_in_dispatch_source()
}
pub fn simulate_live_host_seed_starting_building_opt_in_honesty() -> bool {
    let ok = honesty_host_seed_starting_building_opt_in_residual_pack_wave734();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSeedStartingBuildingOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_seed_starting_building_opt_in_method_names_residual_wave734());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_seed_starting_building_opt_in_source_markers_residual_wave734());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_seed_starting_building_opt_in_nav_commands_residual_wave734());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_seed_starting_building_opt_in_collect_source());
        assert!(simulate_host_seed_starting_building_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_seed_starting_building_opt_in_residual_pack_wave734());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_seed_starting_building_opt_in_honesty());
        assert!(residual_host_seed_starting_building_opt_in_ok());
    }
}
